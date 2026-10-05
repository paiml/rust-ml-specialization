//! The cold-start "Changes to third-party model access" notice.
//!
//! It shows once per profile: the run that dismisses it is the last to see
//! it. So E_4 does not require it to be found; it requires every run to say
//! what it saw. A run records `Dismissed` (present, clicked, gone) or `Absent`
//! (the hub loaded without it). `PresentNotDismissed` and `NotProbed` (the hub
//! tree never loaded) are Red.
//!
//! Trees are the probe's compact dump: `[role, name, has_backend]` rows.

use serde_json::Value;

/// The notice's heading text, matched case-insensitively.
pub const NOTICE_TEXT: &str = "changes to third-party model access";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeState {
    Dismissed,
    Absent,
    PresentNotDismissed,
    NotProbed,
}

impl NoticeState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dismissed => "present->dismissed",
            Self::Absent => "absent",
            Self::PresentNotDismissed => "present-not-dismissed",
            Self::NotProbed => "not-probed",
        }
    }

    pub fn is_green(self) -> bool {
        matches!(self, Self::Dismissed | Self::Absent)
    }
}

/// True when a compact tree shows the notice heading.
pub fn shows_notice(tree: &Value) -> bool {
    tree.as_array().is_some_and(|rows| {
        rows.iter().any(|r| {
            r.get(1)
                .and_then(Value::as_str)
                .is_some_and(|n| n.to_lowercase().contains(NOTICE_TEXT))
        })
    })
}

/// Classify one run. `hub` is the first tree, `None` when the hub never
/// loaded; `after` is the tree read after the Dismiss click, `None` when no
/// click was made.
pub fn classify(hub: Option<&Value>, after: Option<&Value>) -> NoticeState {
    match (hub, after) {
        (None, _) => NoticeState::NotProbed,
        (Some(h), _) if !shows_notice(h) => NoticeState::Absent,
        (Some(_), Some(a)) if !shows_notice(a) => NoticeState::Dismissed,
        _ => NoticeState::PresentNotDismissed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hub tree from the one measured cold start that showed the notice.
    const PRESENT: &str = include_str!("../fixtures/notice-present.ax.json");

    fn present() -> Value {
        serde_json::from_str(PRESENT).unwrap()
    }

    fn without_notice() -> Value {
        let rows = present()
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| {
                let n = r[1].as_str().unwrap_or("").to_lowercase();
                !n.contains(NOTICE_TEXT) && !n.contains("third-party") && n != "dismiss"
            })
            .cloned()
            .collect();
        Value::Array(rows)
    }

    #[test]
    fn positive_control_the_measured_cold_start_shows_the_notice() {
        assert!(shows_notice(&present()));
        assert!(!shows_notice(&without_notice()));
    }

    #[test]
    fn every_run_lands_in_exactly_one_state() {
        let (p, w) = (present(), without_notice());
        assert_eq!(classify(Some(&p), Some(&w)), NoticeState::Dismissed);
        assert_eq!(classify(Some(&w), None), NoticeState::Absent);
        assert_eq!(
            classify(Some(&p), Some(&p)),
            NoticeState::PresentNotDismissed
        );
        assert_eq!(classify(Some(&p), None), NoticeState::PresentNotDismissed);
        assert_eq!(classify(None, None), NoticeState::NotProbed);
        assert_eq!(classify(None, Some(&w)), NoticeState::NotProbed);
    }

    #[test]
    fn only_dismissed_and_absent_are_green() {
        assert!(NoticeState::Dismissed.is_green());
        assert!(NoticeState::Absent.is_green());
        assert!(!NoticeState::PresentNotDismissed.is_green());
        assert!(!NoticeState::NotProbed.is_green());
    }
}
