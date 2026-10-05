//! A workflow judged by named claims (ported from D19).
//!
//! A workflow record is a list of steps. Five named claims must hold. The
//! judge returns the claims a record violates, so a mutant can be checked
//! for being refused for the RIGHT reason, not merely refused.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Load,
    Write,
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub owner: String,
    pub kind: Kind,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
}

pub const CLAIMS: [&str; 5] = [
    "every-step-has-owner",
    "writes-disjoint",
    "reads-after-writes",
    "single-server-load",
    "checker-last",
];

fn step(owner: &str, kind: Kind, reads: &[&str], writes: &[&str]) -> Step {
    Step {
        owner: owner.into(),
        kind,
        reads: reads.iter().map(|s| s.to_string()).collect(),
        writes: writes.iter().map(|s| s.to_string()).collect(),
    }
}

/// The golden record: load once, draft, check last.
pub fn golden() -> Vec<Step> {
    vec![
        step("server", Kind::Load, &[], &[]),
        step("writer", Kind::Write, &[], &["ws/writer/draft.md"]),
        step(
            "checker",
            Kind::Check,
            &["ws/writer/draft.md"],
            &["ws/checker/verdict.md"],
        ),
    ]
}

/// Returns every claim `record` violates, in `CLAIMS` order.
pub fn judge(record: &[Step]) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if record.iter().any(|s| s.owner.trim().is_empty()) {
        bad.push(CLAIMS[0]);
    }
    let mut seen: Vec<&str> = Vec::new();
    let mut dup = false;
    for w in record.iter().flat_map(|s| &s.writes) {
        dup |= seen.contains(&w.as_str());
        seen.push(w);
    }
    if dup {
        bad.push(CLAIMS[1]);
    }
    let mut written: Vec<&str> = Vec::new();
    let mut early = false;
    for s in record {
        early |= s.reads.iter().any(|r| !written.contains(&r.as_str()));
        written.extend(s.writes.iter().map(|w| w.as_str()));
    }
    if early {
        bad.push(CLAIMS[2]);
    }
    if record.iter().filter(|s| s.kind == Kind::Load).count() != 1 {
        bad.push(CLAIMS[3]);
    }
    if record.last().map(|s| s.kind) != Some(Kind::Check) {
        bad.push(CLAIMS[4]);
    }
    bad
}

/// A mutant that must be refused, and the one claim it must be refused for.
pub type KillRow = (&'static str, fn(&mut Vec<Step>), &'static str);

pub fn kill_rows() -> Vec<KillRow> {
    vec![
        ("blank owner", |r| r[1].owner.clear(), CLAIMS[0]),
        (
            "checker overwrites the draft",
            |r| r[2].writes = vec!["ws/writer/draft.md".into()],
            CLAIMS[1],
        ),
        (
            "checker reads a file nobody wrote",
            |r| r[2].reads = vec!["ws/writer/missing.md".into()],
            CLAIMS[2],
        ),
        (
            "second server load",
            |r| r.insert(1, step("server", Kind::Load, &[], &[])),
            CLAIMS[3],
        ),
        (
            "writer runs last",
            |r| r.push(step("writer", Kind::Write, &[], &["ws/writer/notes.md"])),
            CLAIMS[4],
        ),
    ]
}

/// A mutation that changes the record but breaks no claim: it must pass.
pub type SurviveRow = (&'static str, fn(&mut Vec<Step>));

pub fn survive_rows() -> Vec<SurviveRow> {
    vec![
        ("rename an owner", |r| r[1].owner = "drafter".into()),
        ("writer also keeps notes", |r| {
            r[1].writes.push("ws/writer/notes.md".into())
        }),
        ("rename the server owner", |r| {
            r[0].owner = "model-host".into()
        }),
    ]
}
