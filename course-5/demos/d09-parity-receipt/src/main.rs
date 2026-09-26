//! D09 — a parity receipt (lesson 2.2).
//!
//! `apr parity` runs the same prompt on the GPU and on the CPU and compares
//! the logits position by position. This demo keeps its JSON as the receipt
//! and checks the weakest position against the declared cosine threshold. It
//! is a GPU-versus-CPU comparison; it does not compare apr with llama.cpp.
//! apr's exit code 12 means the build cannot measure this architecture: that
//! is a refusal, recorded as NotRun, never as parity.
//!
//! Provable contract: parity-receipt-v1 — every position's GPU/CPU cosine is
//! at least the threshold, apr reports parity, and the receipt holds the
//! per-position numbers that prove it.

use demo_kit::harness::Harness;
use demo_kit::NotRunReason;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const THRESHOLD: f64 = 0.98;
const REFUSED_EXIT: i32 = 12;

fn main() {
    let mut h = Harness::load(env!("CARGO_MANIFEST_DIR"));
    let mut measured: BTreeMap<String, Value> = BTreeMap::new();
    if h.not_run().is_empty() {
        let model = h.dir.join(&h.manifest.model.as_ref().expect("model").path);
        let model = model.to_string_lossy().into_owned();
        let (code, stdout, ms) = h.step("apr", &["parity", &model, "--json"]);
        measured.insert("exit".into(), json!(code));
        measured.insert("wall_ms".into(), json!(ms));
        if code == REFUSED_EXIT {
            h.refuse(NotRunReason::Refused("apr parity (exit 12)".into()));
        } else if let Ok(report) = serde_json::from_slice::<Value>(&stdout) {
            let cosines: Vec<f64> = report["metrics"]
                .as_array()
                .map(|m| {
                    m.iter()
                        .filter_map(|p| p["cosine_similarity"].as_f64())
                        .collect()
                })
                .unwrap_or_default();
            let min = cosines.iter().copied().fold(f64::INFINITY, f64::min);
            println!(
                "positions: {}   weakest cosine: {min:.6}   threshold: {THRESHOLD}",
                cosines.len()
            );
            measured.insert("positions".into(), json!(cosines.len()));
            measured.insert("min_cosine".into(), json!(min));
            measured.insert("parity".into(), report["parity"].clone());
            measured.insert("failed".into(), report["failed"].clone());
            measured.insert("threshold".into(), json!(THRESHOLD));
        }
    }
    let verdict = h.finish(measured.clone());

    assert!(verdict.is_green(), "D09 is {verdict}");
    let min = measured["min_cosine"].as_f64().unwrap_or(0.0);
    assert!(
        min >= THRESHOLD,
        "parity-receipt-v1: weakest cosine {min} < {THRESHOLD}"
    );
    println!("contract: parity-receipt-v1 OK");
}
