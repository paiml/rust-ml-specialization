//! The only reader of `/proc`. It reads exactly: `/proc/<pid>/environ` for an
//! [`AppPid`], `/proc/<pid>/fd` for an [`AppPid`] and for this process, `/proc/net/tcp`, the
//! numeric entry names of `/proc`, and fields 4 and 5 (ppid, pgrp) of
//! `/proc/<pid>/stat`. It also signals and reaps the pids the run started.

use std::collections::BTreeMap;

/// A pid the run started. Only `launch` constructs one, from the app child it
/// spawned; this module, from a pid in [`run_pids`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AppPid(u32);

impl AppPid {
    pub(crate) fn from_child_id(id: u32) -> AppPid {
        AppPid(id)
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

/// Fields 4 and 5 of `/proc/<pid>/stat` (ppid, pgrp), nothing else kept.
fn stat_ids(pid: u32) -> Option<(u32, u32)> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let mut f = s[s.rfind(')')? + 1..].split_whitespace().skip(1);
    Some((f.next()?.parse().ok()?, f.next()?.parse().ok()?))
}

/// `pid:comm:state:ppid:threads:exit_signal` from `/proc/<pid>/stat` (fields 2, 3, 4, 20, 38), so a pid the sweep
/// could not remove is named, and a zombie (`Z`) or a pid in uninterruptible
/// sleep (`D`) is told apart from one that ignored the signal.
pub fn describe(pid: u32) -> String {
    let Ok(s) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return format!("{pid}:gone");
    };
    let comm = s
        .find('(')
        .zip(s.rfind(')'))
        .map_or("?", |(a, b)| &s[a + 1..b]);
    let rest: Vec<&str> = s
        .rfind(')')
        .map_or(Vec::new(), |b| s[b + 1..].split_whitespace().collect());
    let at = |i: usize| rest.get(i).copied().unwrap_or("?");
    // After the comm: state, ppid, ..., num_threads is the 18th field and
    // exit_signal the 36th.
    format!(
        "{pid}:{comm}:{}:ppid={}:threads={}:exit_signal={}",
        at(0),
        at(1),
        at(17),
        at(35)
    )
}

/// Field 5 of `/proc/<pid>/stat` (pgrp).
pub fn pgrp(pid: u32) -> Option<u32> {
    stat_ids(pid).map(|(_, g)| g)
}

fn proc_pids() -> Vec<u32> {
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    rd.flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .collect()
}

/// Every pid whose pgrp is the leader's pid.
pub fn group_members(leader: AppPid) -> Vec<AppPid> {
    proc_pids()
        .into_iter()
        .filter(|p| pgrp(*p) == Some(leader.0))
        .map(AppPid)
        .collect()
}

/// Make this process the child subreaper: a process the run starts that
/// double-forks out of the app's group (a language server, a command an agent
/// runs) is reparented here, not to init, so [`run_pids`] still finds it.
pub fn become_subreaper() -> Result<(), String> {
    rustix::process::set_child_subreaper(Some(rustix::process::getpid()))
        .map_err(|e| format!("child subreaper: {e}"))
}

/// Every pid the run started that is still in `/proc`: the app's group, the
/// leader (a child of this process), every child of this process except `keep` (the processes this
/// process started that are not the run's, such as the display server), and
/// every descendant of those. With [`become_subreaper`] an escaped double fork
/// is a child of this process, so it is in the set.
pub fn run_pids(leader: AppPid, keep: &[u32]) -> Vec<AppPid> {
    let me = std::process::id();
    let ids: Vec<(u32, u32, u32)> = proc_pids()
        .into_iter()
        .filter_map(|p| stat_ids(p).map(|(pp, g)| (p, pp, g)))
        .collect();
    let mut set: std::collections::BTreeSet<u32> = ids
        .iter()
        .filter(|(p, pp, g)| *g == leader.0 || (*pp == me && !keep.contains(p)))
        .map(|(p, _, _)| *p)
        .collect();
    loop {
        let more: Vec<u32> = ids
            .iter()
            .filter(|(p, pp, _)| set.contains(pp) && !set.contains(p))
            .map(|(p, _, _)| *p)
            .collect();
        if more.is_empty() {
            return set.into_iter().map(AppPid).collect();
        }
        set.extend(more);
    }
}

/// Reap each of `pids` that is an exited child of this process. An adopted
/// orphan that dies stays in `/proc` as a zombie until it is reaped. Each pid
/// is named: `waitpid(None, _)` is pid 0, which waits only for children in this
/// process's own group, and the app runs in a group of its own, so its dead
/// processes were never eligible; a wait on any child (-1) would also collect
/// children this process waits for elsewhere, such as the display server.
pub fn reap(pids: &[AppPid]) {
    use rustix::process::{waitpid, Pid, WaitOptions};
    for p in pids {
        if let Some(pid) = Pid::from_raw(p.0 as i32) {
            let _ = waitpid(Some(pid), WaitOptions::NOHANG);
        }
    }
}

/// SIGTERM each pid, reap, and SIGKILL what is left after `grace`.
pub fn kill_pids(pids: &[AppPid], grace: std::time::Duration) {
    use rustix::process::{kill_process, Pid, Signal};
    let live = |p: &AppPid| std::path::Path::new(&format!("/proc/{}", p.0)).exists();
    let send = |sig| {
        for p in pids {
            if let Some(pid) = Pid::from_raw(p.0 as i32) {
                let _ = kill_process(pid, sig);
            }
        }
    };
    send(Signal::TERM);
    let end = std::time::Instant::now() + grace;
    loop {
        reap(pids);
        if !pids.iter().any(live) || std::time::Instant::now() >= end {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    if pids.iter().any(live) {
        send(Signal::KILL);
        std::thread::sleep(std::time::Duration::from_millis(200));
        reap(pids);
    }
}

/// The child's environment, as name -> value. Never written anywhere.
pub fn environ(pid: AppPid) -> Result<BTreeMap<String, String>, String> {
    let raw = std::fs::read(format!("/proc/{}/environ", pid.0)).map_err(|e| e.to_string())?;
    Ok(raw
        .split(|b| *b == 0)
        .filter(|kv| !kv.is_empty())
        .filter_map(|kv| {
            let s = String::from_utf8_lossy(kv);
            let (k, v) = s.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .collect())
}

fn socket_inodes(pid: AppPid) -> Vec<u64> {
    let Ok(rd) = std::fs::read_dir(format!("/proc/{}/fd", pid.0)) else {
        return Vec::new();
    };
    rd.flatten()
        .filter_map(|e| std::fs::read_link(e.path()).ok())
        .filter_map(|l| {
            let s = l.to_string_lossy().into_owned();
            s.strip_prefix("socket:[")?.strip_suffix(']')?.parse().ok()
        })
        .collect()
}

/// Inodes of sockets listening on 127.0.0.1:`port` (state 0A in `/proc/net/tcp`).
pub fn listening_inodes(port: u16) -> Vec<u64> {
    let Ok(t) = std::fs::read_to_string("/proc/net/tcp") else {
        return Vec::new();
    };
    let want = format!("0100007F:{port:04X}");
    t.lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            (f.len() > 9 && f[1] == want && f[3] == "0A").then(|| f[9].parse().ok())?
        })
        .collect()
}

/// Is the listener on `port` held by a member of the leader's group?
pub fn port_owned_by_group(port: u16, leader: AppPid) -> bool {
    let listening = listening_inodes(port);
    !listening.is_empty()
        && group_members(leader)
            .into_iter()
            .flat_map(socket_inodes)
            .any(|i| listening.contains(&i))
}

/// Signal the app's whole group: SIGTERM, then SIGKILL to what is left after
/// `grace`. The pgid is the leader's own pid, so no pid outside the group can
/// be named.
pub fn kill_group(leader: AppPid, grace: std::time::Duration) {
    let Some(pid) = rustix::process::Pid::from_raw(leader.0 as i32) else {
        return;
    };
    let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::TERM);
    let end = std::time::Instant::now() + grace;
    while std::time::Instant::now() < end && !group_members(leader).is_empty() {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    if !group_members(leader).is_empty() {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
}

/// The files `pid` holds open: the link targets of `/proc/<pid>/fd`, with a
/// deleted file's ` (deleted)` suffix dropped. Sockets, pipes and anonymous
/// inodes are not paths and are skipped.
fn open_paths(pid: u32) -> Vec<std::path::PathBuf> {
    let Ok(rd) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return Vec::new();
    };
    rd.flatten()
        .filter_map(|e| std::fs::read_link(e.path()).ok())
        .filter(|l| l.is_absolute())
        .map(|l| {
            let s = l.to_string_lossy();
            match s.strip_suffix(" (deleted)") {
                Some(t) => std::path::PathBuf::from(t),
                None => l,
            }
        })
        .collect()
}

/// Every file under `roots` held open by a pid the run started ([`run_pids`])
/// or by this process, sampled every 200 ms from start to [`FdWatch::stop`].
/// The demo-started pids are exactly the ones teardown sweeps, plus the demo
/// itself. A descriptor opened and closed between two samples
/// is not seen; the rule this serves is about a log written through a held
/// descriptor, which every sample sees.
pub struct FdWatch {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<std::collections::BTreeSet<std::path::PathBuf>>>,
}

impl FdWatch {
    pub fn start(leader: AppPid, keep: Vec<u32>, roots: Vec<std::path::PathBuf>) -> FdWatch {
        use std::sync::atomic::Ordering;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::spawn(move || {
            let mut held = std::collections::BTreeSet::new();
            let me = std::process::id();
            loop {
                let last = flag.load(Ordering::SeqCst);
                let pids = run_pids(leader, &keep).into_iter().map(AppPid::get);
                for p in pids.chain([me]) {
                    held.extend(
                        open_paths(p)
                            .into_iter()
                            .filter(|f| roots.iter().any(|r| f.starts_with(r))),
                    );
                }
                if last {
                    return held;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });
        FdWatch {
            stop,
            handle: Some(handle),
        }
    }

    /// Take one last sample and return everything seen.
    pub fn stop(mut self) -> std::collections::BTreeSet<std::path::PathBuf> {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        self.handle
            .take()
            .and_then(|h| h.join().ok())
            .unwrap_or_default()
    }
}

impl Drop for FdWatch {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;

    fn state(pid: u32) -> Option<char> {
        let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        s[s.rfind(')')? + 1..].trim_start().chars().next()
    }

    #[test]
    fn reap_removes_a_dead_child_in_another_process_group() {
        let child = std::process::Command::new("true")
            .process_group(0)
            .spawn()
            .expect("spawn true");
        let pid = child.id();
        // Not waited through the handle: only `reap` may collect it.
        std::mem::forget(child);
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while state(pid) != Some('Z') && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(state(pid), Some('Z'), "{}", describe(pid));
        assert_ne!(pgrp(pid), pgrp(std::process::id()));
        reap(&[AppPid(pid)]);
        assert_eq!(state(pid), None, "{}", describe(pid));
    }
}
