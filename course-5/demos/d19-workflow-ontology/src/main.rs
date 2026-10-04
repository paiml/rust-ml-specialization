//! D19: the workflow as an ontology.
//!
//! Skeleton: this bin is not built yet. Until its build phase lands it
//! returns NotRun (exit 2) and never Green.
//!
//! Provable contract: d19-run-v1 — the run record conforms to spec/d19-run-v1.yaml, and every mutant in fixtures/mutants.json is killed or survives as its row states.

use std::process::ExitCode;

/// Flipped by the build phase that implements this bin.
const fn built() -> bool {
    false
}

fn main() -> ExitCode {
    if !built() {
        eprintln!("NotRun: d19-workflow-ontology is not built yet");
        return ExitCode::from(2);
    }
    assert!(
        built(),
        "d19-workflow-ontology: unreachable while the skeleton stands"
    );
    println!("contract: d19-run-v1 OK");
    ExitCode::SUCCESS
}
