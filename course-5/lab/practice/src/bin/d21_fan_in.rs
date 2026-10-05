//! Provable contract: fan-in-v1 — all arrival orders reduce to one digest
//! (order_free); a red result refuses the merge and names the lowest red
//! agent (red_refuses); and no agent is stopped before the first red
//! (stops_follow_red).

use practice::fanin::{order_free, reduce, result, stops_follow_red, timeline, Event};

fn main() {
    let green = [
        result(1, false, "alpha ok"),
        result(2, false, "beta ok"),
        result(3, false, "gamma ok"),
    ];
    let one_red = [
        result(1, false, "alpha ok"),
        result(2, false, "beta ok"),
        result(3, true, "gamma FAIL"),
    ];
    let two_red = [
        result(1, false, "alpha ok"),
        result(2, true, "beta FAIL"),
        result(3, true, "gamma FAIL"),
    ];

    for set in [&green[..], &one_red[..], &two_red[..]] {
        assert!(
            order_free(set, reduce),
            "arrival order changed the reduction"
        );
    }
    assert!(reduce(&green).merged);
    assert_eq!(reduce(&one_red).refusal.as_deref(), Some("agent-3"));
    assert_eq!(
        reduce(&two_red).refusal.as_deref(),
        Some("agent-2"),
        "name the LOWEST red agent"
    );
    assert_ne!(reduce(&green).digest, reduce(&one_red).digest);

    let events = timeline(&[(5, 1, false), (3, 3, true), (9, 2, false)]);
    assert!(stops_follow_red(&events), "{events:?}");
    assert!(
        events.contains(&(4, Event::Stopped { agent: 2 })),
        "agent 2 was never stopped"
    );
    let calm = timeline(&[(5, 1, false), (3, 3, false), (9, 2, false)]);
    assert!(
        !calm.iter().any(|(_, e)| matches!(e, Event::Stopped { .. })),
        "stopped without a red"
    );

    for c in ["order_free", "red_refuses", "stops_follow_red"] {
        println!("contract: fan-in-v1/{c} OK");
    }
}
