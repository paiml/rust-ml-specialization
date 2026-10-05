//! D20: fan-out, watchable in the recording.
//!
//! Launches Antigravity 2.8.1 on its own virtual display with the course's
//! dedicated demo profile, creates three agents in the app's agent view, each
//! with its own fixture task and workspace `ws/agent-N/`, waits until all
//! three report done, and writes the run record into its own run directory.
//! Every frame goes through agy-cdp's closed ALLOW set. Exit 0 Green, 1 Red,
//! 2 NotRun.
//!
//! Provable contract: d20-run-v1 — the run record conforms to spec/d20-run-v1.yaml, and app_evidence equals EVIDENCE intersected with the socket-tallied cdp_methods.

mod drive;
mod record;
mod timeline;
mod tree;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use agy_cdp::ax::{self, Page};
use agy_cdp::launch::{self, App, Xvfb};
use agy_cdp::notice;
use agy_cdp::open_under::{self, Roots};
use agy_cdp::operator_listing;
use agy_cdp::proc_probe;
use agy_cdp::transport::Conn;
use agy_cdp::RoleNameMap;
use demo_kit::pace::{self, Pacer};

use drive::Drive;
use record::Facts;

const APP_VERSION: &str = "2.8.1";
const APP_ASAR_SHA256: &str = "cb425e9ac098e9bc7958accc4bedb17a718c7506ae13548a0eb7cef55cdaaf26";

enum Outcome {
    Green,
    Red(String),
    NotRun(String),
}

struct Env {
    profile: PathBuf,
    bin: PathBuf,
    run: PathBuf,
}

/// One terminal line per beat, released on that beat's cue; the times feed
/// the per-step durations printed at the end.
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
                "cue {}: target {:.2} s, shown {:.2} s, cut {:.2} s so far, {late}",
                c.tag, c.target_s, c.shown_s, c.slip_s
            );
        }
    }
}

fn env() -> Result<Env, String> {
    if agy_cdp::role_name_map()? == RoleNameMap::Unmeasured {
        return Err("FixtureUnmeasured: agy-cdp role-name-map.json".into());
    }
    let profile = launch::profile_from_env()?;
    if operator_listing::overlaps(&profile)? {
        return Err("the demo profile overlaps an operator directory".into());
    }
    let bin = launch::app_bin_from_env()?;
    let app = demo_kit::preflight::app_from_launcher(&bin)?;
    if app.version != APP_VERSION || app.asar_sha256 != APP_ASAR_SHA256 {
        return Err(format!("app is not the pinned {APP_VERSION} build"));
    }
    let receipts = std::env::var_os("RFML5_RECEIPTS").ok_or("RFML5_RECEIPTS")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let run_id = format!(
        "{}-{:09}-{}",
        now.as_secs(),
        now.subsec_nanos(),
        std::process::id()
    );
    let run = open_under::create_run_dir(
        PathBuf::from(receipts).as_path(),
        &["d20-agy-app-fanout", &run_id],
    )?;
    Ok(Env { profile, bin, run })
}

fn attach_page(conn: &mut Conn) -> Result<Page, String> {
    let end = Instant::now() + Duration::from_secs(90);
    while Instant::now() < end {
        if let Some(p) = ax::attach(conn, Some("https://127.0.0.1"))? {
            return Ok(p);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Err("no app page target".into())
}

/// The notice, handled as the entry gate does: dismissed or absent is fine.
fn handle_notice(d: &mut Drive) -> Result<notice::NoticeState, String> {
    let hub = tree::dump(&d.snapshot()?);
    let compact = |v: &serde_json::Value| -> serde_json::Value {
        v.as_array()
            .into_iter()
            .flatten()
            .map(|r| serde_json::json!([r[2], r[3], true]))
            .collect()
    };
    let hub = compact(&hub);
    if !notice::shows_notice(&hub) {
        return Ok(notice::classify(Some(&hub), None));
    }
    d.click("button", "dismiss")?;
    std::thread::sleep(Duration::from_secs(2));
    let after = compact(&tree::dump(&d.snapshot()?));
    Ok(notice::classify(Some(&hub), Some(&after)))
}

fn shot(d: &mut Drive, f: &mut Facts) -> Result<(), String> {
    let png = ax::screenshot(&mut d.conn, &d.page)?;
    d.tl_note(&format!("screenshot {} bytes", png.len()));
    f.shots.push(png);
    Ok(())
}

impl Drive<'_> {
    fn tl_note(&self, m: &str) {
        eprintln!("  t={} ms: {m}", self.ms());
    }
}

/// Files under `ws/`, attributed by workspace; anything else is a stray write.
fn workspace_files(roots: &Roots, f: &mut Facts) -> Result<(), String> {
    let all = roots.manifest_run("ws", &|b: &[u8]| record::sha256_hex(b))?;
    for (rel, _, _) in all {
        let slot = (1..=3).find(|n| rel.starts_with(&format!("agent-{n}/")));
        match slot {
            Some(n) => f.files[n - 1].push(format!("ws/{rel}")),
            None => f.stray_writes += 1,
        }
    }
    Ok(())
}

fn fan_out(d: &mut Drive, b: &mut Beats, f: &mut Facts) -> Result<(), String> {
    b.show("D20-B10", "screenshot 1: the hub, before the fan-out")?;
    shot(d, f)?;
    d.start_clock();
    b.show(
        "D20-B11",
        "agent 1: new conversation, fixture-alpha typed into the focused task box",
    )?;
    let before = d.rows()?;
    d.submit_agent(1)?;
    d.find_rows(1, 1, &before)?;
    b.show("D20-B13", "agents 2 and 3: fixture-beta, fixture-gamma")?;
    let before = d.rows()?;
    d.submit_agent(2)?;
    d.submit_agent(3)?;
    d.find_rows(2, 3, &before)?;
    d.wait_all_running(60)?;
    let a = d.tl.all_running_ms.unwrap_or(0);
    b.show(
        "D20-B14",
        &format!("all three running in one tree snapshot at {a} ms"),
    )?;
    b.show("D20-B15", "screenshot 2: fanned out")?;
    shot(d, f)?;
    b.show("D20-B16", "each agent writes its file in its own workspace")?;
    d.run_to_done(300)?;
    b.show("D20-B17", "all three done; screenshot 3")?;
    shot(d, f)?;
    Ok(())
}

fn session(
    e: &Env,
    roots: &Roots,
    app: &mut App,
    b: &mut Beats,
    f: &mut Facts,
) -> Result<Conn, String> {
    let port = app.wait_port(roots, Duration::from_secs(120))?;
    app.check_env(&e.profile)?;
    let mut conn = Conn::open(port)?;
    let page = attach_page(&mut conn)?;
    let mut d = Drive::new(conn, page, roots);
    b.show("D20-B07", "find the agent view in the accessibility tree")?;
    d.wait_for("button", "new conversation", 120)?;
    let n = handle_notice(&mut d)?;
    eprintln!("notice: {}", n.label());
    if !n.is_green() {
        return Err(format!("notice {}", n.label()));
    }
    for k in 1..=3 {
        roots.mkdir_run(&format!("ws/agent-{k}"))?;
    }
    fan_out(&mut d, b, f)?;
    f.tasks = d.tasks.clone();
    f.timeline = d.tl.clone();
    Ok(d.conn)
}

fn finish(conn: Conn, f: &mut Facts) -> Result<(), String> {
    let tally = conn.close_with_snapshot()?;
    f.methods = tally.methods.clone();
    f.first_frame = tally.frames.first().cloned();
    f.keys = tally.keys.clone();
    f.devtools_created = tally.devtools_created();
    f.devtools_in_snapshot = tally.devtools_in_snapshot();
    Ok(())
}

fn live(e: &Env, x: &Xvfb, b: &mut Beats, f: &mut Facts) -> Result<Outcome, String> {
    let roots = Roots::open(&e.profile, &e.run)?;
    b.show(
        "D20-B03",
        &format!(
            "launch Antigravity {APP_VERSION} on {} with the demo profile",
            x.display()
        ),
    )?;
    let mut app = launch::launch_app(&e.bin, &e.profile, x, roots.create_run_fd("app.log")?)?;
    let watch = proc_probe::FdWatch::start(
        app.leader,
        app.keep.clone(),
        operator_listing::canonical(&operator_listing::profile_roots()?),
    );
    let out = session(e, &roots, &mut app, b, f).and_then(|c| finish(c, f));
    b.show(
        "D20-B18",
        "teardown: only the pids this run started; leak sweep",
    )?;
    f.held_open = watch.stop();
    let (left, listening) = app.teardown();
    println!(
        "  leak sweep: {} pids left{}, port listening: {listening}",
        left.len(),
        if left.is_empty() {
            String::new()
        } else {
            let d: Vec<String> = left
                .iter()
                .map(|p| agy_cdp::proc_probe::describe(*p))
                .collect();
            format!(" ({})", d.join(" "))
        }
    );
    if let Err(m) = out {
        return Ok(Outcome::Red(m));
    }
    if !left.is_empty() || listening {
        return Ok(Outcome::Red("leak sweep".into()));
    }
    workspace_files(&roots, f)?;
    Ok(Outcome::Green)
}

fn judge(e: &Env, b: &mut Beats, f: &Facts) -> Result<Outcome, String> {
    let rec = record::build(f);
    let roots = Roots::open(&e.profile, &e.run)?;
    roots.write_run(
        "receipt.json",
        (serde_json::to_string_pretty(&rec).map_err(|x| x.to_string())? + "\n").as_bytes(),
    )?;
    b.show("D20-B20", "write the run record and judge it")?;
    let defects = record::defects(f);
    for d in &defects {
        println!("  defect: {d}");
    }
    let ev = rec["app_evidence"].as_array().map_or(0, Vec::len);
    b.show(
        "D20-B21",
        &format!(
            "{} methods sent, every one in ALLOW; app_evidence {ev}/5",
            f.methods.len()
        ),
    )?;
    b.show(
        "D20-B22",
        &format!(
            "keys {:?}; DevTools targets opened: {}",
            f.keys, f.devtools_created
        ),
    )?;
    if defects.is_empty() {
        Ok(Outcome::Green)
    } else {
        Ok(Outcome::Red(format!("{} defects", defects.len())))
    }
}

fn run(b: &mut Beats, x: &Xvfb) -> Outcome {
    let e = match env() {
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
        ..Facts::default()
    };
    let out = live(&e, x, b, &mut f);
    let j = match (operator_listing::list(), operator_listing::profile_roots()) {
        (Ok(after), Ok(roots)) => operator_listing::judge(
            operator_listing::diff(&before, &after),
            &roots,
            &f.held_open,
        ),
        (Err(m), _) | (_, Err(m)) => return Outcome::Red(format!("operator listing after: {m}")),
    };
    // A change under the operator's profile is the defect, except a log file
    // no demo-started process held open (the operator's own app writing its
    // log); that and ambient churn in the shallow XDG entries are shown, not
    // judged.
    f.operator_touched = !j.touched.is_empty();
    for p in &j.touched {
        let held = operator_listing::is_held(p, &f.held_open);
        eprintln!(
            "operator profile changed: {} (held open by a demo-started pid: {held})",
            p.display()
        );
    }
    let _ = b.show(
        "D20-B19",
        &format!(
            "operator listing: {} entries before; profile {} changed; not judged: {} log writes by the operator's own app, {} ambient host entries",
            before.len(),
            j.touched.len(),
            j.logs.len(),
            j.ambient.len()
        ),
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
            let _ = b.show("D20-B23", "three agents, in the app, at once");
            b.summary();
            assert!(
                b.log.iter().any(|(t, _)| t == "D20-B22"),
                "d20: Green only after the judge"
            );
            println!("contract: d20-run-v1 OK");
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
