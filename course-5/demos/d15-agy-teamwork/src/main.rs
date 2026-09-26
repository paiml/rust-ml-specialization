//! D15 — agy teamwork (lesson 4.1, concept C5 "the traffic cop"): one minter,
//! one reducer, workers report to an inbox.
//!
//! Provable contract: teamwork-inbox-scoped-v1 — the orchestrator (this
//! process) is the sole minter of 3 disjoint tickets and the sole reducer of
//! their results; after dispatch the inbox holds exactly one file per minted
//! ticket, each at most 3 lines, and zero files that were not minted
//! (path scoping / no orphans).
//!
//! Mechanism, labelled honestly: agy 1.2.11 has no scriptable, one-shot
//! `/teamwork` verb. Typed literally in `-p` mode, `/teamwork` is not a
//! registered slash command (absent from `agy -p "/help"`'s list) so it is
//! passed through as plain text and the model *hallucinates* a fake "I spawned
//! a subagent" transcript without invoking any real tool — confirmed by
//! running it and finding no file, process or tool call behind the claim. Its
//! sibling `/teamwork-preview` *is* real (also undocumented, absent from
//! `/help`) but it is an interactive multi-turn project-scaffolding wizard
//! ("what do you want to build? what's the purpose? who's the audience?")
//! that writes a draft plan artifact and waits for answers — it cannot be
//! driven non-interactively to dispatch 3 fixed tickets deterministically.
//! So this demo dispatches with the documented fallback: 3 parallel
//! `agy -p` calls, one per ticket, each told to touch only its own inbox file.

use demo_kit::harness::Harness;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// The 3 disjoint tickets the orchestrator mints, in mint order. Each names
/// exactly one fixture file for its worker to summarise.
const TICKETS: [(&str, &str); 3] = [
    ("d15-t1", "fixture-alpha.txt"),
    ("d15-t2", "fixture-beta.txt"),
    ("d15-t3", "fixture-gamma.txt"),
];

const AGY_PRINT_TIMEOUT: &str = "180s";

/// Runtime `CARGO_MANIFEST_DIR` (set by `cargo run`) wins over the compile-time
/// one, matching the fix in `Harness::load` (a binary reused from a shared
/// target dir must read THIS checkout's fixtures, never the one it was
/// compiled against).
fn manifest_dir() -> PathBuf {
    std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

struct WorkerOutcome {
    ticket: String,
    exit_code: i32,
    elapsed_ms: u128,
    note: String,
}

/// `agy` on an `agy-host` is commonly wrapped to run under a *different* Unix
/// account than the one that invokes it (a privilege-separated credential
/// hand-off), so every file it writes is owned by that other account, not by
/// this process's own one. A directory this process creates with a plain
/// `mkdir` (owned by this process's account, mode 0755) gives the wrapped
/// account only `r-x` on it — enough to read fixtures, never enough to create
/// the inbox file, and the worker hangs forever with no error. Fix: share the
/// directory through a group both accounts already belong to on a correctly
/// provisioned `agy-host` (`setgid` + group `rwx`); fall back to
/// world-writable if that group is absent.
fn make_inbox_shareable(dir: &Path) -> String {
    let c_path = CString::new(dir.as_os_str().as_bytes()).expect("path has no NUL byte");
    let group_name = CString::new("cop-inbox").expect("static literal has no NUL byte");
    unsafe {
        let grp = libc::getgrnam(group_name.as_ptr());
        if !grp.is_null() {
            let gid = (*grp).gr_gid;
            // owner = -1 (as uid_t, i.e. u32::MAX): POSIX "leave unchanged".
            let chown_ok = libc::chown(c_path.as_ptr(), u32::MAX, gid) == 0;
            // setgid + rwxrwxr-x: new files inherit the group and both
            // accounts can create/delete inside it.
            let chmod_ok = libc::chmod(c_path.as_ptr(), 0o2775) == 0;
            if chown_ok && chmod_ok {
                return "shared via cop-inbox group, mode 2775".into();
            }
        }
        if libc::chmod(c_path.as_ptr(), 0o777) == 0 {
            "cop-inbox group unavailable; fell back to world-writable 0777".into()
        } else {
            "chmod FAILED: inbox directory may not be writable by the agy account".into()
        }
    }
}

fn dispatch_worker(ticket: &str, fixture_abs: &Path, inbox_abs: &Path) -> WorkerOutcome {
    let prompt = format!(
        "You are a scoped worker with exactly one job for ticket {ticket}. \
         Read the text file at this exact absolute path: {fixture}. \
         Then write a plain-text summary of at most 3 lines to this exact \
         absolute path, creating the file if it does not exist: {inbox}. \
         Do not create, edit, or delete any other file anywhere on this \
         machine. Do not ask any questions; just do the task and stop.",
        fixture = fixture_abs.display(),
        inbox = inbox_abs.display(),
    );
    let start = Instant::now();
    let out = Command::new("agy")
        .arg("-p")
        .arg(&prompt)
        .args([
            "--dangerously-skip-permissions",
            "--effort",
            "low",
            "--print-timeout",
            AGY_PRINT_TIMEOUT,
        ])
        .output();
    let elapsed_ms = start.elapsed().as_millis();
    match out {
        Ok(o) => WorkerOutcome {
            ticket: ticket.to_string(),
            exit_code: o.status.code().unwrap_or(-1),
            elapsed_ms,
            note: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        Err(e) => WorkerOutcome {
            ticket: ticket.to_string(),
            exit_code: -1,
            elapsed_ms,
            note: format!("failed to spawn agy: {e}"),
        },
    }
}

fn unique_tmp_root() -> PathBuf {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "d15-agy-teamwork-{}-{}-{}",
        std::process::id(),
        d.as_secs(),
        d.subsec_nanos()
    ))
}

fn run_teamwork() -> BTreeMap<String, Value> {
    let fixtures_dir = manifest_dir().join("fixtures");
    let tmp_root = unique_tmp_root();
    let inbox_dir = tmp_root.join("inbox");
    std::fs::create_dir_all(&inbox_dir).expect("create tmp inbox dir");
    let sharing = make_inbox_shareable(&inbox_dir);
    println!("inbox dir: {} ({sharing})", inbox_dir.display());

    println!(
        "mechanism: 3 parallel `agy -p` calls, one per minted ticket \
         (agy 1.2.11 has no scriptable /teamwork verb — see the docstring \
         above for what typing it literally actually does)"
    );

    for (ticket, fixture) in TICKETS {
        println!("minted ticket {ticket} -> fixtures/{fixture}");
    }
    let tickets_minted = TICKETS.len() as u64;

    let start = Instant::now();
    let handles: Vec<_> = TICKETS
        .iter()
        .map(|&(ticket, fixture)| {
            let fixture_abs = fixtures_dir.join(fixture);
            let inbox_abs = inbox_dir.join(format!("{ticket}.md"));
            let ticket = ticket.to_string();
            std::thread::spawn(move || dispatch_worker(&ticket, &fixture_abs, &inbox_abs))
        })
        .collect();
    let outcomes: Vec<WorkerOutcome> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let wall_ms = start.elapsed().as_millis();

    let mut workers_exit_ok = 0u64;
    for o in &outcomes {
        println!(
            "worker {}: exit={} elapsed_ms={} :: {}",
            o.ticket, o.exit_code, o.elapsed_ms, o.note
        );
        if o.exit_code == 0 {
            workers_exit_ok += 1;
        }
    }

    // The orchestrator is the only reducer: read the inbox once, path-scope
    // it, and reduce deterministically by sorting on the ticket id.
    let expected: BTreeSet<String> = TICKETS.iter().map(|(t, _)| format!("{t}.md")).collect();
    let mut present: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(&inbox_dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                present.insert(name, content);
            }
        }
    }
    let inbox_files = present.len() as u64;
    let orphans = present.keys().filter(|n| !expected.contains(*n)).count() as u64;
    let max_lines = present
        .iter()
        .filter(|(n, _)| expected.contains(*n))
        .map(|(_, c)| c.lines().count() as u64)
        .max()
        .unwrap_or(0);

    println!("reduced inbox (sorted by ticket, deterministic):");
    for (name, content) in &present {
        let tag = if expected.contains(name) {
            "ticket"
        } else {
            "ORPHAN"
        };
        println!("  [{tag}] {name}: {} line(s)", content.lines().count());
        for line in content.lines() {
            println!("    | {line}");
        }
    }

    if let Err(e) = std::fs::remove_dir_all(&tmp_root) {
        println!("cleanup: could not remove {} ({e})", tmp_root.display());
    }

    [
        ("tickets_minted", json!(tickets_minted)),
        ("inbox_files", json!(inbox_files)),
        ("max_lines", json!(max_lines)),
        ("orphans", json!(orphans)),
        ("workers_exit_ok", json!(workers_exit_ok)),
        ("wall_ms", json!(wall_ms as u64)),
        ("exit", json!(0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

fn main() {
    let mut h = Harness::load(env!("CARGO_MANIFEST_DIR"));
    println!(
        "D15 agy teamwork: orchestrator mints 3 disjoint tickets, dispatches \
         to agy workers, reduces the inbox"
    );

    let measured = if h.not_run().is_empty() {
        run_teamwork()
    } else {
        BTreeMap::new()
    };

    let tickets_minted = measured.get("tickets_minted").and_then(Value::as_u64);
    let inbox_files = measured.get("inbox_files").and_then(Value::as_u64);
    let max_lines = measured.get("max_lines").and_then(Value::as_u64);
    let orphans = measured.get("orphans").and_then(Value::as_u64);

    let verdict = h.finish(measured);
    assert!(verdict.is_green(), "D15 is {verdict}");
    assert_eq!(
        tickets_minted,
        Some(3),
        "orchestrator mints exactly 3 tickets"
    );
    assert_eq!(
        inbox_files,
        Some(3),
        "inbox has exactly one file per ticket"
    );
    assert!(
        max_lines.is_some_and(|n| n <= 3),
        "every worker result is at most 3 lines"
    );
    assert_eq!(
        orphans,
        Some(0),
        "no file may exist outside the minted tickets"
    );
    println!("contract: teamwork-inbox-scoped-v1 OK");
}
