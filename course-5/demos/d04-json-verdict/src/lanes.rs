//! Review lanes shared by D07 (quorum) and D08 (planted-defect sweep): run
//! `agy` and `claude` as child processes over one diff's text, parse each
//! tool's structured output, and validate it against `schemas/verdict.json`
//! with [`crate::validator`].
//!
//! H-6: this calls the tools *under demonstration* via `std::process::Command`
//! directly — never a shell, never curl. `cargo test` never invokes the real
//! CLIs; only the demo binaries (`d07-quorum`, `d08-planted-defect`) do.

use crate::validator;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Pass,
    Fail,
    NotRun,
}

impl std::fmt::Display for Lane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lane::Pass => write!(f, "PASS"),
            Lane::Fail => write!(f, "FAIL"),
            Lane::NotRun => write!(f, "NotRun"),
        }
    }
}

/// quorum-one-fail-blocks-v1: any FAIL blocks; a NotRun lane is not a pass.
pub fn quorum(lanes: &[Lane]) -> Lane {
    if lanes.contains(&Lane::Fail) {
        Lane::Fail
    } else if lanes.contains(&Lane::NotRun) {
        Lane::NotRun
    } else {
        Lane::Pass
    }
}

#[derive(Debug, Clone)]
pub struct LaneOutcome {
    pub name: &'static str,
    pub lane: Lane,
    pub finding_lines: Vec<i64>,
    pub wall: Duration,
    /// Empty when the lane parsed and validated cleanly; otherwise why it
    /// came back `NotRun`, printed on screen for a human to read.
    pub detail: String,
}

/// The instruction every lane gets, over one diff's raw text. `line` in a
/// finding is the 1-based line number **within this diff text**, counting
/// from the `--- a/...` line — these diffs are reviewed as opaque text, not
/// applied with `patch`, so that is the only line numbering both sides share.
pub fn review_prompt(diff_text: &str) -> String {
    format!(
        "You are reviewing a unified diff of a small Rust change for \
correctness bugs. Reply with ONLY a single JSON object, no prose, matching \
this shape: {{\"verdict\":\"PASS\"|\"FAIL\",\"findings\":[{{\"line\":<int>,\
\"class\":\"<short-tag>\",\"why\":\"<one sentence>\"}}]}}. The \"line\" field \
is the 1-based line number counting from the very first line of the diff \
text below (the \"--- a/...\" line is line 1). If you find a real bug, \
verdict is FAIL with at least one finding pointing at the exact line that \
introduces it; if you find nothing, verdict is PASS with an empty findings \
array.\n\nDIFF:\n{diff_text}"
    )
}

/// Validate a candidate verdict object and pull out what the reducer needs.
fn from_value(v: &Value) -> Result<(Lane, Vec<i64>), String> {
    validator::validate_verdict(v).map_err(|problems| {
        problems
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let verdict = v["verdict"].as_str().unwrap_or("");
    let lines: Vec<i64> = v["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f["line"].as_i64())
        .collect();
    match verdict {
        "PASS" => Ok((Lane::Pass, lines)),
        "FAIL" => Ok((Lane::Fail, lines)),
        other => Err(format!("unexpected verdict {other:?}")),
    }
}

/// `agy -p <prompt> --output-format json --json-schema <schema_path>`;
/// validates the `structured_output` field agy returns alongside its
/// free-text `response` (MEGA-001 §4.3 row 4.2: agy 1.2.11 supports
/// `--json-schema`).
pub fn run_agy_lane(schema_path: &Path, diff_text: &str) -> LaneOutcome {
    let prompt = review_prompt(diff_text);
    let start = Instant::now();
    let run = Command::new("agy")
        .arg("-p")
        .arg(&prompt)
        .arg("--output-format")
        .arg("json")
        .arg("--json-schema")
        .arg(schema_path)
        .output();
    finish_lane("agy", start.elapsed(), run, |top| {
        top.get("structured_output")
            .cloned()
            .ok_or_else(|| "agy output had no structured_output field".to_string())
    })
}

/// `claude -p <prompt> --output-format json`; parses the `result` field
/// (the assistant's text reply) as JSON and validates that.
pub fn run_claude_lane(diff_text: &str) -> LaneOutcome {
    let prompt = review_prompt(diff_text);
    let start = Instant::now();
    let run = Command::new("claude")
        .arg("-p")
        .arg(&prompt)
        .arg("--output-format")
        .arg("json")
        .output();
    finish_lane("claude", start.elapsed(), run, |top| {
        let result_str = top
            .get("result")
            .and_then(Value::as_str)
            .ok_or_else(|| "claude output had no string result field".to_string())?;
        serde_json::from_str(result_str)
            .map_err(|e| format!("claude result was not JSON: {e} ({result_str:?})"))
    })
}

fn finish_lane(
    name: &'static str,
    wall: Duration,
    run: std::io::Result<std::process::Output>,
    extract: impl FnOnce(&Value) -> Result<Value, String>,
) -> LaneOutcome {
    let mk = |lane: Lane, lines: Vec<i64>, detail: String| LaneOutcome {
        name,
        lane,
        finding_lines: lines,
        wall,
        detail,
    };
    let out = match run {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            return mk(
                Lane::NotRun,
                vec![],
                format!(
                    "{name} exited {}: {}",
                    o.status,
                    String::from_utf8_lossy(&o.stderr)
                ),
            )
        }
        Err(e) => return mk(Lane::NotRun, vec![], format!("{name} did not run: {e}")),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let top: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            return mk(
                Lane::NotRun,
                vec![],
                format!("{name} output was not JSON: {e}"),
            )
        }
    };
    let candidate = match extract(&top) {
        Ok(v) => v,
        Err(e) => return mk(Lane::NotRun, vec![], e),
    };
    match from_value(&candidate) {
        Ok((lane, lines)) => mk(lane, lines, String::new()),
        Err(e) => mk(Lane::NotRun, vec![], e),
    }
}

/// The always-refused apr lane at this pin, labelled — never faked, never
/// actually invoked (`--json-schema` is refused by `apr =0.69.3`; see D04).
pub fn apr_lane_notrun_label() -> String {
    demo_kit::Verdict::NotRun {
        reasons: vec![demo_kit::NotRunReason::Refused("--json-schema".to_string())],
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quorum_truth_table_matches_one_fail_blocks() {
        let lanes = [Lane::Pass, Lane::Fail, Lane::NotRun];
        for a in lanes {
            for b in lanes {
                for c in lanes {
                    let v = quorum(&[a, b, c]);
                    let any_fail = [a, b, c].contains(&Lane::Fail);
                    let all_pass = [a, b, c] == [Lane::Pass; 3];
                    assert_eq!(v == Lane::Fail, any_fail, "{a:?} {b:?} {c:?}");
                    assert_eq!(v == Lane::Pass, all_pass, "{a:?} {b:?} {c:?}");
                }
            }
        }
    }

    #[test]
    fn from_value_reads_fail_and_lines() {
        let v = json!({"verdict": "FAIL", "findings": [{"line": 29, "class": "x", "why": "y"}]});
        assert_eq!(from_value(&v), Ok((Lane::Fail, vec![29])));
    }

    #[test]
    fn from_value_reads_pass_with_no_findings() {
        let v = json!({"verdict": "PASS", "findings": []});
        assert_eq!(from_value(&v), Ok((Lane::Pass, vec![])));
    }

    #[test]
    fn from_value_rejects_a_schema_violation() {
        let v = json!({"verdict": "MAYBE", "findings": []});
        assert!(from_value(&v).is_err());
    }

    #[test]
    fn apr_lane_label_matches_the_harness_display() {
        assert_eq!(apr_lane_notrun_label(), "NotRun{Refused(--json-schema)}");
    }
}
