//! E_4 (ph4 entry gate): probe the Antigravity app's controls through agy-cdp.
//!
//! Skeleton: this bin is not built yet. Until its build phase lands it
//! returns NotRun (exit 2) and never Green.
//!
//! Provable contract: e3-probe-v1 — all six controls are found in the accessibility tree and the F1/F12 DevTools counts are recorded; the role/name map is written only into the run directory.

use std::process::ExitCode;

/// Flipped by the build phase that implements this bin.
const fn built() -> bool {
    false
}

fn main() -> ExitCode {
    if !built() {
        eprintln!("NotRun: e3-probe is not built yet");
        return ExitCode::from(2);
    }
    assert!(built(), "e3-probe: unreachable while the skeleton stands");
    println!("contract: e3-probe-v1 OK");
    ExitCode::SUCCESS
}
