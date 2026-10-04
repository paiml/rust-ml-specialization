//! The whole D18 flow against `d18-fake-apr-serve`, without a GPU (spec §5.1,
//! CI): one server, two agents, both schedules, digests from disk, the run
//! record judged by pv. A missing pv fails here, it never skips (§4 item 6).

use d18_two_agents_one_server::flow::{self, Config, Screen};
use d18_two_agents_one_server::{CHECKER_FILE, WRITER_FILE};
use std::path::{Path, PathBuf};

const FAKE: &str = env!("CARGO_BIN_EXE_d18-fake-apr-serve");

/// Records every cue and line, in order, so the beat order is testable.
#[derive(Default)]
struct Tape {
    cues: Vec<String>,
    lines: Vec<String>,
}

impl Screen for Tape {
    fn cue(&mut self, tag: &str) {
        self.cues.push(tag.to_string());
    }
    fn line(&mut self, text: &str) {
        self.lines.push(text.to_string());
    }
}

fn run_root(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "d18-flow-{name}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
    ));
    std::fs::create_dir_all(&d).expect("run root");
    d
}

fn fake_config(run_dir: &Path) -> Config {
    Config {
        apr: PathBuf::from(FAKE),
        model: PathBuf::from("fake-model.gguf"),
        gpu: false,
        run_dir: run_dir.to_path_buf(),
        apr_version: "0.70.1".into(),
    }
}

fn spec_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("spec")
}

#[test]
fn the_fake_flow_holds_every_rust_assert_and_the_shapes_are_green() {
    let root = run_root("green");
    let mut tape = Tape::default();
    let out = flow::run(&fake_config(&root), &mut tape).expect("flow runs");
    for (name, ok) in &out.checks {
        assert!(ok.is_ok(), "{name}: {ok:?}");
    }
    assert_eq!(out.checks.len(), 6, "{:?}", out.checks.keys());
    assert_eq!(out.record["writer_files"], serde_json::json!([WRITER_FILE]));
    assert_eq!(
        out.record["checker_files"],
        serde_json::json!([CHECKER_FILE])
    );
    assert_eq!(
        out.record["digest_sequential"],
        out.record["digest_pipelined"]
    );
    assert_eq!(out.record["server_pid_start"], out.record["server_pid_end"]);

    let bytes = serde_json::to_vec_pretty(&out.record).expect("record");
    let shapes = demo_kit::shapes::judge(&spec_dir(), &bytes, &root);
    assert!(shapes.is_green(), "{shapes:?}");
    assert_eq!(shapes.focus_nodes_n, Some(5));
}

/// The cues the flow emits are the beat sheet's on-screen events, in sheet
/// order; beats that only narrate over a visible screen have none.
#[test]
fn cues_are_in_sheet_order() {
    let root = run_root("cues");
    let mut tape = Tape::default();
    flow::run(&fake_config(&root), &mut tape).expect("flow runs");
    assert_eq!(tape.cues, flow::FLOW_CUES);
    let all: Vec<&str> = flow::TITLE_CUES
        .iter()
        .chain(flow::PREFLIGHT_CUES)
        .chain(flow::FLOW_CUES)
        .chain(flow::JUDGE_CUES)
        .copied()
        .collect();
    let mut sorted = all.clone();
    sorted.sort();
    assert_eq!(all, sorted, "cues are in sheet order");
    sorted.dedup();
    assert_eq!(all.len(), sorted.len(), "no cue twice");
}

/// The judge's negative control: the planted record is Red with exactly the
/// committed `.expect` lines.
#[test]
fn the_planted_record_is_refused_with_the_expected_messages() {
    let root = run_root("planted");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (ok, why) = flow::planted_control(dir, &root);
    assert!(ok, "{why}");
}

/// A server that dies before it is healthy is an error, never a pass.
#[test]
fn a_server_that_never_comes_up_is_an_error() {
    let root = run_root("dead");
    let mut cfg = fake_config(&root);
    cfg.apr = PathBuf::from("/nonexistent/apr");
    let mut tape = Tape::default();
    assert!(flow::run(&cfg, &mut tape).is_err());
}
