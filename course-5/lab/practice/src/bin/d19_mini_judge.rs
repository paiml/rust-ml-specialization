//! Provable contract: workflow-judge-v1 — the golden record violates no
//! claim; every kill-row mutant is refused for exactly its named claim; and
//! every survive-row mutant passes.

use practice::judge::{golden, judge, kill_rows, survive_rows};

fn main() {
    assert!(
        judge(&golden()).is_empty(),
        "golden record refused: {:?}",
        judge(&golden())
    );
    let kills = kill_rows();
    for (label, mutate, claim) in &kills {
        let mut r = golden();
        mutate(&mut r);
        assert_eq!(judge(&r), vec![*claim], "kill row `{label}`");
    }
    let survivors = survive_rows();
    for (label, mutate) in &survivors {
        let mut r = golden();
        mutate(&mut r);
        assert!(
            judge(&r).is_empty(),
            "survive row `{label}` was refused: {:?}",
            judge(&r)
        );
    }
    assert_eq!(kills.len(), 5, "one kill row per claim");
    println!(
        "contract: workflow-judge-v1 OK ({} killed, {} survived)",
        kills.len(),
        survivors.len()
    );
}
