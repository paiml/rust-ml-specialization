//! demo-kit: the harness every course-5 demo runs inside.
//!
//! A demo is a test that someone can also record. This crate supplies the
//! parts that make that true:
//!
//! - [`manifest`]: `demo.toml`, the single source for test, receipt and card.
//! - [`pin`]: exact pins only. A floor (`>=`, `^`, `~`, bare) is refused.
//! - [`sha`]: weights and fixture-tree digests.
//! - [`preflight`]: everything that makes a demo `NotRun` before it starts.
//! - [`serve`]: a resident server whose process group dies with its guard.
//! - [`verdict`]: Green only when every assertion held and nothing refused.
//! - [`receipt`]: the JSON record a narration may cite, and nothing else.
//! - [`harness`]: the shared run loop — preflight, assert, decide, receipt.

pub mod harness;
pub mod manifest;
pub mod pin;
pub mod preflight;
pub mod receipt;
pub mod serve;
pub mod sha;
pub mod verdict;

pub use manifest::DemoManifest;
pub use verdict::{NotRunReason, Verdict};
