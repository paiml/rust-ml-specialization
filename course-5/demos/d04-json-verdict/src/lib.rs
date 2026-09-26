//! Library half of d04-json-verdict: the client-side schema validator (used
//! directly by the D04 binary) and the review-lane plumbing D07 and D08
//! build their quorum and planted-defect sweeps on top of.

pub mod lanes;
pub mod validator;
