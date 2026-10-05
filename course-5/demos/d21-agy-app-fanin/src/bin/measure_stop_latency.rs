//! E_6: measure the stop latency L that D21 asserts against.
//!
//! Launches the pinned app on its own virtual display with the demo profile,
//! and takes stop samples one agent at a time: start an agent on a long
//! step, wait until its sidebar row reads running, decide to stop it, read
//! the tree, click that agent's own `Stop execution` control (its node,
//! `DOM.getBoxModel`, `Input.dispatchMouseEvent` at the box centre), and
//! poll the tree until that row first reads stopped. One sample is decision
//! to that read. `RFML5_STOP_SAMPLES` sets the count (default 30); fewer
//! than 30 is an exploratory run that writes `samples.tsv` and exits 2.
//! Exit 0 Green, 1 Red, 2 NotRun.
//!
//! Provable contract: stop-latency-v1 — at least 30 stop samples are taken on the demo profile and L = max(p99, 2 x p95), rounded up, is written beside them in the run directory.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use agy_cdp::launch::{self, Xvfb};
use agy_cdp::open_under::Roots;
use agy_cdp::transport::Conn;

use d21_agy_app_fanin::drive::Drive;
use d21_agy_app_fanin::latency::{self, MIN_SAMPLES};
use d21_agy_app_fanin::session::{self, Env, APP_ASAR_SHA256, APP_VERSION};
use d21_agy_app_fanin::tree::RowState;

/// How long a stop may take before the sample is a failure.
const STOP_WITHIN: Duration = Duration::from_secs(30);
/// How long an agent is left running, granting its prompt, before the stop.
const SETTLE: Duration = Duration::from_secs(6);

enum Outcome {
    Green(u64),
    Red(String),
    NotRun(String),
}

fn wanted() -> Result<usize, String> {
    match std::env::var("RFML5_STOP_SAMPLES") {
        Ok(v) => v
            .parse()
            .map_err(|_| format!("RFML5_STOP_SAMPLES={v:?} is not a count")),
        Err(_) => Ok(MIN_SAMPLES),
    }
}

fn prompt(k: usize, roots: &Roots) -> Result<String, String> {
    let dir = roots.run_path(&format!("ws/sample-{k:02}"))?;
    Ok(format!(
        "stop-sample-{k:02}: in the folder {} run the shell command `sleep 600`, wait for it to finish, then create result.md there containing the line stop-sample-{k:02} PASS. Do not read, create or edit any other file.",
        dir.display()
    ))
}

/// Wait until slot 0 reads running, granting any prompt, and keep it running
/// for [`SETTLE`]; true once it has been running that long.
fn settle(d: &mut Drive) -> Result<bool, String> {
    let end = Instant::now() + Duration::from_secs(60);
    let mut since: Option<Instant> = None;
    while Instant::now() < end {
        let nodes = d.snapshot()?;
        if d.grant(&nodes)? {
            eprintln!("  t={} ms: granted a permission prompt", d.ms());
            continue;
        }
        match d.states(&nodes)[0] {
            Some(RowState::Running) => {
                let s = *since.get_or_insert_with(Instant::now);
                if s.elapsed() >= SETTLE {
                    return Ok(true);
                }
            }
            Some(RowState::Idle) if since.is_some() => return Ok(false),
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(false)
}

/// One sample in ms, or `None` when the agent did not hold a running state.
fn sample(d: &mut Drive, k: usize, roots: &Roots) -> Result<Option<f64>, String> {
    d.agents[0] = None;
    d.tasks[0] = None;
    let tag = format!("stop-sample-{k:02}");
    let text = prompt(k, roots)?;
    roots.mkdir_run(&format!("ws/sample-{k:02}"))?;
    d.new_agent(0, &tag, &text)?;
    if !settle(d)? {
        eprintln!("  sample {k}: the agent never held a running state; skipped");
        return Ok(None);
    }
    // Trees are kept from the decision to the stopped read, as D21 keeps
    // every tree; the dump write is part of each sample.
    d.dump = true;
    let decided = d.ms_f();
    let nodes = d.snapshot()?;
    let stop = d.click_stop(&nodes, 0)?;
    let end = Instant::now() + STOP_WITHIN;
    loop {
        let nodes = d.snapshot()?;
        if d.stopped(&nodes, 0) {
            let ms = d.ms_f() - decided;
            d.dump = false;
            eprintln!(
                "  sample {k}: control {} sent at {} ms, stopped after {ms:.1} ms",
                stop.control, stop.sent_ms
            );
            std::thread::sleep(Duration::from_secs(1));
            d.archive(0)?;
            std::thread::sleep(Duration::from_secs(1));
            return Ok(Some(ms));
        }
        if Instant::now() > end {
            return Err(format!("sample {k}: not stopped within {STOP_WITHIN:?}"));
        }
    }
}

fn measure(e: &Env, x: &Xvfb, n: usize) -> Result<Outcome, String> {
    let roots = Roots::open(&e.profile, &e.run)?;
    let mut app = launch::launch_app(&e.bin, &e.profile, x, roots.create_run_fd("app.log")?)?;
    let out = (|| -> Result<Vec<f64>, String> {
        let port = app.wait_port(&roots, Duration::from_secs(120))?;
        app.check_env(&e.profile)?;
        let mut conn = Conn::open(port)?;
        let page = session::attach_page(&mut conn)?;
        let mut d = Drive::new(conn, page, &roots, 1);
        d.dump = false;
        d.wait_for("button", "new conversation", 120)?;
        if d.dismiss_notice()? {
            eprintln!("notice: dismissed");
        }
        let mut samples = Vec::new();
        let mut k = 0;
        while samples.len() < n && k < n * 2 {
            k += 1;
            if let Some(s) = sample(&mut d, k, &roots)? {
                samples.push(s);
            }
        }
        let tally = d.conn.close_with_snapshot()?;
        if tally.devtools_created() + tally.devtools_in_snapshot() != 0 {
            return Err("a DevTools target was opened".into());
        }
        eprintln!("methods: {:?}", tally.methods);
        Ok(samples)
    })();
    let (left, listening) = app.teardown();
    eprintln!(
        "leak sweep: {} pids left, port listening: {listening}",
        left.len()
    );
    let samples = out?;
    if !left.is_empty() || listening {
        return Ok(Outcome::Red("leak sweep".into()));
    }
    let tsv: String = samples
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}\t{s:.1}\n", i + 1))
        .collect();
    roots.write_run("samples.tsv", tsv.as_bytes())?;
    if let Some((p95, p99, l)) = latency::bound(&samples) {
        eprintln!(
            "{} samples: p95 {p95:.1} ms, p99 {p99:.1} ms, L {l} ms",
            samples.len()
        );
    }
    if samples.len() < MIN_SAMPLES {
        return Ok(Outcome::NotRun(format!(
            "{} samples taken; L needs {MIN_SAMPLES}",
            samples.len()
        )));
    }
    let body = latency::fixture(&samples, APP_VERSION, APP_ASAR_SHA256)?.pretty();
    roots.write_run("stop-latency.json", body.as_bytes())?;
    let l = latency::l_from_fixture(&body)?.ok_or("the written fixture is the sentinel")?;
    Ok(Outcome::Green(l))
}

fn run(x: &Xvfb) -> Outcome {
    let n = match wanted() {
        Ok(n) => n,
        Err(m) => return Outcome::NotRun(m),
    };
    let e = match session::env(&["d21-agy-app-fanin", "measure-stop-latency"]) {
        Ok(e) => e,
        Err(m) => return Outcome::NotRun(m),
    };
    measure(&e, x, n).unwrap_or_else(Outcome::Red)
}

fn main() -> ExitCode {
    let x = match launch::display_override().and_then(Xvfb::start) {
        Ok(x) => x,
        Err(m) => {
            eprintln!("NotRun: {m}");
            return ExitCode::from(2);
        }
    };
    eprintln!("display: {}", x.display());
    match run(&x) {
        Outcome::Green(l) => {
            assert!(l > 0, "measure-stop-latency: L must be positive");
            println!("L = {l} ms");
            println!("contract: stop-latency-v1 OK");
            ExitCode::SUCCESS
        }
        Outcome::Red(m) => {
            eprintln!("Red: {m}");
            ExitCode::from(1)
        }
        Outcome::NotRun(m) => {
            eprintln!("NotRun: {m}");
            ExitCode::from(2)
        }
    }
}
