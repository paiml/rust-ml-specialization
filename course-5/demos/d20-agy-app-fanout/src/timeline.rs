//! The three agents' timeline on one monotonic clock (ms since the fan-out
//! started). Every update comes from ONE tree snapshot, so the all-running
//! moment is, by construction, one snapshot holding three running rows.

use crate::tree::RowState;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Timeline {
    pub started_ms: [Option<u64>; 3],
    pub done_ms: [Option<u64>; 3],
    /// The first snapshot in which all three rows read running.
    pub all_running_ms: Option<u64>,
    /// How many rows read running in that snapshot (3 when set).
    pub all_running_count: usize,
}

impl Timeline {
    /// Fold one snapshot's three row states, taken at `t_ms`.
    pub fn observe(&mut self, states: [Option<RowState>; 3], t_ms: u64) {
        for (i, s) in states.iter().enumerate() {
            match s {
                Some(RowState::Running) if self.started_ms[i].is_none() => {
                    self.started_ms[i] = Some(t_ms)
                }
                Some(RowState::Idle)
                    if self.started_ms[i].is_some() && self.done_ms[i].is_none() =>
                {
                    self.done_ms[i] = Some(t_ms)
                }
                _ => {}
            }
        }
        let running = states
            .iter()
            .filter(|s| **s == Some(RowState::Running))
            .count();
        if running == 3 && self.all_running_ms.is_none() {
            self.all_running_ms = Some(t_ms);
            self.all_running_count = running;
        }
    }

    pub fn all_done(&self) -> bool {
        self.done_ms.iter().all(Option::is_some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RowState::{Idle, Running};

    #[test]
    fn concurrent_run_sets_the_all_running_moment_once() {
        let mut t = Timeline::default();
        t.observe([Some(Running), None, None], 100);
        t.observe([Some(Running), Some(Running), Some(Running)], 900);
        t.observe([Some(Running), Some(Running), Some(Running)], 1900);
        t.observe([Some(Idle), Some(Running), Some(Idle)], 5000);
        t.observe([Some(Idle), Some(Idle), Some(Idle)], 6000);
        assert_eq!(t.started_ms, [Some(100), Some(900), Some(900)]);
        assert_eq!(t.all_running_ms, Some(900));
        assert_eq!(t.done_ms, [Some(5000), Some(6000), Some(5000)]);
        assert!(t.all_done());
    }

    #[test]
    fn sequential_run_never_sees_all_running() {
        let mut t = Timeline::default();
        t.observe([Some(Running), None, None], 100);
        t.observe([Some(Idle), Some(Running), None], 200);
        t.observe([Some(Idle), Some(Idle), Some(Running)], 300);
        t.observe([Some(Idle), Some(Idle), Some(Idle)], 400);
        assert!(t.all_done());
        assert_eq!(t.all_running_ms, None);
    }

    #[test]
    fn idle_before_running_is_not_done() {
        let mut t = Timeline::default();
        t.observe([Some(Idle), Some(Idle), Some(Idle)], 50);
        assert!(!t.all_done());
    }
}
