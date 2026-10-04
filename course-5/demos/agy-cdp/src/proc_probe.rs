//! The only reader of `/proc`. It reads exactly: `/proc/<pid>/environ` for an
//! [`AppPid`], `/proc/<pid>/fd` for an [`AppPid`], `/proc/net/tcp`, the
//! numeric entry names of `/proc`, and field 5 (pgrp) of `/proc/<pid>/stat`.

use std::collections::BTreeMap;

/// A pid in the app's process group. Only `launch` constructs one, from the
/// app child it spawned, or from a pid whose pgrp equals that child's pid.
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

/// Field 5 of `/proc/<pid>/stat` (pgrp), nothing else kept.
pub fn pgrp(pid: u32) -> Option<u32> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = &s[s.rfind(')')? + 1..];
    after.split_whitespace().nth(2)?.parse().ok()
}

/// Every pid whose pgrp is the leader's pid.
pub fn group_members(leader: AppPid) -> Vec<AppPid> {
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    rd.flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter(|p| pgrp(*p) == Some(leader.0))
        .map(AppPid)
        .collect()
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
