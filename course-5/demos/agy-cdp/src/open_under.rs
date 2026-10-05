//! Every file the driver touches goes through here. Each root's directory
//! descriptor is opened once; a relative path is walked one normal component
//! at a time with `openat(.., O_NOFOLLOW)`, so a symlink anywhere is refused
//! and nothing is canonicalized and then re-opened.
//!
//! Under the demo profile only two things can be named ([`ProfilePath`]),
//! both read-only. Writes go only under the run directory, created
//! `O_CREAT | O_EXCL`.

use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{openat, Dir, Mode, OFlags, CWD};

/// The two profile paths the driver may read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilePath {
    /// `user-data/DevToolsActivePort`
    DevToolsActivePort,
    /// `extensions/` (read for its manifest only)
    Extensions,
}

impl ProfilePath {
    fn rel(self) -> &'static str {
        match self {
            ProfilePath::DevToolsActivePort => "user-data/DevToolsActivePort",
            ProfilePath::Extensions => "extensions",
        }
    }
}

/// The two roots, each held by a directory descriptor.
pub struct Roots {
    profile: OwnedFd,
    run: OwnedFd,
    run_path: PathBuf,
}

/// Split `rel` into normal components, refusing anything else.
pub fn components(rel: &str) -> Result<Vec<String>, String> {
    let p = Path::new(rel);
    let mut out = Vec::new();
    for c in p.components() {
        match c {
            Component::Normal(s) => out.push(s.to_string_lossy().into_owned()),
            _ => return Err(format!("open_under: `{rel}` is not a plain relative path")),
        }
    }
    if out.is_empty() {
        return Err("open_under: empty path".into());
    }
    Ok(out)
}

fn dir_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

fn open_root(p: &Path) -> Result<OwnedFd, String> {
    openat(CWD, p, dir_flags(), Mode::empty()).map_err(|e| format!("root {}: {e}", p.display()))
}

/// Canonicalize a path (preflight only; nothing is re-opened by this name).
pub fn canonical(p: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()))
}

/// Create `<receipts>/<parts…>` (each part a plain name) and return it.
pub fn create_run_dir(receipts: &Path, parts: &[&str]) -> Result<PathBuf, String> {
    let mut p = receipts.to_path_buf();
    for part in parts {
        components(part)?;
        p.push(part);
    }
    std::fs::create_dir_all(&p).map_err(|e| format!("run dir: {e}"))?;
    Ok(p)
}

fn walk_dirs(root: &OwnedFd, dirs: &[String], create: bool) -> Result<Option<OwnedFd>, String> {
    let mut cur: Option<OwnedFd> = None;
    for d in dirs {
        let base = cur.as_ref().unwrap_or(root);
        if create {
            let _ = rustix::fs::mkdirat(base, d.as_str(), Mode::from_raw_mode(0o755));
        }
        let next = openat(base, d.as_str(), dir_flags(), Mode::empty())
            .map_err(|e| format!("open_under {d}: {e}"))?;
        cur = Some(next);
    }
    Ok(cur)
}

fn open_leaf(root: &OwnedFd, rel: &str, flags: OFlags, create: bool) -> Result<OwnedFd, String> {
    let parts = components(rel)?;
    let (leaf, dirs) = parts.split_last().ok_or("empty")?;
    let parent = walk_dirs(root, dirs, create)?;
    let base = parent.as_ref().unwrap_or(root);
    openat(
        base,
        leaf.as_str(),
        flags | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o644),
    )
    .map_err(|e| format!("open_under {rel}: {e}"))
}

fn read_fd(fd: OwnedFd) -> Result<Vec<u8>, String> {
    let mut f = std::fs::File::from(fd);
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

impl Roots {
    /// Open both roots once. `profile` must already be canonical.
    pub fn open(profile: &Path, run: &Path) -> Result<Roots, String> {
        Ok(Roots {
            profile: open_root(profile)?,
            run: open_root(run)?,
            run_path: run.to_path_buf(),
        })
    }

    /// The run directory's path (for arguments handed to a child).
    pub fn run_path(&self, rel: &str) -> Result<PathBuf, String> {
        let mut p = self.run_path.clone();
        for c in components(rel)? {
            p.push(c);
        }
        Ok(p)
    }

    /// Read `user-data/DevToolsActivePort`.
    pub fn read_devtools_port(&self) -> Result<Vec<u8>, String> {
        let fd = open_leaf(
            &self.profile,
            ProfilePath::DevToolsActivePort.rel(),
            OFlags::RDONLY,
            false,
        )?;
        read_fd(fd)
    }

    /// `(relative path, size, digest)` of every regular file under
    /// `extensions/`, digested by `hash` as it is read.
    pub fn extensions_manifest(
        &self,
        hash: &dyn Fn(&[u8]) -> String,
    ) -> Result<Vec<(String, u64, String)>, String> {
        let root = walk_dirs(
            &self.profile,
            &[ProfilePath::Extensions.rel().to_string()],
            false,
        )?
        .ok_or("extensions")?;
        let mut out = Vec::new();
        manifest_dir(&root, "", hash, &mut out)?;
        out.sort();
        Ok(out)
    }

    /// Create a directory chain under the run directory.
    pub fn mkdir_run(&self, rel: &str) -> Result<(), String> {
        walk_dirs(&self.run, &components(rel)?, true).map(|_| ())
    }

    /// Create a new file under the run directory (`O_CREAT | O_EXCL`).
    pub fn write_run(&self, rel: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        let fd = open_leaf(
            &self.run,
            rel,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
            true,
        )?;
        let mut f = std::fs::File::from(fd);
        f.write_all(bytes).map_err(|e| e.to_string())?;
        self.run_path(rel)
    }

    /// A new file under the run directory, as a descriptor (child stdio).
    pub fn create_run_fd(&self, rel: &str) -> Result<OwnedFd, String> {
        open_leaf(
            &self.run,
            rel,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
            true,
        )
    }

    /// Read a file under the run directory.
    pub fn read_run(&self, rel: &str) -> Result<Vec<u8>, String> {
        read_fd(open_leaf(&self.run, rel, OFlags::RDONLY, false)?)
    }

    /// `(relative path, size, digest)` of every regular file under `rel` of
    /// the run directory.
    pub fn manifest_run(
        &self,
        rel: &str,
        hash: &dyn Fn(&[u8]) -> String,
    ) -> Result<Vec<(String, u64, String)>, String> {
        let root = walk_dirs(&self.run, &components(rel)?, false)?.ok_or("run dir")?;
        let mut out = Vec::new();
        manifest_dir(&root, "", hash, &mut out)?;
        out.sort();
        Ok(out)
    }
}

fn manifest_dir(
    dir: &OwnedFd,
    prefix: &str,
    hash: &dyn Fn(&[u8]) -> String,
    out: &mut Vec<(String, u64, String)>,
) -> Result<(), String> {
    let names: Vec<(String, bool)> = Dir::read_from(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            (n != "." && n != "..").then(|| (n, e.file_type() == rustix::fs::FileType::Directory))
        })
        .collect();
    for (n, is_dir) in names {
        let rel = if prefix.is_empty() {
            n.clone()
        } else {
            format!("{prefix}/{n}")
        };
        if is_dir {
            let sub = openat(dir, n.as_str(), dir_flags(), Mode::empty())
                .map_err(|e| format!("{rel}: {e}"))?;
            manifest_dir(&sub, &rel, hash, out)?;
            continue;
        }
        let Ok(fd) = openat(
            dir,
            n.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) else {
            out.push((rel, 0, "unreadable".into()));
            continue;
        };
        let bytes = read_fd(fd)?;
        out.push((rel, bytes.len() as u64, hash(&bytes)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_relative_paths_pass() {
        assert!(components("a/b.json").is_ok());
        assert!(components("../x").is_err());
        assert!(components("/etc/passwd").is_err());
        assert!(components("a/../b").is_err());
        assert!(components("").is_err());
    }
}
