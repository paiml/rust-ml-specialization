//! `cargo run -p xtask -- nightly` (#792 lane b) — run EVERY demo against the
//! dev `apr` and print one `Green`/`Red`/`NotRun` line per demo plus a summary
//! JSON. Demos are enumerated from the workspace, never listed here. A Red or
//! NotRun never fails the run (the nightly must not block the gate); the exit
//! code is nonzero only when the runner itself could not run.

use crate::demo_dirs;
use demo_kit::DemoManifest;
use std::path::Path;
use std::process::Command;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Outcome {
    Green,
    Red,
    NotRun,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Self::Green => "Green",
            Self::Red => "Red",
            Self::NotRun => "NotRun",
        }
    }
}

pub struct Report {
    pub lines: Vec<String>,
    pub summary: String,
}

/// One demo: NotRun when it needs a GPU the host lacks or has no steps to run
/// (an absent measurement is not a pass); Red on the first failing step.
fn run_demo(root: &Path, dir: &Path, has_gpu: bool) -> (Outcome, String) {
    let m = match DemoManifest::load(&dir.join("demo.toml")) {
        Ok(m) => m,
        Err(e) => return (Outcome::Red, format!("demo.toml: {e}")),
    };
    if m.host_class.contains("cuda") && !has_gpu {
        return (
            Outcome::NotRun,
            format!("host_class {} but no GPU lane", m.host_class),
        );
    }
    if m.step.is_empty() {
        return (Outcome::NotRun, "no [[step]] to run".into());
    }
    for s in &m.step {
        match Command::new("sh")
            .arg("-c")
            .arg(&s.cmd)
            .current_dir(root)
            .output()
        {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                return (
                    Outcome::Red,
                    format!("`{}` exited {}", s.cmd, o.status.code().unwrap_or(-1)),
                )
            }
            Err(e) => return (Outcome::Red, format!("`{}` did not start: {e}", s.cmd)),
        }
    }
    (Outcome::Green, format!("{} step(s) passed", m.step.len()))
}

pub fn run(root: &Path, has_gpu: bool) -> Report {
    let mut lines = Vec::new();
    let mut counts = [0usize; 3];
    for d in demo_dirs(root) {
        let name = d
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let (o, why) = run_demo(root, &d, has_gpu);
        counts[o as usize] += 1;
        lines.push(format!("{} {name}: {why}", o.label()));
    }
    let summary = format!(
        "{{\"demos\":{},\"green\":{},\"red\":{},\"not_run\":{}}}",
        lines.len(),
        counts[0],
        counts[1],
        counts[2]
    );
    Report { lines, summary }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo(root: &Path, id: &str, host: &str, steps: &[&str]) {
        let d = root.join(id);
        std::fs::create_dir_all(&d).unwrap();
        let mut t = format!(
            "id = \"{id}\"\ntitle = \"t\"\nlesson = \"rfml5/2.2\"\napr = \"=0.1.0\"\nhost_class = \"{host}\"\n"
        );
        for s in steps {
            t.push_str(&format!("[[step]]\ncmd = \"{s}\"\n"));
        }
        t.push_str(
            "[assert]\nexit = 0\n[record]\ntarget_duration_s = 300\nresolution = \"1920x1080\"\n",
        );
        std::fs::write(d.join("demo.toml"), t).unwrap();
    }

    fn root(name: &str) -> std::path::PathBuf {
        let r = std::env::temp_dir().join(format!("xtask-nightly-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn one_green_one_red_get_their_own_lines() {
        let r = root("gr");
        demo(&r, "d01-ok", "any", &["true"]);
        demo(&r, "d02-bad", "any", &["true", "exit 3"]);
        let rep = run(&r, false);
        assert!(rep.lines[0].starts_with("Green d01-ok"), "{:?}", rep.lines);
        assert!(
            rep.lines[1].starts_with("Red d02-bad") && rep.lines[1].contains("exited 3"),
            "{:?}",
            rep.lines
        );
        assert_eq!(
            rep.summary,
            "{\"demos\":2,\"green\":1,\"red\":1,\"not_run\":0}"
        );
    }

    #[test]
    fn adding_a_crate_changes_the_count_with_no_edit_to_the_runner() {
        let r = root("count");
        demo(&r, "d01-ok", "any", &["true"]);
        assert!(run(&r, false).summary.starts_with("{\"demos\":1"));
        demo(&r, "d03-new", "any", &["true"]);
        assert!(run(&r, false).summary.starts_with("{\"demos\":2"));
    }

    #[test]
    fn a_cuda_demo_without_a_gpu_lane_is_not_run_not_green() {
        let r = root("cuda");
        demo(&r, "d04-gpu", "gx10-cuda", &["true"]);
        assert_eq!(run(&r, false).lines[0].split(' ').next(), Some("NotRun"));
        assert_eq!(run(&r, true).lines[0].split(' ').next(), Some("Green"));
    }

    #[test]
    fn a_demo_with_no_steps_is_not_run() {
        let r = root("nosteps");
        demo(&r, "d05-empty", "any", &[]);
        assert!(run(&r, false).lines[0].starts_with("NotRun d05-empty"));
    }
}
