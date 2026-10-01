//! The run loop every demo shares: load `demo.toml`, preflight, let the demo
//! measure, check `[assert]` against what it measured, decide, write the
//! receipt. Flipping any expected value in `demo.toml` turns the run Red —
//! the per-demo falsifier (MEGA-001 W-2) is a property of this module.
//!
//! `[assert]` values are either a literal (equality; numbers compare as f64)
//! or a string `"<op> <number>"` with op one of `== != >= <= > <`.

use crate::manifest::DemoManifest;
use crate::preflight::{preflight, Probe, SystemProbe};
use crate::receipt::{self, Receipt};
use crate::verdict::{decide, Verdict};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Harness {
    pub manifest: DemoManifest,
    pub dir: PathBuf,
    pub receipt: Receipt,
    preflight: Vec<crate::NotRunReason>,
}

impl Harness {
    /// Load `<dir>/demo.toml` and preflight against the real tools.
    /// `dir` is the compile-time `env!("CARGO_MANIFEST_DIR")`. When `cargo run`
    /// supplies the runtime `CARGO_MANIFEST_DIR`, that wins: a binary reused
    /// from a shared target dir must read THIS checkout's demo.toml, never the
    /// one it happened to be compiled in.
    pub fn load(dir: &str) -> Self {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| dir.to_string());
        Self::load_with(Path::new(&dir), &SystemProbe)
    }

    pub fn load_with(dir: &Path, probe: &dyn Probe) -> Self {
        let manifest =
            DemoManifest::load(&dir.join("demo.toml")).unwrap_or_else(|e| panic!("demo.toml: {e}"));
        let pre = preflight(&manifest, dir, probe);
        let apr_version = if manifest.uses_apr() {
            probe.version("apr").unwrap_or_else(|| "absent".into())
        } else {
            "none".into()
        };
        let backend = if manifest.host_class.contains("cuda") {
            "cuda"
        } else {
            "cpu"
        };
        let mut receipt = Receipt::new(&manifest.id, &apr_version, backend);
        if manifest.uses_agy() {
            receipt.agy_version = probe.version("agy");
        }
        receipt.model_sha256 = manifest.model.as_ref().map(|m| m.sha256.clone());
        Harness {
            manifest,
            dir: dir.to_path_buf(),
            receipt,
            preflight: pre,
        }
    }

    /// Reasons this demo must not run. A demo checks this before doing work.
    pub fn not_run(&self) -> &[crate::NotRunReason] {
        &self.preflight
    }

    /// Record a refusal discovered at run time (e.g. a verb exiting with its
    /// REFUSED code). The demo then finishes NotRun, whatever it measured.
    pub fn refuse(&mut self, reason: crate::NotRunReason) {
        self.preflight.push(reason);
    }

    /// Run one step: `program args…`, recording it in the receipt. Returns
    /// (exit code, stdout, wall ms).
    pub fn step(&mut self, program: &str, args: &[&str]) -> (i32, Vec<u8>, u128) {
        let cmd = std::iter::once(program)
            .chain(args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        println!("$ {cmd}");
        let t = std::time::Instant::now();
        let out = std::process::Command::new(program).args(args).output();
        let ms = t.elapsed().as_millis();
        let (code, stdout) = match out {
            Ok(o) => (o.status.code().unwrap_or(-1), o.stdout),
            Err(e) => {
                println!("  spawn failed: {e}");
                (-1, Vec::new())
            }
        };
        self.receipt.step(&cmd, code, &stdout);
        (code, stdout, ms)
    }

    /// Decide the verdict from `measured`, print it, and write the receipt
    /// when `RFML5_RECEIPTS` is set. Returns the verdict.
    pub fn finish(&mut self, measured: BTreeMap<String, Value>) -> Verdict {
        let assertions = if self.preflight.is_empty() {
            check_assertions(&self.manifest.assert, &measured)
        } else {
            BTreeMap::new()
        };
        self.receipt.measured = measured;
        self.receipt.assertions = assertions.clone();
        self.receipt.verdict = decide(&self.preflight, &assertions);
        for (k, ok) in &assertions {
            let got = self
                .receipt
                .measured
                .get(k)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "<missing>".into());
            let want = &self.manifest.assert[k];
            println!(
                "  assert {k}: expected {want}, measured {got} -> {}",
                if *ok { "ok" } else { "FAIL" }
            );
        }
        println!("verdict: {}", self.receipt.verdict);
        let repo = self.dir.join("../../..");
        match receipt::receipts_root(&repo) {
            Ok(root) => match self.receipt.write(&root) {
                Ok((path, sha)) => println!("receipt: {} sha256={sha}", path.display()),
                Err(e) => println!("receipt: NOT WRITTEN ({e})"),
            },
            Err(e) => println!("receipt: not written ({e})"),
        }
        self.receipt.verdict.clone()
    }
}

/// Evaluate every `[assert]` entry against `measured`. A missing measurement
/// is a failed assertion, never a skipped one.
pub fn check_assertions(
    expected: &BTreeMap<String, toml::Value>,
    measured: &BTreeMap<String, Value>,
) -> BTreeMap<String, bool> {
    expected
        .iter()
        .map(|(k, want)| {
            let ok = measured.get(k).is_some_and(|got| holds(want, got));
            (k.clone(), ok)
        })
        .collect()
}

fn holds(want: &toml::Value, got: &Value) -> bool {
    match want {
        toml::Value::String(s) => match parse_op(s) {
            Some((op, n)) => got.as_f64().is_some_and(|g| op_holds(op, g, n)),
            None => got.as_str() == Some(s.as_str()),
        },
        toml::Value::Integer(i) => got.as_f64() == Some(*i as f64),
        toml::Value::Float(f) => got.as_f64() == Some(*f),
        toml::Value::Boolean(b) => got.as_bool() == Some(*b),
        _ => false,
    }
}

fn parse_op(s: &str) -> Option<(&str, f64)> {
    let s = s.trim();
    for op in ["==", "!=", ">=", "<=", ">", "<"] {
        if let Some(rest) = s.strip_prefix(op) {
            return rest.trim().parse().ok().map(|n| (op, n));
        }
    }
    None
}

fn op_holds(op: &str, g: f64, n: f64) -> bool {
    match op {
        "==" => g == n,
        "!=" => g != n,
        ">=" => g >= n,
        "<=" => g <= n,
        ">" => g > n,
        "<" => g < n,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn exp(pairs: &[(&str, toml::Value)]) -> BTreeMap<String, toml::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn meas(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn literals_and_operators() {
        let e = exp(&[
            ("exit", toml::Value::Integer(0)),
            ("parse_rate", toml::Value::String("== 1.0".into())),
            ("planted_found", toml::Value::String(">= 7".into())),
            ("digest", toml::Value::String("abc".into())),
        ]);
        let m = meas(&[
            ("exit", json!(0)),
            ("parse_rate", json!(1.0)),
            ("planted_found", json!(8)),
            ("digest", json!("abc")),
        ]);
        assert!(check_assertions(&e, &m).values().all(|ok| *ok));
    }

    /// W-2 per-demo falsifier: flip one expected value → that assertion fails.
    #[test]
    fn flipping_one_expected_value_is_red() {
        let m = meas(&[("workers", json!(8)), ("exit", json!(0))]);
        let good = exp(&[
            ("workers", toml::Value::Integer(8)),
            ("exit", toml::Value::Integer(0)),
        ]);
        assert!(crate::verdict::decide(&[], &check_assertions(&good, &m)).is_green());
        let flipped = exp(&[
            ("workers", toml::Value::Integer(9)),
            ("exit", toml::Value::Integer(0)),
        ]);
        assert!(!crate::verdict::decide(&[], &check_assertions(&flipped, &m)).is_green());
    }

    #[test]
    fn missing_measurement_fails() {
        let e = exp(&[("ttft_ms", toml::Value::String("> 0".into()))]);
        assert!(!check_assertions(&e, &BTreeMap::new())["ttft_ms"]);
    }
}
