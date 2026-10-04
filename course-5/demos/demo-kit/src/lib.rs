//! demo-kit: the harness every course-5 demo runs inside.
//!
//! A demo is a test that someone can also record. This crate supplies the
//! parts that make that true:
//!
//! - [`manifest`]: `demo.toml`, the single source for test, receipt and card.
//! - [`pin`]: exact pins only (and one series form, for `apr`).
//! - [`sha`]: weights and fixture-tree digests.
//! - [`preflight`]: everything that makes a demo `NotRun` before it starts.
//! - [`shapes`]: pv's SHACL gate over a run record, as a verdict.
//! - [`verdict`]: Green only when every assertion held and nothing refused.
//! - [`receipt`]: the JSON record a narration may cite, and nothing else.
//!
//! Behind the default `process` feature (they spawn processes, open sockets
//! or call `libc`):
//!
//! - [`serve`]: a resident server whose process group dies with its guard.
//! - [`harness`]: the shared run loop — preflight, assert, decide, receipt.
//! - [`fake_serve`]: the body of the `apr serve run` test double.
//!
//! The confined crates (`agy-cdp`, d20, d21) depend on demo-kit with
//! `default-features = false`, so none of that code is in their closure; the
//! xtask confinement lint skips exactly the modules gated on `process`.

#[cfg(feature = "process")]
pub mod fake_serve;
#[cfg(feature = "process")]
pub mod harness;
pub mod manifest;
pub mod pin;
pub mod preflight;
pub mod receipt;
#[cfg(feature = "process")]
pub mod serve;
pub mod sha;
pub mod shapes;
pub mod verdict;

pub use manifest::DemoManifest;
pub use verdict::{NotRunReason, Verdict};
