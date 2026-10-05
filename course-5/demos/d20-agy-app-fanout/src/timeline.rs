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
    /// The first snapshot, before all three ran, that showed an approval
    /// prompt. A blocked agent is not running, so once this is set the
    /// all-running moment can no longer be claimed.
    pub approval_ms: Option<u64>,
}

impl Timeline {
    /// Fold one snapshot's three row states, taken at `t_ms`; `approval` is
    /// whether that snapshot showed an approval prompt anywhere.
    pub fn observe(&mut self, states: [Option<RowState>; 3], approval: bool, t_ms: u64) {
        if approval && self.all_running_ms.is_none() && self.approval_ms.is_none() {
            self.approval_ms = Some(t_ms);
        }
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
        if running == 3 && self.all_running_ms.is_none() && self.approval_ms.is_none() {
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
        t.observe([Some(Running), None, None], false, 100);
        t.observe([Some(Running), Some(Running), Some(Running)], false, 900);
        t.observe([Some(Running), Some(Running), Some(Running)], false, 1900);
        t.observe([Some(Idle), Some(Running), Some(Idle)], false, 5000);
        t.observe([Some(Idle), Some(Idle), Some(Idle)], false, 6000);
        assert_eq!(t.started_ms, [Some(100), Some(900), Some(900)]);
        assert_eq!(t.all_running_ms, Some(900));
        assert_eq!(t.done_ms, [Some(5000), Some(6000), Some(5000)]);
        assert!(t.all_done());
    }

    #[test]
    fn sequential_run_never_sees_all_running() {
        let mut t = Timeline::default();
        t.observe([Some(Running), None, None], false, 100);
        t.observe([Some(Idle), Some(Running), None], false, 200);
        t.observe([Some(Idle), Some(Idle), Some(Running)], false, 300);
        t.observe([Some(Idle), Some(Idle), Some(Idle)], false, 400);
        assert!(t.all_done());
        assert_eq!(t.all_running_ms, None);
    }

    /// Falsifier for 06:47Z: three rows read running while one pane asked
    /// for write access. That is blocked, not concurrent, and stays Red even
    /// after the prompt is gone.
    #[test]
    fn three_rows_behind_an_approval_prompt_are_not_all_running() {
        let mut t = Timeline::default();
        t.observe([Some(Running), Some(Running), Some(Running)], true, 900);
        t.observe([Some(Running), Some(Running), Some(Running)], false, 1900);
        assert_eq!(t.approval_ms, Some(900));
        assert_eq!(t.all_running_ms, None);
    }

    #[test]
    fn idle_before_running_is_not_done() {
        let mut t = Timeline::default();
        t.observe([Some(Idle), Some(Idle), Some(Idle)], false, 50);
        assert!(!t.all_done());
    }
}
