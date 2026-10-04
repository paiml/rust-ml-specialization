//! E_6: measure the stop latency L that D21 asserts against.
//!
//! Skeleton: this bin is not built yet. Until its build phase lands it
//! returns NotRun (exit 2) and never Green.
//!
//! Provable contract: stop-latency-v1 — at least 30 stop samples are taken on the demo profile and L = max(p99, 2 x p95), rounded up, is written beside them in the run directory.

use std::process::ExitCode;

/// Flipped by the build phase that implements this bin.
const fn built() -> bool {
    false
}

fn main() -> ExitCode {
    if !built() {
        eprintln!("NotRun: measure-stop-latency is not built yet");
        return ExitCode::from(2);
    }
    assert!(
        built(),
        "measure-stop-latency: unreachable while the skeleton stands"
    );
    println!("contract: stop-latency-v1 OK");
    ExitCode::SUCCESS
}
