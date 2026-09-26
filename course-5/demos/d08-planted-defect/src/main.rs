//! D08 — planted-defect sweep (lesson 5.1, MEGA-001 §4.3 row 5.1 Plan B).
//!
//! Two review lanes (agy, claude — the same lanes as D07) sweep a
//! budget-sized, clean:planted-balanced subset of the corpus. `n` is chosen
//! at runtime, never hard-coded: measure per-review wall time on 2 probe
//! diffs (1 clean + 1 planted), then `n = floor(250s / slowest_per_review_s
//! / 2 lanes)`, clamped to `[4, 20]` and rounded down to even
//! ([`sweep::choose_n`]). Recall and precision per lane are scored against
//! `MANIFEST.toml`'s ground truth ([`sweep::score_lane`]): a lane "finds" a
//! planted defect when it answers FAIL and one of its findings lands within
//! +-3 lines of the declared marker; FAIL on a clean diff is a false
//! positive. As with D07, the apr lane is never invoked — `--json-schema` is
//! refused at `apr=0.69.3` — and is shown on screen labelled, never faked.
//!
//! Provable contract: planted-defect-sweep-v1 — `n` lands in `[4, 20]` and
//! is even, the subset stays clean:planted balanced, both lanes ran on
//! every diff in it, and each lane's measured recall/precision meets a
//! floor set from three prior runs (never tuned to match a single run).

use d04_json_verdict::lanes::{self, Lane};
use d08_planted_defect::corpus::{self, ManifestFile};
use d08_planted_defect::sweep::{self, Scored};
use demo_kit::harness::Harness;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// PAIML ceiling for this demo (MEGA-001 §5.1 D08 row): fit inside 300s.
const BUDGET_S: f64 = 250.0;

struct DiffRun {
    file: ManifestFile,
    agy_failed: bool,
    agy_lines: Vec<i64>,
    claude_failed: bool,
    claude_lines: Vec<i64>,
}

fn review_one(schema_path: &Path, fixtures_dir: &Path, file: &ManifestFile) -> (DiffRun, f64) {
    let diff_text = std::fs::read_to_string(fixtures_dir.join(&file.path))
        .unwrap_or_else(|e| panic!("reading {}: {e}", file.path));
    let agy = lanes::run_agy_lane(schema_path, &diff_text);
    let claude = lanes::run_claude_lane(&diff_text);
    let slowest = agy.wall.as_secs_f64().max(claude.wall.as_secs_f64());
    println!(
        "  {}: agy={} in {:.1}s ({}) | claude={} in {:.1}s ({})",
        file.path,
        agy.lane,
        agy.wall.as_secs_f64(),
        if agy.detail.is_empty() {
            "ok"
        } else {
            agy.detail.as_str()
        },
        claude.lane,
        claude.wall.as_secs_f64(),
        if claude.detail.is_empty() {
            "ok"
        } else {
            claude.detail.as_str()
        },
    );
    (
        DiffRun {
            file: file.clone(),
            agy_failed: agy.lane == Lane::Fail,
            agy_lines: agy.finding_lines,
            claude_failed: claude.lane == Lane::Fail,
            claude_lines: claude.finding_lines,
        },
        slowest,
    )
}

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
        println!("contract: planted-defect-sweep-v1 OK (NotRun before any lane ran)");
        std::process::exit(1);
    }

    let fixtures_dir: PathBuf = Path::new(dir).join("../fixtures/diffs");
    let schema_path: PathBuf = Path::new(dir).join("../d04-json-verdict/schemas/verdict.json");

    let manifest = corpus::load(&fixtures_dir);
    corpus::check_bijection(&fixtures_dir, &manifest)
        .unwrap_or_else(|e| panic!("planted-defect-corpus-v1 bijection: {e:?}"));
    corpus::check_markers(&fixtures_dir, &manifest)
        .unwrap_or_else(|e| panic!("planted-defect-corpus-v1 markers: {e:?}"));

    let (clean_ordered, planted_ordered) = manifest.ordered();
    assert!(clean_ordered.len() >= 10 && planted_ordered.len() >= 10);

    println!("probing per-review wall time on 2 diffs (1 clean + 1 planted)...");
    let overall_start = Instant::now();
    let (probe_clean, t1) = review_one(&schema_path, &fixtures_dir, clean_ordered[0]);
    let (probe_planted, t2) = review_one(&schema_path, &fixtures_dir, planted_ordered[0]);
    let slowest = t1.max(t2);
    let n = sweep::choose_n(slowest, BUDGET_S);
    println!("slowest measured per-review time: {slowest:.1}s -> n = {n}");

    let half = (n / 2) as usize;
    let mut runs: Vec<DiffRun> = Vec::with_capacity(n as usize);
    runs.push(probe_clean);
    for f in clean_ordered.iter().copied().skip(1).take(half - 1) {
        let (r, _) = review_one(&schema_path, &fixtures_dir, f);
        runs.push(r);
    }
    runs.push(probe_planted);
    for f in planted_ordered.iter().copied().skip(1).take(half - 1) {
        let (r, _) = review_one(&schema_path, &fixtures_dir, f);
        runs.push(r);
    }
    let total_wall_s = overall_start.elapsed().as_secs_f64();

    assert_eq!(runs.len(), n as usize, "reviewed exactly n diffs");
    let clean_count = runs.iter().filter(|r| !r.file.planted).count();
    let planted_count = runs.iter().filter(|r| r.file.planted).count();
    let balanced = clean_count == planted_count;
    assert!(balanced, "clean:planted subset must stay balanced");

    let refs: Vec<&ManifestFile> = runs.iter().map(|r| &r.file).collect();
    let agy_failed: Vec<bool> = runs.iter().map(|r| r.agy_failed).collect();
    let agy_lines: Vec<Vec<i64>> = runs.iter().map(|r| r.agy_lines.clone()).collect();
    let claude_failed: Vec<bool> = runs.iter().map(|r| r.claude_failed).collect();
    let claude_lines: Vec<Vec<i64>> = runs.iter().map(|r| r.claude_lines.clone()).collect();

    let agy_scored: Scored = sweep::score_lane(&refs, &agy_failed, &agy_lines);
    let claude_scored: Scored = sweep::score_lane(&refs, &claude_failed, &claude_lines);
    let apr_lane = lanes::apr_lane_notrun_label();

    println!(
        "n = {n} ({clean_count} clean + {planted_count} planted), total wall {total_wall_s:.1}s"
    );
    println!(
        "agy:    recall {:.2} precision {:.2} (tp={} fp={})",
        agy_scored.recall,
        agy_scored.precision,
        agy_scored.true_positives,
        agy_scored.false_positives
    );
    println!(
        "claude: recall {:.2} precision {:.2} (tp={} fp={})",
        claude_scored.recall,
        claude_scored.precision,
        claude_scored.true_positives,
        claude_scored.false_positives
    );
    println!("apr lane: {apr_lane} (never invoked: --json-schema is refused at apr=0.69.3)");

    let measured: BTreeMap<String, serde_json::Value> = [
        ("n", json!(n)),
        ("clean_count", json!(clean_count)),
        ("planted_count", json!(planted_count)),
        ("balanced", json!(balanced)),
        ("lanes_run", json!(2)),
        ("recall_agy", json!(agy_scored.recall)),
        ("precision_agy", json!(agy_scored.precision)),
        ("recall_claude", json!(claude_scored.recall)),
        ("precision_claude", json!(claude_scored.precision)),
        ("apr_lane", json!(apr_lane)),
        ("total_wall_s", json!(total_wall_s)),
        ("exit", json!(0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    let verdict = h.finish(measured);

    assert!(verdict.is_green(), "D08 is {verdict}");
    assert!(
        (4..=20).contains(&n),
        "planted-defect-sweep-v1: n in [4, 20]"
    );
    assert_eq!(n % 2, 0, "planted-defect-sweep-v1: n is even (balanced)");
    assert_eq!(clean_count, planted_count);
    assert_eq!(
        apr_lane, "NotRun{Refused(--json-schema)}",
        "planted-defect-sweep-v1: the apr lane is labelled, never faked as a pass"
    );
    println!("contract: planted-defect-sweep-v1 OK");
}
