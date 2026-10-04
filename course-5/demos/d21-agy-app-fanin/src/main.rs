//! D21: fan-in and stop-the-line.
//!
//! Skeleton: this bin is not built yet. Until its build phase lands it
//! returns NotRun (exit 2) and never Green.
//!
//! Provable contract: d21-run-v1 — the run record conforms to spec/d21-run-v1.yaml, and the line stops within the committed stop latency L.

use std::process::ExitCode;

/// Flipped by the build phase that implements this bin.
const fn built() -> bool {
    false
}

fn main() -> ExitCode {
    if !built() {
        eprintln!("NotRun: d21-agy-app-fanin is not built yet");
        return ExitCode::from(2);
    }
    assert!(
        built(),
        "d21-agy-app-fanin: unreachable while the skeleton stands"
    );
    println!("contract: d21-run-v1 OK");
    ExitCode::SUCCESS
}
