//! D04 — json-verdict, Plan B (lesson 3.2, MEGA-001 §4.3 row 3.2).
//!
//! `--json-schema` does not exist in `apr run --help` or `apr serve run
//! --help` at the pin `apr =0.69.3` (probed directly: neither help text
//! contains the flag). A demo that needed it to produce a Green verdict
//! would therefore never legitimately go Green at this pin — so this demo's
//! claim is narrower and honest: preflight refuses the flag, the harness
//! reports `NotRun{Refused(--json-schema)}`, and the program exits non-zero.
//! It never fabricates a passing run to stand in for the real one (S-3).
//!
//! What *is* demonstrated, and does go green as ordinary Rust unit tests:
//! the client-side validator a lane would run against `{verdict, findings[]}`
//! structured output (`src/validator.rs`), checked against a digest-pinned
//! copy of `schemas/verdict.json` so the client trusts its own file, never
//! whatever the server happens to be serving.
//!
//! Provable contract: json-verdict-refusal-v1 — at apr=0.69.3 this demo's
//! preflight is exactly `[Refused(--json-schema)]`, its verdict is never
//! Green, and the schema copy on disk matches its pinned digest.

use d04_json_verdict::validator;
use demo_kit::harness::Harness;
use demo_kit::NotRunReason;
use serde_json::json;
use std::collections::BTreeMap;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let mut h = Harness::load(dir);

    let not_run = h.not_run().to_vec();
    println!("preflight: {not_run:?}");

    // Demonstrate the client-side validator regardless of the refusal above:
    // it is a standalone contract, exercised here on a fixed sample so the
    // run has something concrete to print (unit tests in validator.rs cover
    // it exhaustively).
    let sample = json!({
        "verdict": "FAIL",
        "findings": [{"line": 29, "class": "off-by-one", "why": "0..=len reads one past the end"}]
    });
    let validator_ok = validator::validate_verdict(&sample).is_ok();
    println!("validator self-check on a sample verdict: {validator_ok}");

    let schema_path = std::path::Path::new(dir).join("schemas/verdict.json");
    let pinned = h
        .manifest
        .schema_copy_sha256
        .clone()
        .expect("demo.toml pins schema_copy_sha256");
    let schema_copy_matches = validator::check_schema_copy(&schema_path, &pinned).is_ok();
    println!("schema copy sha256 matches its pin: {schema_copy_matches}");

    let measured: BTreeMap<String, serde_json::Value> = [
        ("exit", json!(0)),
        ("parse_rate", json!(if validator_ok { 1.0 } else { 0.0 })),
        ("schema_copy_matches", json!(schema_copy_matches)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    let verdict = h.finish(measured);

    assert!(
        !verdict.is_green(),
        "json-verdict-refusal-v1: D04 must never be Green at apr=0.69.3 (--json-schema is refused), got {verdict}"
    );
    assert_eq!(
        not_run,
        vec![NotRunReason::Refused("--json-schema".to_string())],
        "json-verdict-refusal-v1: preflight must be exactly NotRun{{Refused(--json-schema)}}"
    );
    assert!(
        validator_ok && schema_copy_matches,
        "the client-side validator and its digest-pinned schema copy must both hold even though the server verb is refused"
    );
    println!("contract: json-verdict-refusal-v1 OK");

    // Honest exit: nothing Green happened here. A refusal is on screen, not
    // papered over with a zero exit code (MEGA-001 §0.4).
    std::process::exit(1);
}
