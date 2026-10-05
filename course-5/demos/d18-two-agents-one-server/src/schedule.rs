//! The two schedules, run against the same server process (spec §1, D18).
//!
//! - *sequential*: the writer finishes all four items, then the checker runs.
//! - *pipelined*: the writer and the checker run on their own threads; the
//!   checker works on item i while the writer produces item i+1.
//!
//! Every timestamp comes from one monotonic [`Clock`] per schedule, read by
//! the orchestrator around each role's step: a step starts before its request
//! and ends after its line reached the role's sink. The checker reads each
//! answer from the writer's file on disk, never from memory.

use crate::agents::{self, Llm};
use crate::checks::{self, Times};
use crate::sink::{Registry, Role, Sink};
use crate::{CHECKER_FILE, QUESTIONS, WRITER_FILE};
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Sequential,
    Pipelined,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Sequential => "sequential",
            Kind::Pipelined => "pipelined",
        }
    }
}

/// What one schedule measured, all of it read back from disk or the clock.
#[derive(Debug, Clone)]
pub struct ScheduleRun {
    pub kind: Kind,
    pub times: [Times; 4],
    pub elapsed_ms: u64,
    pub writer_files: Vec<String>,
    pub checker_files: Vec<String>,
    /// sha256 over the two output files, recomputed from disk after the run.
    pub digest: String,
    /// Parsed from the checker's file after the run.
    pub decisions: [String; 4],
}

/// The orchestrator's one monotonic clock for a schedule: ms since its start.
#[derive(Debug, Clone, Copy)]
pub struct Clock(Instant);

impl Clock {
    pub fn start() -> Self {
        Clock(Instant::now())
    }
    pub fn ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

type Span = (u64, u64);

fn io(e: std::io::Error) -> String {
    e.to_string()
}

fn write_item(llm: &dyn Llm, clock: &Clock, sink: &mut Sink, i: usize) -> Result<Span, String> {
    let (id, question) = QUESTIONS[i];
    let start = clock.ms();
    let answer = llm.chat(agents::WRITER_SYSTEM, question, agents::WRITER_MAX_TOKENS)?;
    sink.append_line(&agents::line(id, "answer", &answer))
        .map_err(io)?;
    Ok((start, clock.ms()))
}

fn check_item(
    llm: &dyn Llm,
    clock: &Clock,
    sink: &mut Sink,
    root: &Path,
    i: usize,
) -> Result<Span, String> {
    let (id, question) = QUESTIONS[i];
    let start = clock.ms();
    let answers = std::fs::read_to_string(root.join(WRITER_FILE)).map_err(io)?;
    let answer = agents::field_for(&answers, id, "answer")
        .ok_or_else(|| format!("checker: {WRITER_FILE} has no answer for {id}"))?;
    let reply = llm.chat(
        agents::CHECKER_SYSTEM,
        &agents::checker_user(question, &answer),
        agents::CHECKER_MAX_TOKENS,
    )?;
    let decision = agents::parse_decision(&reply)?;
    sink.append_line(&agents::line(id, "decision", decision))
        .map_err(io)?;
    Ok((start, clock.ms()))
}

fn sequential(llm: &dyn Llm, clock: &Clock, reg: &Registry) -> Result<[Times; 4], String> {
    let mut t = [Times::default(); 4];
    let mut w = reg.open(Role::Writer, WRITER_FILE).map_err(io)?;
    for (i, item) in t.iter_mut().enumerate() {
        (item.writer_start, item.writer_end) = write_item(llm, clock, &mut w, i)?;
    }
    drop(w);
    let mut c = reg.open(Role::Checker, CHECKER_FILE).map_err(io)?;
    for (i, item) in t.iter_mut().enumerate() {
        (item.checker_start, item.checker_end) = check_item(llm, clock, &mut c, reg.root(), i)?;
    }
    Ok(t)
}

fn writer_loop(
    llm: &dyn Llm,
    clock: &Clock,
    reg: &Registry,
    tx: mpsc::Sender<usize>,
) -> Result<[Span; 4], String> {
    let mut w = reg.open(Role::Writer, WRITER_FILE).map_err(io)?;
    let mut spans = [(0, 0); 4];
    for (i, span) in spans.iter_mut().enumerate() {
        *span = write_item(llm, clock, &mut w, i)?;
        tx.send(i)
            .map_err(|_| "pipelined: the checker stopped".to_string())?;
    }
    Ok(spans)
}

fn checker_loop(
    llm: &dyn Llm,
    clock: &Clock,
    reg: &Registry,
    rx: mpsc::Receiver<usize>,
) -> Result<[Span; 4], String> {
    let mut c = reg.open(Role::Checker, CHECKER_FILE).map_err(io)?;
    let mut spans = [(0, 0); 4];
    let mut n = 0;
    for i in rx {
        spans[i] = check_item(llm, clock, &mut c, reg.root(), i)?;
        n += 1;
    }
    if n != 4 {
        return Err(format!("pipelined: the checker saw {n} of 4 items"));
    }
    Ok(spans)
}

fn pipelined(llm: &dyn Llm, clock: &Clock, reg: &Registry) -> Result<[Times; 4], String> {
    let (tx, rx) = mpsc::channel();
    let (w, c) = std::thread::scope(|s| {
        let writer = s.spawn(|| writer_loop(llm, clock, reg, tx));
        let checker = s.spawn(|| checker_loop(llm, clock, reg, rx));
        (writer.join(), checker.join())
    });
    let w = w.map_err(|_| "pipelined: the writer panicked".to_string())??;
    let c = c.map_err(|_| "pipelined: the checker panicked".to_string())??;
    let mut t = [Times::default(); 4];
    for (i, item) in t.iter_mut().enumerate() {
        *item = Times {
            writer_start: w[i].0,
            writer_end: w[i].1,
            checker_start: c[i].0,
            checker_end: c[i].1,
        };
    }
    Ok(t)
}

/// Run one schedule with its outputs under `root` (`root/out/…`).
pub fn run(kind: Kind, llm: &dyn Llm, root: &Path) -> Result<ScheduleRun, String> {
    let reg = Registry::new(root);
    let clock = Clock::start();
    let times = match kind {
        Kind::Sequential => sequential(llm, &clock, &reg)?,
        Kind::Pipelined => pipelined(llm, &clock, &reg)?,
    };
    let elapsed_ms = clock.ms();
    Ok(ScheduleRun {
        kind,
        times,
        elapsed_ms,
        writer_files: reg.files(Role::Writer),
        checker_files: reg.files(Role::Checker),
        digest: checks::digest_from_disk(root)?,
        decisions: checks::decisions_from_disk(root)?,
    })
}
