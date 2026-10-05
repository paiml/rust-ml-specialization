//! Running `pv lint` on a materialised judge directory. Gated on the
//! `process` feature, because it spawns pv.

use super::{classify, materialise, ShapesOutcome};
use crate::verdict::{NotRunReason, Verdict};
use std::path::Path;

/// `judge(spec_dir, record_json, run_root)` with `pv` resolved on this
/// process's `PATH`.
pub fn judge(spec_dir: &Path, record_json: &[u8], run_root: &Path) -> ShapesOutcome {
    judge_with(None, spec_dir, record_json, run_root)
}

/// As [`judge`], with an explicit `PATH` for pv (tests put a shim first on it
/// without touching the process environment).
pub fn judge_with(
    path_env: Option<&std::ffi::OsStr>,
    spec_dir: &Path,
    record_json: &[u8],
    run_root: &Path,
) -> ShapesOutcome {
    let run = match materialise(spec_dir, record_json, run_root) {
        Ok(r) => r,
        Err(why) => return ShapesOutcome::not_run(why),
    };
    let Some(pv) = crate::preflight::which_in(path_env, "pv") else {
        let mut o = ShapesOutcome::from_verdict(Verdict::NotRun {
            reasons: vec![NotRunReason::MissingTool("pv".into())],
        });
        o.run_dir = Some(run);
        return o;
    };
    let mut cmd = std::process::Command::new(pv);
    cmd.arg("lint")
        .arg(run.join("spec"))
        .args(["--gate", "shapes", "--format", "json"]);
    if let Some(p) = path_env {
        cmd.env("PATH", p);
    }
    let mut o = match cmd.output() {
        Ok(out) => {
            let _ = std::fs::write(run.join("out.json"), &out.stdout);
            let _ = std::fs::write(run.join("err.txt"), &out.stderr);
            classify(out.status.code(), &out.stdout)
        }
        Err(e) => ShapesOutcome::not_run(format!("pv lint: {e}")),
    };
    o.run_dir = Some(run);
    o
}
