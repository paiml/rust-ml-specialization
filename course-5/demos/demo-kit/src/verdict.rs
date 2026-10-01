//! The only three outcomes a demo has. Refusal is never success (MEGA-001 §0.4):
//! any preflight reason makes the demo `NotRun`, whatever its assertions say.
//! A demo with no assertions cannot be Green either — a gate that checks
//! nothing is not a gate.

use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", content = "detail")]
pub enum NotRunReason {
    /// A verb or flag in `needs` is refused by the pinned tool.
    Refused(String),
    /// The pin in demo.toml is not exact.
    PinRefused(String),
    /// The installed tool is not the pinned version.
    VersionMismatch {
        tool: String,
        pinned: String,
        found: String,
    },
    /// The model file's sha256 is not the declared one (or the file is missing).
    WeightsMismatch { expected: String, found: String },
    /// Fixture tree hash differs from the declared one.
    FixturesMismatch { expected: String, found: String },
    /// apr resolves to a path that is not forjar-declared (H-3).
    UndeclaredApr(String),
    /// Another job holds a GPU reservation (H-7): never wait-and-pass.
    GpuLockHeld,
    /// A tool the demo needs is not installed.
    MissingTool(String),
}

impl fmt::Display for NotRunReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(v) => write!(f, "Refused({v})"),
            Self::PinRefused(p) => write!(f, "PinRefused({p})"),
            Self::VersionMismatch {
                tool,
                pinned,
                found,
            } => write!(f, "VersionMismatch({tool}: pinned {pinned}, found {found})"),
            Self::WeightsMismatch { .. } => write!(f, "WeightsMismatch"),
            Self::FixturesMismatch { .. } => write!(f, "FixturesMismatch"),
            Self::UndeclaredApr(p) => write!(f, "UndeclaredApr({p})"),
            Self::GpuLockHeld => write!(f, "GpuLockHeld"),
            Self::MissingTool(t) => write!(f, "MissingTool({t})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict")]
pub enum Verdict {
    Green,
    Red { failed: Vec<String> },
    NotRun { reasons: Vec<NotRunReason> },
}

impl Verdict {
    pub fn is_green(&self) -> bool {
        matches!(self, Verdict::Green)
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Green => write!(f, "Green"),
            Verdict::Red { failed } => write!(f, "Red{{{}}}", failed.join(", ")),
            Verdict::NotRun { reasons } => {
                let r: Vec<String> = reasons.iter().map(|r| r.to_string()).collect();
                write!(f, "NotRun{{{}}}", r.join(", "))
            }
        }
    }
}

/// Decide a verdict. Preflight reasons win over everything; then any failed
/// assertion is Red; an empty assertion set is Red (nothing was checked).
pub fn decide(preflight: &[NotRunReason], assertions: &BTreeMap<String, bool>) -> Verdict {
    if !preflight.is_empty() {
        return Verdict::NotRun {
            reasons: preflight.to_vec(),
        };
    }
    if assertions.is_empty() {
        return Verdict::Red {
            failed: vec!["<no assertions>".into()],
        };
    }
    let failed: Vec<String> = assertions
        .iter()
        .filter(|(_, ok)| !**ok)
        .map(|(k, _)| k.clone())
        .collect();
    if failed.is_empty() {
        Verdict::Green
    } else {
        Verdict::Red { failed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_pass() -> BTreeMap<String, bool> {
        [("parse_rate".to_string(), true), ("exit".to_string(), true)].into()
    }

    /// demo-refusal-not-green-v1: a refused `needs` verb is NotRun even when
    /// every assertion passes.
    #[test]
    fn demo_refusal_not_green_v1() {
        let v = decide(
            &[NotRunReason::Refused("--json-schema".into())],
            &all_pass(),
        );
        assert!(!v.is_green());
        assert_eq!(v.to_string(), "NotRun{Refused(--json-schema)}");
    }

    #[test]
    fn green_needs_assertions_and_no_refusals() {
        assert!(decide(&[], &all_pass()).is_green());
        assert!(
            !decide(&[], &BTreeMap::new()).is_green(),
            "empty set is not Green"
        );
    }

    #[test]
    fn one_failed_assertion_is_red() {
        let mut a = all_pass();
        a.insert("planted_found".into(), false);
        assert_eq!(
            decide(&[], &a),
            Verdict::Red {
                failed: vec!["planted_found".into()]
            }
        );
    }

    #[test]
    fn every_reason_blocks_green() {
        let reasons = [
            NotRunReason::Refused("apr serve".into()),
            NotRunReason::PinRefused(">=0.69".into()),
            NotRunReason::VersionMismatch {
                tool: "apr".into(),
                pinned: "0.69.3".into(),
                found: "0.69.1".into(),
            },
            NotRunReason::WeightsMismatch {
                expected: "a".into(),
                found: "b".into(),
            },
            NotRunReason::FixturesMismatch {
                expected: "a".into(),
                found: "b".into(),
            },
            NotRunReason::UndeclaredApr("/home/x/apr".into()),
            NotRunReason::GpuLockHeld,
            NotRunReason::MissingTool("agy".into()),
        ];
        for r in reasons {
            assert!(
                !decide(std::slice::from_ref(&r), &all_pass()).is_green(),
                "{r}"
            );
        }
    }
}
