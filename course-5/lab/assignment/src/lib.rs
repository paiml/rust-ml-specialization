//! Capstone assignment: a fail-closed parity sweep.
//!
//! You implement the five functions marked `todo!()`. Everything else is
//! given. `cargo run -- <part>` checks one part against its named Provable
//! contract and prints `contract: <name> OK` only when the contract holds.
//! `cargo test` runs the visible tests. The grader runs further tests that you
//! cannot see, so a function that only satisfies `main` will not score.
//!
//! Rules for this file (the grader refuses a submission that breaks them):
//! - Do not print, exit, read the environment, or touch files, processes or
//!   the network. Your functions compute values and return them.
//! - No `unsafe`, and no `include!` or other macros that read other files.
//!
//! | Part | Function           | Provable contract            |
//! |------|--------------------|------------------------------|
//! | 1    | `fan_out_reduce`   | `fanout-independent-v1`      |
//! | 2    | `reduce_to_bytes`  | `reduce-deterministic-v1`    |
//! | 3    | `gate`             | `receipt-gate-v1`            |
//! | 4    | `quorum`           | `quorum-one-fail-blocks-v1`  |
//! | 5    | `parity_sweep`     | `parity-sweep-fail-closed-v1`|

/// A lane's verdict. `NotRun` carries the reason it did not run.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Verdict {
    Green,
    Red,
    NotRun(&'static str),
}

/// What a lane left behind. `present` is false when the receipt file was
/// expected but is missing or empty.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Receipt {
    pub present: bool,
    pub assertions_passed: bool,
}

/// GIVEN. Worker `id`'s result. It depends only on `id`, so no schedule can
/// change it.
pub fn worker_result(id: usize) -> u64 {
    let x = (id as u64).wrapping_mul(2_654_435_761).wrapping_add(17);
    x ^ (x >> 13)
}

/// PART 1 — Contract `fanout-independent-v1`.
///
/// Fold the results of the workers in `completion_order` into one value.
/// Each worker `id` contributes `worker_result(id).wrapping_mul(id as u64 + 1)`,
/// and contributions are combined with XOR, starting from 0.
///
/// The answer must not depend on the order of `completion_order`. An empty
/// slice folds to 0.
pub fn fan_out_reduce(completion_order: &[usize]) -> u64 {
    let _ = completion_order;
    todo!("part 1: fold every worker's contribution with XOR")
}

/// PART 2 — Contract `reduce-deterministic-v1`.
///
/// Serialize `(id, value)` pairs as the bytes of `"{id}:{value};"` for each
/// pair, in ascending order of `id`, whatever order they arrived in. Ids are
/// unique. An empty slice serializes to no bytes.
///
/// Example: `[(2, 9), (0, 5)]` → `b"0:5;2:9;"`.
pub fn reduce_to_bytes(results: &[(usize, u64)]) -> Vec<u8> {
    let _ = results;
    todo!("part 2: canonical, byte-identical serialization")
}

/// PART 3 — Contract `receipt-gate-v1`.
///
/// Turn a lane's receipt into a verdict. No receipt is never Green:
/// - `None` → `NotRun("missing receipt")`
/// - present == false → `NotRun("receipt not present")`
/// - present and assertions passed → `Green`
/// - present and assertions failed → `Red`
pub fn gate(receipt: Option<Receipt>) -> Verdict {
    let _ = receipt;
    todo!("part 3: the only road to Green is a present receipt that passed")
}

/// PART 4 — Contract `quorum-one-fail-blocks-v1`.
///
/// Combine lane verdicts, failing closed:
/// - any `Red` → `Red`
/// - else any `NotRun(_)` → `NotRun("lane not run")`
/// - else, if there is at least one lane and all are `Green` → `Green`
/// - no lanes at all → `NotRun("no lanes")` (an empty quorum is not a pass)
pub fn quorum(lanes: &[Verdict]) -> Verdict {
    let _ = lanes;
    todo!("part 4: one FAIL blocks; NotRun and an empty quorum are not passes")
}

/// PART 5 — Contract `parity-sweep-fail-closed-v1`.
///
/// The capstone: each lane's receipt goes through `gate`, and the lane
/// verdicts go through `quorum`. Use your two functions; do not re-derive
/// the rules.
pub fn parity_sweep(receipts: &[Option<Receipt>]) -> Verdict {
    let _ = receipts;
    todo!("part 5: gate every lane, then take the quorum")
}
