//! The operator's own state, listed by path, inode, size, mtime and ctime
//! before and after a run. This module may only `read_dir` and
//! `symlink_metadata`: content is never opened, because these directories hold
//! credentials. It is also the one place that may spell the environment names
//! the app's confined environment uses, so the launcher takes them from here.

use std::collections::BTreeMap;
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
    Ok(vec![
        (home.join(".config/Antigravity"), usize::MAX),
        (home.join(".gemini"), usize::MAX),
        (home.join(".antigravity"), usize::MAX),
        (home.join(".config"), 2),
        (home.join(".local/share"), 2),
        (home.join(".cache"), 2),
        (home.join(".local/state"), 2),
    ])
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
    let mut out = Listing::new();
    for (d, depth) in operator_dirs()? {
        walk(&d, depth, &mut out);
    }
    Ok(out)
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
    Ok(operator_dirs()?
        .into_iter()
        .filter(|(_, depth)| *depth == usize::MAX)
        .map(|(d, _)| d)
        .collect())
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
