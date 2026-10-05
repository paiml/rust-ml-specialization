//! Paced recording: each on-screen step starts on its narration cue.
//!
//! A narrated demo is recorded to a take that already exists. The take is
//! measured into a beat-times file, one `tag<TAB>seconds` line per beat, where
//! `seconds` is when that beat's first word is spoken. A demo calls
//! [`Pacer::cue`] immediately before it shows a beat's on-screen event; the
//! call waits until that beat's time less [`LEAD_S`], so in one continuous
//! capture every event appears up to 0.15 s early and never late.
//!
//! The pacer never speeds anything up and never skips output. A live step that
//! runs past the next cue shifts that cue and every later one by the overrun,
//! and [`cuts`] names it: that many seconds of idle screen, just before the
//! late event, are cut in the edit. After the cut every event is back on its
//! beat. Without the shift a cut would leave every later event early.
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
    /// Idle seconds cut from the recording up to and including this event.
    pub slip_s: f64,
}

impl CueRecord {
    /// Late means that, after the cuts, the event is shown after its spoken
    /// word.
    pub fn late(&self) -> bool {
        self.shown_s - self.slip_s > self.target_s
    }
}

/// The edit: `(tag, raw seconds where the cut ends, length)` for every cue
/// that absorbed an overrun. The cut is idle screen ending at the event.
pub fn cuts(all: &[CueRecord]) -> Vec<(String, f64, f64)> {
    let mut prev = 0.0;
    let mut out = Vec::new();
    for c in all {
        if c.slip_s > prev {
            out.push((c.tag.clone(), c.shown_s, c.slip_s - prev));
        }
        prev = c.slip_s;
    }
    out
}

/// `pace: N cues, K late[: tag +x.xs, …][; cut tag x.xs idle ending at y.ys, …]`.
/// The cuts are printed because, with the shift, an overrun is never late.
pub fn summary(all: &[CueRecord]) -> String {
    let late: Vec<String> = all
        .iter()
        .filter(|c| c.late())
        .map(|c| format!("{} +{:.1}s", c.tag, c.shown_s - c.slip_s - c.target_s))
        .collect();
    let cut: Vec<String> = cuts(all)
        .iter()
        .map(|(tag, at, len)| format!("{tag} {len:.1}s idle ending at {at:.1}s"))
        .collect();
    format!(
        "pace: {} cues, {} late{}{}{}{}",
        all.len(),
        late.len(),
        if late.is_empty() { "" } else { ": " },
        late.join(", "),
        if cut.is_empty() { "" } else { "; cut " },
        cut.join(", ")
    )
}

#[derive(Debug)]
pub struct Pacer {
    t0: Instant,
    beats: BTreeMap<String, f64>,
    log: std::sync::Mutex<Vec<CueRecord>>,
    slip: std::sync::Mutex<f64>,
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
            slip: std::sync::Mutex::new(0.0),
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

    /// Wait until `tag`'s time less [`LEAD_S`], shifted by the overruns so far,
    /// then return. A cue reached after its spoken word adds the overrun to
    /// the shift. An unknown tag is an error: a step the take does not narrate
    /// has no cue.
    pub fn cue(&self, tag: &str) -> Result<(), String> {
        let target = *self
            .beats
            .get(tag)
            .ok_or_else(|| format!("no beat {tag} in RFML5_BEATS"))?;
        let base = (target - LEAD_S).max(0.0);
        let mut slip = self.slip.lock().map_err(|_| "pacer poisoned")?;
        let now = self.now_s();
        let shown = if now < base + *slip {
            std::thread::sleep(Duration::from_secs_f64(base + *slip - now));
            self.now_s()
        } else {
            if now - *slip > target {
                // One clock reading for both, so the absorbed overrun nets to
                // exactly the lead.
                *slip = now - base;
            }
            now
        };
        let rec = CueRecord {
            tag: tag.to_string(),
            target_s: target,
            shown_s: shown,
            slip_s: *slip,
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
    fn an_overrun_becomes_a_cut_and_shifts_every_later_cue() {
        let mut b = BTreeMap::new();
        b.insert("x".to_string(), 0.0);
        b.insert("y".to_string(), 0.4);
        let p = Pacer::new(b);
        std::thread::sleep(Duration::from_millis(150));
        p.cue("x").unwrap();
        p.cue("y").unwrap();
        let r = p.report();
        let c = cuts(&r);
        assert_eq!(c.len(), 1, "{r:?}");
        assert_eq!(c[0].0, "x");
        assert!(c[0].2 >= 0.15, "{c:?}");
        // y waited out the shift: without it y would release at 0.3 s raw.
        assert!(r[1].shown_s >= 0.3 + c[0].2 - 0.01, "{r:?}");
        for e in &r {
            assert!(!e.late(), "{e:?}");
            assert!(e.target_s - (e.shown_s - e.slip_s) <= 0.15, "{e:?}");
        }
    }

    #[test]
    fn a_record_shown_after_its_word_net_of_cuts_is_late() {
        let r = CueRecord {
            tag: "x".into(),
            target_s: 1.0,
            shown_s: 3.0,
            slip_s: 1.5,
        };
        assert!(r.late());
        assert!(!CueRecord { slip_s: 2.0, ..r }.late());
    }

    #[test]
    fn the_summary_names_the_cuts_an_on_time_run_has_none() {
        let r = |tag: &str, target_s, shown_s, slip_s| CueRecord {
            tag: tag.into(),
            target_s,
            shown_s,
            slip_s,
        };
        assert_eq!(summary(&[r("a", 1.0, 0.9, 0.0)]), "pace: 1 cues, 0 late");
        assert_eq!(
            summary(&[
                r("a", 1.0, 0.9, 0.0),
                r("b", 2.0, 4.9, 3.0),
                r("c", 3.0, 5.9, 3.0)
            ]),
            "pace: 3 cues, 0 late; cut b 3.0s idle ending at 4.9s"
        );
    }
}
