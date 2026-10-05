//! Each exercise's checks must REFUSE a well-formed but wrong input, not only
//! accept the right one.

use practice::fanin::{
    order_free, reduce, result, stops_follow_red, AgentResult, Event, Reduction,
};
use practice::judge::{golden, judge, kill_rows, CLAIMS};
use practice::one_server::{run, Agent, Sinks, DRAFT};

#[test]
fn d18_writer_cannot_create_in_the_checker_sink() {
    let mut s = Sinks::default();
    assert!(s
        .create_new(Agent::Writer, 1, "ws/checker/verdict.md", "PASS")
        .is_err());
}

#[test]
fn d18_a_file_is_created_once() {
    let mut s = Sinks::default();
    s.create_new(Agent::Writer, 1, DRAFT, "a").unwrap();
    assert!(s.create_new(Agent::Writer, 2, DRAFT, "b").is_err());
}

#[test]
fn d18_tampered_draft_fails_the_check() {
    let r = run("q", &[Agent::Writer, Agent::Checker], true);
    assert_eq!(r.sinks.files["ws/checker/verdict.md"].2, "FAIL");
}

#[test]
fn d19_every_claim_has_a_kill_row() {
    let named: Vec<&str> = kill_rows().iter().map(|k| k.2).collect();
    for c in CLAIMS {
        assert!(named.contains(&c), "no mutant tests {c}");
    }
}

#[test]
fn d19_a_judge_that_passes_everything_is_caught() {
    let lazy = |_: &[practice::judge::Step]| -> Vec<&'static str> { Vec::new() };
    let (_, mutate, _) = &kill_rows()[0];
    let mut r = golden();
    mutate(&mut r);
    assert!(lazy(&r).is_empty() && !judge(&r).is_empty());
}

fn first_arrival(results: &[AgentResult]) -> Reduction {
    let mut x = reduce(results);
    x.digest = u64::from(results[0].agent);
    x
}

#[test]
fn d21_an_order_sensitive_reducer_is_refused() {
    let set = [
        result(1, false, "a"),
        result(2, false, "b"),
        result(3, true, "c"),
    ];
    assert!(!order_free(&set, first_arrival));
}

#[test]
fn d21_a_stop_before_the_red_is_refused() {
    let early = [
        (2, Event::Stopped { agent: 1 }),
        (
            3,
            Event::Finished {
                agent: 3,
                red: true,
            },
        ),
    ];
    assert!(!stops_follow_red(&early));
}
