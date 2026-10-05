//! D21's driver library, shared by its two bins: `d21-agy-app-fanin` (the
//! demo) and `measure-stop-latency` (the E_6 entry gate that measures L).
//!
//! Modules: `json` (the record writer), `tree` (the reduced accessibility
//! tree and the agent rows), `drive` (the control path through agy-cdp),
//! `session` (preflight, launch, attach), `latency` (L from the samples and
//! from the embedded fixture), `reduce` (the pure fan-in reducer and its one
//! `permutations()` helper) and `record` (the run record and its checks).

pub mod drive;
pub mod json;
pub mod latency;
pub mod record;
pub mod reduce;
pub mod session;
pub mod tree;
