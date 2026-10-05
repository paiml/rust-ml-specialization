//! Provable contract: one-server-v1 — two agents share one model server that
//! loads exactly once (one_load); each writes only its own sink
//! (disjoint_writes); the checker reads the draft only after it is written
//! (causality); every schedule produces the same bytes
//! (schedule_independence); and the verdict is the checker's (checker_decides).

use practice::one_server::{run, Agent, Sinks, DRAFT, VERDICT};

fn main() {
    let prompt = "Is 17 prime?";
    let sequential = run(prompt, &[Agent::Writer, Agent::Checker], false);
    let checker_first = run(
        prompt,
        &[Agent::Checker, Agent::Checker, Agent::Writer],
        false,
    );

    for r in [&sequential, &checker_first] {
        // one_load
        assert_eq!(r.server.loads, 1, "the model server loaded more than once");
        // disjoint_writes: every file sits in its creator's sink
        for (path, (owner, _, _)) in &r.sinks.files {
            assert!(path.starts_with(owner.sink()), "{owner:?} wrote {path}");
        }
        // causality
        let written_at = r.sinks.files[DRAFT].1;
        let read_at = r.checker_read_at.expect("the checker never read the draft");
        assert!(
            read_at > written_at,
            "checker read at {read_at}, draft written at {written_at}"
        );
    }
    // disjoint_writes, refused path: the checker may not create the draft.
    let mut s = Sinks::default();
    assert!(s.create_new(Agent::Checker, 1, DRAFT, "forged").is_err());
    // schedule_independence
    assert_eq!(
        sequential.sinks.bytes(),
        checker_first.sinks.bytes(),
        "schedule changed the output"
    );
    // checker_decides: a tampered draft is failed by the checker, and only
    // the checker's file holds a verdict.
    let tampered = run(prompt, &[Agent::Writer, Agent::Checker], true);
    assert_eq!(sequential.sinks.files[VERDICT].2, "PASS");
    assert_eq!(tampered.sinks.files[VERDICT].2, "FAIL");
    assert!(!sequential.sinks.files[DRAFT].2.contains("PASS"));

    for c in [
        "one_load",
        "disjoint_writes",
        "causality",
        "schedule_independence",
        "checker_decides",
    ] {
        println!("contract: one-server-v1/{c} OK");
    }
}
