//! The Rust asserts beyond the shapes (spec §5.1), one falsifier each. Every
//! survive row of D19's mutant table (s01–s06) is a defect the shapes cannot
//! see; each test below plants it and watches the owning assert refuse it,
//! after first showing that the same assert accepts the unplanted input.

use d18_two_agents_one_server::checks::{self, Times};
use d18_two_agents_one_server::ident::{self, Ident};
use d18_two_agents_one_server::sink::{Registry, Role};
use d18_two_agents_one_server::{record, CHECKER_FILE, WRITER_FILE};
use serde_json::Value;
use std::path::{Path, PathBuf};

const GOLDEN: &str = include_str!("../fixtures/receipt.golden.json");

fn golden() -> Value {
    serde_json::from_str(GOLDEN).expect("golden parses")
}

/// (sequential, pipelined) per-item times, read from the golden record.
fn golden_times() -> ([Times; 4], [Times; 4]) {
    record::times_from(&golden()).expect("golden carries all 32 times")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "d18-asserts-{name}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
    ));
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn write_outputs(root: &Path, answers: &str, verdicts: &str) {
    let reg = Registry::new(root);
    reg.open(Role::Writer, WRITER_FILE)
        .and_then(|mut s| s.append_line(answers))
        .expect("writer sink");
    reg.open(Role::Checker, CHECKER_FILE)
        .and_then(|mut s| s.append_line(verdicts))
        .expect("checker sink");
}

// ---- s06: pipelined_overlaps ------------------------------------------------

#[test]
fn pipelined_overlaps_accepts_the_golden_run() {
    let (seq, pipe) = golden_times();
    assert_eq!(checks::pipelined_overlaps(&seq, &pipe), Ok(()));
}

/// s06: a "pipelined" run in which every writer starts only after the previous
/// item's checker ended is a sequential run under the pipelined label.
#[test]
fn s06_a_pipelined_run_that_never_overlapped_is_refused() {
    let (seq, mut pipe) = golden_times();
    let mut t = 0;
    for item in pipe.iter_mut() {
        *item = Times {
            writer_start: t,
            writer_end: t + 100,
            checker_start: t + 101,
            checker_end: t + 200,
        };
        t += 201;
    }
    let err = checks::pipelined_overlaps(&seq, &pipe).expect_err("s06 must be refused");
    assert!(err.contains("pipelined"), "{err}");
}

#[test]
fn a_sequential_run_with_two_requests_in_flight_is_refused() {
    let (mut seq, pipe) = golden_times();
    // q2's writer starts before q1's writer ended
    seq[1].writer_start = seq[0].writer_end - 1;
    let err = checks::pipelined_overlaps(&seq, &pipe).expect_err("overlap in sequential");
    assert!(err.contains("sequential"), "{err}");
}

// ---- s04: one clock ---------------------------------------------------------

#[test]
fn one_clock_accepts_the_golden_run() {
    let (seq, pipe) = golden_times();
    assert_eq!(checks::one_clock(&seq, 9290), Ok(()));
    assert_eq!(checks::one_clock(&pipe, 8820), Ok(()));
}

/// s04: one item shifted 100 s later so that q2 follows q3.
#[test]
fn s04_an_item_shifted_100_s_later_is_refused() {
    let (seq, _) = golden_times();
    let mut shifted = seq;
    for v in [
        &mut shifted[1].writer_start,
        &mut shifted[1].writer_end,
        &mut shifted[1].checker_start,
        &mut shifted[1].checker_end,
    ] {
        *v += 100_000;
    }
    assert!(checks::one_clock(&shifted, 9290).is_err());
    // and with an elapsed long enough to admit it, order alone still refuses
    assert!(checks::one_clock(&shifted, 200_000).is_err());
}

#[test]
fn a_stamp_past_the_schedule_elapsed_time_is_refused() {
    let (seq, _) = golden_times();
    assert!(checks::one_clock(&seq, 9289).is_err());
}

// ---- s05: role-file binding by construction ---------------------------------

#[test]
fn s05_a_second_open_of_one_path_is_an_error_not_a_second_entry() {
    let root = scratch("s05");
    let reg = Registry::new(&root);
    let _first = reg.open(Role::Writer, WRITER_FILE).expect("first open");
    assert!(reg.open(Role::Writer, WRITER_FILE).is_err(), "create_new");
    assert_eq!(reg.files(Role::Writer), vec![WRITER_FILE.to_string()]);
    assert_eq!(reg.files(Role::Checker), Vec::<String>::new());
}

#[test]
fn disjoint_writes_accepts_exactly_one_file_per_role() {
    let w = vec![WRITER_FILE.to_string()];
    let c = vec![CHECKER_FILE.to_string()];
    assert_eq!(checks::disjoint_writes(&w, &c), Ok(()));
    // FALSIFY-D18-005 in Rust too: a writer that also wrote the verdicts
    let both = vec![WRITER_FILE.to_string(), CHECKER_FILE.to_string()];
    assert!(checks::disjoint_writes(&both, &c).is_err());
    assert!(checks::disjoint_writes(&w, &w).is_err());
    assert!(checks::disjoint_writes(&w, &[]).is_err());
}

// ---- s01: digests are recomputed --------------------------------------------

#[test]
fn s01_a_digest_that_is_not_the_bytes_on_disk_is_refused() {
    let root = scratch("s01");
    write_outputs(
        &root,
        r#"{"id":"q1","answer":"a"}"#,
        r#"{"id":"q1","decision":"accept"}"#,
    );
    let real = checks::digest_from_disk(&root).expect("digest");
    assert_eq!(checks::digest_recomputed(&root, &real), Ok(()));
    let empty = demo_kit::sha::sha256_bytes(b"");
    assert!(checks::digest_recomputed(&root, &empty).is_err());
}

// ---- s03: decisions come from the checker's file -----------------------------

#[test]
fn s03_a_flipped_decision_is_refused() {
    let root = scratch("s03");
    let verdicts = [
        r#"{"id":"q1","decision":"accept"}"#,
        r#"{"id":"q2","decision":"accept"}"#,
        r#"{"id":"q3","decision":"reject"}"#,
        r#"{"id":"q4","decision":"accept"}"#,
    ]
    .join("\n");
    write_outputs(&root, r#"{"id":"q1","answer":"a"}"#, &verdicts);
    let read = checks::decisions_from_disk(&root).expect("parse");
    assert_eq!(read, ["accept", "accept", "reject", "accept"]);
    let flipped = ["accept", "accept", "accept", "accept"].map(String::from);
    assert!(checks::decisions_from_checker(&root, &flipped).is_err());
    assert_eq!(checks::decisions_from_checker(&root, &read), Ok(()));
}

// ---- s02: server identity ---------------------------------------------------

fn me() -> Ident {
    ident::read(std::process::id()).expect("own /proc entry")
}

#[test]
fn start_ticks_survive_a_comm_with_spaces_and_parens() {
    let stat =
        "41210 (apr serve) (x)) S 1 41210 41210 0 -1 4194560 1 2 3 4 5 6 7 8 20 0 9 0 918273 1 2";
    assert_eq!(ident::start_ticks_from_stat(stat), Some(918_273));
    assert_eq!(ident::start_ticks_from_stat("garbage"), None);
}

#[test]
fn one_load_accepts_one_process_seen_twice() {
    let a = me();
    let pinned = a.exe.clone();
    assert_eq!(checks::one_load(1, &a, &me(), &pinned), Ok(()));
}

/// s02: a pid the shapes cannot tell from the real one. The Rust side reads
/// identity from the child handle and `/proc`, so a different process, a
/// respawn under the same pid, or an exe that is not the pinned binary fails.
#[test]
fn s02_a_different_process_or_binary_is_refused() {
    let a = me();
    let pinned = a.exe.clone();
    let mut respawn = a.clone();
    respawn.start_ticks += 1;
    assert!(checks::one_load(1, &a, &respawn, &pinned).is_err());
    let mut other = a.clone();
    other.pid = 41388;
    assert!(checks::one_load(1, &a, &other, &pinned).is_err());
    assert!(checks::one_load(1, &a, &a, Path::new("/nonexistent/apr")).is_err());
    assert!(checks::one_load(2, &a, &a, &pinned).is_err());
}

// ---- the record the shapes judge -------------------------------------------

/// The record keys are exactly the golden's, so the closed shapes see no
/// undeclared property and miss no declared one.
#[test]
fn record_keys_are_the_golden_keys() {
    let g = golden();
    let (seq, pipe) = golden_times();
    let id = Ident {
        pid: 41210,
        start_ticks: 918_273,
        exe: PathBuf::from("/x"),
    };
    let decisions = ["accept", "accept", "reject", "accept"].map(String::from);
    let digest = g["digest_sequential"].as_str().unwrap().to_string();
    let r = record::build(&record::Inputs {
        apr_version: "0.70.1",
        model_sha256: d18_two_agents_one_server::MODEL_SHA256,
        server_spawns: 1,
        start: &id,
        end: &id,
        writer_files: &[WRITER_FILE.to_string()],
        checker_files: &[CHECKER_FILE.to_string()],
        digest_sequential: &digest,
        digest_pipelined: &digest,
        seq: &seq,
        pipe: &pipe,
        decisions: &decisions,
    });
    assert_eq!(
        r, g,
        "a record built from the golden's values is the golden"
    );
}
