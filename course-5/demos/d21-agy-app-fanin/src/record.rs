//! The run record (`spec/d21-run-v1.yaml`) and the claims the shapes cannot
//! express, asserted here in Rust: `stopped_agents = running_at_red`,
//! `red_seen > 0`, `stopped(N) − red_seen ≤ L`, each stop a click on that
//! agent's own control, the lowest-id refusal, one digest over six orders,
//! `app_evidence = EVIDENCE ∩ tally` and discovery as the first frame.

use std::collections::{BTreeMap, BTreeSet};

use agy_cdp::methods::{ALLOW, EVIDENCE};

use crate::json::J;
use crate::reduce::Reduction;

pub const TASKS: [&str; 3] = ["fixture-alpha", "fixture-beta", "fixture-gamma"];
pub const KEYS: [&str; 3] = ["Enter", "Escape", "Tab"];
pub const ORDERS: [&str; 6] = ["1-2-3", "1-3-2", "2-1-3", "2-3-1", "3-1-2", "3-2-1"];

/// Below this many bytes a 1920x1080 screenshot is one flat colour.
pub const MIN_SHOT_BYTES: usize = 20_000;

/// The pure acceptance check on one agent's `result.md`: Green only when it
/// reads exactly `<task> PASS`. A missing file is not red (the agent made no
/// claim); any other content is red.
pub fn acceptance_red(task: &str, body: Option<&[u8]>) -> bool {
    match body {
        None => false,
        Some(b) => String::from_utf8_lossy(b).trim() != format!("{task} PASS"),
    }
}

/// One agent's stop, on the fan-out clock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StopFact {
    pub sent_ms: u64,
    pub stopped_ms: u64,
    /// The click went to the `Stop execution` button in that agent's row.
    pub own_control: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub app_version: String,
    pub asar_sha256: String,
    pub l_ms: Option<u64>,
    pub tasks: [Option<String>; 3],
    pub agents_started: u64,
    pub red_agent: Option<u8>,
    pub red_seen_ms: u64,
    /// Stops of agent-1 and agent-2.
    pub stops: [Option<StopFact>; 2],
    pub running_at_red: Vec<String>,
    pub stopped_agents: Vec<String>,
    pub writes_after_stop: u64,
    pub orders: Vec<(String, Reduction)>,
    pub stray_writes: u64,
    pub shots: Vec<Vec<u8>>,
    pub methods: BTreeMap<String, u64>,
    pub first_frame: Option<String>,
    /// `Target.setDiscoverTargets` frames, and how many carried exactly
    /// `{"discover": true}`.
    pub discover_frames: usize,
    pub discover_true: usize,
    pub keys: BTreeSet<String>,
    pub inserted: Vec<String>,
    pub prompts: Vec<String>,
    pub devtools_created: u64,
    pub devtools_in_snapshot: u64,
    pub operator_touched: bool,
    /// Files under the operator profile that a demo-started process held
    /// open during the run; a changed log file in this set is not exempt.
    pub held_open: BTreeSet<std::path::PathBuf>,
}

pub fn app_evidence(methods: &BTreeMap<String, u64>) -> Vec<&'static str> {
    EVIDENCE
        .iter()
        .copied()
        .filter(|m| methods.get(*m).is_some_and(|c| *c > 0))
        .collect()
}

pub fn build(f: &Facts) -> J {
    let stop = |i: usize| f.stops[i].unwrap_or_default();
    let first = f.orders.first().map(|o| &o.1);
    let kv = |k: &str, v: J| (k.to_string(), v);
    J::Obj(vec![
        kv("demo", J::str("d21-agy-app-fanin")),
        kv("app_version", J::str(&f.app_version)),
        kv("app_asar_sha256", J::str(&f.asar_sha256)),
        kv("cdp_protocol", J::str("1.3")),
        kv("operator_profile_touched", J::Bool(f.operator_touched)),
        kv("devtools_targets_opened", J::int(f.devtools_created)),
        kv(
            "cdp_methods",
            J::Arr(
                f.methods
                    .iter()
                    .map(|(m, c)| J::Obj(vec![kv("method", J::str(m)), kv("count", J::int(*c))]))
                    .collect(),
            ),
        ),
        kv("app_evidence", J::strs(app_evidence(&f.methods))),
        kv("keys_sent", J::strs(&f.keys)),
        kv("agents_started", J::int(f.agents_started)),
        kv(
            "red_agent",
            f.red_agent
                .map_or(J::Null, |a| J::str(format!("agent-{a}"))),
        ),
        kv("red_seen_ms", J::int(f.red_seen_ms)),
        kv("stop_agent_1_sent_ms", J::int(stop(0).sent_ms)),
        kv("stop_agent_1_stopped_ms", J::int(stop(0).stopped_ms)),
        kv("stop_agent_2_sent_ms", J::int(stop(1).sent_ms)),
        kv("stop_agent_2_stopped_ms", J::int(stop(1).stopped_ms)),
        kv("running_at_red", J::strs(&f.running_at_red)),
        kv("stopped_agents", J::strs(&f.stopped_agents)),
        kv("writes_after_stop", J::int(f.writes_after_stop)),
        kv("merged", J::Bool(first.is_some_and(|r| r.merged))),
        kv(
            "refusal_names",
            J::strs(first.map(|r| r.refusal_names.clone()).unwrap_or_default()),
        ),
        kv("reduce_orders", J::strs(f.orders.iter().map(|o| &o.0))),
        kv(
            "reduce_digest_per_order",
            J::strs(f.orders.iter().map(|o| &o.1.digest)),
        ),
        kv("stray_writes", J::int(f.stray_writes)),
    ])
}

fn nonblank(png: &[u8]) -> bool {
    png.starts_with(b"\x89PNG\r\n\x1a\n") && png.len() >= MIN_SHOT_BYTES
}

fn line_defects(f: &Facts) -> Vec<String> {
    let mut out = Vec::new();
    if f.agents_started != 3 {
        out.push(format!("fan_out: {} agents started", f.agents_started));
    }
    for (i, t) in TASKS.iter().enumerate() {
        if f.tasks[i].as_deref() != Some(*t) {
            out.push(format!("fan_out: agent {} view did not show {t}", i + 1));
        }
    }
    if f.red_agent != Some(3) {
        out.push(format!("stop_the_line: red agent {:?}", f.red_agent));
    }
    if f.red_seen_ms == 0 {
        out.push("stop_the_line: red_seen_ms is 0".into());
    }
    if f.running_at_red != f.stopped_agents {
        out.push(format!(
            "stop_the_line: stopped {:?} but running at red {:?}",
            f.stopped_agents, f.running_at_red
        ));
    }
    let Some(l) = f.l_ms else {
        out.push("stop_the_line: no committed stop latency L".into());
        return out;
    };
    for (i, s) in f.stops.iter().enumerate() {
        let n = i + 1;
        let Some(s) = s else {
            out.push(format!("stop_the_line: agent {n} was never stopped"));
            continue;
        };
        if !(f.red_seen_ms <= s.sent_ms && s.sent_ms <= s.stopped_ms) {
            out.push(format!(
                "stop_the_line: agent {n} red/sent/stopped out of order"
            ));
        }
        if s.stopped_ms.saturating_sub(f.red_seen_ms) > l {
            out.push(format!(
                "stop_the_line: agent {n} stopped {} ms after red, L is {l} ms",
                s.stopped_ms.saturating_sub(f.red_seen_ms)
            ));
        }
        if !s.own_control {
            out.push(format!(
                "stop_the_line: agent {n} stop was not its own control"
            ));
        }
    }
    if f.writes_after_stop != 0 {
        out.push(format!(
            "stop_the_line: {} writes after stop",
            f.writes_after_stop
        ));
    }
    out
}

fn fan_in_defects(f: &Facts) -> Vec<String> {
    let mut out = Vec::new();
    let labels: Vec<&str> = f.orders.iter().map(|o| o.0.as_str()).collect();
    if labels != ORDERS {
        out.push(format!("order_free: orders {labels:?}"));
    }
    let digests: BTreeSet<&str> = f.orders.iter().map(|o| o.1.digest.as_str()).collect();
    let verdicts: BTreeSet<(bool, Vec<String>)> = f
        .orders
        .iter()
        .map(|o| (o.1.merged, o.1.refusal_names.clone()))
        .collect();
    if digests.len() != 1 || verdicts.len() != 1 {
        out.push(format!("order_free: {} digests", digests.len()));
    }
    for (label, r) in &f.orders {
        if r.merged || r.refusal_names != ["agent-3"] {
            out.push(format!(
                "refuse_on_red: order {label} merged={} names {:?}",
                r.merged, r.refusal_names
            ));
        }
    }
    out
}

fn channel_defects(f: &Facts) -> Vec<String> {
    let mut out = Vec::new();
    let have = app_evidence(&f.methods);
    let missing: Vec<&str> = EVIDENCE
        .iter()
        .copied()
        .filter(|m| !have.contains(m))
        .collect();
    if !missing.is_empty() {
        out.push(format!("app_driven: evidence missing {missing:?}"));
    }
    if f.first_frame.as_deref() != Some("Target.setDiscoverTargets") {
        out.push(format!("app_driven: first frame {:?}", f.first_frame));
    }
    if f.discover_frames != 1 || f.discover_true != 1 {
        out.push(format!(
            "app_driven: {} discovery frames, {} with discover true",
            f.discover_frames, f.discover_true
        ));
    }
    if let Some(m) = f.methods.keys().find(|m| !ALLOW.contains(&m.as_str())) {
        out.push(format!("no_js: {m} is not in ALLOW"));
    }
    if let Some(k) = f.keys.iter().find(|k| !KEYS.contains(&k.as_str())) {
        out.push(format!("no_js: key {k}"));
    }
    if let Some(t) = f.inserted.iter().find(|t| !f.prompts.contains(t)) {
        out.push(format!(
            "no_js: inserted text is not a committed prompt: {t:?}"
        ));
    }
    if f.devtools_created + f.devtools_in_snapshot != 0 {
        out.push("no_js: a DevTools target was opened".into());
    }
    out
}

fn evidence_defects(f: &Facts) -> Vec<String> {
    let mut out = Vec::new();
    let distinct: BTreeSet<&Vec<u8>> = f.shots.iter().collect();
    if f.shots.len() != 3 || distinct.len() != 3 {
        out.push(format!(
            "evidence: {} shots, {} distinct",
            f.shots.len(),
            distinct.len()
        ));
    }
    if let Some(i) = f.shots.iter().position(|s| !nonblank(s)) {
        out.push(format!("evidence: screenshot {} is blank", i + 1));
    }
    if f.stray_writes != 0 {
        out.push(format!(
            "operator_untouched: {} stray writes",
            f.stray_writes
        ));
    }
    if f.operator_touched {
        out.push("operator_untouched: the operator profile was touched".into());
    }
    out
}

/// Every defect in one run, empty when the record holds every claim.
pub fn defects(f: &Facts) -> Vec<String> {
    let mut out = line_defects(f);
    out.extend(fan_in_defects(f));
    out.extend(channel_defects(f));
    out.extend(evidence_defects(f));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::top_level_keys;
    use crate::reduce::{reduce, reduce_orders, AgentResult};

    fn png(seed: u8) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
        v.resize(MIN_SHOT_BYTES + 10, seed);
        v
    }

    type Plant = Box<dyn Fn(&mut Facts)>;

    fn results() -> Vec<AgentResult> {
        (1..=3)
            .map(|a| AgentResult {
                agent: a,
                red: a == 3,
                files: vec![],
            })
            .collect()
    }

    fn green() -> Facts {
        let mut methods = BTreeMap::new();
        for m in EVIDENCE
            .iter()
            .chain(["Input.dispatchKeyEvent", "Page.captureScreenshot"].iter())
        {
            methods.insert(m.to_string(), 2);
        }
        let stop = |s, t| {
            Some(StopFact {
                sent_ms: s,
                stopped_ms: t,
                own_control: true,
            })
        };
        Facts {
            app_version: "2.8.1".into(),
            asar_sha256: "x".into(),
            l_ms: Some(2000),
            tasks: TASKS.map(|t| Some(t.to_string())),
            agents_started: 3,
            red_agent: Some(3),
            red_seen_ms: 5120,
            stops: [stop(5131, 5602), stop(5133, 5710)],
            running_at_red: vec!["agent-1".into(), "agent-2".into()],
            stopped_agents: vec!["agent-1".into(), "agent-2".into()],
            writes_after_stop: 0,
            orders: reduce_orders(&results(), reduce),
            stray_writes: 0,
            shots: vec![png(1), png(2), png(3)],
            methods,
            first_frame: Some("Target.setDiscoverTargets".into()),
            discover_frames: 1,
            discover_true: 1,
            keys: ["Enter".to_string()].into(),
            inserted: vec!["p1".into()],
            prompts: vec!["p1".into(), "p2".into()],
            devtools_created: 0,
            devtools_in_snapshot: 0,
            operator_touched: false,
            held_open: BTreeSet::new(),
        }
    }

    #[test]
    fn acceptance_is_red_only_on_a_wrong_claim() {
        assert!(!acceptance_red("fixture-gamma", None));
        assert!(!acceptance_red(
            "fixture-gamma",
            Some(b"fixture-gamma PASS\n")
        ));
        assert!(acceptance_red(
            "fixture-gamma",
            Some(b"fixture-gamma FAIL\n")
        ));
        assert!(acceptance_red("fixture-gamma", Some(b"")));
    }

    #[test]
    fn green_facts_have_no_defects_and_the_record_has_every_shape_key() {
        let f = green();
        assert_eq!(defects(&f), Vec::<String>::new());
        let text = build(&f).pretty();
        let golden = include_str!("../fixtures/receipt.golden.json");
        assert_eq!(top_level_keys(&text), top_level_keys(golden));
        assert!(text.contains("\"merged\": false"));
        assert!(text.contains("\"refusal_names\": [\n    \"agent-3\"\n  ]"));
    }

    #[test]
    fn each_planted_defect_is_named() {
        let cases: Vec<(&str, Plant)> = vec![
            (
                "red/sent/stopped",
                Box::new(|f| f.stops[0].as_mut().unwrap().sent_ms = 5000),
            ),
            (
                "agent 2 red/sent",
                Box::new(|f| f.stops[1].as_mut().unwrap().stopped_ms = 5132),
            ),
            (
                "L is 2000",
                Box::new(|f| f.stops[1].as_mut().unwrap().stopped_ms = 9000),
            ),
            (
                "own control",
                Box::new(|f| f.stops[0].as_mut().unwrap().own_control = false),
            ),
            ("never stopped", Box::new(|f| f.stops[1] = None)),
            (
                "running at red",
                Box::new(|f| {
                    f.stopped_agents.pop();
                }),
            ),
            ("red_seen_ms is 0", Box::new(|f| f.red_seen_ms = 0)),
            ("red agent", Box::new(|f| f.red_agent = Some(2))),
            ("writes after stop", Box::new(|f| f.writes_after_stop = 1)),
            ("no committed", Box::new(|f| f.l_ms = None)),
            ("orders", Box::new(|f| f.orders.truncate(5))),
            (
                "2 digests",
                Box::new(|f| f.orders[4].1.digest = "0".repeat(64)),
            ),
            ("refuse_on_red", Box::new(|f| f.orders[0].1.merged = true)),
            (
                "Input.insertText",
                Box::new(|f| {
                    f.methods.remove("Input.insertText");
                }),
            ),
            (
                "Runtime.evaluate",
                Box::new(|f| {
                    f.methods.insert("Runtime.evaluate".into(), 1);
                }),
            ),
            (
                "key F12",
                Box::new(|f| {
                    f.keys.insert("F12".into());
                }),
            ),
            (
                "first frame",
                Box::new(|f| f.first_frame = Some("Target.getTargets".into())),
            ),
            ("discovery frames", Box::new(|f| f.discover_true = 0)),
            (
                "committed prompt",
                Box::new(|f| f.inserted.push("x".into())),
            ),
            ("DevTools", Box::new(|f| f.devtools_in_snapshot = 1)),
            ("2 distinct", Box::new(|f| f.shots[1] = f.shots[0].clone())),
            (
                "screenshot 3 is blank",
                Box::new(|f| f.shots[2] = b"\x89PNG\r\n\x1a\n".to_vec()),
            ),
            ("stray writes", Box::new(|f| f.stray_writes = 1)),
            ("operator profile", Box::new(|f| f.operator_touched = true)),
            ("2 agents started", Box::new(|f| f.agents_started = 2)),
            ("agent 2 view", Box::new(|f| f.tasks[1] = None)),
        ];
        for (needle, plant) in cases {
            let mut f = green();
            plant(&mut f);
            let d = defects(&f);
            assert!(d.iter().any(|x| x.contains(needle)), "{needle}: {d:?}");
        }
    }
}
