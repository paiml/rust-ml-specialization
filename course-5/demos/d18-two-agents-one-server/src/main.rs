//! D18: two agents, one server.
//!
//! Skeleton: this bin is not built yet. Until its build phase lands it
//! returns NotRun (exit 2) and never Green.
//!
//! Provable contract: d18-run-v1 — the run record conforms to spec/d18-run-v1.yaml under `pv lint --gate shapes`, and the planted record is refused with exactly receipt.planted.expect.

use std::process::ExitCode;

/// Flipped by the build phase that implements this bin.
const fn built() -> bool {
    false
}

fn main() -> ExitCode {
    if !built() {
        eprintln!("NotRun: d18-two-agents-one-server is not built yet");
        return ExitCode::from(2);
    }
    assert!(
        built(),
        "d18-two-agents-one-server: unreachable while the skeleton stands"
    );
    println!("contract: d18-run-v1 OK");
    ExitCode::SUCCESS
}
