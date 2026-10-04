//! D20: fan-out, watchable in the recording.
//!
//! Skeleton: this bin is not built yet. Until its build phase lands it
//! returns NotRun (exit 2) and never Green.
//!
//! Provable contract: d20-run-v1 — the run record conforms to spec/d20-run-v1.yaml, and app_evidence equals EVIDENCE intersected with the socket-tallied cdp_methods.

use std::process::ExitCode;

/// Flipped by the build phase that implements this bin.
const fn built() -> bool {
    false
}

fn main() -> ExitCode {
    if !built() {
        eprintln!("NotRun: d20-agy-app-fanout is not built yet");
        return ExitCode::from(2);
    }
    assert!(
        built(),
        "d20-agy-app-fanout: unreachable while the skeleton stands"
    );
    println!("contract: d20-run-v1 OK");
    ExitCode::SUCCESS
}
