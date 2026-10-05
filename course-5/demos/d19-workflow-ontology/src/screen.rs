//! The terminal D19 draws on: lines to stdout, cues to the pacer (if any).
//!
//! Each cue's time since the screen opened is kept, paced or not, so an
//! unpaced run measures how long each beat's screen step takes. A cue the
//! beat file does not carry is reported on stderr and the run goes on: the
//! take may be shorter than the sheet, and the run never bends to it.

use demo_kit::pace::{CueRecord, Pacer};
use std::time::Instant;

pub struct Screen<'a> {
    pacer: Option<&'a Pacer>,
    t0: Instant,
    cues: Vec<(String, f64)>,
}

impl<'a> Screen<'a> {
    pub fn new(pacer: Option<&'a Pacer>) -> Self {
        Screen {
            pacer,
            t0: Instant::now(),
            cues: Vec::new(),
        }
    }

    /// Release `tag`'s on-screen event: wait for the pacer, then note the time.
    pub fn cue(&mut self, tag: &str) {
        if let Err(e) = demo_kit::pace::cue(self.pacer, tag) {
            eprintln!("pace: {tag}: {e}");
        }
        self.cues
            .push((tag.to_string(), self.t0.elapsed().as_secs_f64()));
    }

    pub fn line(&self, text: &str) {
        println!("{text}");
    }

    pub fn cue_times(&self) -> &[(String, f64)] {
        &self.cues
    }

    /// Seconds from each cue to the next (the last to `end_s`): how long the
    /// run spends in each beat's window.
    pub fn windows(&self, end_s: f64) -> Vec<(String, f64)> {
        windows(&self.cues, end_s)
    }

    pub fn elapsed_s(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    /// Pacing on: a one-line late-beat summary. Pacing off: each beat's
    /// window, so the take's beat times can be measured against the run.
    pub fn report(&self) {
        match self.pacer {
            Some(p) => eprintln!("{}", late_summary(&p.report())),
            None => {
                let w: Vec<String> = self
                    .windows(self.elapsed_s())
                    .iter()
                    .map(|(tag, s)| format!("{tag}={s:.2}"))
                    .collect();
                eprintln!("beat windows (unpaced, s): {}", w.join(" "));
            }
        }
    }
}

pub fn windows(cues: &[(String, f64)], end_s: f64) -> Vec<(String, f64)> {
    cues.iter()
        .enumerate()
        .map(|(i, (tag, at))| {
            let next = cues.get(i + 1).map_or(end_s, |c| c.1);
            (tag.clone(), (next - at).max(0.0))
        })
        .collect()
}

/// `pace: N cues, K late[: …][; cut …]`, as [`demo_kit::pace::summary`].
pub fn late_summary(all: &[CueRecord]) -> String {
    demo_kit::pace::summary(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_run_cue_to_cue_and_the_last_to_the_end() {
        let c = vec![("B01".to_string(), 0.0), ("B02".into(), 1.5)];
        assert_eq!(
            windows(&c, 4.0),
            vec![("B01".into(), 1.5), ("B02".into(), 2.5)]
        );
        assert!(windows(&[], 1.0).is_empty());
    }

    #[test]
    fn the_late_summary_names_only_late_cues() {
        let r = |tag: &str, target_s, shown_s| CueRecord {
            tag: tag.into(),
            target_s,
            shown_s,
            slip_s: 0.0,
        };
        assert_eq!(late_summary(&[r("B01", 1.0, 0.9)]), "pace: 1 cues, 0 late");
        assert_eq!(
            late_summary(&[r("B01", 1.0, 0.9), r("B02", 2.0, 2.5)]),
            "pace: 2 cues, 1 late: B02 +0.5s"
        );
    }

    #[test]
    fn an_unpaced_cue_is_timed_and_never_waits() {
        let mut s = Screen::new(None);
        s.cue("D19-B01");
        s.cue("D19-B02");
        assert_eq!(s.cue_times().len(), 2);
        assert!(s.elapsed_s() < 1.0);
    }
}
