//! demo-pin-v1 (sha arm), end to end: a wrong weights sha runs through
//! `Harness::load_with` and `finish` to a `NotRun{WeightsMismatch}` receipt on
//! disk, and the verdict is not green (so a demo's `assert!(is_green())` exits
//! nonzero) even though every assertion would have passed.

use demo_kit::harness::Harness;
use demo_kit::preflight::Probe;
use demo_kit::{sha, NotRunReason, Verdict};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// No tools: the demo declares `apr = "none"`, so only the weights arm runs.
struct NoTools;

impl Probe for NoTools {
    fn which(&self, _: &str) -> Option<PathBuf> {
        None
    }
    fn version(&self, _: &str) -> Option<String> {
        None
    }
    fn available(&self, _: &str) -> bool {
        true
    }
    fn gpu_reserved(&self) -> bool {
        false
    }
    fn declared_apr(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}

/// `<t>/repo/course-5/demos/d` holds the demo; receipts go to `<t>/out`, outside the repo.
fn demo_dir(t: &Path, sha256: &str) -> PathBuf {
    let d = t.join("repo/course-5/demos/d");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("model.gguf"), b"the bytes on disk").unwrap();
    std::fs::write(
        d.join("demo.toml"),
        format!(
            r#"id = "d99-weights"
title = "t"
lesson = "rfml5/3.1"
apr = "none"
agy = "none"
model = {{ name = "m", sha256 = "{sha256}", path = "model.gguf" }}
host_class = "cpu"

[assert]
exit = 0

[record]
target_duration_s = 300
resolution = "1920x1080"
"#
        ),
    )
    .unwrap();
    d
}

fn measured() -> BTreeMap<String, Value> {
    BTreeMap::from([("exit".to_string(), json!(0))])
}

fn receipts(out: &Path) -> Vec<Value> {
    let mut found = Vec::new();
    let mut stack = vec![out.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in std::fs::read_dir(&p).into_iter().flatten().flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let text = std::fs::read_to_string(&path).unwrap();
                found.push(serde_json::from_str(&text).unwrap());
            }
        }
    }
    found
}

#[test]
fn wrong_weights_sha_ends_in_a_notrun_receipt_and_a_non_green_verdict() {
    let t = std::env::temp_dir().join(format!("dk-weights-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&t);
    let out = t.join("out");
    std::env::set_var("RFML5_RECEIPTS", &out);

    // wrong sha: NotRun, receipt written, never green, assertions not consulted
    let d = demo_dir(&t, &"0".repeat(64));
    let mut h = Harness::load_with(&d, &NoTools);
    assert!(matches!(
        h.not_run(),
        [NotRunReason::WeightsMismatch { expected, .. }] if *expected == "0".repeat(64)
    ));
    let v = h.finish(measured());
    assert!(
        matches!(&v, Verdict::NotRun { reasons } if reasons.len() == 1),
        "{v}"
    );
    assert!(!v.is_green());
    let r = receipts(&out);
    assert_eq!(r.len(), 1, "exactly one receipt: {r:?}");
    assert_eq!(r[0]["verdict"]["verdict"], "NotRun");
    assert_eq!(r[0]["verdict"]["reasons"][0]["reason"], "WeightsMismatch");
    assert_eq!(
        r[0]["verdict"]["reasons"][0]["detail"]["expected"],
        "0".repeat(64)
    );
    assert!(r[0]["assertions"].as_object().unwrap().is_empty());

    // anti-vacuity: the right sha, same everything else, is green
    let good = sha::sha256_file(&d.join("model.gguf")).unwrap();
    let d = demo_dir(&t, &good);
    let mut h = Harness::load_with(&d, &NoTools);
    assert!(h.not_run().is_empty());
    assert!(h.finish(measured()).is_green());

    std::fs::remove_dir_all(&t).unwrap();
}
