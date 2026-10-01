//! D11 — refusals shown by name, not silently (MEGA-001 §0.4, §4.3 Plan B row
//! 3.2, §5 "a refused verb shown by name with `removed_by`").
//!
//! Provable contract: refusal-named-v1 — every apr verb/flag this course's
//! Plan B rows depend on is probed live against the pinned binary
//! (`std::process::Command`, no shell); a refused one is reported `REFUSED`
//! by name with whatever reason apr itself gives, never as `available`;
//! reason-not-invented-v1 — when apr's own output carries no removed_by/reason
//! (a bare clap "unexpected argument" is not a named refusal), the table says
//! `apr reported no removed_by` rather than inventing one.
//!
//! At the pin (`apr 0.69.3`) this demo found:
//! - `--json-schema` is absent from `apr run --help` and `apr serve run
//!   --help`; invoking it is a generic clap "unexpected argument" error, not
//!   a named capability refusal. No `removed_by` field exists anywhere apr
//!   emits JSON at this pin (`apr capability --json`, `apr devices --json`).
//! - `apr ptx` is refused as a whole verb: `--help` marks it "unavailable in
//!   this build" and invoking it exits 9 with a named reason.
//! - `apr devices --json` lists `metal` and `hip` backends `unavailable`
//!   with a reason (carried, oddly, in the `vendor` field); `wgpu` and `cuda`
//!   are `ready`.
//! - `apr finetune --help` / `apr distill --help` show no refusal marker for
//!   any architecture at this pin: the capability registry does not mark
//!   them refused, so this table does not claim they are.

use demo_kit::harness::Harness;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::process::Command;

const APR: &str = "apr";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Available,
    Refused,
}

impl Status {
    fn word(self) -> &'static str {
        match self {
            Status::Available => "available",
            Status::Refused => "REFUSED",
        }
    }
}

#[derive(Debug, Clone)]
struct CapRow {
    verb: String,
    probe_cmd: String,
    status: Status,
    /// Exactly what apr reported (a `removed_by`/reason field, or the text of
    /// a named refusal message). `None` means apr gave nothing of the kind —
    /// never filled in with an invented value.
    reason: Option<String>,
}

/// Run the pinned `apr` directly — no shell, so no quoting hazard and no
/// substitute binary can intervene (H-6, H-3).
fn run_apr(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(APR)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn `apr {}`: {e}", args.join(" ")));
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

/// A bare clap "unexpected argument" is the parser rejecting an unknown flag,
/// not apr naming a refused capability. Anything else apr prints on a
/// non-zero exit is treated as its own named reason.
fn named_reason(err: &str) -> Option<String> {
    let t = err.trim();
    if t.is_empty() || t.contains("unexpected argument") {
        None
    } else {
        Some(t.to_string())
    }
}

/// Probe a flag a Plan B row depends on. Availability is read from apr's own
/// `--help` (the documented surface), not from an exit code — a missing
/// model file would also exit non-zero and must not be confused with the
/// flag being refused.
fn probe_flag(sub: &[&str], flag: &str) -> CapRow {
    let mut help_args: Vec<&str> = sub.to_vec();
    help_args.push("--help");
    let (_hc, help_out, _he) = run_apr(&help_args);
    let in_help = help_out.contains(flag);

    let probe_path = format!("/nonexistent/d11-refusal-probe-{}.apr", std::process::id());
    let mut inv_args: Vec<&str> = sub.to_vec();
    inv_args.push(&probe_path);
    inv_args.push(flag);
    inv_args.push("x");
    let probe_cmd = format!("apr {} {probe_path} {flag} x", sub.join(" "));
    let (_code, _out, err) = run_apr(&inv_args);

    let status = if in_help {
        Status::Available
    } else {
        Status::Refused
    };
    let reason = if status == Status::Refused {
        named_reason(&err)
    } else {
        None
    };
    CapRow {
        verb: format!("{} {flag}", sub.join(" ")),
        probe_cmd,
        status,
        reason,
    }
}

/// Probe a whole verb that a build may not compile in at all (`apr ptx`).
///
/// Finding: `apr ptx --help` (the leaf help) does **not** carry the
/// "unavailable in this build" marker anywhere in its text — only the
/// top-level `apr --help` command listing does, on the `ptx` line. The leaf
/// help's own text says "this line is the only thing a user sees before
/// running the command, so it has to say so", but that line lives one level
/// up. So this probe reads the top-level listing, not the leaf help, and
/// corroborates with the actual invocation (which does refuse, at exit 9).
fn probe_ptx() -> CapRow {
    let (_hc, top_help, _he) = run_apr(&["--help"]);
    let refused_in_listing = top_help.contains("unavailable in this build");

    let probe_path = format!("/nonexistent/d11-refusal-probe-{}.ptx", std::process::id());
    let (code, _out, err) = run_apr(&["ptx", &probe_path]);
    let probe_cmd = format!("apr ptx {probe_path}");

    // Corroborate: the listing says refused, and the invocation actually
    // exits non-zero. Neither signal alone is trusted over the other.
    let status = if refused_in_listing && code != 0 {
        Status::Refused
    } else {
        Status::Available
    };
    let reason = if status == Status::Refused {
        named_reason(&err)
    } else {
        None
    };
    CapRow {
        verb: "ptx".to_string(),
        probe_cmd,
        status,
        reason,
    }
}

/// A verb that `--help` never marks refused at this pin (`finetune`,
/// `distill`). Reported `available` unless apr's own help text says
/// otherwise — never assumed refused because the course spec once guessed it
/// might be.
fn probe_verb_help(verb: &str) -> CapRow {
    let (_hc, help_out, _he) = run_apr(&[verb, "--help"]);
    let refused = help_out.contains("unavailable in this build");
    CapRow {
        verb: verb.to_string(),
        probe_cmd: format!("apr {verb} --help"),
        status: if refused {
            Status::Refused
        } else {
            Status::Available
        },
        reason: None,
    }
}

/// `apr devices --json`'s registry: a backend is `available` if any entry of
/// that kind reports `status.state == "ready"`; otherwise `REFUSED` with
/// whatever reason the first `unavailable` entry carries (the schema puts it
/// in `status.vendor`, not a field named `reason` or `removed_by` — apr's
/// choice, not this demo's).
fn probe_backend(kind: &str, entries: &[Value]) -> CapRow {
    let matching: Vec<&Value> = entries
        .iter()
        .filter(|e| e.get("kind").and_then(Value::as_str) == Some(kind))
        .collect();
    let ready = matching.iter().any(|e| {
        e.get("status")
            .and_then(|s| s.get("state"))
            .and_then(Value::as_str)
            == Some("ready")
    });
    let status = if ready {
        Status::Available
    } else {
        Status::Refused
    };
    let reason = if status == Status::Refused {
        matching
            .iter()
            .find_map(|e| {
                e.get("status")
                    .and_then(|s| s.get("vendor"))
                    .and_then(Value::as_str)
            })
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    } else {
        None
    };
    CapRow {
        verb: format!("devices backend={kind}"),
        probe_cmd: "apr devices --json".to_string(),
        status,
        reason,
    }
}

/// Recursively search a JSON value for a key by this exact name. Used to
/// check, not assume, whether apr's own JSON ever carries a `removed_by`
/// field at this pin.
fn json_has_key(v: &Value, key: &str) -> bool {
    match v {
        Value::Object(map) => {
            map.keys().any(|k| k == key) || map.values().any(|vv| json_has_key(vv, key))
        }
        Value::Array(a) => a.iter().any(|vv| json_has_key(vv, key)),
        _ => false,
    }
}

fn render_row(r: &CapRow) -> String {
    let reason = r.reason.clone().unwrap_or_else(|| match r.status {
        Status::Refused => "apr reported no removed_by".to_string(),
        Status::Available => "-".to_string(),
    });
    format!("{:<26} {:<10} {}", r.verb, r.status.word(), reason)
}

fn main() {
    let mut h = Harness::load(env!("CARGO_MANIFEST_DIR"));

    if !h.not_run().is_empty() {
        for r in h.not_run() {
            println!("preflight: {r}");
        }
        let verdict = h.finish(BTreeMap::new());
        panic!("D11 preflight failed, cannot probe apr: {verdict}");
    }

    // apr capability --json: does this pin's registry ever use `removed_by`?
    let (cap_code, cap_out, _cap_err) = run_apr(&["capability", "--json"]);
    assert_eq!(cap_code, 0, "apr capability --json exited {cap_code}");
    let cap_json: Value =
        serde_json::from_str(&cap_out).expect("apr capability --json is valid JSON");
    let capability_has_removed_by = json_has_key(&cap_json, "removed_by");

    // apr devices --json: the backend registry Plan B's finetune/distill row
    // would consult for wgpu/metal.
    let (dev_code, dev_out, _dev_err) = run_apr(&["devices", "--json"]);
    assert_eq!(dev_code, 0, "apr devices --json exited {dev_code}");
    let dev_json: Value = serde_json::from_str(&dev_out).expect("apr devices --json is valid JSON");
    let devices_has_removed_by = json_has_key(&dev_json, "removed_by");
    let entries = dev_json
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        !entries.is_empty(),
        "apr devices --json reported no entries: a registry check over nothing is not a check"
    );

    let rows: Vec<CapRow> = vec![
        probe_flag(&["run"], "--json-schema"),
        probe_flag(&["serve", "run"], "--json-schema"),
        probe_ptx(),
        probe_backend("cuda", &entries),
        probe_backend("cpu", &entries),
        probe_backend("wgpu", &entries),
        probe_backend("metal", &entries),
        probe_backend("hip", &entries),
        probe_verb_help("finetune"),
        probe_verb_help("distill"),
    ];

    println!("D11 refusals — probed against the pinned apr, live, no shell:");
    println!(
        "{:<26} {:<10} removed_by / reason (apr's own text)",
        "verb", "status"
    );
    for r in &rows {
        println!("  probe: {}", r.probe_cmd);
        println!("  {}", render_row(r));
    }

    let refused: Vec<&CapRow> = rows
        .iter()
        .filter(|r| r.status == Status::Refused)
        .collect();
    let available: Vec<&CapRow> = rows
        .iter()
        .filter(|r| r.status == Status::Available)
        .collect();
    let refusals_named = refused.iter().filter(|r| r.reason.is_some()).count();
    let refusals_unnamed = refused.len() - refusals_named;

    let json_schema_refused =
        rows[0].status == Status::Refused && rows[1].status == Status::Refused;
    let metal_refused = rows
        .iter()
        .any(|r| r.verb == "devices backend=metal" && r.status == Status::Refused);
    let wgpu_available = rows
        .iter()
        .any(|r| r.verb == "devices backend=wgpu" && r.status == Status::Available);
    let ptx_refused = rows
        .iter()
        .any(|r| r.verb == "ptx" && r.status == Status::Refused);

    // refusal-named-v1: the printed line for every REFUSED row says REFUSED,
    // never `available`, and vice versa — the print path cannot silently
    // launder a refusal into a pass.
    let never_shown_as_working = rows.iter().all(|r| {
        let line = render_row(r);
        match r.status {
            Status::Refused => line.contains("REFUSED") && !line.contains("available"),
            Status::Available => line.contains("available") && !line.contains("REFUSED"),
        }
    });

    println!(
        "summary: {} checked, {} REFUSED ({} named, {} unnamed), {} available",
        rows.len(),
        refused.len(),
        refusals_named,
        refusals_unnamed,
        available.len()
    );
    println!(
        "apr capability --json has a `removed_by` key: {capability_has_removed_by}; apr devices --json has one: {devices_has_removed_by}"
    );

    let measured: BTreeMap<String, Value> = [
        ("verbs_checked", json!(rows.len())),
        ("refused_count", json!(refused.len())),
        ("available_count", json!(available.len())),
        ("refusals_named", json!(refusals_named)),
        ("refusals_unnamed", json!(refusals_unnamed)),
        ("json_schema_refused", json!(json_schema_refused)),
        ("ptx_refused", json!(ptx_refused)),
        ("metal_refused", json!(metal_refused)),
        ("wgpu_available", json!(wgpu_available)),
        (
            "capability_json_has_removed_by",
            json!(capability_has_removed_by),
        ),
        ("devices_json_has_removed_by", json!(devices_has_removed_by)),
        ("never_shown_as_working", json!(never_shown_as_working)),
        ("exit", json!(0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    let verdict = h.finish(measured);

    assert!(verdict.is_green(), "D11 is {verdict}");
    assert!(
        json_schema_refused,
        "refusal-named-v1: --json-schema must be REFUSED at this pin"
    );
    assert!(
        refusals_named >= 1,
        "refusal-named-v1: at least one refusal must carry apr's own reason"
    );
    assert!(
        never_shown_as_working,
        "refusal-named-v1: a REFUSED verb must never print as available"
    );
    assert!(
        ptx_refused,
        "refusal-named-v1: apr ptx is unavailable in this build"
    );
    assert!(
        metal_refused,
        "refusal-named-v1: the metal backend is unavailable in this build"
    );
    assert!(
        wgpu_available,
        "reason-not-invented-v1: wgpu is ready here and must not be shown REFUSED"
    );
    println!("contract: refusal-named-v1 OK");
    println!("contract: reason-not-invented-v1 OK");
}
