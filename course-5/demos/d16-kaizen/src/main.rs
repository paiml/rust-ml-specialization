//! D16 — kaizen (lesson 5.2 capstone, concept C10): baseline -> ratchet from
//! committed receipts.
//!
//! Provable contract: ratchet-monotone-v1 — a new accepted baseline is never
//! written unless every metric already accepted holds or improves against it,
//! and the demo itself is never Green unless at least one Green receipt was
//! actually found (a gate with nothing behind it is not a gate).
//!
//! This reads receipts (the JSON shape from `demo_kit::receipt::Receipt`, not
//! that type itself — it derives `Serialize` only, never `Deserialize`, by
//! design: a receipt is written once and read as data) from
//! `$RFML5_RECEIPTS/<demo-id>/**`, groups them by demo id, and for each id:
//!
//! - picks a metric set: `wall_ms` (direction `lower`) if the latest Green
//!   receipt measured it; otherwise the deterministic fallback named in
//!   MEGA-001 §D16 — a synthetic `green_run_count` (direction `higher`), plus
//!   `shuffles_identical` (direction `higher`) when a receipt happens to carry
//!   it, as D13's do;
//! - computes baseline (that metric's value on the chronologically first
//!   Green receipt, or `1` for the synthetic count) and current (the latest
//!   Green receipt, or the live count);
//! - compares against the **accepted** baseline in `baseline.toml` when one
//!   is already committed there, falling back to the freshly bootstrapped
//!   value otherwise.
//!
//! Design note (not a demo-kit change): the ratchet direction table lives in
//! `baseline.toml`, not in `[ratchet]` inside `demo.toml`, because the shared
//! `DemoManifest` (`demo-kit`, not a path this ticket may edit) parses
//! `demo.toml` with `#[serde(deny_unknown_fields)]` — an extra table there
//! would break `Harness::load` and `xtask verify` for every demo, not only
//! this one.

use demo_kit::harness::Harness;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Runtime `CARGO_MANIFEST_DIR` (set by `cargo run`) wins over the
/// compile-time one, matching the fix already landed in `Harness::load`.
fn manifest_dir() -> PathBuf {
    std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Direction {
    Higher,
    Lower,
}

impl Direction {
    fn holds(self, current: f64, baseline: f64) -> bool {
        match self {
            Direction::Higher => current >= baseline,
            Direction::Lower => current <= baseline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RatchetEntry {
    direction: Direction,
    baseline: f64,
}

/// demo id -> metric name -> accepted entry.
type BaselineFile = BTreeMap<String, BTreeMap<String, RatchetEntry>>;

fn load_baseline(path: &Path) -> BaselineFile {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_baseline(path: &Path, file: &BaselineFile) -> std::io::Result<()> {
    let header = "# Accepted kaizen baselines (ratchet-monotone-v1). Rewritten only by\n\
                  # `cargo run -p d16-kaizen -- --update`, and only when every metric held\n\
                  # or improved. See src/main.rs for the schema.\n\n";
    let body = toml::to_string_pretty(file).map_err(std::io::Error::other)?;
    std::fs::write(path, format!("{header}{body}"))
}

/// ratchet-monotone-v1: `proposed` is accepted only if, for every metric that
/// already had an accepted entry in `old`, the proposed value holds or
/// improves against it. A metric with no prior entry is always accepted —
/// there is nothing to regress against yet.
fn try_update(old: &BaselineFile, proposed: &BaselineFile) -> Result<BaselineFile, Vec<String>> {
    let mut regressions = Vec::new();
    for (demo, metrics) in proposed {
        for (metric, new_entry) in metrics {
            if let Some(old_entry) = old.get(demo).and_then(|m| m.get(metric)) {
                if !new_entry
                    .direction
                    .holds(new_entry.baseline, old_entry.baseline)
                {
                    regressions.push(format!(
                        "{demo}.{metric}: accepted {} -> proposed {} regresses ({:?}-is-better)",
                        old_entry.baseline, new_entry.baseline, new_entry.direction
                    ));
                }
            }
        }
    }
    if !regressions.is_empty() {
        return Err(regressions);
    }
    let mut merged = old.clone();
    for (demo, metrics) in proposed {
        let slot = merged.entry(demo.clone()).or_default();
        for (metric, entry) in metrics {
            slot.insert(metric.clone(), entry.clone());
        }
    }
    Ok(merged)
}

/// The receipt JSON shape this demo reads, kept deliberately minimal (no
/// `deny_unknown_fields`): a receipt carries more than this, and a field this
/// demo does not know about must never break parsing.
#[derive(Debug, Clone, Deserialize)]
struct RawReceipt {
    id: String,
    run_id: String,
    #[serde(default)]
    measured: BTreeMap<String, JsonValue>,
    verdict: RawVerdict,
}

#[derive(Debug, Clone, Deserialize)]
struct RawVerdict {
    verdict: String,
}

struct ReceiptRow {
    run_id: String,
    measured: BTreeMap<String, JsonValue>,
}

/// `(secs, nanos)` from a `receipt::run_id()` string (`"{secs}-{nanos:09}-{pid}"`)
/// — sorts receipts into the order they were actually written.
fn run_order_key(run_id: &str) -> (u64, u64) {
    let mut parts = run_id.splitn(3, '-');
    let secs = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let nanos = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (secs, nanos)
}

fn walk_json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_json_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            out.push(path);
        }
    }
}

/// Every Green receipt found under `root`, grouped by demo id and sorted
/// chronologically within each group.
fn load_green_receipts(root: &Path) -> BTreeMap<String, Vec<ReceiptRow>> {
    let mut files = Vec::new();
    walk_json_files(root, &mut files);
    let mut by_id: BTreeMap<String, Vec<ReceiptRow>> = BTreeMap::new();
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let Ok(raw) = serde_json::from_str::<RawReceipt>(&text) else {
            continue;
        };
        if raw.verdict.verdict != "Green" {
            continue;
        }
        by_id.entry(raw.id).or_default().push(ReceiptRow {
            run_id: raw.run_id,
            measured: raw.measured,
        });
    }
    for rows in by_id.values_mut() {
        rows.sort_by_key(|r| run_order_key(&r.run_id));
    }
    by_id
}

struct Row {
    demo: String,
    metric: String,
    direction: Direction,
    baseline: f64,
    current: f64,
    holds: bool,
}

/// The metric set to ratchet for one demo id, deterministic and derived only
/// from what its latest Green receipt actually measured (MEGA-001 D16: prefer
/// `wall_ms`; fall back to a count of Green runs, plus `shuffles_identical`
/// when present).
fn metrics_for(latest_measured: &BTreeMap<String, JsonValue>) -> Vec<(String, Direction)> {
    let mut out = Vec::new();
    if latest_measured.contains_key("wall_ms") {
        out.push(("wall_ms".to_string(), Direction::Lower));
    }
    out.push(("green_run_count".to_string(), Direction::Higher));
    if latest_measured.contains_key("shuffles_identical") {
        out.push(("shuffles_identical".to_string(), Direction::Higher));
    }
    out
}

fn metric_value(
    metric: &str,
    green_run_count: usize,
    measured: &BTreeMap<String, JsonValue>,
) -> Option<f64> {
    if metric == "green_run_count" {
        Some(green_run_count as f64)
    } else {
        measured.get(metric).and_then(JsonValue::as_f64)
    }
}

/// Runs the kaizen sweep: reads `root`, compares against `accepted`, prints
/// the baseline-vs-current table, and returns the evaluated rows plus the
/// proposed baseline an `--update` would try to accept.
fn sweep(
    root: &Path,
    accepted: &BaselineFile,
) -> (Vec<Row>, BaselineFile, u64 /* green receipts found */) {
    let by_id = load_green_receipts(root);
    let mut rows = Vec::new();
    let mut proposed: BaselineFile = BaselineFile::new();
    let mut green_receipts_found = 0u64;

    for (demo, receipts) in &by_id {
        green_receipts_found += receipts.len() as u64;
        let Some(first) = receipts.first() else {
            continue;
        };
        let Some(latest) = receipts.last() else {
            continue;
        };
        for (metric, direction) in metrics_for(&latest.measured) {
            let Some(current) = metric_value(&metric, receipts.len(), &latest.measured) else {
                continue;
            };
            let bootstrap = metric_value(&metric, 1, &first.measured).unwrap_or(current);
            let baseline = accepted
                .get(demo)
                .and_then(|m| m.get(&metric))
                .map(|e| e.baseline)
                .unwrap_or(bootstrap);
            let holds = direction.holds(current, baseline);
            proposed.entry(demo.clone()).or_default().insert(
                metric.clone(),
                RatchetEntry {
                    direction,
                    baseline: current,
                },
            );
            rows.push(Row {
                demo: demo.clone(),
                metric,
                direction,
                baseline,
                current,
                holds,
            });
        }
    }
    (rows, proposed, green_receipts_found)
}

fn print_table(rows: &[Row]) {
    println!(
        "{:<16} {:<20} {:<7} {:>14} {:>14}  verdict",
        "demo", "metric", "dir", "baseline", "current"
    );
    for r in rows {
        let dir = match r.direction {
            Direction::Higher => "higher",
            Direction::Lower => "lower",
        };
        println!(
            "{:<16} {:<20} {:<7} {:>14} {:>14}  {}",
            r.demo,
            r.metric,
            dir,
            r.baseline,
            r.current,
            if r.holds {
                "hold/improve"
            } else {
                "REGRESSION"
            }
        );
    }
}

fn run_kaizen(do_update: bool) -> BTreeMap<String, JsonValue> {
    let receipts_root = std::env::var("RFML5_RECEIPTS").map(PathBuf::from).ok();
    let baseline_path = manifest_dir().join("baseline.toml");
    let accepted = load_baseline(&baseline_path);

    let (rows, proposed, green_receipts_found) = match &receipts_root {
        Some(root) if root.is_dir() => sweep(root, &accepted),
        Some(root) => {
            println!(
                "RFML5_RECEIPTS={} does not exist; nothing to audit",
                root.display()
            );
            (Vec::new(), BaselineFile::new(), 0)
        }
        None => {
            println!("RFML5_RECEIPTS is not set; nothing to audit (never a floor-less gate)");
            (Vec::new(), BaselineFile::new(), 0)
        }
    };

    print_table(&rows);

    let demos_audited = rows
        .iter()
        .map(|r| r.demo.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .len() as u64;
    let metrics_checked = rows.len() as u64;
    let ratchet_holds = metrics_checked > 0 && rows.iter().all(|r| r.holds);

    if do_update {
        match try_update(&accepted, &proposed) {
            Ok(merged) => match save_baseline(&baseline_path, &merged) {
                Ok(()) => println!(
                    "--update: accepted a new baseline at {} ({} demo(s), {} metric(s))",
                    baseline_path.display(),
                    merged.len(),
                    merged.values().map(|m| m.len()).sum::<usize>()
                ),
                Err(e) => println!("--update: computed a new baseline but could not write it: {e}"),
            },
            Err(reasons) => {
                println!(
                    "--update: REFUSED (ratchet-monotone-v1) — a new baseline never regresses:"
                );
                for r in &reasons {
                    println!("  - {r}");
                }
            }
        }
    }

    [
        (
            "green_receipts_found",
            serde_json::json!(green_receipts_found),
        ),
        ("demos_audited", serde_json::json!(demos_audited)),
        ("metrics_checked", serde_json::json!(metrics_checked)),
        ("ratchet_holds", serde_json::json!(ratchet_holds)),
        ("exit", serde_json::json!(0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

fn main() {
    let mut h = Harness::load(env!("CARGO_MANIFEST_DIR"));
    println!("D16 kaizen: baseline vs current from committed receipts");
    let do_update = std::env::args().any(|a| a == "--update");

    let measured = if h.not_run().is_empty() {
        run_kaizen(do_update)
    } else {
        BTreeMap::new()
    };

    let green_receipts_found = measured
        .get("green_receipts_found")
        .and_then(JsonValue::as_u64);
    let ratchet_holds = measured.get("ratchet_holds").and_then(JsonValue::as_bool);

    let verdict = h.finish(measured);
    assert!(verdict.is_green(), "D16 is {verdict}");
    assert!(
        green_receipts_found.is_some_and(|n| n >= 1),
        "no floor-less gate: at least one Green receipt must exist"
    );
    assert_eq!(
        ratchet_holds,
        Some(true),
        "ratchet-monotone-v1: current is never worse than baseline"
    );
    println!("contract: ratchet-monotone-v1 OK");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(direction: Direction, baseline: f64) -> RatchetEntry {
        RatchetEntry {
            direction,
            baseline,
        }
    }

    fn file(demo: &str, metric: &str, e: RatchetEntry) -> BaselineFile {
        let mut m = BTreeMap::new();
        m.insert(metric.to_string(), e);
        let mut f = BaselineFile::new();
        f.insert(demo.to_string(), m);
        f
    }

    /// ratchet-monotone-v1 (mutation): a proposed baseline that regresses one
    /// metric is refused outright, even though the file has other metrics.
    #[test]
    fn ratchet_monotone_v1_refuses_a_regression() {
        let old = file(
            "d13-reducer",
            "green_run_count",
            entry(Direction::Higher, 10.0),
        );
        let worse = file(
            "d13-reducer",
            "green_run_count",
            entry(Direction::Higher, 5.0),
        );
        let err = try_update(&old, &worse).expect_err("a regression must be refused");
        assert!(err.iter().any(|r| r.contains("green_run_count")), "{err:?}");
    }

    #[test]
    fn ratchet_monotone_v1_accepts_hold_or_improve() {
        let old = file(
            "d13-reducer",
            "green_run_count",
            entry(Direction::Higher, 10.0),
        );
        let held = file(
            "d13-reducer",
            "green_run_count",
            entry(Direction::Higher, 10.0),
        );
        assert!(try_update(&old, &held).is_ok());
        let improved = file(
            "d13-reducer",
            "green_run_count",
            entry(Direction::Higher, 11.0),
        );
        let merged = try_update(&old, &improved).expect("an improvement is accepted");
        assert_eq!(merged["d13-reducer"]["green_run_count"].baseline, 11.0);
    }

    #[test]
    fn ratchet_monotone_v1_lower_direction_regression_is_refused() {
        let old = file("d09-parity", "wall_ms", entry(Direction::Lower, 100.0));
        let worse = file("d09-parity", "wall_ms", entry(Direction::Lower, 150.0));
        assert!(try_update(&old, &worse).is_err());
        let better = file("d09-parity", "wall_ms", entry(Direction::Lower, 80.0));
        assert!(try_update(&old, &better).is_ok());
    }

    #[test]
    fn a_new_metric_with_no_prior_entry_is_always_accepted() {
        let old = BaselineFile::new();
        let fresh = file(
            "d13-reducer",
            "shuffles_identical",
            entry(Direction::Higher, 1000.0),
        );
        assert!(try_update(&old, &fresh).is_ok());
    }

    #[test]
    fn direction_holds_matches_higher_and_lower() {
        assert!(Direction::Higher.holds(5.0, 5.0));
        assert!(Direction::Higher.holds(6.0, 5.0));
        assert!(!Direction::Higher.holds(4.0, 5.0));
        assert!(Direction::Lower.holds(5.0, 5.0));
        assert!(Direction::Lower.holds(4.0, 5.0));
        assert!(!Direction::Lower.holds(6.0, 5.0));
    }

    #[test]
    fn run_order_key_sorts_chronologically() {
        assert!(run_order_key("100-000000001-1") < run_order_key("100-000000002-1"));
        assert!(run_order_key("100-999999999-1") < run_order_key("101-000000000-1"));
    }
}
