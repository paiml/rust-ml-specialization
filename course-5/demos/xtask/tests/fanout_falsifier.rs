//! D14 falsifier: run `fanout.sh` against stub `apr`/`curl`/`nvidia-smi`
//! (bash fixtures, no GPU) and check that the contract holds when the fan-out
//! is faithful and fails — exit 1, `contract: FAIL` — when it is not.

use std::path::PathBuf;
use std::process::Command;

fn demo_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../d14-fanout")
}

/// Run the script with the stubs first on PATH; `faults` are extra env vars.
fn run(arm: &str, faults: &[(&str, &str)]) -> (i32, String) {
    let work = std::env::temp_dir().join(format!("d14-falsifier-{}-{arm}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    let model = work.join("Qwen3.5-4B-Q4_K_M.gguf");
    std::fs::write(&model, b"stub weights").unwrap();
    let sha = Command::new("sha256sum").arg(&model).output().unwrap();
    let sha = String::from_utf8(sha.stdout).unwrap();
    let sha = sha.split_whitespace().next().unwrap().to_string();

    let stubs = demo_dir().join("tests/stubs").canonicalize().unwrap();
    let path = format!("{}:{}", stubs.display(), std::env::var("PATH").unwrap());
    let mut cmd = Command::new("bash");
    cmd.arg(demo_dir().join("fanout.sh"))
        .env("PATH", path)
        .env("RFML5_MODELS", &work)
        .env("RFML5_MODEL_SHA", sha)
        .env("RFML5_RECEIPTS", work.join("receipts"));
    for (k, v) in faults {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
}

#[test]
fn arm_a_faithful_stub_passes() {
    let (code, text) = run("a", &[]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains("contract: fanout-disjoint-v1 + reduce-deterministic-v1 OK"),
        "{text}"
    );
}

#[test]
fn arm_b_one_port_answering_differently_fails() {
    let (code, text) = run("b", &[("STUB_BAD_PORT", "8082")]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("contract: FAIL reduce-deterministic-v1"),
        "{text}"
    );
    assert!(!text.contains("contract: fanout-disjoint-v1 + reduce-deterministic-v1 OK"));
}

#[test]
fn arm_c_dropped_id_fails() {
    let (code, text) = run("c-drop", &[("STUB_DROP_PROMPT", "shard")]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("contract: FAIL fanout-disjoint-v1"), "{text}");
}

#[test]
fn arm_c_duplicated_id_fails() {
    let (code, text) = run("c-dup", &[("STUB_DUP_PROMPT", "shard")]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("contract: FAIL fanout-disjoint-v1"), "{text}");
}
