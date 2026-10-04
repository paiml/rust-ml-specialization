//! D18: two agents, one server. A writer and a checker share one resident
//! `apr serve`; the run goes sequential, then pipelined, on the same process,
//! and the record of what was measured is judged by pv against the contract.
//!
//! Provable contract: d18-run-v1 — the run record conforms to spec/d18-run-v1.yaml under `pv lint --gate shapes`, and the planted record is refused with exactly receipt.planted.expect.
//!
//! Environment: `RFML5_RECEIPTS` (receipts and the run's output files, outside
//! this repo), `RFML5_MODELS` (the directory holding the pinned model), and
//! optionally `RFML5_BEATS` (beat times; unset means no pacing). Exit codes:
//! 0 Green, 1 Red, 2 NotRun.

use d18_two_agents_one_server::flow::{self, Config, Screen, Terminal};
use d18_two_agents_one_server::{MODEL_FILE, MODEL_SHA256};
use demo_kit::harness::Harness;
use demo_kit::pace::Pacer;
use demo_kit::{NotRunReason, Verdict};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const ID: &str = "d18-two-agents-one-server";

fn title(screen: &mut dyn Screen) {
    screen.cue("D18-B01");
    screen.line("D18  Two agents, one server");
    screen.line("     a writer and a checker share one resident model server");
    screen.line("     who decides, who writes which file, and does the schedule change the bytes?");
    screen.line("");
}

fn env_dir(h: &mut Harness, var: &str) -> Option<PathBuf> {
    match std::env::var(var) {
        Ok(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => {
            h.refuse(NotRunReason::EnvUnset(var.into()));
            None
        }
    }
}

/// The model, by content: its sha256 must be the pinned one.
fn check_model(h: &mut Harness, screen: &mut dyn Screen, models: &Path) -> PathBuf {
    let model = models.join(MODEL_FILE);
    let found = demo_kit::sha::sha256_file(&model).unwrap_or_else(|_| "missing".into());
    screen.line(&format!("  model {MODEL_FILE} sha256 {found}"));
    if found != MODEL_SHA256 {
        h.refuse(NotRunReason::WeightsMismatch {
            expected: MODEL_SHA256.into(),
            found,
        });
    }
    model
}

/// The pinned `apr` (preflight refuses one that is not the declared binary),
/// with its binary sha256 recorded in the Receipt.
fn check_apr(h: &mut Harness, screen: &mut dyn Screen) -> Option<PathBuf> {
    let Some(apr) = demo_kit::preflight::which_in(None, "apr") else {
        h.refuse(NotRunReason::MissingTool("apr".into()));
        return None;
    };
    let sha = demo_kit::sha::sha256_file(&apr).unwrap_or_else(|_| "unreadable".into());
    screen.line(&format!(
        "  apr {} (series 0.70.x) binary sha256 {sha}",
        h.receipt.apr_version
    ));
    h.receipt.tools.insert("apr_sha256".into(), sha);
    Some(apr)
}

/// Step 1. Returns the run's config, or `None` with the reasons on screen.
fn preflight(h: &mut Harness, screen: &mut dyn Screen) -> Option<Config> {
    screen.cue("D18-B04");
    screen.line("[1] preflight: the server and the model, pinned by content");
    let apr = check_apr(h, screen);
    let receipts = env_dir(h, "RFML5_RECEIPTS");
    let model = env_dir(h, "RFML5_MODELS").map(|m| check_model(h, screen, &m));
    screen.cue("D18-B05");
    let gpu_held = h
        .not_run()
        .iter()
        .any(|r| matches!(r, NotRunReason::GpuLockHeld));
    screen.line(if gpu_held {
        "  GPU: another job holds a reservation; this run does not share the card"
    } else {
        "  GPU: no reservation is held by another job"
    });
    for r in h.not_run() {
        screen.line(&format!("  NotRun: {r}"));
    }
    let (apr, receipts, model) = (apr?, receipts?, model?);
    if !h.not_run().is_empty() {
        return None;
    }
    let run_dir = receipts.join(ID).join(&h.receipt.run_id);
    if let Err(e) = std::fs::create_dir_all(&run_dir) {
        h.refuse(NotRunReason::EnvUnset(format!("RFML5_RECEIPTS ({e})")));
        return None;
    }
    Some(Config {
        apr,
        model,
        gpu: true,
        run_dir,
        apr_version: h.receipt.apr_version.clone(),
    })
}

/// Step 5: write the record, judge it and the planted control, finish.
fn judge(h: &mut Harness, cfg: &Config, out: &flow::Out, screen: &mut dyn Screen) -> Verdict {
    screen.cue("D18-B19");
    let bytes = serde_json::to_vec_pretty(&out.record).unwrap_or_default();
    let record_path = cfg.run_dir.join("run-record.json");
    let written = std::fs::write(&record_path, &bytes).is_ok();
    screen.line(&format!(
        "[6] run record written: {written}; pv lint --gate shapes"
    ));
    let shapes = demo_kit::shapes::judge(&h.dir.join("spec"), &bytes, &cfg.run_dir.join("judge"));
    screen.line(&format!(
        "  shapes: {} (focus nodes {:?})",
        shapes.verdict, shapes.focus_nodes_n
    ));
    let (planted_ok, planted_why) = flow::planted_control(&h.dir, &cfg.run_dir.join("planted"));
    screen.line(&format!("  planted: {planted_why}"));
    let mut measured: BTreeMap<String, Value> = BTreeMap::new();
    measured.insert("shapes".into(), json!(shapes.verdict.to_string()));
    let want = h.manifest.assert.get("planted").and_then(|v| v.as_str());
    let planted = match want {
        Some(s) if planted_ok => s.to_string(),
        _ => planted_why,
    };
    measured.insert("planted".into(), json!(planted));
    measured.insert("exit".into(), json!(0));
    h.set_shapes(shapes, &bytes);

    screen.cue("D18-B20");
    screen.line(&format!(
        "  server at the end: pid {}, start ticks {} (at spawn: pid {}, ticks {})",
        out.end.pid, out.end.start_ticks, out.start.pid, out.start.start_ticks
    ));
    for (name, r) in &out.checks {
        measured.insert(name.clone(), json!(r.is_ok()));
        match r {
            Ok(()) => screen.line(&format!("  {name}: ok")),
            Err(why) => screen.line(&format!("  {name}: FAIL {why}")),
        }
    }
    h.finish(measured)
}

fn run(h: &mut Harness, screen: &mut dyn Screen) -> Verdict {
    let Some(cfg) = preflight(h, screen) else {
        return h.finish(BTreeMap::new());
    };
    match flow::run(&cfg, screen) {
        Ok(out) => judge(h, &cfg, &out, screen),
        Err(why) => {
            screen.line(&format!("run failed: {why}"));
            let mut measured = BTreeMap::new();
            measured.insert("exit".into(), json!(1));
            h.finish(measured)
        }
    }
}

/// Pacing on: a one-line late-beat summary. Pacing off: each cue's time, so
/// the take's beat windows can be measured against the run.
fn report(screen: &Terminal, pacer: Option<&Pacer>) {
    match pacer {
        Some(p) => {
            let all = p.report();
            let late: Vec<String> = all
                .iter()
                .filter(|c| c.late())
                .map(|c| format!("{} +{:.1}s", c.tag, c.shown_s - c.target_s))
                .collect();
            eprintln!(
                "pace: {} cues, {} late{}{}",
                all.len(),
                late.len(),
                if late.is_empty() { "" } else { ": " },
                late.join(", ")
            );
        }
        None => {
            let t: Vec<String> = screen
                .cue_times()
                .iter()
                .map(|(tag, s)| format!("{tag}@{s:.2}"))
                .collect();
            eprintln!("cues (unpaced, s): {}", t.join(" "));
        }
    }
}

fn exit_code(v: &Verdict) -> u8 {
    match v {
        Verdict::Green => 0,
        Verdict::Red { .. } => 1,
        Verdict::NotRun { .. } => 2,
    }
}

fn main() -> Result<ExitCode, String> {
    let pacer = demo_kit::pace::Pacer::from_env()?;
    let mut screen = Terminal::new(pacer.as_ref());
    title(&mut screen);
    let mut h = Harness::load(env!("CARGO_MANIFEST_DIR"));
    let verdict = run(&mut h, &mut screen);
    let code = exit_code(&verdict);
    if code != 0 {
        report(&screen, pacer.as_ref());
        return Ok(ExitCode::from(code));
    }
    assert!(verdict.is_green(), "{ID}: exit 0 only on Green");
    screen.cue("D18-B21");
    println!("contract: d18-run-v1 OK");
    report(&screen, pacer.as_ref());
    Ok(ExitCode::SUCCESS)
}
