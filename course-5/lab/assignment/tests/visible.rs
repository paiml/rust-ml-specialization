//! Visible tests: a quick check while you work. The grader runs more.

use parity_sweep::{fan_out_reduce, gate, parity_sweep, quorum, reduce_to_bytes, Receipt, Verdict};

#[test]
fn part1_empty_fan_out_is_zero() {
    assert_eq!(fan_out_reduce(&[]), 0);
}

#[test]
fn part2_example_from_the_docs() {
    assert_eq!(reduce_to_bytes(&[(2, 9), (0, 5)]), b"0:5;2:9;".to_vec());
}

#[test]
fn part3_missing_receipt_is_not_run() {
    assert_eq!(gate(None), Verdict::NotRun("missing receipt"));
}

#[test]
fn part4_one_red_blocks() {
    assert_eq!(
        quorum(&[Verdict::Green, Verdict::Red, Verdict::Green]),
        Verdict::Red
    );
}

#[test]
fn part5_one_good_lane_is_green() {
    let ok = Some(Receipt {
        present: true,
        assertions_passed: true,
    });
    assert_eq!(parity_sweep(&[ok]), Verdict::Green);
}
