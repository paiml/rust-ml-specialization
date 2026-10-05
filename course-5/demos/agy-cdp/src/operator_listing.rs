//! The operator's own state, listed by path, inode, size, mtime and ctime
//! before and after a run. This module may only `read_dir` and
//! `symlink_metadata`: content is never opened, because these directories hold
//! credentials. It is also the one place that may spell the environment names
//! the app's confined environment uses, so the launcher takes them from here.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// `HOME` for the app: the profile's `home/`.
pub const CONFINED_HOME: (&str, &str) = ("HOME", "home");

/// Every `XDG_*` the app gets, with its subdirectory of the profile's `xdg/`.
pub const CONFINED_XDG: [(&str, &str); 5] = [
    ("XDG_CONFIG_HOME", "xdg/config"),
    ("XDG_DATA_HOME", "xdg/data"),
    ("XDG_CACHE_HOME", "xdg/cache"),
    ("XDG_STATE_HOME", "xdg/state"),
    ("XDG_RUNTIME_DIR", "xdg/runtime"),
];

/// One entry: `(inode, size, mtime s, mtime ns, ctime s, ctime ns)`.
pub type Stamp = (u64, u64, i64, i64, i64, i64);

/// A listing keyed by path.
pub type Listing = BTreeMap<PathBuf, Stamp>;

/// The operator directories, each with how deep it is walked. The app and
/// gemini directories are walked whole; the XDG defaults are walked to depth 2.
pub fn operator_dirs() -> Result<Vec<(PathBuf, usize)>, String> {
    let home = std::env::home_dir().ok_or("no home directory")?;
    Ok(operator_dirs_in(&home))
}

/// [`operator_dirs`] under a given home, so the judgment can be proved on a
/// scratch home instead of the operator's.
pub fn operator_dirs_in(home: &Path) -> Vec<(PathBuf, usize)> {
    vec![
        (home.join(".config/Antigravity"), usize::MAX),
        (home.join(".gemini"), usize::MAX),
        (home.join(".antigravity"), usize::MAX),
        (home.join(".config"), 2),
        (home.join(".local/share"), 2),
        (home.join(".cache"), 2),
        (home.join(".local/state"), 2),
    ]
}

fn stamp(p: &Path) -> Option<Stamp> {
    let m = std::fs::symlink_metadata(p).ok()?;
    Some((
        m.ino(),
        m.size(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}

fn walk(dir: &Path, depth: usize, out: &mut Listing) {
    if let Some(s) = stamp(dir) {
        out.insert(dir.to_path_buf(), s);
    }
    if depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let is_dir = std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir());
        if is_dir {
            walk(&p, depth - 1, out);
        } else if let Some(s) = stamp(&p) {
            out.insert(p, s);
        }
    }
}

/// List every operator directory.
pub fn list() -> Result<Listing, String> {
    Ok(list_dirs(&operator_dirs()?))
}

/// List the given directories, each to its depth.
pub fn list_dirs(dirs: &[(PathBuf, usize)]) -> Listing {
    let mut out = Listing::new();
    for (d, depth) in dirs {
        walk(d, *depth, &mut out);
    }
    out
}

/// Is `p` equal to, inside, or containing any operator directory?
pub fn overlaps(p: &Path) -> Result<bool, String> {
    Ok(operator_dirs()?
        .iter()
        .map(|(d, _)| std::fs::canonicalize(d).unwrap_or_else(|_| d.clone()))
        .any(|d| p.starts_with(&d) || d.starts_with(p)))
}

/// Entries that differ between two listings (added, removed or changed).
pub fn diff(before: &Listing, after: &Listing) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = before
        .iter()
        .filter(|(k, v)| after.get(*k) != Some(*v))
        .map(|(k, _)| k.clone())
        .collect();
    out.extend(after.keys().filter(|k| !before.contains_key(*k)).cloned());
    out
}

/// The directories that ARE the operator's profile: the ones walked whole.
/// A run that changes anything under them has touched the operator.
pub fn profile_roots() -> Result<Vec<PathBuf>, String> {
    Ok(roots_of(&operator_dirs()?))
}

/// The whole-walked directories of `dirs`.
pub fn roots_of(dirs: &[(PathBuf, usize)]) -> Vec<PathBuf> {
    dirs.iter()
        .filter(|(_, depth)| *depth == usize::MAX)
        .map(|(d, _)| d.clone())
        .collect()
}

/// The profile roots with symlinks resolved, as `/proc/<pid>/fd` names them.
/// A root that does not exist is kept as written.
pub fn canonical(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .map(|r| std::fs::canonicalize(r).unwrap_or_else(|_| r.clone()))
        .collect()
}

/// Split changed paths into `(profile, ambient)`. Ambient paths are the XDG
/// defaults' shallow entries, which other software on the host writes all the
/// time (a no-demo control over 50 s measured 18 such changes and none under a
/// profile root); they are counted and shown, never judged.
pub fn split(changed: Vec<PathBuf>, roots: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    changed
        .into_iter()
        .partition(|p| roots.iter().any(|r| p.starts_with(r)))
}

/// A file under a profile root with a directory named `log` between the root
/// and the file: the ruled `*/log/*`. The `log` directory's own entry is not
/// a log file and stays judged.
pub fn is_log(p: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|r| {
        p.strip_prefix(r).is_ok_and(|rest| {
            let parts: Vec<_> = rest.components().collect();
            parts.len() >= 2
                && parts[..parts.len() - 1]
                    .iter()
                    .any(|c| c.as_os_str() == "log")
        })
    })
}

/// The listing judged.
#[derive(Debug, Default, PartialEq)]
pub struct Judged {
    /// The defect: a profile change that is not a log file, or a changed log
    /// file that a demo-started process held open.
    pub touched: Vec<PathBuf>,
    /// Changed log files no demo-started process held open: the operator's
    /// own app writing its own log. Shown, not judged.
    pub logs: Vec<PathBuf>,
    /// Shallow XDG entries other host software writes. Shown, not judged.
    pub ambient: Vec<PathBuf>,
}

/// Judge the changed paths. `held` is every path a demo-started process held
/// open during the run (see [`crate::proc_probe::FdWatch`]), symlinks
/// resolved. A log file is exempt only when it is not in `held`, so the
/// exemption can never cover a demo writing through an open descriptor.
/// Whether `p`, as named or as resolved, was held open by a sampled pid.
pub fn is_held(p: &Path, held: &BTreeSet<PathBuf>) -> bool {
    held.contains(p) || std::fs::canonicalize(p).is_ok_and(|r| held.contains(&r))
}

pub fn judge(changed: Vec<PathBuf>, roots: &[PathBuf], held: &BTreeSet<PathBuf>) -> Judged {
    let (profile, ambient) = split(changed, roots);
    let mut j = Judged {
        ambient,
        ..Judged::default()
    };
    for p in profile {
        if is_log(&p, roots) && !is_held(&p, held) {
            j.logs.push(p);
        } else {
            j.touched.push(p);
        }
    }
    j
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proc_probe::{AppPid, FdWatch};
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command};
    use std::time::Duration;

    #[test]
    fn a_change_under_a_profile_root_is_profile_and_the_rest_is_ambient() {
        let roots = vec![
            PathBuf::from("/h/.gemini"),
            PathBuf::from("/h/.config/Antigravity"),
        ];
        let (p, a) = split(
            vec![
                PathBuf::from("/h/.gemini/x"),
                PathBuf::from("/h/.config/Antigravity"),
                PathBuf::from("/h/.config/other/y"),
                PathBuf::from("/h/.local/state/t"),
            ],
            &roots,
        );
        assert_eq!(p.len(), 2, "{p:?}");
        assert_eq!(a.len(), 2, "{a:?}");
    }

    #[test]
    fn profile_roots_are_exactly_the_whole_walked_dirs() {
        let r = profile_roots().unwrap();
        assert_eq!(r.len(), 3);
        assert!(r.iter().any(|d| d.ends_with(".gemini")));
        assert!(!r.iter().any(|d| d.ends_with(".cache")));
    }

    #[test]
    fn only_a_file_below_a_log_dir_under_a_root_is_a_log() {
        let roots = vec![PathBuf::from("/h/.gemini")];
        assert!(is_log(Path::new("/h/.gemini/cli/log/a.log"), &roots));
        assert!(is_log(Path::new("/h/.gemini/log/a"), &roots));
        assert!(!is_log(Path::new("/h/.gemini/cli/log"), &roots));
        assert!(!is_log(Path::new("/h/.gemini/cli/logs/a.log"), &roots));
        assert!(!is_log(Path::new("/h/.gemini/cli/a.log"), &roots));
        assert!(!is_log(Path::new("/x/log/a.log"), &roots));
    }

    /// A scratch home with one profile log file and one profile config file.
    fn scratch_home(name: &str) -> PathBuf {
        let h = std::env::temp_dir().join(format!("ol-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&h);
        std::fs::create_dir_all(h.join(".gemini/cli/log")).unwrap();
        std::fs::write(h.join(".gemini/cli/log/cli.log"), "start\n").unwrap();
        std::fs::write(h.join(".gemini/settings.json"), "{}").unwrap();
        h
    }

    /// A process-group leader appending to `log` every 100 ms through one
    /// descriptor it opened once, as an app writes its log.
    fn writer(log: &Path) -> Child {
        Command::new("sh")
            .args([
                "-c",
                "exec 3>>\"$1\"; while :; do echo x >&3; sleep 0.1; done",
                "sh",
            ])
            .arg(log)
            .process_group(0)
            .spawn()
            .unwrap()
    }

    /// How long each planted run lasts. `RFML5_CONTROL_SECS` repeats them at
    /// a real demo's length; unset, one second.
    fn secs() -> f64 {
        std::env::var("RFML5_CONTROL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0)
    }

    /// One planted run on a scratch home: `plant` runs while `watched` (a
    /// group leader standing in for the app) is sampled. Returns the judgment.
    fn planted(
        name: &str,
        plant: impl FnOnce(&Path) -> Vec<Child>,
        watched: Option<usize>,
    ) -> Judged {
        let home = scratch_home(name);
        let dirs = operator_dirs_in(&home);
        let roots = roots_of(&dirs);
        let before = list_dirs(&dirs);
        std::thread::sleep(Duration::from_millis(20));
        let mut kids = plant(&home);
        let idle = Command::new("sleep")
            .arg("600")
            .process_group(0)
            .spawn()
            .unwrap();
        let leader = match watched {
            Some(i) => kids[i].id(),
            None => idle.id(),
        };
        // The other children stand in for processes the demo did not start.
        let keep = kids
            .iter()
            .chain([&idle])
            .map(Child::id)
            .filter(|p| *p != leader)
            .collect();
        let watch = FdWatch::start(AppPid::from_child_id(leader), keep, canonical(&roots));
        std::thread::sleep(Duration::from_secs_f64(secs()));
        let held = watch.stop();
        kids.push(idle);
        for k in &mut kids {
            let _ = k.kill();
            let _ = k.wait();
        }
        let j = judge(diff(&before, &list_dirs(&dirs)), &roots, &held);
        eprintln!("{name}: {j:?}");
        let _ = std::fs::remove_dir_all(&home);
        j
    }

    #[test]
    fn planted_non_log_write_is_red() {
        let j = planted(
            "nonlog",
            |h| {
                std::fs::write(h.join(".gemini/settings.json"), "{\"x\":1}").unwrap();
                vec![]
            },
            None,
        );
        assert_eq!(j.touched.len(), 1, "{j:?}");
        assert!(j.touched[0].ends_with(".gemini/settings.json"));
    }

    #[test]
    fn planted_log_write_by_a_demo_started_pid_is_red() {
        let j = planted(
            "demolog",
            |h| vec![writer(&h.join(".gemini/cli/log/cli.log"))],
            Some(0),
        );
        assert_eq!(j.touched.len(), 1, "{j:?}");
        assert!(j.touched[0].ends_with("cli/log/cli.log"));
        assert!(j.logs.is_empty(), "{j:?}");
    }

    #[test]
    fn a_log_written_by_a_process_the_demo_did_not_start_is_exempt() {
        let j = planted(
            "otherlog",
            |h| vec![writer(&h.join(".gemini/cli/log/cli.log"))],
            None,
        );
        assert!(j.touched.is_empty(), "{j:?}");
        assert_eq!(j.logs.len(), 1, "{j:?}");
    }

    /// A group leader whose log writer double-forks out of the group (its own
    /// session) and outlives it, as the app's language server did. Returned
    /// only once the leader has exited, so the watch never sees the writer
    /// inside the group. The writer stops by itself after the planted run.
    fn escaper(log: &Path) -> Child {
        let ticks = (secs() * 10.0) as u64 + 50;
        let mut c = Command::new("sh")
            .args([
                "-c",
                "setsid sh -c 'exec 3>>\"$1\"; i=0; while [ $i -lt $2 ]; do echo x >&3; sleep 0.1; i=$((i+1)); done' sh \"$1\" \"$2\" & sleep 0.3",
                "sh",
            ])
            .arg(log)
            .arg(ticks.to_string())
            .process_group(0)
            .spawn()
            .unwrap();
        let _ = c.wait();
        c
    }

    #[test]
    fn a_log_written_by_a_double_fork_out_of_the_group_is_red() {
        crate::proc_probe::become_subreaper().unwrap();
        let j = planted(
            "escaped",
            |h| vec![escaper(&h.join(".gemini/cli/log/cli.log"))],
            Some(0),
        );
        assert_eq!(j.touched.len(), 1, "{j:?}");
        assert!(j.touched[0].ends_with("cli/log/cli.log"));
        assert!(j.logs.is_empty(), "{j:?}");
    }

    #[test]
    fn no_demo_control_on_a_scratch_home_is_green() {
        let j = planted("control", |_| vec![], None);
        assert_eq!(j, Judged::default());
    }

    /// The operator's real home, listed by metadata only, with no demo running,
    /// for `RFML5_CONTROL_SECS`. Run by hand: it reads the real home.
    #[test]
    #[ignore]
    fn no_demo_control_on_the_real_home_is_green() {
        let dirs = operator_dirs().unwrap();
        let roots = roots_of(&dirs);
        let before = list_dirs(&dirs);
        std::thread::sleep(Duration::from_secs_f64(secs()));
        let j = judge(diff(&before, &list_dirs(&dirs)), &roots, &BTreeSet::new());
        eprintln!(
            "real-home control {} s: touched {}, logs {}, ambient {}",
            secs(),
            j.touched.len(),
            j.logs.len(),
            j.ambient.len()
        );
        assert!(j.touched.is_empty(), "{:?}", j.touched);
    }
}
