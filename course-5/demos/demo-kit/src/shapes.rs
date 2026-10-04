//! The shapes judge (spec §4 item 3): pv's SHACL gate over one run record.
//!
//! The run record is what the shapes judge; the [`crate::receipt::Receipt`] is
//! the harness's envelope and references the record by sha256. `pv lint`
//! resolves a contract's `ref` against the parent of the spec directory, so a
//! judge run is laid out as `<run_root>/judge-<n>/{spec/*.yaml, receipt.json}`.
//!
//! | pv result | Verdict |
//! |---|---|
//! | exit 0, Pass, `pc_shape = fired`, nothing unarmed, W3C n/n with n > 0 | Green |
//! | exit 0 with any of those failing | Red ("a pass that proves nothing") |
//! | exit 1 | Red, carrying every `findings[].message` |
//! | exit 2, exit 3, or pv not found | NotRun |
//! | anything else | Red |
//!
//! Green is also refused when a passing run carries a finding or a violation:
//! a pass that reports a defect is not a pass (the `pass-with-finding`
//! sabotage). [`classify`] is pure; [`judge`] runs pv and needs the `process`
//! feature.

use crate::verdict::{NotRunReason, Verdict};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShapesOutcome {
    pub verdict: Verdict,
    /// pv's exit code; `None` when pv did not run or died by signal.
    pub exit: Option<i32>,
    /// Every `findings[].message`, in pv's order.
    pub findings: Vec<String>,
    pub focus_nodes_n: Option<u64>,
    pub shapes_n: Option<u64>,
    pub plant_violations: Option<u64>,
    pub w3c_cases_passed: Option<u64>,
    pub w3c_cases_n: Option<u64>,
    /// Why a Red or NotRun outcome is not Green, in one line ("" when Green).
    pub why: String,
    /// The judge run directory (never written into a receipt).
    #[serde(skip)]
    pub run_dir: Option<std::path::PathBuf>,
}

impl ShapesOutcome {
    /// An outcome carrying only a verdict (tests and refusals).
    pub fn from_verdict(verdict: Verdict) -> Self {
        let why = match &verdict {
            Verdict::Green => String::new(),
            other => other.to_string(),
        };
        ShapesOutcome {
            verdict,
            exit: None,
            findings: Vec::new(),
            focus_nodes_n: None,
            shapes_n: None,
            plant_violations: None,
            w3c_cases_passed: None,
            w3c_cases_n: None,
            why,
            run_dir: None,
        }
    }

    fn not_run(why: String) -> Self {
        let mut o = Self::from_verdict(Verdict::NotRun {
            reasons: vec![NotRunReason::ShapesNotRun(why.clone())],
        });
        o.why = why;
        o
    }

    pub fn is_green(&self) -> bool {
        self.verdict.is_green()
    }
}

fn num(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(Value::as_u64)
}

fn show(v: Option<&Value>) -> String {
    match v {
        None => "null".into(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// Why an exit-0 report is not Green, or `None` when it is. The order is the
/// order the reasons are reported in; the first that fails is named.
pub fn pass_defect(j: &Value) -> Option<String> {
    if j.get("verdict").and_then(Value::as_str) != Some("Pass") {
        return Some(format!("verdict {}", show(j.get("verdict"))));
    }
    match j.get("findings").and_then(Value::as_array) {
        Some(a) if a.is_empty() => {}
        Some(a) => return Some(format!("findings {}", a.len())),
        None => return Some("findings null".into()),
    }
    if j.get("violations").and_then(Value::as_u64) != Some(0) {
        return Some(format!("violations {}", show(j.get("violations"))));
    }
    armed_defect(j)
}

/// The arming half of [`pass_defect`]: the planted-check shape fired, every
/// shape was armed, and the W3C conformance cases all passed.
fn armed_defect(j: &Value) -> Option<String> {
    if j.get("pc_shape").and_then(Value::as_str) != Some("fired") {
        return Some(format!("pc_shape {}", show(j.get("pc_shape"))));
    }
    match j.get("not_armed_shapes").and_then(Value::as_array) {
        Some(a) if a.is_empty() => {}
        _ => return Some(format!("not armed {}", show(j.get("not_armed_shapes")))),
    }
    if j.get("unarmed_violations").and_then(Value::as_u64) != Some(0) {
        return Some(format!("unarmed {}", show(j.get("unarmed_violations"))));
    }
    let (p, n) = (num(j, "w3c_cases_passed"), num(j, "w3c_cases_n"));
    if n.unwrap_or(0) == 0 || p != n {
        return Some(format!(
            "w3c {}/{}",
            show(j.get("w3c_cases_passed")),
            show(j.get("w3c_cases_n"))
        ));
    }
    None
}

/// Map pv's exit code and stdout to an outcome, per the table above.
pub fn classify(exit: Option<i32>, stdout: &[u8]) -> ShapesOutcome {
    let json: Option<Value> = serde_json::from_slice(stdout).ok();
    let mut o = ShapesOutcome::from_verdict(Verdict::Green);
    o.exit = exit;
    if let Some(j) = &json {
        o.findings = j
            .get("findings")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|f| f.get("message").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        o.focus_nodes_n = num(j, "focus_nodes_n");
        o.shapes_n = num(j, "shapes_n");
        o.plant_violations = num(j, "plant_violations");
        o.w3c_cases_passed = num(j, "w3c_cases_passed");
        o.w3c_cases_n = num(j, "w3c_cases_n");
    }
    let red = |o: &mut ShapesOutcome, failed: Vec<String>, why: String| {
        o.verdict = Verdict::Red { failed };
        o.why = why;
    };
    match exit {
        Some(0) => match &json {
            None => red(&mut o, vec!["no json".into()], "no json".into()),
            Some(j) => {
                if let Some(why) = pass_defect(j) {
                    let line = format!("a pass that proves nothing: {why}");
                    red(&mut o, vec![line], why);
                }
            }
        },
        Some(1) => {
            if json.is_none() {
                red(&mut o, vec!["no json".into()], "no json".into());
            } else if o.findings.is_empty() {
                red(
                    &mut o,
                    vec!["exit 1 with no findings".into()],
                    "exit 1 with no findings".into(),
                );
            } else {
                let f = o.findings.clone();
                let why = format!("{} findings", f.len());
                red(&mut o, f, why);
            }
        }
        Some(c @ (2 | 3)) => {
            let mut n = ShapesOutcome::not_run(format!("pv lint exit {c}"));
            n.exit = exit;
            n.findings = o.findings;
            return n;
        }
        Some(c) => red(
            &mut o,
            vec![format!("pv lint exit {c}")],
            format!("exit {c}"),
        ),
        None => red(
            &mut o,
            vec!["pv lint killed by a signal".into()],
            "signal".into(),
        ),
    }
    o
}

/// Is `p`, or any directory above it, a git work tree (holds `.git`)?
pub fn inside_git(p: &Path) -> bool {
    let canon = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon.ancestors().any(|a| a.join(".git").exists())
}

/// Materialise `<run_root>/judge-<n>/{spec/*.yaml, receipt.json}` in a
/// directory no other call shares: `create_dir` fails when the name exists,
/// and the next `n` is tried. A `run_root` inside a git work tree is refused.
pub fn materialise(
    spec_dir: &Path,
    record_json: &[u8],
    run_root: &Path,
) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(run_root).map_err(|e| format!("run root: {e}"))?;
    if inside_git(run_root) {
        return Err("run root is inside a git work tree (M4); refusing".into());
    }
    let mut yamls: Vec<std::path::PathBuf> = std::fs::read_dir(spec_dir)
        .map_err(|e| format!("spec dir: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.is_file())
        .collect();
    yamls.sort();
    if yamls.is_empty() {
        return Err("spec dir holds no contract".into());
    }
    let mut n = 0u32;
    let run = loop {
        let candidate = run_root.join(format!("judge-{n}"));
        match std::fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(format!("judge dir: {e}")),
        }
    };
    std::fs::create_dir(run.join("spec")).map_err(|e| format!("judge spec: {e}"))?;
    for y in &yamls {
        let name = y.file_name().expect("a file has a name");
        std::fs::copy(y, run.join("spec").join(name)).map_err(|e| format!("copy contract: {e}"))?;
    }
    std::fs::write(run.join("receipt.json"), record_json).map_err(|e| format!("record: {e}"))?;
    Ok(run)
}

#[cfg(feature = "process")]
mod run;
#[cfg(feature = "process")]
pub use run::{judge, judge_with};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pass() -> Value {
        json!({"verdict":"Pass","findings":[],"violations":0,"pc_shape":"fired",
               "not_armed_shapes":[],"unarmed_violations":0,"w3c_cases_passed":19,
               "w3c_cases_n":19,"focus_nodes_n":5,"shapes_n":2,"plant_violations":25})
    }

    fn bytes(v: &Value) -> Vec<u8> {
        serde_json::to_vec(v).unwrap()
    }

    #[test]
    fn clean_pass_is_green_and_counts_are_carried() {
        let o = classify(Some(0), &bytes(&pass()));
        assert!(o.is_green(), "{o:?}");
        assert_eq!(
            (o.focus_nodes_n, o.shapes_n, o.plant_violations),
            (Some(5), Some(2), Some(25))
        );
    }

    /// Every way an exit-0 report can prove nothing is Red, for its own reason.
    #[test]
    fn a_pass_that_proves_nothing_is_red() {
        let cases: [(&str, Value, &str); 8] = [
            ("verdict", json!("Fail"), "verdict Fail"),
            ("findings", json!([{"message": "x"}]), "findings 1"),
            ("violations", json!(1), "violations 1"),
            ("pc_shape", json!("not-fired"), "pc_shape not-fired"),
            ("not_armed_shapes", json!(["s"]), "not armed [\"s\"]"),
            ("unarmed_violations", json!(1), "unarmed 1"),
            ("w3c_cases_passed", json!(18), "w3c 18/19"),
            ("w3c_cases_n", json!(0), "w3c 19/0"),
        ];
        for (k, v, why) in cases {
            let mut j = pass();
            j[k] = v;
            let o = classify(Some(0), &bytes(&j));
            assert!(!o.is_green(), "{k}");
            assert_eq!(o.why, why, "{k}");
        }
        let mut j = pass();
        j.as_object_mut().unwrap().remove("pc_shape");
        assert_eq!(classify(Some(0), &bytes(&j)).why, "pc_shape null");
        assert_eq!(classify(Some(0), b"").why, "no json");
    }

    #[test]
    fn exit_one_carries_every_message() {
        let j = json!({"verdict":"Fail","findings":[{"message":"b"},{"message":"a"}]});
        let o = classify(Some(1), &bytes(&j));
        assert_eq!(
            o.verdict,
            Verdict::Red {
                failed: vec!["b".into(), "a".into()]
            }
        );
        let empty = json!({"verdict":"Fail","findings":[]});
        assert_eq!(
            classify(Some(1), &bytes(&empty)).why,
            "exit 1 with no findings"
        );
    }

    #[test]
    fn decline_and_malformed_are_notrun_anything_else_red() {
        for c in [2, 3] {
            assert!(matches!(
                classify(Some(c), b"").verdict,
                Verdict::NotRun { .. }
            ));
        }
        for c in [Some(4), Some(-1), Some(101), None] {
            assert!(matches!(classify(c, b"").verdict, Verdict::Red { .. }));
        }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("dk-shapes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn each_call_gets_a_fresh_judge_dir() {
        let d = scratch("fresh");
        std::fs::create_dir_all(d.join("spec")).unwrap();
        std::fs::write(d.join("spec/c.yaml"), "x: 1\n").unwrap();
        let root = d.join("runs");
        let a = materialise(&d.join("spec"), b"{}", &root).unwrap();
        let b = materialise(&d.join("spec"), b"{}", &root).unwrap();
        assert_ne!(a, b);
        assert!(a.ends_with("judge-0") && b.ends_with("judge-1"));
        assert!(b.join("spec/c.yaml").is_file() && b.join("receipt.json").is_file());
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// M4: a run root inside a git work tree is refused, never judged.
    #[test]
    fn run_root_inside_git_is_refused() {
        let d = scratch("git");
        std::fs::create_dir_all(d.join("repo/.git")).unwrap();
        std::fs::create_dir_all(d.join("spec")).unwrap();
        std::fs::write(d.join("spec/c.yaml"), "x: 1\n").unwrap();
        let e = materialise(&d.join("spec"), b"{}", &d.join("repo/deep/runs")).unwrap_err();
        assert!(e.contains("git work tree"), "{e}");
        let e = materialise(&d.join("nospec"), b"{}", &d.join("runs")).unwrap_err();
        assert!(e.contains("spec dir"), "{e}");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
