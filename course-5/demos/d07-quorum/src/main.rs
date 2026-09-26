//! D07 — heterogeneous quorum (lesson 4.2, MEGA-001 §4.3 row 4.2 Plan B).
//!
//! Two independently-implemented reviewers look at the SAME planted diff: an
//! agy lane (`agy -p ... --output-format json --json-schema
//! schemas/verdict.json`) and a claude lane (`claude -p ... --output-format
//! json`, whose `result` field is parsed and validated with D04's client-side
//! validator). A third, apr, lane is never invoked at all — `--json-schema`
//! is refused by `apr =0.69.3` (see D04) — and is shown on screen labelled
//! `NotRun{Refused(--json-schema)}`. It is never faked as a pass, and it is
//! kept out of this demo's own `needs` so a refused verb the demo is
//! deliberately not calling cannot make the whole demo `NotRun` (R-8 /
//! MEGA-001 §4.3 row 4.2).
//!
//! Provable contract: quorum-heterogeneous-v1 — reviewing one planted diff
//! with 2 live lanes plus 1 labelled-refused lane, any FAIL blocks (the
//! standing quorum-one-fail-blocks-v1 truth table, reused from D13), NotRun
//! is never counted as a pass, and the blocking lane(s) are named on screen.

use d04_json_verdict::lanes::{self, Lane};
use demo_kit::harness::Harness;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

/// The one diff this demo reviews: a real, catalogued planted defect
/// (MANIFEST.toml: off-by-one, marker line 29 — `for i in 0..=self.len`).
const PLANTED_DIFF: &str = "planted-01.diff";

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let mut h = Harness::load(dir);

    if !h.not_run().is_empty() {
        println!("preflight: {:?}", h.not_run());
        let verdict = h.finish(BTreeMap::new());
        assert!(
            !verdict.is_green(),
            "a demo with a preflight reason must not be Green"
        );
        println!("contract: quorum-heterogeneous-v1 OK (NotRun before any lane ran)");
        std::process::exit(1);
    }

    let fixtures_dir = Path::new(dir).join("../fixtures/diffs");
    let schema_path = Path::new(dir).join("../d04-json-verdict/schemas/verdict.json");
    let diff_text = std::fs::read_to_string(fixtures_dir.join(PLANTED_DIFF))
        .unwrap_or_else(|e| panic!("reading {PLANTED_DIFF}: {e}"));

    println!("reviewing {PLANTED_DIFF} with 2 live lanes + 1 labelled-refused lane");

    let agy = lanes::run_agy_lane(&schema_path, &diff_text);
    println!(
        "agy lane: {} in {:?} ({})",
        agy.lane,
        agy.wall,
        if agy.detail.is_empty() {
            "ok"
        } else {
            agy.detail.as_str()
        }
    );

    let claude = lanes::run_claude_lane(&diff_text);
    println!(
        "claude lane: {} in {:?} ({})",
        claude.lane,
        claude.wall,
        if claude.detail.is_empty() {
            "ok"
        } else {
            claude.detail.as_str()
        }
    );

    let apr_lane = lanes::apr_lane_notrun_label();
    println!("apr lane: {apr_lane} (never invoked: --json-schema is refused at apr=0.69.3)");

    let live = [("agy", agy.lane), ("claude", claude.lane)];
    let overall = lanes::quorum(&[agy.lane, claude.lane, Lane::NotRun]);
    let blocked_by: Vec<&str> = live
        .iter()
        .filter(|(_, lane)| *lane == Lane::Fail)
        .map(|(name, _)| *name)
        .collect();
    println!("quorum: {overall} — blocked by: {blocked_by:?}");

    let lanes_run = live.len() as u32; // apr never ran; it is not counted here
    let measured: BTreeMap<String, serde_json::Value> = [
        ("lanes_run", json!(lanes_run)),
        ("quorum", json!(overall.to_string())),
        ("blocked_by_count", json!(blocked_by.len())),
        ("apr_lane", json!(apr_lane)),
        ("exit", json!(0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    let verdict = h.finish(measured);

    assert!(verdict.is_green(), "D07 is {verdict}");
    assert_eq!(
        lanes_run, 2,
        "quorum-heterogeneous-v1: exactly 2 live lanes ran"
    );
    assert_eq!(
        overall,
        Lane::Fail,
        "quorum-heterogeneous-v1: the planted diff must trip the quorum"
    );
    assert!(
        !blocked_by.is_empty(),
        "quorum-heterogeneous-v1: at least one lane must be named as blocking"
    );
    assert_eq!(
        apr_lane, "NotRun{Refused(--json-schema)}",
        "quorum-heterogeneous-v1: the apr lane is labelled, never faked as a pass"
    );
    println!("contract: quorum-heterogeneous-v1 OK");
}
