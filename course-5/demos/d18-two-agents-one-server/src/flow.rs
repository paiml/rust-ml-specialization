//! The D18 run, steps 2–4 of spec §5.1: one server, both schedules, digests
//! from disk, the record, and the Rust asserts over that record. The bin adds
//! the title card, the preflight and step 5 (judge, receipt, contract line).
//!
//! Every on-screen event of the beat sheet that happens here is preceded by a
//! [`Screen::cue`] with its beat tag, in sheet order ([`FLOW_CUES`]). Beats
//! that only narrate over a screen already visible carry no cue.

use crate::agents::Http;
use crate::checks;
use crate::ident::{self, Ident};
use crate::record;
use crate::schedule::{self, Kind, ScheduleRun};
use crate::{CHECKER_FILE, WRITER_FILE};
use serde_json::Value;
use std::collections::BTreeMap;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The title card (B01; B02–B03 narrate over it).
pub const TITLE_CUES: &[&str] = &["D18-B01"];
/// The preflight lines (B04), then the GPU line (B05).
pub const PREFLIGHT_CUES: &[&str] = &["D18-B04", "D18-B05"];
/// Server spawn (B06), its identity (B07; B08 narrates over it), the two
/// schedules (B09–B15) and the digests (B16, B17; B18 narrates over them).
pub const FLOW_CUES: &[&str] = &[
    "D18-B06", "D18-B07", "D18-B09", "D18-B10", "D18-B11", "D18-B12", "D18-B13", "D18-B14",
    "D18-B15", "D18-B16", "D18-B17",
];
/// The judge (B19), the end-of-run checks (B20), the contract line (B21).
pub const JUDGE_CUES: &[&str] = &["D18-B19", "D18-B20", "D18-B21"];

/// Where the run's lines and beat cues go.
pub trait Screen {
    /// The beat `tag`'s on-screen event is next.
    fn cue(&mut self, tag: &str);
    fn line(&mut self, text: &str);
}

/// The terminal: lines to stdout, cues to the pacer (if any). Each cue's
/// time since the terminal opened is kept, paced or not, so an unpaced run
/// measures how long each beat's screen step takes.
pub struct Terminal<'a> {
    pacer: Option<&'a demo_kit::pace::Pacer>,
    t0: std::time::Instant,
    cues: Vec<(String, f64)>,
}

impl<'a> Terminal<'a> {
    pub fn new(pacer: Option<&'a demo_kit::pace::Pacer>) -> Self {
        Terminal {
            pacer,
            t0: std::time::Instant::now(),
            cues: Vec::new(),
        }
    }

    /// (tag, seconds since the terminal opened) for every cue, in order.
    pub fn cue_times(&self) -> &[(String, f64)] {
        &self.cues
    }
}

impl Screen for Terminal<'_> {
    fn cue(&mut self, tag: &str) {
        if let Err(e) = demo_kit::pace::cue(self.pacer, tag) {
            eprintln!("pace: {tag}: {e}");
        }
        self.cues
            .push((tag.to_string(), self.t0.elapsed().as_secs_f64()));
    }
    fn line(&mut self, text: &str) {
        println!("{text}");
    }
}

pub struct Config {
    /// The server binary: the pinned `apr`, or `d18-fake-apr-serve` in CI.
    pub apr: PathBuf,
    pub model: PathBuf,
    /// Offload every layer (`--gpu-layers all`); the fake ignores it.
    pub gpu: bool,
    /// Holds `sequential/out/…` and `pipelined/out/…`; outside any git tree.
    pub run_dir: PathBuf,
    pub apr_version: String,
}

pub struct Out {
    /// The six Rust asserts, by their `[assert]` names, over the record.
    pub checks: BTreeMap<String, Result<(), String>>,
    pub record: Value,
    pub start: Ident,
    pub end: Ident,
    pub load_ms: u128,
}

fn free_addr() -> Result<SocketAddr, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("free port: {e}"))?;
    l.local_addr().map_err(|e| format!("free port: {e}"))
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn show_schedule(run: &ScheduleRun, screen: &mut dyn Screen) {
    screen.cue("D18-B10");
    screen.line(&format!("  writer opened  {:?}", run.writer_files));
    screen.line(&format!("  checker opened {:?}", run.checker_files));
    screen.cue("D18-B11");
    for (id, d) in crate::item_ids().iter().zip(&run.decisions) {
        screen.line(&format!("  {id} {d:<6} (read from {CHECKER_FILE})"));
    }
}

fn show_pipelined(run: &ScheduleRun, screen: &mut dyn Screen) {
    screen.cue("D18-B13");
    for (i, w) in run.times.windows(2).enumerate() {
        screen.line(&format!(
            "  checker q{} [{}, {}) ms  while writer q{} [{}, {}) ms",
            i + 1,
            w[0].checker_start,
            w[0].checker_end,
            i + 2,
            w[1].writer_start,
            w[1].writer_end
        ));
    }
    screen.cue("D18-B14");
    for (id, t) in crate::item_ids().iter().zip(&run.times) {
        screen.line(&format!(
            "  {id} writer ends {} ms <= checker starts {} ms",
            t.writer_end, t.checker_start
        ));
    }
    screen.cue("D18-B15");
    screen.line("  speed: not claimed (the server queues requests; the point is roles)");
}

/// The six Rust asserts, each over what the record says, against the disk
/// and the clock.
fn check_record(
    cfg: &Config,
    rec: &Value,
    roots: [&Path; 2],
    runs: [&ScheduleRun; 2],
    ids: [&Ident; 2],
) -> BTreeMap<String, Result<(), String>> {
    let mut c = BTreeMap::new();
    let spawns = rec["server_spawns"].as_u64().unwrap_or(0);
    c.insert(
        "one_load".into(),
        checks::one_load(u32::try_from(spawns).unwrap_or(0), ids[0], ids[1], &cfg.apr),
    );
    let files = |k: &str| -> Vec<String> {
        rec[k]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut disjoint = checks::disjoint_writes(&files("writer_files"), &files("checker_files"));
    for r in runs {
        disjoint = disjoint.and(checks::disjoint_writes(&r.writer_files, &r.checker_files));
    }
    c.insert("disjoint_writes".into(), disjoint);
    let digest = |k: &str| rec[k].as_str().unwrap_or_default().to_string();
    c.insert(
        "digests_recomputed".into(),
        checks::digest_recomputed(roots[0], &digest("digest_sequential")).and(
            checks::digest_recomputed(roots[1], &digest("digest_pipelined")),
        ),
    );
    let decisions: [String; 4] =
        crate::item_ids().map(|id| rec[id]["decision"].as_str().unwrap_or_default().to_string());
    c.insert(
        "decisions_from_checker".into(),
        checks::decisions_from_checker(roots[0], &decisions)
            .and(checks::decisions_from_checker(roots[1], &decisions)),
    );
    let times = record::times_from(rec).ok_or_else(|| "record: a time is missing".to_string());
    c.insert(
        "one_clock".into(),
        times.clone().and_then(|(seq, pipe)| {
            checks::one_clock(&seq, runs[0].elapsed_ms)
                .and(checks::one_clock(&pipe, runs[1].elapsed_ms))
        }),
    );
    c.insert(
        "pipelined_overlaps".into(),
        times.and_then(|(seq, pipe)| checks::pipelined_overlaps(&seq, &pipe)),
    );
    c
}

/// Spawn the one server and run both schedules on it.
pub fn run(cfg: &Config, screen: &mut dyn Screen) -> Result<Out, String> {
    let addr = free_addr()?;
    let port = addr.port().to_string();
    let model = cfg.model.to_string_lossy().into_owned();
    let mut args = vec!["serve", "run", model.as_str(), "--port", port.as_str()];
    if cfg.gpu {
        args.extend(["--gpu-layers", "all"]);
    }
    screen.cue("D18-B06");
    screen.line(&format!(
        "[2] one server: apr serve run {} --port {port}{}",
        name_of(&cfg.model),
        if cfg.gpu { " --gpu-layers all" } else { "" }
    ));
    let apr = cfg.apr.to_string_lossy().into_owned();
    let guard =
        demo_kit::serve::ServeGuard::spawn(&apr, &args, addr, "/health", Duration::from_secs(300))?;
    let server_spawns = 1u32;
    let pid = guard.pid().ok_or("server: no pid")?;
    let start = ident::read(pid)?;
    screen.line(&format!("  healthy after {} ms", guard.load_ms));
    screen.cue("D18-B07");
    screen.line(&format!(
        "  server pid {pid}, start ticks {}, exe {}",
        start.start_ticks,
        name_of(&start.exe)
    ));

    let llm = Http { addr };
    let seq_root = cfg.run_dir.join(Kind::Sequential.name());
    let pipe_root = cfg.run_dir.join(Kind::Pipelined.name());
    screen.cue("D18-B09");
    screen.line("[3] sequential: the writer answers q1..q4, then the checker reads them");
    let seq = schedule::run(Kind::Sequential, &llm, &seq_root)?;
    show_schedule(&seq, screen);

    screen.cue("D18-B12");
    screen.line(&format!("[4] pipelined, on the same server (pid {pid})"));
    let pipe = schedule::run(Kind::Pipelined, &llm, &pipe_root)?;
    show_pipelined(&pipe, screen);

    screen.cue("D18-B16");
    screen.line(&format!(
        "[5] sha256({WRITER_FILE} || {CHECKER_FILE}), recomputed from disk"
    ));
    let digest_sequential = checks::digest_from_disk(&seq_root)?;
    let digest_pipelined = checks::digest_from_disk(&pipe_root)?;
    screen.cue("D18-B17");
    screen.line(&format!("  sequential {digest_sequential}"));
    screen.line(&format!("  pipelined  {digest_pipelined}"));
    screen.line(&format!(
        "  match: {}",
        if digest_sequential == digest_pipelined {
            "yes"
        } else {
            "NO"
        }
    ));

    let end = ident::read(pid)?;
    let load_ms = guard.load_ms;
    drop(guard);

    let rec = record::build(&record::Inputs {
        apr_version: &cfg.apr_version,
        model_sha256: crate::MODEL_SHA256,
        server_spawns,
        start: &start,
        end: &end,
        writer_files: &seq.writer_files,
        checker_files: &seq.checker_files,
        digest_sequential: &digest_sequential,
        digest_pipelined: &digest_pipelined,
        seq: &seq.times,
        pipe: &pipe.times,
        decisions: &seq.decisions,
    });
    let checks = check_record(
        cfg,
        &rec,
        [&seq_root, &pipe_root],
        [&seq, &pipe],
        [&start, &end],
    );
    Ok(Out {
        checks,
        record: rec,
        start,
        end,
        load_ms,
    })
}

/// The judge's negative control (spec §5.1 step 5): the committed planted
/// record must be Red, its findings byte-equal to the committed `.expect`
/// lines. Returns (holds, what was seen).
pub fn planted_control(demo_dir: &Path, run_root: &Path) -> (bool, String) {
    let fixtures = demo_dir.join("fixtures");
    let (Ok(planted), Ok(expect)) = (
        std::fs::read(fixtures.join("receipt.planted.json")),
        std::fs::read_to_string(fixtures.join("receipt.planted.expect")),
    ) else {
        return (false, "planted fixtures unreadable".into());
    };
    let o = demo_kit::shapes::judge(&demo_dir.join("spec"), &planted, run_root);
    if !matches!(o.verdict, demo_kit::Verdict::Red { .. }) {
        return (
            false,
            format!("planted record judged {}: {}", o.verdict, o.why),
        );
    }
    let mut got = o.findings.clone();
    got.sort();
    let want: Vec<&str> = expect.lines().collect();
    if got != want {
        return (
            false,
            format!(
                "planted findings differ from the .expect: got {} lines, want {}",
                got.len(),
                want.len()
            ),
        );
    }
    (
        true,
        format!("Red, {} findings, byte-equal to the .expect", got.len()),
    )
}
