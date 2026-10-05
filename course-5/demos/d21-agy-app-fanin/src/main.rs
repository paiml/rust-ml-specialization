//! D21: fan-in and stop-the-line.
//!
//! The D20 driver, on the same isolated profile and display: three agents in
//! the app's agent view. Agents 1 and 2 hold a long step; agent 3's fixture
//! task is planted to fail its acceptance check. The first tree read that
//! shows agent 3 finished with a red result is `red_seen`; the agents that
//! read running in that same tree are `running_at_red`, and the driver then
//! clicks each one's own `Stop execution` control and reads it back stopped.
//! No stop is sent before red. The reducer refuses the merge, naming the
//! lowest red agent, identically in all six arrival orders. Exit 0 Green,
//! 1 Red, 2 NotRun.
//!
//! The red-to-stop path runs without pacing (it is bounded by L); the beats
//! that narrate it are shown, on cue, after it has finished.
//!
//! Provable contract: d21-run-v1 — the run record conforms to spec/d21-run-v1.yaml, and the line stops within the committed stop latency L.

use std::collections::BTreeSet;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use agy_cdp::ax;
use agy_cdp::launch::{self, App, Xvfb};
use agy_cdp::open_under::Roots;
use agy_cdp::operator_listing;
use agy_cdp::transport::Conn;
use demo_kit::pace::{self, Pacer};

use d21_agy_app_fanin::drive::{Drive, AGENTS};
use d21_agy_app_fanin::latency;
use d21_agy_app_fanin::record::{self, Facts, StopFact, TASKS};
use d21_agy_app_fanin::reduce::{self, AgentResult};
use d21_agy_app_fanin::session::{self, Env, APP_ASAR_SHA256, APP_VERSION};
use d21_agy_app_fanin::tree::{self, RNode, RowState};

type Manifest = Vec<(String, u64, String)>;
/// `(slot, sent_ms, stopped_ms, control)`; control is -1 when it was not
/// that row's own button.
type StopRow = (usize, u64, u64, i64);

enum Outcome {
    Green,
    Red(String),
    NotRun(String),
}

/// One terminal line per beat, released on that beat's cue.
struct Beats {
    pacer: Option<Pacer>,
    start: Instant,
    log: Vec<(String, f64)>,
}

impl Beats {
    fn show(&mut self, tag: &str, line: &str) -> Result<(), String> {
        pace::cue(self.pacer.as_ref(), tag)?;
        self.log
            .push((tag.to_string(), self.start.elapsed().as_secs_f64()));
        println!("[{tag}] {line}");
        Ok(())
    }

    fn summary(&self) {
        let mut prev = 0.0;
        for (tag, t) in &self.log {
            eprintln!(
                "step {tag}: at {t:.2} s, {:.2} s since the previous beat",
                t - prev
            );
            prev = *t;
        }
        let Some(p) = &self.pacer else { return };
        for c in p.report() {
            let late = if c.late() { "LATE" } else { "on time" };
            eprintln!(
                "cue {}: target {:.2} s, shown {:.2} s, {late}",
                c.tag, c.target_s, c.shown_s
            );
        }
    }
}

/// What the stop path measured, shown on the beats after it.
#[derive(Default)]
struct Line {
    red_seen_ms: u64,
    running_at_red: Vec<usize>,
    stops: Vec<StopRow>,
    at_stop: Manifest,
}

fn prompt(slot: usize, roots: &Roots) -> Result<String, String> {
    let task = TASKS[slot];
    let dir = roots.run_path(&format!("ws/agent-{}", slot + 1))?;
    Ok(if slot < 2 {
        format!(
            "{task}: in the folder {} run the shell command `sleep 600`, wait for it to finish, then create result.md there whose only content is the line {task} PASS. Do not read, create or edit any other file.",
            dir.display()
        )
    } else {
        format!(
            "{task}: create one file named result.md in the folder {} whose only content is the line {task} FAIL. Do not read, create or edit any other file and do not run any command.",
            dir.display()
        )
    })
}

fn shot(d: &mut Drive, roots: &Roots, f: &mut Facts) -> Result<(), String> {
    let png = ax::screenshot(&mut d.conn, &d.page)?;
    roots.write_run(&format!("shot-{}.png", f.shots.len() + 1), &png)?;
    f.shots.push(png);
    Ok(())
}

fn manifest(roots: &Roots) -> Result<Manifest, String> {
    roots.manifest_run("ws", &|b: &[u8]| demo_kit::sha::sha256_bytes(b))
}

/// Entries added, removed or changed between two workspace manifests.
fn changed(before: &Manifest, after: &Manifest) -> u64 {
    let a: BTreeSet<_> = before.iter().collect();
    let b: BTreeSet<_> = after.iter().collect();
    a.symmetric_difference(&b).count() as u64
}

fn result_body(roots: &Roots, slot: usize) -> Option<Vec<u8>> {
    roots
        .read_run(&format!("ws/agent-{}/result.md", slot + 1))
        .ok()
        .filter(|b| !b.is_empty())
}

/// Answer the open view's prompt, or visit the next agent that still needs
/// its task read back or is running.
fn tend(d: &mut Drive, nodes: &[RNode]) -> Result<(), String> {
    if d.grant(nodes)? {
        return Ok(());
    }
    let states = d.states(nodes);
    let wanted: Vec<bool> = (0..AGENTS)
        .map(|i| d.tasks[i].is_none() || states[i] == Some(RowState::Running))
        .collect();
    d.rotate(&wanted)
}

/// Watch until the first tree that shows agent 3 finished, with a red
/// result on disk; that tree is returned.
fn wait_red(d: &mut Drive, roots: &Roots, secs: u64) -> Result<Vec<RNode>, String> {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        let nodes = d.snapshot()?;
        d.read_back(&nodes, &TASKS);
        let done3 = d.seen_running[2] && d.states(&nodes)[2] == Some(RowState::Idle);
        if done3 && record::acceptance_red(TASKS[2], result_body(roots, 2).as_deref()) {
            return Ok(nodes);
        }
        tend(d, &nodes)?;
        std::thread::sleep(Duration::from_millis(800));
    }
    Err(format!("agent 3 was not red within {secs} s"))
}

/// Poll the tree until every slot in `sent` reads stopped: the first read
/// that shows each one stopped, in ms.
fn read_stopped(d: &mut Drive, sent: &[usize]) -> Result<Vec<Option<u64>>, String> {
    let until = Instant::now() + Duration::from_secs(30);
    let mut stopped: Vec<Option<u64>> = vec![None; AGENTS];
    while sent.iter().any(|i| stopped[*i].is_none()) {
        if Instant::now() > until {
            return Err("a stopped agent did not read stopped within 30 s".into());
        }
        let now = d.snapshot()?;
        let t = d.ms();
        for &i in sent {
            if stopped[i].is_none() && d.stopped(&now, i) {
                stopped[i] = Some(t);
            }
        }
    }
    Ok(stopped)
}

/// Watch until agent 3 is red, then stop the line at once: every agent the
/// red tree shows running, each by its own control, read back stopped.
fn watch_and_stop(d: &mut Drive, roots: &Roots, secs: u64) -> Result<Line, String> {
    let nodes = wait_red(d, roots, secs)?;
    let mut line = Line {
        red_seen_ms: d.ms(),
        ..Line::default()
    };
    line.running_at_red = d
        .states(&nodes)
        .iter()
        .enumerate()
        .filter(|(_, s)| **s == Some(RowState::Running))
        .map(|(i, _)| i)
        .collect();
    let mut sent = Vec::new();
    for &i in &line.running_at_red {
        sent.push((i, d.click_stop(&nodes, i)?));
    }
    let stopped = read_stopped(d, &line.running_at_red)?;
    line.at_stop = manifest(roots)?;
    for (i, s) in sent {
        let own = d.agents[i].and_then(|l| tree::row_button(&nodes, l, tree::RUNNING_BUTTON));
        let control = if own == Some(s.control) {
            s.control
        } else {
            -1
        };
        line.stops
            .push((i, s.sent_ms, stopped[i].unwrap_or(0), control));
    }
    Ok(line)
}

fn fan_out(d: &mut Drive, roots: &Roots, b: &mut Beats, f: &mut Facts) -> Result<Manifest, String> {
    b.show("D21-B05", "screenshot 1: the hub; start three agents")?;
    shot(d, roots, f)?;
    let prompts: Vec<String> = (0..AGENTS)
        .map(|i| prompt(i, roots))
        .collect::<Result<_, _>>()?;
    f.prompts = prompts.clone();
    d.start_clock();
    d.new_agent(0, TASKS[0], &prompts[0])?;
    d.new_agent(1, TASKS[1], &prompts[1])?;
    b.show(
        "D21-B06",
        "agents 1 and 2: fixture-alpha, fixture-beta, each a long step in its own workspace",
    )?;
    b.show(
        "D21-B07",
        "agent 3: fixture-gamma, planted to fail its acceptance check",
    )?;
    d.new_agent(2, TASKS[2], &prompts[2])?;
    f.agents_started = d.agents.iter().filter(|a| a.is_some()).count() as u64;
    let line = watch_and_stop(d, roots, 300)?;
    f.red_agent = Some(3);
    f.red_seen_ms = line.red_seen_ms;
    f.running_at_red = line
        .running_at_red
        .iter()
        .map(|i| format!("agent-{}", i + 1))
        .collect();
    for &(i, sent, stopped, control) in &line.stops {
        f.stopped_agents.push(format!("agent-{}", i + 1));
        if i < 2 {
            f.stops[i] = Some(StopFact {
                sent_ms: sent,
                stopped_ms: stopped,
                own_control: control >= 0,
            });
        }
    }
    d.open(0)?;
    std::thread::sleep(Duration::from_secs(1));
    shot(d, roots, f)?;
    show_line(b, f, &line)?;
    stop_check(d, roots, b, f, &line)?;
    Ok(line.at_stop)
}

fn show_line(b: &mut Beats, f: &Facts, line: &Line) -> Result<(), String> {
    b.show(
        "D21-B08",
        &format!("red_seen_ms = {}: agent-3 marked red", line.red_seen_ms),
    )?;
    b.show(
        "D21-B09",
        "pure Rust check on ws/agent-3/result.md: not `fixture-gamma PASS`, so red",
    )?;
    b.show(
        "D21-B10",
        &format!("running_at_red = {:?}", f.running_at_red),
    )?;
    b.show(
        "D21-B11",
        "read from the same accessibility tree that showed agent-3 finished",
    )?;
    let join = |each: &dyn Fn(&StopRow) -> String| {
        line.stops.iter().map(each).collect::<Vec<_>>().join("; ")
    };
    b.show(
        "D21-B12",
        &format!(
            "click on each row's own Stop execution control: {}",
            join(&|(i, sent, _, c)| format!("stop_agent_{}_sent_ms = {sent} (node {c})", i + 1))
        ),
    )?;
    b.show(
        "D21-B13",
        &format!(
            "stopped, read back from the tree: {}",
            join(&|(i, _, stopped, _)| format!("stop_agent_{}_stopped_ms = {stopped}", i + 1))
        ),
    )?;
    b.show(
        "D21-B14",
        &join(&|(i, sent, stopped, _)| {
            format!(
                "agent-{}: red {} <= sent {sent} <= stopped {stopped}",
                i + 1,
                line.red_seen_ms
            )
        }),
    )?;
    let worst = line
        .stops
        .iter()
        .map(|s| s.2.saturating_sub(line.red_seen_ms))
        .max()
        .unwrap_or(0);
    b.show(
        "D21-B15",
        &format!(
            "slowest stop {worst} ms after red; L = {} ms",
            f.l_ms.map_or("unmeasured".into(), |l| l.to_string())
        ),
    )?;
    b.show(
        "D21-B16",
        &format!(
            "workspace snapshot at the stopped moment: {} files",
            line.at_stop.len()
        ),
    )
}

fn stop_check(
    d: &mut Drive,
    roots: &Roots,
    b: &mut Beats,
    f: &mut Facts,
    line: &Line,
) -> Result<(), String> {
    let results: Vec<AgentResult> = (0..AGENTS)
        .map(|i| AgentResult {
            agent: i as u8 + 1,
            red: record::acceptance_red(TASKS[i], result_body(roots, i).as_deref()),
            files: line
                .at_stop
                .iter()
                .filter(|(p, _, _)| p.starts_with(&format!("agent-{}/", i + 1)))
                .map(|(p, s, h)| (format!("ws/{p}"), *s, h.clone()))
                .collect(),
        })
        .collect();
    b.show("D21-B17", "the reducer over the three results")?;
    f.orders = reduce::reduce_orders(&results, reduce::reduce);
    let first = &f.orders[0].1;
    b.show(
        "D21-B18",
        &format!(
            "merged = {}, refusal_names = {:?}",
            first.merged, first.refusal_names
        ),
    )?;
    let digests: BTreeSet<&str> = f.orders.iter().map(|o| o.1.digest.as_str()).collect();
    let labels: Vec<&str> = f.orders.iter().map(|o| o.0.as_str()).collect();
    b.show("D21-B19", &format!("all six orders: {}", labels.join(" ")))?;
    b.show(
        "D21-B20",
        &format!(
            "{} distinct digest: {}",
            digests.len(),
            digests.iter().next().copied().unwrap_or("-")
        ),
    )?;
    d.open(2)?;
    std::thread::sleep(Duration::from_secs(1));
    shot(d, roots, f)
}

fn session(
    e: &Env,
    roots: &Roots,
    app: &mut App,
    b: &mut Beats,
    f: &mut Facts,
) -> Result<(Conn, Manifest), String> {
    let port = app.wait_port(roots, Duration::from_secs(120))?;
    app.check_env(&e.profile)?;
    let mut conn = Conn::open(port)?;
    let page = session::attach_page(&mut conn)?;
    let mut d = Drive::new(conn, page, roots, AGENTS);
    b.show(
        "D21-B04",
        "the app's own environment checked: every path inside the demo profile",
    )?;
    d.wait_for("button", "new conversation", 120)?;
    if d.dismiss_notice()? {
        eprintln!("notice: dismissed");
    }
    for k in 1..=AGENTS {
        roots.mkdir_run(&format!("ws/agent-{k}"))?;
    }
    let out = fan_out(&mut d, roots, b, f);
    f.tasks = [0, 1, 2].map(|i| d.tasks[i].clone());
    let at_stop = out?;
    Ok((d.conn, at_stop))
}

fn finish(conn: Conn, f: &mut Facts) -> Result<(), String> {
    let tally = conn.close_with_snapshot()?;
    f.methods = tally.methods.clone();
    f.first_frame = tally.frames.first().cloned();
    f.keys = tally.keys.clone();
    f.inserted = tally.inserted.clone();
    f.discover_frames = tally.discover_params.len();
    f.discover_true = tally
        .discover_params
        .iter()
        .filter(|p| p["discover"] == true && p.as_object().is_some_and(|o| o.len() == 1))
        .count();
    f.devtools_created = tally.devtools_created();
    f.devtools_in_snapshot = tally.devtools_in_snapshot();
    Ok(())
}

fn live(e: &Env, x: &Xvfb, b: &mut Beats, f: &mut Facts) -> Result<Outcome, String> {
    let roots = Roots::open(&e.profile, &e.run)?;
    b.show(
        "D21-B03",
        &format!(
            "launch Antigravity {APP_VERSION} on {} with the demo profile",
            x.display()
        ),
    )?;
    let mut app = launch::launch_app(&e.bin, &e.profile, x, roots.create_run_fd("app.log")?)?;
    let out =
        session(e, &roots, &mut app, b, f).and_then(|(c, at_stop)| finish(c, f).map(|()| at_stop));
    b.show(
        "D21-B21",
        "teardown: only the pids this run started; workspace snapshot again",
    )?;
    let (left, listening) = app.teardown();
    println!(
        "  leak sweep: {} pids left, port listening: {listening}",
        left.len()
    );
    let at_stop = match out {
        Ok(m) => m,
        Err(m) => return Ok(Outcome::Red(m)),
    };
    if !left.is_empty() || listening {
        return Ok(Outcome::Red("leak sweep".into()));
    }
    let after = manifest(&roots)?;
    f.writes_after_stop = changed(&at_stop, &after);
    f.stray_writes = after
        .iter()
        .filter(|(p, _, _)| !(1..=AGENTS).any(|n| p.starts_with(&format!("agent-{n}/"))))
        .count() as u64;
    b.show(
        "D21-B22",
        &format!("writes_after_stop = {}", f.writes_after_stop),
    )?;
    Ok(Outcome::Green)
}

fn judge(e: &Env, b: &mut Beats, f: &Facts) -> Result<Outcome, String> {
    let rec = record::build(f).pretty();
    let roots = Roots::open(&e.profile, &e.run)?;
    roots.write_run("receipt.json", rec.as_bytes())?;
    b.show(
        "D21-B23",
        &format!(
            "record written; {} CDP methods sent, every one in ALLOW; keys {:?}; DevTools opened {}",
            f.methods.len(),
            f.keys,
            f.devtools_created
        ),
    )?;
    let defects = record::defects(f);
    for d in &defects {
        println!("  defect: {d}");
    }
    b.show("D21-B24", &format!("judged: {} defects", defects.len()))?;
    if defects.is_empty() {
        Ok(Outcome::Green)
    } else {
        Ok(Outcome::Red(format!("{} defects", defects.len())))
    }
}

fn run(b: &mut Beats, x: &Xvfb) -> Outcome {
    let l_ms = match latency::committed_l() {
        Ok(Some(l)) => l,
        Ok(None) => return Outcome::NotRun("FixtureUnmeasured: stop-latency.json".into()),
        Err(m) => return Outcome::NotRun(m),
    };
    let e = match session::env(&["d21-agy-app-fanin"]) {
        Ok(e) => e,
        Err(m) => return Outcome::NotRun(m),
    };
    let before = match operator_listing::list() {
        Ok(l) => l,
        Err(m) => return Outcome::NotRun(m),
    };
    let mut f = Facts {
        app_version: APP_VERSION.into(),
        asar_sha256: APP_ASAR_SHA256.into(),
        l_ms: Some(l_ms),
        ..Facts::default()
    };
    let shown = b
        .show("D21-B01", "three agents at once; one of them will fail")
        .and_then(|()| b.show("D21-B02", "watch what the driver does when it does"));
    if let Err(m) = shown {
        return Outcome::NotRun(m);
    }
    let out = live(&e, x, b, &mut f);
    let (profile, ambient) = match (operator_listing::list(), operator_listing::profile_roots()) {
        (Ok(after), Ok(roots)) => {
            operator_listing::split(operator_listing::diff(&before, &after), &roots)
        }
        (Err(m), _) | (_, Err(m)) => return Outcome::Red(format!("operator listing after: {m}")),
    };
    // As in D20: a change under the operator's profile is the defect; ambient
    // churn in the shallow XDG entries is shown but not judged.
    f.operator_touched = !profile.is_empty();
    for p in &profile {
        eprintln!("operator profile changed: {}", p.display());
    }
    println!(
        "  operator listing: {} entries before; profile {} changed; ambient host state {} changed (not judged)",
        before.len(),
        profile.len(),
        ambient.len()
    );
    match out {
        Ok(Outcome::Green) => judge(&e, b, &f).unwrap_or_else(Outcome::Red),
        Ok(o) => o,
        Err(m) => Outcome::Red(m),
    }
}

fn main() -> ExitCode {
    let x = match launch::display_override().and_then(Xvfb::start) {
        Ok(x) => x,
        Err(m) => {
            eprintln!("NotRun: {m}");
            return ExitCode::from(2);
        }
    };
    let pacer = match Pacer::from_env() {
        Ok(p) => p,
        Err(m) => {
            eprintln!("NotRun: {m}");
            return ExitCode::from(2);
        }
    };
    let t0 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    eprintln!("display: {}", x.display());
    eprintln!("t0_unix_ms: {t0}");
    let mut b = Beats {
        pacer,
        start: Instant::now(),
        log: Vec::new(),
    };
    let out = run(&mut b, &x);
    let code = match out {
        Outcome::Green => {
            let _ = b.show(
                "D21-B25",
                "receipt: the line stopped on red, and the merge was refused",
            );
            let _ = b.show("D21-B26", "contract: d21-run-v1");
            b.summary();
            assert!(
                b.log.iter().any(|(t, _)| t == "D21-B24"),
                "d21: Green only after the judge"
            );
            println!("contract: d21-run-v1 OK");
            return ExitCode::SUCCESS;
        }
        Outcome::Red(m) => {
            eprintln!("Red: {m}");
            1
        }
        Outcome::NotRun(m) => {
            eprintln!("NotRun: {m}");
            2
        }
    };
    b.summary();
    ExitCode::from(code)
}
