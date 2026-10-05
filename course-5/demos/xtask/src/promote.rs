//! `xtask promote-fixture <gate> [--check | --replace]` (spec §4.5).
//!
//! An entry gate measures once and writes into its own run directory; this
//! copies that one file to the one committed fixture its consumers embed.
//! Source and destination are named by a closed enum, never a free path.
//!
//! Run-directory layout, read here and written by the gate bins:
//!
//! ```text
//! $RFML5_RECEIPTS/<crate>/<gate>/<run_id>/<file>
//!   agy-cdp/e3-probe/<run_id>/role-name-map.json
//!   d21-agy-app-fanin/measure-stop-latency/<run_id>/stop-latency.json
//! ```
//!
//! `<run_id>` is demo-kit's `<secs>-<nanos:09>-<pid>`, so the latest run is the
//! greatest name once compared as `(secs, nanos)`.

use std::path::{Path, PathBuf};

/// The bootstrap sentinel committed before the first measurement.
#[cfg(test)]
pub const SENTINEL: &str = "{\"status\":\"unmeasured\"}\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    E3Probe,
    MeasureStopLatency,
}

impl Gate {
    pub fn parse(s: &str) -> Option<Gate> {
        match s {
            "e3-probe" => Some(Gate::E3Probe),
            "measure-stop-latency" => Some(Gate::MeasureStopLatency),
            _ => None,
        }
    }

    /// `(crate, gate dir, file)`
    fn source(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Gate::E3Probe => ("agy-cdp", "e3-probe", "role-name-map.json"),
            Gate::MeasureStopLatency => (
                "d21-agy-app-fanin",
                "measure-stop-latency",
                "stop-latency.json",
            ),
        }
    }

    /// Destination, relative to the demos root.
    pub fn destination(self) -> &'static str {
        match self {
            Gate::E3Probe => "agy-cdp/fixtures/role-name-map.json",
            Gate::MeasureStopLatency => "d21-agy-app-fanin/fixtures/stop-latency.json",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Check,
    Promote,
    Replace,
}

pub fn is_sentinel(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .is_ok_and(|v| v == serde_json::json!({"status": "unmeasured"}))
}

fn run_key(name: &str) -> Option<(u64, u64)> {
    let mut it = name.splitn(3, '-');
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

/// The latest run directory under `<receipts>/<crate>/<gate>/`.
pub fn latest_run(receipts: &Path, gate: Gate) -> Result<PathBuf, String> {
    let (krate, g, _) = gate.source();
    let dir = receipts.join(krate).join(g);
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("{krate}/{g}: no runs ({e})"))?;
    rd.filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            run_key(&n).map(|k| (k, e.path()))
        })
        .max_by_key(|(k, _)| *k)
        .map(|(_, p)| p)
        .ok_or_else(|| format!("{krate}/{g}: no run directory"))
}

/// `--check`: the committed fixture parses and is not the sentinel.
pub fn check(demos: &Path, gate: Gate) -> Result<String, String> {
    let dest = demos.join(gate.destination());
    let bytes = std::fs::read(&dest).map_err(|e| format!("{}: {e}", gate.destination()))?;
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|e| format!("{}: not JSON: {e}", gate.destination()))?;
    if is_sentinel(&bytes) {
        return Err(format!(
            "{}: still the unmeasured sentinel",
            gate.destination()
        ));
    }
    Ok(format!(
        "{} measured, sha256 {}",
        gate.destination(),
        demo_kit::sha::sha256_bytes(&bytes)
    ))
}

/// Copy the latest run's file to the fixture; returns the sha256 written.
pub fn promote(receipts: &Path, demos: &Path, gate: Gate, mode: Mode) -> Result<String, String> {
    if mode == Mode::Check {
        return check(demos, gate);
    }
    let (_, _, file) = gate.source();
    let src = latest_run(receipts, gate)?.join(file);
    let bytes = std::fs::read(&src).map_err(|e| format!("source {file}: {e}"))?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(format!("source {file} is empty"));
    }
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|e| format!("source {file}: not JSON: {e}"))?;
    if is_sentinel(&bytes) {
        return Err(format!("source {file} is the unmeasured sentinel"));
    }
    let dest = demos.join(gate.destination());
    if let Ok(old) = std::fs::read(&dest) {
        if old != bytes && !is_sentinel(&old) && mode != Mode::Replace {
            return Err(format!(
                "{} holds a different measurement; re-measuring is a decision: pass --replace",
                gate.destination()
            ));
        }
    }
    std::fs::write(&dest, &bytes).map_err(|e| format!("{}: {e}", gate.destination()))?;
    Ok(demo_kit::sha::sha256_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(name: &str) -> (PathBuf, PathBuf) {
        let base =
            std::env::temp_dir().join(format!("xtask-promote-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (r, d) = (base.join("receipts"), base.join("demos"));
        std::fs::create_dir_all(d.join("agy-cdp/fixtures")).unwrap();
        std::fs::write(d.join(Gate::E3Probe.destination()), SENTINEL).unwrap();
        (r, d)
    }

    fn run(r: &Path, id: &str, body: &str) {
        let p = r.join("agy-cdp/e3-probe").join(id);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("role-name-map.json"), body).unwrap();
    }

    #[test]
    fn sentinel_fails_check_and_is_replaced_by_the_latest_run() {
        let (r, d) = roots("latest");
        assert!(check(&d, Gate::E3Probe).is_err());
        assert!(
            promote(&r, &d, Gate::E3Probe, Mode::Promote).is_err(),
            "no runs"
        );
        run(&r, "1700000000-000000009-1", "{\"old\":1}");
        run(&r, "1700000000-000000010-2", "{\"new\":1}");
        run(&r, "999-000000000-3", "{\"older\":1}");
        let sha = promote(&r, &d, Gate::E3Probe, Mode::Promote).unwrap();
        let written = std::fs::read(d.join(Gate::E3Probe.destination())).unwrap();
        assert_eq!(written, b"{\"new\":1}");
        assert_eq!(sha, demo_kit::sha::sha256_bytes(&written));
        assert!(check(&d, Gate::E3Probe).is_ok());
    }

    #[test]
    fn a_differing_measurement_needs_replace() {
        let (r, d) = roots("replace");
        run(&r, "1-000000000-1", "{\"a\":1}");
        promote(&r, &d, Gate::E3Probe, Mode::Promote).unwrap();
        run(&r, "2-000000000-1", "{\"a\":2}");
        let e = promote(&r, &d, Gate::E3Probe, Mode::Promote).unwrap_err();
        assert!(e.contains("--replace"), "{e}");
        promote(&r, &d, Gate::E3Probe, Mode::Replace).unwrap();
        // the same bytes again are not a decision
        promote(&r, &d, Gate::E3Probe, Mode::Promote).unwrap();
    }

    #[test]
    fn empty_garbage_or_sentinel_sources_are_refused() {
        let (r, d) = roots("refuse");
        for body in ["", "  \n", "not json", SENTINEL] {
            run(&r, "5-000000000-1", body);
            assert!(
                promote(&r, &d, Gate::E3Probe, Mode::Promote).is_err(),
                "{body:?}"
            );
        }
        assert!(is_sentinel(SENTINEL.as_bytes()));
    }

    #[test]
    fn committed_fixtures_are_the_sentinel_until_measured() {
        let demos = crate::demos_root();
        for g in [Gate::E3Probe, Gate::MeasureStopLatency] {
            let b = std::fs::read(demos.join(g.destination())).unwrap();
            assert!(serde_json::from_slice::<serde_json::Value>(&b).is_ok());
        }
        assert_eq!(Gate::parse("e3-probe"), Some(Gate::E3Probe));
        assert_eq!(Gate::parse("../x"), None);
    }
}
