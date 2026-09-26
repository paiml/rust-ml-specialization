//! D13 — a deterministic reducer (lessons 2.2 and 5.2).
//!
//! An eval of ITEMS prompts is split into WORKERS disjoint partitions. Each
//! worker scores its partition on its own thread and sends the result back
//! whenever it finishes, so arrival order changes from run to run. The reducer
//! keys results by item and serialises them in item order, so the output bytes
//! do not depend on who finished first.
//!
//! Provable contract: fanout-independent-v1 + reduce-deterministic-v1 — the
//! reduced bytes are identical under every one of SHUFFLES LCG-shuffled arrival
//! orders and equal to a serial run; receipt-gate-v1 — a result without a
//! receipt makes the sweep NotRun, never Green; quorum-one-fail-blocks-v1 —
//! over all 27 three-lane rows, any FAIL is FAIL and NotRun is never a pass.

use demo_kit::harness::Harness;
use demo_kit::sha;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::mpsc;
use std::thread;

const ITEMS: u32 = 240;
const WORKERS: u32 = 8;
const SHUFFLES: u32 = 1000;

/// One worker's output: (item, score) pairs for its partition, and whether
/// it came back with a receipt (a digest of what it scored).
#[derive(Clone, Debug)]
struct WorkerResult {
    worker: u32,
    scores: Vec<(u32, u32)>,
    receipt: Option<String>,
}

/// Deterministic stand-in for "run the model on prompt i and grade it".
fn score(item: u32) -> u32 {
    // FNV-1a over the item id; 0..=99.
    let mut h: u32 = 0x811c_9dc5;
    for b in item.to_le_bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h % 100
}

fn run_worker(worker: u32) -> WorkerResult {
    let scores: Vec<(u32, u32)> = (0..ITEMS)
        .filter(|i| i % WORKERS == worker) // disjoint partitions
        .map(|i| (i, score(i)))
        .collect();
    let digest = sha::sha256_bytes(format!("{scores:?}").as_bytes());
    WorkerResult {
        worker,
        scores,
        receipt: Some(digest),
    }
}

/// The reducer: key by item, serialise in item order.
fn reduce(results: &[WorkerResult]) -> Vec<u8> {
    let merged: BTreeMap<u32, u32> = results
        .iter()
        .flat_map(|r| r.scores.iter().copied())
        .collect();
    let mut out = Vec::new();
    for (item, s) in merged {
        out.extend_from_slice(format!("item={item} score={s}\n").as_bytes());
    }
    out
}

/// The tempting wrong reducer: append in arrival order.
fn naive_reduce(results: &[WorkerResult]) -> Vec<u8> {
    let mut out = Vec::new();
    for r in results {
        for (item, s) in &r.scores {
            out.extend_from_slice(format!("item={item} score={s}\n").as_bytes());
        }
    }
    out
}

/// Numerical Recipes LCG: deterministic, stdlib-only.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Green,
    NotRun,
}

/// receipt-gate-v1: no receipt, not Green.
fn sweep_verdict(results: &[WorkerResult]) -> Verdict {
    if results.len() == WORKERS as usize && results.iter().all(|r| r.receipt.is_some()) {
        Verdict::Green
    } else {
        Verdict::NotRun
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane {
    Pass,
    Fail,
    NotRun,
}

/// quorum-one-fail-blocks-v1: any FAIL blocks; a NotRun lane is not a pass.
fn quorum(lanes: &[Lane]) -> Lane {
    if lanes.contains(&Lane::Fail) {
        Lane::Fail
    } else if lanes.contains(&Lane::NotRun) {
        Lane::NotRun
    } else {
        Lane::Pass
    }
}

fn main() {
    let mut h = Harness::load(env!("CARGO_MANIFEST_DIR"));
    println!("D13 deterministic reducer: {ITEMS} items, {WORKERS} workers, {SHUFFLES} shuffles");

    // Fan out: one thread per partition, results arrive in completion order.
    let (tx, rx) = mpsc::channel();
    for w in 0..WORKERS {
        let tx = tx.clone();
        thread::spawn(move || tx.send(run_worker(w)).expect("reducer alive"));
    }
    drop(tx);
    let arrived: Vec<WorkerResult> = rx.iter().collect();
    let order: Vec<u32> = arrived.iter().map(|r| r.worker).collect();
    println!("arrival order this run: {order:?}");

    let threaded = reduce(&arrived);
    let serial = reduce(&(0..WORKERS).map(run_worker).collect::<Vec<_>>());

    // fanout-independent-v1: shuffle arrival order SHUFFLES times.
    let mut lcg = Lcg(0x5eed);
    let mut shuffled = arrived.clone();
    let (mut equal, mut naive_divergent) = (0u32, 0u32);
    let naive_baseline = naive_reduce(&arrived);
    for _ in 0..SHUFFLES {
        lcg.shuffle(&mut shuffled);
        if reduce(&shuffled) == threaded {
            equal += 1;
        }
        if naive_reduce(&shuffled) != naive_baseline {
            naive_divergent += 1;
        }
    }
    println!("keyed reducer: {equal}/{SHUFFLES} shuffles byte-identical");
    println!("naive reducer: {naive_divergent}/{SHUFFLES} shuffles changed its output");

    // receipt-gate-v1: drop one worker's receipt.
    let mut missing = arrived.clone();
    missing[0].receipt = None;
    let gate_green = sweep_verdict(&arrived);
    let gate_missing = sweep_verdict(&missing);
    println!("receipt gate: all receipts -> {gate_green:?}; one missing -> {gate_missing:?}");

    // quorum-one-fail-blocks-v1: every three-lane row.
    let lanes = [Lane::Pass, Lane::Fail, Lane::NotRun];
    let mut rows = 0u32;
    let mut quorum_ok = true;
    for a in lanes {
        for b in lanes {
            for c in lanes {
                let v = quorum(&[a, b, c]);
                let any_fail = [a, b, c].contains(&Lane::Fail);
                let all_pass = [a, b, c] == [Lane::Pass; 3];
                quorum_ok &= (v == Lane::Fail) == any_fail && (v == Lane::Pass) == all_pass;
                rows += 1;
            }
        }
    }
    println!("quorum truth table: {rows} rows, one-fail-blocks holds: {quorum_ok}");

    let digest = sha::sha256_bytes(&threaded);
    println!("reduced output sha256: {digest}");
    let measured: BTreeMap<String, serde_json::Value> = [
        ("items", json!(ITEMS)),
        ("workers", json!(WORKERS)),
        ("shuffles", json!(SHUFFLES)),
        ("shuffles_identical", json!(equal)),
        ("serial_equals_threaded", json!(serial == threaded)),
        ("naive_divergent", json!(naive_divergent)),
        (
            "receipt_gate_blocks",
            json!(gate_green == Verdict::Green && gate_missing == Verdict::NotRun),
        ),
        ("quorum_rows", json!(rows)),
        ("quorum_one_fail_blocks", json!(quorum_ok)),
        ("reduce_sha256", json!(digest)),
        ("exit", json!(0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let verdict = h.finish(measured);

    assert!(verdict.is_green(), "D13 is {verdict}");
    assert_eq!(equal, SHUFFLES, "fanout-independent-v1");
    assert_eq!(serial, threaded, "reduce-deterministic-v1");
    assert_eq!(
        (gate_green, gate_missing),
        (Verdict::Green, Verdict::NotRun),
        "receipt-gate-v1"
    );
    assert!(quorum_ok && rows == 27, "quorum-one-fail-blocks-v1");
    println!("contract: fanout-independent-v1 OK");
    println!("contract: reduce-deterministic-v1 OK");
    println!("contract: receipt-gate-v1 OK");
    println!("contract: quorum-one-fail-blocks-v1 OK");
}
