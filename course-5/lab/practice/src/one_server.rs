//! Two agents, one model server (ported from D18).
//!
//! A writer agent drafts an answer and a checker agent judges it. Both share
//! ONE mock model server. Each agent may create files only in its own sink,
//! and a file can be created once, like `File::create_new`.

use crate::fnv1a;
use std::collections::BTreeMap;

/// Stands in for a real model server: loads once, answers deterministically.
#[derive(Debug, Default)]
pub struct MockServer {
    pub loads: u32,
}

impl MockServer {
    /// Loads the model if it is not loaded yet. Calling it twice is cheap.
    pub fn ensure_loaded(&mut self) {
        if self.loads == 0 {
            self.loads += 1;
        }
    }

    pub fn chat(&self, prompt: &str) -> String {
        assert!(self.loads > 0, "chat before the model was loaded");
        format!("answer:{:016x}", fnv1a(prompt.as_bytes()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Agent {
    Writer,
    Checker,
}

impl Agent {
    pub fn sink(self) -> &'static str {
        match self {
            Agent::Writer => "ws/writer/",
            Agent::Checker => "ws/checker/",
        }
    }
}

/// The shared workspace. Every file records who created it and when.
#[derive(Debug, Default)]
pub struct Sinks {
    pub files: BTreeMap<String, (Agent, u32, String)>,
}

impl Sinks {
    /// Creates `path` for `owner` at `tick`. Refuses a path outside the
    /// owner's sink, and refuses a path that already exists.
    pub fn create_new(
        &mut self,
        owner: Agent,
        tick: u32,
        path: &str,
        body: &str,
    ) -> Result<(), String> {
        if !path.starts_with(owner.sink()) {
            return Err(format!("{owner:?} may not write {path}"));
        }
        if self.files.contains_key(path) {
            return Err(format!("{path} already exists"));
        }
        self.files
            .insert(path.to_string(), (owner, tick, body.to_string()));
        Ok(())
    }

    /// The bytes of every file, in path order, without ticks: what the run
    /// produced, independent of when.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (path, (owner, _, body)) in &self.files {
            out.extend_from_slice(format!("{path}\t{owner:?}\t{body}\n").as_bytes());
        }
        out
    }
}

pub const DRAFT: &str = "ws/writer/draft.md";
pub const VERDICT: &str = "ws/checker/verdict.md";

/// What one run produced.
#[derive(Debug)]
pub struct Run {
    pub server: MockServer,
    pub sinks: Sinks,
    /// Tick at which the checker read the draft.
    pub checker_read_at: Option<u32>,
}

/// The checker's rule: a draft passes when it is a well-formed answer to
/// the prompt the writer was given.
pub fn check(prompt: &str, draft: &str, server: &MockServer) -> &'static str {
    if draft == server.chat(prompt) {
        "PASS"
    } else {
        "FAIL"
    }
}

/// Runs both agents, stepping them in the order `schedule` names (it is
/// repeated until both finish). A checker stepped before the draft exists
/// waits; it never reads a file that is not there.
pub fn run(prompt: &str, schedule: &[Agent], tamper: bool) -> Run {
    let mut server = MockServer::default();
    let mut sinks = Sinks::default();
    let (mut writer_done, mut checker_done) = (false, false);
    let mut checker_read_at = None;
    let mut tick = 0u32;
    while !(writer_done && checker_done) {
        for &agent in schedule {
            tick += 1;
            assert!(tick < 1_000, "schedule {schedule:?} never finishes");
            match agent {
                Agent::Writer if !writer_done => {
                    server.ensure_loaded();
                    let mut draft = server.chat(prompt);
                    if tamper {
                        draft.push('!');
                    }
                    sinks
                        .create_new(Agent::Writer, tick, DRAFT, &draft)
                        .expect("writer's own sink");
                    writer_done = true;
                }
                Agent::Checker if !checker_done => {
                    server.ensure_loaded();
                    let Some((_, _, draft)) = sinks.files.get(DRAFT).cloned() else {
                        continue; // not written yet: wait
                    };
                    checker_read_at = Some(tick);
                    let verdict = check(prompt, &draft, &server);
                    sinks
                        .create_new(Agent::Checker, tick, VERDICT, verdict)
                        .expect("checker's own sink");
                    checker_done = true;
                }
                _ => {}
            }
        }
    }
    Run {
        server,
        sinks,
        checker_read_at,
    }
}
