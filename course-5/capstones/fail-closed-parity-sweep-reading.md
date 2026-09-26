# Capstone Reading: A Fail-Closed Parity Sweep

This reading is the capstone for lesson 5.2, "A Fail-Closed Parity Sweep," in
*Agentic Patterns: Parallel Subagent ML with Rust* (Course 5). The lesson
teaches one idea: a fan-out/reduce pipeline for parallel subagent workers is
only trustworthy if it is fail-closed at every seam — the fan-out, the
reduce, the receipt that gates a verdict, the quorum across lanes, and the
ratchet that carries a baseline forward.

Each section below states one **named Contract** in prose, then a small,
stdlib-only Rust program that enforces it with `assert!`/`assert_eq!` and
prints `contract: <name> OK` on success. No section uses `proptest`, `rand`,
or any crate outside the standard library — every property sweep below is
driven by the same small linear congruential generator (LCG), so the
"randomness" is fully deterministic and reproducible run to run.

Every snippet was executed on the Rust Playground (`POST
https://play.rust-lang.org/execute`, `stable`, `2021` edition) before this
page was published: each run returned `success: true`, exit status 0, and
stdout containing its contract line. Each snippet was then minted as a gist
(`POST https://play.rust-lang.org/meta/gist`) via `rmedia playground publish`
— the automation layer described in `docs/specifications/playground-proofs.md`
— which re-verifies locally and remotely before it will mint. The raw
execute responses (request, `stdout`, `stderr`, `success`) are archived
alongside this reading under `run-logs/<contract>.json`. A gist with no
matching `contract: … OK` run log is, by the project's own rule, RED.

## Fan-out

**Contract `fanout-independent-v1`:** worker results are independent of
scheduling order. A fan-out step hands the same pure input to every worker
and lets them finish in whatever order the scheduler gives them. If the
final reduced value changes depending on *which worker happens to finish
first*, the pipeline has a hidden ordering dependency — a race dressed up as
parallelism. This snippet gives each of 16 workers a result that depends
only on its own id, then folds all 16 results with a commutative,
associative reducer (XOR-based) over 1000 LCG-shuffled completion orders. If
the fold were sensitive to order, at least one of the 1000 shuffles would
disagree with the unshuffled baseline.

```rust
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed ^ 0x9E3779B97F4A7C15)
    }

    fn next_u32(&mut self) -> u32 {
        // Numerical Recipes LCG constants.
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    fn shuffle<T>(&mut self, v: &mut [T]) {
        let len = v.len();
        for i in (1..len).rev() {
            let j = (self.next_u32() as usize) % (i + 1);
            v.swap(i, j);
        }
    }
}

const N_WORKERS: usize = 16;

// Each worker's result depends only on its own id: no shared mutable
// state, so scheduling order cannot change any individual result.
fn worker_result(id: usize) -> u64 {
    let x = (id as u64).wrapping_mul(2_654_435_761).wrapping_add(17);
    x ^ (x >> 13)
}

// The reducer folds per-worker contributions with XOR, which is
// commutative and associative: the fold order cannot change the total.
fn fan_out_reduce(completion_order: &[usize]) -> u64 {
    completion_order
        .iter()
        .fold(0u64, |acc, &id| acc ^ worker_result(id).wrapping_mul(id as u64 + 1))
}

fn main() {
    let baseline_order: Vec<usize> = (0..N_WORKERS).collect();
    let expected = fan_out_reduce(&baseline_order);

    let trials = 1000;
    let mut order: Vec<usize> = (0..N_WORKERS).collect();
    for trial in 0..trials {
        let mut lcg = Lcg::new(trial as u64 + 1);
        lcg.shuffle(&mut order);
        // Sanity: the shuffle really did permute the worker set.
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, baseline_order, "shuffle must stay a permutation of worker ids");

        let observed = fan_out_reduce(&order);
        assert_eq!(
            observed, expected,
            "trial {trial}: reduce depended on scheduling order {order:?}"
        );
    }

    println!("contract: fanout-independent-v1 OK");
}
```

▶ Run on Playground: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=cb9fc60e5b85008fcd8396876ac83882

## Reduce

**Contract `reduce-deterministic-v1`:** same inputs into the reducer produce
byte-identical output. Independence of order (the fan-out contract above) is
necessary but not sufficient: two runs could still land on equal *values*
while serializing to different bytes, which is enough to break a
content-addressed cache or a receipt hash. This snippet has 12 workers write
`(id, payload)` pairs into a map in a shuffled arrival order, then serializes
the reduced map by walking it in sorted-key order and formatting each entry
as `id:value;`. The serialized byte vector is compared, byte for byte,
against the unshuffled baseline across 1000 LCG-shuffled arrival orders.

```rust
use std::collections::BTreeMap;

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed ^ 0x9E3779B97F4A7C15)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    fn shuffle<T>(&mut self, v: &mut [T]) {
        let len = v.len();
        for i in (1..len).rev() {
            let j = (self.next_u32() as usize) % (i + 1);
            v.swap(i, j);
        }
    }
}

const N_WORKERS: usize = 12;

fn worker_payload(id: usize) -> u64 {
    (id as u64).wrapping_mul(0x2545_F491_4F6C_DD1D).wrapping_add(7)
}

// The reduce step: insert every (id, payload) pair into a map in
// `arrival_order`, then serialize deterministically by walking the map
// in sorted-key order (BTreeMap iteration order is always ascending by
// key, regardless of insertion order).
fn reduce_to_bytes(arrival_order: &[usize]) -> Vec<u8> {
    let mut reduced: BTreeMap<usize, u64> = BTreeMap::new();
    for &id in arrival_order {
        reduced.insert(id, worker_payload(id));
    }
    let mut out = String::new();
    for (id, value) in &reduced {
        out.push_str(&format!("{id}:{value};"));
    }
    out.into_bytes()
}

fn main() {
    let baseline_order: Vec<usize> = (0..N_WORKERS).collect();
    let expected_bytes = reduce_to_bytes(&baseline_order);
    assert!(!expected_bytes.is_empty(), "reduced output must not be empty");

    let trials = 1000;
    let mut order: Vec<usize> = (0..N_WORKERS).collect();
    for trial in 0..trials {
        let mut lcg = Lcg::new(trial as u64 + 101);
        lcg.shuffle(&mut order);

        let observed_bytes = reduce_to_bytes(&order);
        assert_eq!(
            observed_bytes, expected_bytes,
            "trial {trial}: reduce output was not byte-identical for arrival order {order:?}"
        );
    }

    println!("contract: reduce-deterministic-v1 OK");
}
```

▶ Run on Playground: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=945161a5b89c6730f926d5921884eaec

## Receipt

**Contract `receipt-gate-v1`:** no receipt means the verdict is never Green.
A `Verdict` in this pipeline is one of `Green`, `Red`, or `NotRun{reason}`.
The only way to reach `Green` is a receipt that is both present and whose
assertions passed; a missing receipt, or one that exists but is marked
absent, must fall to `NotRun`, never to `Green`. This snippet checks the
three non-receipt shapes explicitly, checks the one `Green` path and the one
`Red` path explicitly, and then sweeps 1000 LCG-generated receipt states —
including the "no receipt at all" case — asserting on every single one that
a missing or absent receipt is never scored `Green`.

```rust
#[derive(Debug, PartialEq, Eq, Clone)]
enum Verdict {
    Green,
    Red,
    NotRun(&'static str),
}

#[derive(Debug, Clone, Copy)]
struct Receipt {
    present: bool,
    assertions_passed: bool,
}

// The gate: the only path to `Verdict::Green` requires a present receipt
// whose assertions passed. Every other combination is NotRun or Red.
fn gate(receipt: Option<Receipt>) -> Verdict {
    match receipt {
        None => Verdict::NotRun("missing receipt"),
        Some(r) if !r.present => Verdict::NotRun("receipt not present"),
        Some(r) if r.assertions_passed => Verdict::Green,
        Some(_) => Verdict::Red,
    }
}

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed ^ 0x9E3779B97F4A7C15)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    fn next_bool(&mut self) -> bool {
        self.next_u32() % 2 == 0
    }
}

fn main() {
    // Exhaustive check of the three receipt shapes that must never be Green.
    assert_eq!(gate(None), Verdict::NotRun("missing receipt"));
    assert_eq!(
        gate(Some(Receipt { present: false, assertions_passed: true })),
        Verdict::NotRun("receipt not present")
    );
    assert_eq!(
        gate(Some(Receipt { present: false, assertions_passed: false })),
        Verdict::NotRun("receipt not present")
    );

    // The only Green path: present + passed.
    assert_eq!(
        gate(Some(Receipt { present: true, assertions_passed: true })),
        Verdict::Green
    );
    // Present + failed assertions -> Red, still not Green.
    assert_eq!(
        gate(Some(Receipt { present: true, assertions_passed: false })),
        Verdict::Red
    );

    // Property sweep: over 1000 LCG-generated receipt states, Green
    // appears if and only if present && assertions_passed; a missing or
    // not-present receipt is never Green.
    let mut lcg = Lcg::new(4242);
    let mut green_count = 0usize;
    let mut not_run_count = 0usize;
    for _ in 0..1000 {
        let has_receipt = lcg.next_bool();
        let receipt = if has_receipt {
            Some(Receipt {
                present: lcg.next_bool(),
                assertions_passed: lcg.next_bool(),
            })
        } else {
            None
        };

        let verdict = gate(receipt);

        let receipt_missing_or_absent = match receipt {
            None => true,
            Some(r) => !r.present,
        };
        if receipt_missing_or_absent {
            assert_ne!(verdict, Verdict::Green, "a missing/absent receipt produced Green");
        }
        match verdict {
            Verdict::Green => green_count += 1,
            Verdict::NotRun(_) => not_run_count += 1,
            Verdict::Red => {}
        }
    }
    assert!(green_count > 0, "sweep never exercised the Green path");
    assert!(not_run_count > 0, "sweep never exercised the NotRun path");

    println!("contract: receipt-gate-v1 OK");
}
```

▶ Run on Playground: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=ec97d65b005e78aa85ec4a8f3610806a

## Quorum

**Contract `quorum-one-fail-blocks-v1`:** any FAIL blocks the quorum. When
several lanes (say, one per subagent) each produce a `Verdict`, the quorum
across them must fail closed: one `Red` lane makes the whole quorum `Red`,
full stop, regardless of what the other lanes say. And a lane that never ran
— `NotRun` — is not a pass either: with zero `Red` lanes but at least one
`NotRun` lane, the quorum must land on `NotRun`, never `Green`. Only a
quorum where every lane is `Green` may itself be `Green`. This snippet
builds the exhaustive truth table for 3 lanes over the 3 verdict states
(3³ = 27 combinations) and asserts the rule holds on every row.

```rust
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Verdict {
    Green,
    Red,
    NotRun,
}

const STATES: [Verdict; 3] = [Verdict::Green, Verdict::Red, Verdict::NotRun];

// Quorum rule: any Red wins outright (fail-closed). Otherwise, any
// NotRun blocks a Green result. Only all-Green resolves to Green.
fn quorum(lanes: &[Verdict]) -> Verdict {
    if lanes.iter().any(|v| *v == Verdict::Red) {
        return Verdict::Red;
    }
    if lanes.iter().any(|v| *v == Verdict::NotRun) {
        return Verdict::NotRun;
    }
    Verdict::Green
}

fn main() {
    let mut combos_checked = 0usize;
    let mut saw_red_blocks = false;
    let mut saw_not_run_blocks = false;

    // Exhaustive truth table: every combination of 3 lanes over the 3
    // verdict states (3^3 = 27 rows).
    for &a in STATES.iter() {
        for &b in STATES.iter() {
            for &c in STATES.iter() {
                let lanes = [a, b, c];
                let result = quorum(&lanes);
                combos_checked += 1;

                let any_red = lanes.iter().any(|v| *v == Verdict::Red);
                let any_not_run = lanes.iter().any(|v| *v == Verdict::NotRun);
                let all_green = lanes.iter().all(|v| *v == Verdict::Green);

                // Named contract: any FAIL (Red) -> FAIL (Red), full stop.
                if any_red {
                    assert_eq!(result, Verdict::Red, "lanes {lanes:?}: a FAIL lane did not block");
                    saw_red_blocks = true;
                    continue;
                }

                // No Red present. NotRun is not a pass: it can never
                // resolve to Green even with zero failures.
                if any_not_run {
                    assert_ne!(result, Verdict::Green, "lanes {lanes:?}: NotRun counted as a pass");
                    assert_eq!(result, Verdict::NotRun);
                    saw_not_run_blocks = true;
                    continue;
                }

                // Only remaining case: every lane Green.
                assert!(all_green, "unreachable: no Red, no NotRun, but not all Green");
                assert_eq!(result, Verdict::Green, "lanes {lanes:?}: all-Green did not resolve Green");
            }
        }
    }

    assert_eq!(combos_checked, 27, "truth table must cover all 3^3 combinations");
    assert!(saw_red_blocks, "truth table never exercised a Red-blocks row");
    assert!(saw_not_run_blocks, "truth table never exercised a NotRun-blocks row");

    println!("contract: quorum-one-fail-blocks-v1 OK");
}
```

▶ Run on Playground: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=cd6f8e57183796c05c131a71362d9d84

## Kaizen

**Contract `ratchet-monotone-v1`:** a new baseline never regresses. Once a
run has established a baseline, the only way to move it forward is with a
candidate that is an improvement or a tie; a worse candidate is refused, and
the prior baseline is kept unchanged. This is the mechanism that keeps
"kaizen" (continuous improvement) honest — nothing downstream of the
baseline can silently slide backwards. This snippet runs 1000 independent
LCG-generated sequences of 50 candidate scores each, applying the ratchet at
every step, and asserts that the recorded baseline history is non-decreasing
across every single sequence. It also asserts that the sweep genuinely
exercised both an accepted improvement and a refused regression, so the
property isn't holding vacuously.

```rust
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed ^ 0x9E3779B97F4A7C15)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    // Bounded score so regressions and improvements both show up often.
    fn next_score(&mut self, bound: u32) -> u32 {
        self.next_u32() % bound
    }
}

// The ratchet: `Ok(new_baseline)` on improvement-or-equal, `Err(kept)` on
// a regression, where `kept` is the unchanged prior baseline.
fn ratchet(baseline: u32, candidate: u32) -> Result<u32, u32> {
    if candidate >= baseline {
        Ok(candidate)
    } else {
        Err(baseline)
    }
}

fn run_sequence(seed: u64, steps: usize) -> (Vec<u32>, usize) {
    let mut lcg = Lcg::new(seed);
    let mut baseline = lcg.next_score(1000);
    let mut history = vec![baseline];
    let mut refusals = 0usize;

    for _ in 0..steps {
        let candidate = lcg.next_score(1000);
        match ratchet(baseline, candidate) {
            Ok(new_baseline) => baseline = new_baseline,
            Err(kept) => {
                refusals += 1;
                baseline = kept;
            }
        }
        history.push(baseline);
    }
    (history, refusals)
}

fn main() {
    let sequences = 1000;
    let steps_per_sequence = 50;
    let mut total_refusals = 0usize;
    let mut total_improvements = 0usize;

    for seed in 0..sequences {
        let (history, refusals) = run_sequence(seed as u64 + 1, steps_per_sequence);
        total_refusals += refusals;
        total_improvements += steps_per_sequence - refusals;

        // The contract: baseline history is non-decreasing, i.e. the
        // ratchet never lets a later baseline fall below an earlier one.
        for window in history.windows(2) {
            assert!(
                window[1] >= window[0],
                "seed {seed}: baseline regressed from {} to {}",
                window[0],
                window[1]
            );
        }
    }

    assert!(total_refusals > 0, "property sweep never exercised a regression refusal");
    assert!(total_improvements > 0, "property sweep never exercised an accepted improvement");

    println!("contract: ratchet-monotone-v1 OK");
}
```

▶ Run on Playground: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=edb7d19e0dba8b88f130a41cbd327f06

## Contract summary

| Section | Contract | Asserted by | Gist URL | Run date |
|---|---|---|---|---|
| Fan-out | `fanout-independent-v1` | 1000 LCG-shuffled completion orders vs. an unshuffled baseline, `assert_eq!` per trial | https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=cb9fc60e5b85008fcd8396876ac83882 | 2026-09-26 |
| Reduce | `reduce-deterministic-v1` | 1000 LCG-shuffled arrival orders, byte-for-byte `assert_eq!` of the serialized reduced map | https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=945161a5b89c6730f926d5921884eaec | 2026-09-26 |
| Receipt | `receipt-gate-v1` | exhaustive check of the 3 non-Green receipt shapes plus a 1000-sample LCG sweep asserting `Green` never appears without a present, passed receipt | https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=ec97d65b005e78aa85ec4a8f3610806a | 2026-09-26 |
| Quorum | `quorum-one-fail-blocks-v1` | exhaustive 3-lane × 3-state truth table (27 rows) asserting any `Red` forces `Red` and `NotRun` never resolves to `Green` | https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=cd6f8e57183796c05c131a71362d9d84 | 2026-09-26 |
| Kaizen | `ratchet-monotone-v1` | 1000 LCG-generated 50-step score sequences, asserting the baseline history is non-decreasing in every sequence | https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=edb7d19e0dba8b88f130a41cbd327f06 | 2026-09-26 |

Every gist above was executed on the Playground (`success: true`, exit 0,
stdout containing its `contract: … OK` line) before it was minted. The raw
execute responses are archived as `run-logs/<contract>.json` in this
directory — the falsifier for this reading is a gist with no matching run
log showing that exact line.
