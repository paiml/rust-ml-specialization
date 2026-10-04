//! E_4 (ph4 entry gate): probe the Antigravity app's controls through agy-cdp.
//!
//! Launches the app on its own scratch display with the demo profile, dumps
//! the hub's accessibility tree into the run directory, finds the six
//! controls, presses F1 and F12 with `xdotool` on that same display while
//! counting DevTools `Target.targetCreated` events, and writes the role/name
//! map into its own run directory. `xtask promote-fixture e3-probe` copies it
//! out for commit. Exit 0 Green, 1 Red, 2 NotRun.
//!
//! Provable contract: e3-probe-v1 — all six controls are found in the accessibility tree and the F1/F12 DevTools counts are recorded; the role/name map is written only into the run directory.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use agy_cdp::ax::{self, Node, Page};
use agy_cdp::launch::{self, App, XdotoolKey, Xvfb};
use agy_cdp::methods::Key;
use agy_cdp::open_under::{self, Roots};
use agy_cdp::operator_listing;
use agy_cdp::transport::Conn;
use serde_json::{json, Value};

/// The probe prompt: the one text this bin may insert.
const PROBE_PROMPT: &str = include_str!("../fixtures/probe-prompt.txt");

/// `(control, roles, name needles)`: a control is the first node whose role
/// is listed and whose name contains a needle.
const CONTROLS: [(&str, &[&str], &[&str]); 6] = [
    ("dismiss", &["button"], &["dismiss"]),
    (
        "new_agent",
        &["button", "link"],
        &["new agent", "new conversation", "start agent", "new"],
    ),
    ("task_box", &["textbox", "combobox", "searchbox"], &[""]),
    ("send", &["button"], &["send", "submit"]),
    (
        "agent_state",
        &["StaticText", "status", "generic", "button", "link"],
        &["running", "working", "thinking", "generating"],
    ),
    ("stop", &["button"], &["stop", "cancel"]),
];

enum Outcome {
    Green(usize),
    Red(String),
    NotRun(String),
}

struct Env {
    profile: PathBuf,
    bin: PathBuf,
    run: PathBuf,
}

fn env() -> Result<Env, String> {
    let profile = launch::profile_from_env()?;
    if operator_listing::overlaps(&profile)? {
        return Err("RFML5_AGY_PROFILE overlaps an operator directory".into());
    }
    let bin = launch::app_bin_from_env()?;
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
        &["agy-cdp", "e3-probe", &run_id],
    )?;
    Ok(Env { profile, bin, run })
}

fn find_control<'a>(nodes: &'a [Node], roles: &[&str], needles: &[&str]) -> Option<&'a Node> {
    needles.iter().find_map(|n| {
        roles.iter().find_map(|r| {
            ax::find(nodes, r, n)
                .into_iter()
                .find(|x| x.backend.is_some())
        })
    })
}

fn wait_tree(conn: &mut Conn, page: &Page, needle: &str, secs: u64) -> Result<Vec<Node>, String> {
    let end = Instant::now() + Duration::from_secs(secs);
    loop {
        let nodes = ax::tree(conn, page)?;
        let hit = nodes.iter().any(|n| n.name.to_lowercase().contains(needle));
        if hit || Instant::now() > end {
            return Ok(nodes);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn attach_page(conn: &mut Conn) -> Result<Page, String> {
    let end = Instant::now() + Duration::from_secs(90);
    while Instant::now() < end {
        if let Some(p) = ax::attach(conn, Some("https://127.0.0.1"))? {
            return Ok(p);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    ax::attach(conn, None)?.ok_or_else(|| "no app page target".into())
}

fn compact(nodes: &[Node]) -> Value {
    Value::Array(
        nodes
            .iter()
            .filter(|n| !n.role.is_empty() && n.role != "none" && n.role != "InlineTextBox")
            .map(|n| json!([n.role, n.name, n.backend.is_some()]))
            .collect(),
    )
}

fn record(found: &mut BTreeMap<String, Value>, nodes: &[Node]) {
    for (c, roles, needles) in CONTROLS {
        if found.contains_key(c) {
            continue;
        }
        if let Some(n) = find_control(nodes, roles, needles) {
            found.insert(c.into(), json!({"role": n.role, "name": n.name}));
        }
    }
}

fn click_control(conn: &mut Conn, page: &Page, c: usize) -> Result<bool, String> {
    let nodes = ax::tree(conn, page)?;
    let (_, roles, needles) = CONTROLS[c];
    match find_control(&nodes, roles, needles) {
        Some(n) => ax::click(conn, page, n).map(|_| true),
        None => Ok(false),
    }
}

/// Walk the hub: dismiss, new agent, task box, prompt, send; collect controls.
fn probe_controls(
    conn: &mut Conn,
    page: &Page,
    roots: &Roots,
) -> Result<BTreeMap<String, Value>, String> {
    let mut found = BTreeMap::new();
    let first = wait_tree(conn, page, "agent", 120)?;
    roots.write_run("ax-tree-0.json", compact(&first).to_string().as_bytes())?;
    record(&mut found, &first);
    for step in 0..2 {
        click_control(conn, page, step)?;
        std::thread::sleep(Duration::from_secs(3));
        let nodes = ax::tree(conn, page)?;
        record(&mut found, &nodes);
    }
    click_control(conn, page, 2)?;
    std::thread::sleep(Duration::from_secs(1));
    if ax::insert_text_checked(conn, page, PROBE_PROMPT.trim()).is_ok() {
        ax::press(conn, page, Key::Enter)?;
    }
    for i in 1..=6 {
        std::thread::sleep(Duration::from_secs(3));
        let nodes = ax::tree(conn, page)?;
        roots.write_run(
            &format!("ax-tree-{i}.json"),
            compact(&nodes).to_string().as_bytes(),
        )?;
        record(&mut found, &nodes);
    }
    Ok(found)
}

/// Press F1 then F12 on the scratch display; DevTools targets created per key.
fn devtools_test(conn: &mut Conn, page: &Page, x: &Xvfb) -> Result<Value, String> {
    let mut out = json!({});
    for (k, label) in [(XdotoolKey::F1, "F1"), (XdotoolKey::F12, "F12")] {
        let before = conn.tally.devtools_created();
        let code = launch::xdotool_key(x, k)?;
        std::thread::sleep(Duration::from_secs(4));
        ax::tree(conn, page)?;
        out[label] = json!({"xdotool_exit": code, "devtools_targets_created": conn.tally.devtools_created() - before});
    }
    Ok(out)
}

fn session(e: &Env, roots: &Roots, x: &Xvfb, app: &mut App) -> Result<Outcome, String> {
    let port = match app.wait_port(roots, Duration::from_secs(120)) {
        Ok(p) => p,
        Err(m) => return Ok(Outcome::NotRun(m)),
    };
    app.check_env(&e.profile)?;
    let mut conn = Conn::open(port)?;
    let page = attach_page(&mut conn)?;
    let controls = probe_controls(&mut conn, &page, roots)?;
    let devtools = devtools_test(&mut conn, &page, x)?;
    let tally = conn.close_with_snapshot()?;
    let map = json!({
        "schema": "role-name-map-v1",
        "app_version": "2.8.1",
        "xdotool": launch::xdotool_version()?,
        "controls": controls,
        "devtools": devtools,
        "devtools_in_snapshot": tally.devtools_in_snapshot(),
        "cdp_methods": tally.methods,
        "keys_sent": tally.keys,
    });
    roots.write_run(
        "role-name-map.json",
        (serde_json::to_string_pretty(&map).map_err(|e| e.to_string())? + "\n").as_bytes(),
    )?;
    let n = map["controls"].as_object().map_or(0, |m| m.len());
    eprintln!(
        "controls found: {n}/6 {:?}",
        map["controls"]
            .as_object()
            .map(|m| m.keys().collect::<Vec<_>>())
    );
    eprintln!("devtools: {}", map["devtools"]);
    if n == 6 {
        Ok(Outcome::Green(n))
    } else {
        Ok(Outcome::Red(format!("{n}/6 controls found")))
    }
}

fn run() -> Outcome {
    let e = match env() {
        Ok(e) => e,
        Err(m) => return Outcome::NotRun(m),
    };
    let before = match operator_listing::list() {
        Ok(l) => l,
        Err(m) => return Outcome::NotRun(m),
    };
    let x = match launch::display_override().and_then(Xvfb::start) {
        Ok(x) => x,
        Err(m) => return Outcome::NotRun(m),
    };
    eprintln!("display: {}", x.display());
    let res = (|| {
        let roots = Roots::open(&e.profile, &e.run)?;
        let mut app = launch::launch_app(&e.bin, &e.profile, &x, roots.create_run_fd("app.log")?)?;
        let out = session(&e, &roots, &x, &mut app);
        if let Err(m) = &out {
            eprintln!("session: {m}");
        }
        let (left, listening) = app.teardown();
        if !left.is_empty() || listening {
            return Ok(Outcome::Red(format!(
                "leak sweep: {} pids left, port listening {listening}",
                left.len()
            )));
        }
        out
    })();
    let changed = operator_listing::list()
        .map(|after| operator_listing::diff(&before, &after))
        .unwrap_or_else(|m| vec![PathBuf::from(m)]);
    eprintln!(
        "operator listing: {} entries, {} changed",
        before.len(),
        changed.len()
    );
    for c in changed.iter().take(20) {
        eprintln!("  changed: {}", c.display());
    }
    match res {
        Ok(Outcome::Green(_)) if !changed.is_empty() => {
            Outcome::Red("operator listing changed".into())
        }
        Ok(o) => o,
        Err(m) => Outcome::Red(m),
    }
}

fn main() -> ExitCode {
    match run() {
        Outcome::Green(n) => {
            assert_eq!(n, 6, "e3-probe: Green only with 6/6 controls");
            println!("contract: e3-probe-v1 OK");
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
