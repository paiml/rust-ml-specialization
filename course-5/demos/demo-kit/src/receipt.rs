//! Receipts (MEGA-001 §5.3). Narration and on-screen text may cite only fields
//! of the receipt the recorded run wrote (H-4).
//!
//! Written to `<RFML5_RECEIPTS>/<id>/<host>/<apr-version>/<run-id>.json`.
//! Receipts carry host names, so they belong in the private catalogue, never in
//! this public repository: `RFML5_RECEIPTS` must be set, and a path inside this
//! repository is refused.

use crate::sha;
use crate::verdict::Verdict;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct StepRecord {
    pub cmd: String,
    pub exit: i32,
    pub stdout_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Receipt {
    pub id: String,
    pub run_id: String,
    pub recorded: bool,
    pub apr_version: String,
    pub apr_tarball_sha: Option<String>,
    pub agy_version: Option<String>,
    pub model_sha256: Option<String>,
    pub host: String,
    pub backend: String,
    pub steps: Vec<StepRecord>,
    pub measured: BTreeMap<String, serde_json::Value>,
    pub assertions: BTreeMap<String, bool>,
    pub verdict: Verdict,
}

impl Receipt {
    pub fn new(id: &str, apr_version: &str, backend: &str) -> Self {
        Receipt {
            id: id.into(),
            run_id: run_id(),
            recorded: std::env::var("RFML5_RECORDED").is_ok_and(|v| v == "1"),
            apr_version: apr_version.into(),
            apr_tarball_sha: None,
            agy_version: None,
            model_sha256: None,
            host: hostname(),
            backend: backend.into(),
            steps: Vec::new(),
            measured: BTreeMap::new(),
            assertions: BTreeMap::new(),
            verdict: Verdict::NotRun { reasons: vec![] },
        }
    }

    pub fn step(&mut self, cmd: &str, exit: i32, stdout: &[u8]) {
        self.steps.push(StepRecord {
            cmd: cmd.into(),
            exit,
            stdout_sha256: sha::sha256_bytes(stdout),
        });
    }

    pub fn relative_path(&self) -> PathBuf {
        PathBuf::from(&self.id)
            .join(&self.host)
            .join(&self.apr_version)
            .join(format!("{}.json", self.run_id))
    }

    /// Write under `root`; returns the file path and the receipt's sha256.
    pub fn write(&self, root: &Path) -> Result<(PathBuf, String), String> {
        let path = root.join(self.relative_path());
        std::fs::create_dir_all(path.parent().expect("has parent"))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, &json).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok((path, sha::sha256_bytes(&json)))
    }
}

/// Resolve the receipts root from `RFML5_RECEIPTS`, refusing any path inside
/// the public repository that contains `repo_marker`.
pub fn receipts_root(repo_root: &Path) -> Result<PathBuf, String> {
    let root = std::env::var("RFML5_RECEIPTS")
        .map(PathBuf::from)
        .map_err(|_| {
            "RFML5_RECEIPTS is not set (receipts belong in the private catalogue)".to_string()
        })?;
    check_outside(&root, repo_root)?;
    Ok(root)
}

pub fn check_outside(root: &Path, repo_root: &Path) -> Result<(), String> {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if canon(root).starts_with(canon(repo_root)) {
        return Err(format!(
            "refusing to write receipts inside the public repo: {}",
            root.display()
        ));
    }
    Ok(())
}

fn run_id() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{}-{:09}-{}",
        d.as_secs(),
        d.subsec_nanos(),
        std::process::id()
    )
}

pub fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown-host".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_under_id_host_version() {
        let d = std::env::temp_dir().join(format!("dk-receipt-{}", std::process::id()));
        let mut r = Receipt::new("d13-reducer", "none", "cpu");
        r.step(
            "cargo run -p d13-reducer",
            0,
            b"contract: reduce-deterministic-v1 OK\n",
        );
        r.assertions.insert("exit".into(), true);
        r.verdict = crate::verdict::decide(&[], &r.assertions);
        let (path, digest) = r.write(&d).unwrap();
        assert!(path.starts_with(d.join("d13-reducer").join(hostname()).join("none")));
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(digest, sha::sha256_bytes(&bytes));
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["verdict"]["verdict"], "Green");
        assert_eq!(v["steps"][0]["exit"], 0);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn refuses_receipts_inside_public_repo() {
        let repo = std::env::temp_dir().join(format!("dk-repo-{}", std::process::id()));
        std::fs::create_dir_all(repo.join("receipts")).unwrap();
        assert!(check_outside(&repo.join("receipts"), &repo).is_err());
        assert!(check_outside(&std::env::temp_dir().join("elsewhere"), &repo).is_ok());
        std::fs::remove_dir_all(&repo).unwrap();
    }
}
