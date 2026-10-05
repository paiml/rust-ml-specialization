//! The stop latency L (spec §5.4): at least 30 samples on the demo profile,
//! `L = max(p99, 2 × p95)`, rounded up to the next whole millisecond.
//!
//! One sample is the time from the stop decision, taken just before the
//! tree read that names the agent's own stop control, to the first tree
//! read reporting that agent stopped. It therefore holds what D21's
//! `stopped − red_seen` holds: a tree read, the box model, the click and
//! the read-back.

use crate::json::J;

/// The fewest samples that may set L.
pub const MIN_SAMPLES: usize = 30;

/// The committed fixture, embedded so no confined bin reads it at run time.
pub const FIXTURE: &str = include_str!("../fixtures/stop-latency.json");

/// Nearest-rank percentile `q` (0 < q ≤ 1) of `samples`; `None` when empty.
pub fn percentile(samples: &[f64], q: f64) -> Option<f64> {
    if samples.is_empty() || !(q > 0.0 && q <= 1.0) {
        return None;
    }
    let mut s = samples.to_vec();
    s.sort_by(f64::total_cmp);
    let rank = (q * s.len() as f64).ceil() as usize;
    s.get(rank.clamp(1, s.len()) - 1).copied()
}

/// `(p95, p99, L)` in ms; `L` is `max(p99, 2 × p95)` rounded up.
pub fn bound(samples: &[f64]) -> Option<(f64, f64, u64)> {
    let p95 = percentile(samples, 0.95)?;
    let p99 = percentile(samples, 0.99)?;
    Some((p95, p99, p99.max(2.0 * p95).ceil() as u64))
}

/// One decimal place, as an integer count of tenths (the fixture keeps
/// integers only).
fn tenths(ms: f64) -> u64 {
    (ms * 10.0).round() as u64
}

/// The fixture body for `samples` (ms), or an error below [`MIN_SAMPLES`].
pub fn fixture(samples: &[f64], app_version: &str, asar: &str) -> Result<J, String> {
    if samples.len() < MIN_SAMPLES {
        return Err(format!(
            "{} samples, at least {MIN_SAMPLES} are needed",
            samples.len()
        ));
    }
    let (p95, p99, l) = bound(samples).ok_or("no samples")?;
    Ok(J::Obj(vec![
        ("gate".into(), J::str("measure-stop-latency")),
        ("app_version".into(), J::str(app_version)),
        ("app_asar_sha256".into(), J::str(asar)),
        ("rule".into(), J::str("L = max(p99, 2 x p95), rounded up to the next ms; nearest-rank percentiles")),
        ("sample".into(), J::str("ms from the stop decision, just before the tree read that names the agent's own Stop execution control, through DOM.getBoxModel and Input.dispatchMouseEvent at the box centre, to the first accessibility-tree read reporting that agent stopped")),
        ("samples".into(), J::int(samples.len() as u64)),
        (
            "samples_tenths_ms".into(),
            J::Arr(samples.iter().map(|s| J::int(tenths(*s))).collect()),
        ),
        ("p95_tenths_ms".into(), J::int(tenths(p95))),
        ("p99_tenths_ms".into(), J::int(tenths(p99))),
        ("l_ms".into(), J::int(l)),
    ]))
}

/// The integer after `"key":` in a pretty fixture.
fn int_field(text: &str, key: &str) -> Option<u64> {
    let at = text.find(&format!("\"{key}\":"))? + key.len() + 3;
    let rest = text[at..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// L from a fixture: `Ok(None)` for the unmeasured sentinel, `Ok(Some(L))`
/// for a measurement of at least [`MIN_SAMPLES`] samples, an error otherwise.
pub fn l_from_fixture(text: &str) -> Result<Option<u64>, String> {
    if text.trim() == "{\"status\":\"unmeasured\"}" {
        return Ok(None);
    }
    let n = int_field(text, "samples").ok_or("stop-latency.json: no samples count")?;
    if (n as usize) < MIN_SAMPLES {
        return Err(format!("stop-latency.json: {n} samples"));
    }
    let l = int_field(text, "l_ms").ok_or("stop-latency.json: no l_ms")?;
    if l == 0 {
        return Err("stop-latency.json: l_ms is 0".into());
    }
    Ok(Some(l))
}

/// L from the embedded fixture.
pub fn committed_l() -> Result<Option<u64>, String> {
    l_from_fixture(FIXTURE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles() {
        let s: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&s, 0.95), Some(95.0));
        assert_eq!(percentile(&s, 0.99), Some(99.0));
        assert_eq!(percentile(&s, 1.0), Some(100.0));
        assert_eq!(percentile(&[], 0.5), None);
        let s30: Vec<f64> = (1..=30).map(f64::from).collect();
        assert_eq!(percentile(&s30, 0.95), Some(29.0));
        assert_eq!(percentile(&s30, 0.99), Some(30.0));
    }

    #[test]
    fn l_is_the_larger_of_p99_and_twice_p95_rounded_up() {
        let s30: Vec<f64> = (1..=30).map(f64::from).collect();
        assert_eq!(bound(&s30), Some((29.0, 30.0, 58)));
        let mut tail: Vec<f64> = vec![100.0; 29];
        tail.push(1000.0);
        assert_eq!(bound(&tail).map(|b| b.2), Some(1000));
        let frac = vec![100.2; 30];
        assert_eq!(bound(&frac).map(|b| b.2), Some(201));
    }

    #[test]
    fn the_fixture_round_trips_and_refuses_too_few_samples() {
        let s: Vec<f64> = (0..30).map(|i| 400.0 + f64::from(i)).collect();
        let text = fixture(&s, "2.8.1", "x").unwrap().pretty();
        assert_eq!(l_from_fixture(&text), Ok(Some(856)));
        assert!(fixture(&s[..29], "2.8.1", "x").is_err());
        assert_eq!(l_from_fixture("{\"status\":\"unmeasured\"}\n"), Ok(None));
        let short = text.replace("\"samples\": 30", "\"samples\": 3");
        assert!(l_from_fixture(&short).is_err());
        assert!(l_from_fixture("{}").is_err());
    }

    #[test]
    fn the_committed_fixture_parses() {
        assert!(committed_l().is_ok());
    }
}
