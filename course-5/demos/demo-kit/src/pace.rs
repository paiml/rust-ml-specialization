//! Paced recording: each on-screen step starts on its narration cue.
//!
//! A narrated demo is recorded to a take that already exists. The take is
//! measured into a beat-times file, one `tag<TAB>seconds` line per beat, where
//! `seconds` is when that beat's first word is spoken. A demo calls
//! [`Pacer::cue`] immediately before it shows a beat's on-screen event; the
//! call waits until that beat's time less [`LEAD_S`], so in one continuous
//! capture every event appears up to 0.15 s early and never late.
//!
//! The pacer never speeds anything up and never skips output. A step that runs
//! past the next cue makes that cue late, and [`Pacer::report`] says so; the
//! recording is then edited only where the screen is idle.
//!
//! With `RFML5_BEATS` unset there is no pacing: tests and CI run at full speed.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// How far ahead of the spoken word an event is shown. Inside the 0.15 s
/// tolerance, so a cue that is honoured is never late.
pub const LEAD_S: f64 = 0.10;

/// One cue as it was honoured.
#[derive(Debug, Clone, PartialEq)]
pub struct CueRecord {
    pub tag: String,
    /// The beat time from the file, seconds since t0.
    pub target_s: f64,
    /// When the event was released, seconds since t0.
    pub shown_s: f64,
}

impl CueRecord {
    /// Late means the event was shown after its spoken word.
    pub fn late(&self) -> bool {
        self.shown_s > self.target_s
    }
}

#[derive(Debug)]
pub struct Pacer {
    t0: Instant,
    beats: BTreeMap<String, f64>,
    log: std::sync::Mutex<Vec<CueRecord>>,
}

/// Parse `tag<TAB>seconds[<TAB>…]` lines. Blank lines and `#` comments are
/// skipped. A duplicate tag or an unparseable time is an error, not a skip.
pub fn parse_beats(text: &str) -> Result<BTreeMap<String, f64>, String> {
    let mut beats = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split('\t');
        let tag = cols.next().unwrap_or_default().trim();
        let secs = cols.next().unwrap_or_default().trim();
        let t: f64 = secs
            .parse()
            .map_err(|_| format!("beats line {}: time {secs:?} is not a number", n + 1))?;
        if tag.is_empty() || !t.is_finite() || t < 0.0 {
            return Err(format!("beats line {}: bad tag or time", n + 1));
        }
        if beats.insert(tag.to_string(), t).is_some() {
            return Err(format!("beats line {}: tag {tag} appears twice", n + 1));
        }
    }
    Ok(beats)
}

impl Pacer {
    /// A pacer whose clock starts now.
    pub fn new(beats: BTreeMap<String, f64>) -> Self {
        Self {
            t0: Instant::now(),
            beats,
            log: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// `RFML5_BEATS` names a beat-times file. Unset: `Ok(None)`, no pacing.
    pub fn from_env() -> Result<Option<Self>, String> {
        let Ok(path) = std::env::var("RFML5_BEATS") else {
            return Ok(None);
        };
        let text = std::fs::read_to_string(&path).map_err(|e| format!("RFML5_BEATS: {e}"))?;
        parse_beats(&text).map(|b| Some(Self::new(b)))
    }

    /// Seconds since t0.
    pub fn now_s(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    /// Wait until `tag`'s time less [`LEAD_S`], then return. An unknown tag is
    /// an error: a step the take does not narrate has no cue.
    pub fn cue(&self, tag: &str) -> Result<(), String> {
        let target = *self
            .beats
            .get(tag)
            .ok_or_else(|| format!("no beat {tag} in RFML5_BEATS"))?;
        let release = (target - LEAD_S).max(0.0);
        let now = self.now_s();
        if now < release {
            std::thread::sleep(Duration::from_secs_f64(release - now));
        }
        let rec = CueRecord {
            tag: tag.to_string(),
            target_s: target,
            shown_s: self.now_s(),
        };
        if let Ok(mut log) = self.log.lock() {
            log.push(rec);
        }
        Ok(())
    }

    /// Every cue honoured so far, in order.
    pub fn report(&self) -> Vec<CueRecord> {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
    }
}

/// Optional pacing: a `None` pacer makes every cue a no-op.
pub fn cue(pacer: Option<&Pacer>, tag: &str) -> Result<(), String> {
    match pacer {
        Some(p) => p.cue(tag),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_refuses() {
        let b = parse_beats("# c\nD18-B01\t0.5\tHere are\nD18-B02\t6.25\n").unwrap();
        assert_eq!(b["D18-B02"], 6.25);
        assert!(parse_beats("A\t1\nA\t2\n").is_err());
        assert!(parse_beats("A\tx\n").is_err());
        assert!(parse_beats("A\t-1\n").is_err());
    }

    #[test]
    fn a_cue_is_early_by_the_lead_and_never_late() {
        let mut b = BTreeMap::new();
        b.insert("x".to_string(), 0.3);
        let p = Pacer::new(b);
        p.cue("x").unwrap();
        let r = &p.report()[0];
        assert!(!r.late(), "{r:?}");
        assert!(r.target_s - r.shown_s <= 0.15, "{r:?}");
        assert!(p.cue("missing").is_err());
    }

    #[test]
    fn a_step_that_overruns_is_reported_late() {
        let mut b = BTreeMap::new();
        b.insert("x".to_string(), 0.0);
        let p = Pacer::new(b);
        std::thread::sleep(Duration::from_millis(20));
        p.cue("x").unwrap();
        assert!(p.report()[0].late());
    }
}
