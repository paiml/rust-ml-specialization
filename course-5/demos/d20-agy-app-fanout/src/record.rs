//! The run record (`spec/d20-run-v1.yaml`) and the checks the shapes cannot
//! express: `app_evidence = EVIDENCE ∩ tally`, discovery as the first frame,
//! the all-running moment from one snapshot, and non-blank screenshots.

use std::collections::{BTreeMap, BTreeSet};

use agy_cdp::methods::{ALLOW, EVIDENCE};
use serde_json::{json, Value};

use crate::timeline::Timeline;

pub const TASKS: [&str; 3] = ["fixture-alpha", "fixture-beta", "fixture-gamma"];
pub const KEYS: [&str; 3] = ["Enter", "Escape", "Tab"];

/// A 1920x1080 frame of one flat colour encodes to a few kilobytes; a frame
/// with the app's text and chrome on it is far larger. Below this many bytes a
/// screenshot is treated as blank.
pub const MIN_SHOT_BYTES: usize = 20_000;

#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub app_version: String,
    pub asar_sha256: String,
    /// The task each agent's own view showed, read back from the tree.
    pub tasks: [Option<String>; 3],
    pub timeline: Timeline,
    /// Workspace-relative files, `ws/agent-N/<name>`.
    pub files: [Vec<String>; 3],
    pub stray_writes: u64,
    pub shots: Vec<Vec<u8>>,
    pub methods: BTreeMap<String, u64>,
    pub first_frame: Option<String>,
    pub keys: BTreeSet<String>,
    pub devtools_created: u64,
    pub devtools_in_snapshot: u64,
    pub operator_touched: bool,
    /// Files under the operator profile that a demo-started process held
    /// open during the run; a changed log file in this set is not exempt.
    pub held_open: BTreeSet<std::path::PathBuf>,
}

pub fn sha256_hex(b: &[u8]) -> String {
    demo_kit::sha::sha256_bytes(b)
}

/// `EVIDENCE ∩ {method sent at least once}`, in EVIDENCE order.
pub fn app_evidence(methods: &BTreeMap<String, u64>) -> Vec<&'static str> {
    EVIDENCE
        .iter()
        .copied()
        .filter(|m| methods.get(*m).is_some_and(|c| *c > 0))
        .collect()
}

fn state(t: &Timeline, i: usize) -> &'static str {
    match (t.started_ms[i], t.done_ms[i]) {
        (_, Some(_)) => "done",
        (Some(_), None) => "running",
        _ => "unseen",
    }
}

pub fn build(f: &Facts) -> Value {
    let mut r = json!({
        "demo": "d20-agy-app-fanout",
        "app_version": f.app_version,
        "app_asar_sha256": f.asar_sha256,
        "cdp_protocol": "1.3",
        "operator_profile_touched": f.operator_touched,
        "devtools_targets_opened": f.devtools_created,
        "stray_writes": f.stray_writes,
        "cdp_methods": f.methods.iter().map(|(m, c)| json!({"method": m, "count": c})).collect::<Vec<_>>(),
        "app_evidence": app_evidence(&f.methods),
        "keys_sent": f.keys,
        "screenshots_sha256": f.shots.iter().map(|s| sha256_hex(s)).collect::<Vec<_>>(),
    });
    for i in 0..3 {
        let n = i + 1;
        let t = &f.timeline;
        r[format!("agent_{n}_task")] = json!(f.tasks[i]);
        r[format!("agent_{n}_state")] = json!(state(t, i));
        r[format!("agent_{n}_files")] = json!(f.files[i]);
        r[format!("agent_{n}_started_ms")] = json!(t.started_ms[i]);
        r[format!("agent_{n}_done_ms")] = json!(t.done_ms[i]);
    }
    r["all_running_seen_ms"] = json!(f.timeline.all_running_ms);
    r
}

fn nonblank(png: &[u8]) -> bool {
    png.starts_with(b"\x89PNG\r\n\x1a\n") && png.len() >= MIN_SHOT_BYTES
}

fn agent_defects(f: &Facts, i: usize) -> Vec<String> {
    let n = i + 1;
    let t = &f.timeline;
    let mut out = Vec::new();
    if f.tasks[i].as_deref() != Some(TASKS[i]) {
        out.push(format!("fan_out: agent {n} view did not show {}", TASKS[i]));
    }
    if t.done_ms[i].is_none() {
        out.push(format!("fan_out: agent {n} never reported done"));
    }
    let prefix = format!("ws/agent-{n}/");
    if f.files[i].is_empty() || f.files[i].iter().any(|p| !p.starts_with(&prefix)) {
        out.push(format!(
            "disjoint_workspaces: agent {n} files {:?}",
            f.files[i]
        ));
    }
    let a = t.all_running_ms.unwrap_or(0);
    let ordered = t.started_ms[i].is_some_and(|s| s <= a) && t.done_ms[i].is_some_and(|d| a <= d);
    if !ordered {
        out.push(format!(
            "concurrent: agent {n} started/done do not bracket all_running"
        ));
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
    if let Some(m) = f.methods.keys().find(|m| !ALLOW.contains(&m.as_str())) {
        out.push(format!("no_js: {m} is not in ALLOW"));
    }
    if let Some(k) = f.keys.iter().find(|k| !KEYS.contains(&k.as_str())) {
        out.push(format!("no_js: key {k}"));
    }
    if f.devtools_created + f.devtools_in_snapshot != 0 {
        out.push("no_js: a DevTools target was opened".into());
    }
    out
}

fn evidence_defects(f: &Facts) -> Vec<String> {
    let mut out = Vec::new();
    let t = &f.timeline;
    if !(t.all_running_count == 3 && t.all_running_ms.is_some_and(|a| a > 0)) {
        out.push("concurrent: no single snapshot held three running agents".into());
    }
    if let Some(a) = t.approval_ms {
        out.push(format!(
            "concurrent: an agent waited on an approval prompt at {a} ms; blocked is not running"
        ));
    }
    let distinct: BTreeSet<String> = f.shots.iter().map(|s| sha256_hex(s)).collect();
    if f.shots.len() != 3 || distinct.len() != 3 {
        out.push(format!(
            "evidence_distinct: {} shots, {} distinct",
            f.shots.len(),
            distinct.len()
        ));
    }
    if let Some(i) = f.shots.iter().position(|s| !nonblank(s)) {
        out.push(format!("evidence_distinct: screenshot {} is blank", i + 1));
    }
    if f.stray_writes != 0 {
        out.push(format!(
            "disjoint_workspaces: {} stray writes",
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
    let mut out: Vec<String> = (0..3).flat_map(|i| agent_defects(f, i)).collect();
    out.extend(channel_defects(f));
    out.extend(evidence_defects(f));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(seed: u8) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
        v.resize(MIN_SHOT_BYTES + 10, seed);
        v
    }

    type Plant = Box<dyn Fn(&mut Facts)>;

    fn green() -> Facts {
        let mut methods = BTreeMap::new();
        for m in EVIDENCE
            .iter()
            .chain(["Input.dispatchKeyEvent", "Page.captureScreenshot"].iter())
        {
            methods.insert(m.to_string(), 2);
        }
        Facts {
            app_version: "2.8.1".into(),
            asar_sha256: "cb425e9ac098e9bc7958accc4bedb17a718c7506ae13548a0eb7cef55cdaaf26".into(),
            tasks: TASKS.map(|t| Some(t.to_string())),
            timeline: Timeline {
                started_ms: [Some(1200), Some(1350), Some(1500)],
                done_ms: [Some(41000), Some(43500), Some(45200)],
                all_running_ms: Some(1620),
                all_running_count: 3,
                approval_ms: None,
            },
            files: [1, 2, 3].map(|n| vec![format!("ws/agent-{n}/result.md")]),
            stray_writes: 0,
            shots: vec![png(1), png(2), png(3)],
            methods,
            first_frame: Some("Target.setDiscoverTargets".into()),
            keys: ["Enter".to_string()].into(),
            devtools_created: 0,
            devtools_in_snapshot: 0,
            operator_touched: false,
            held_open: BTreeSet::new(),
        }
    }

    #[test]
    fn green_facts_have_no_defects_and_the_record_has_every_shape_key() {
        let f = green();
        assert_eq!(defects(&f), Vec::<String>::new());
        let r = build(&f);
        let golden: Value =
            serde_json::from_str(include_str!("../fixtures/receipt.golden.json")).unwrap();
        let keys = |v: &Value| {
            v.as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(keys(&r), keys(&golden));
        assert_eq!(r["app_evidence"], golden["app_evidence"]);
    }

    #[test]
    fn each_planted_defect_is_named() {
        let cases: Vec<(&str, Plant)> = vec![
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
            ("2 distinct", Box::new(|f| f.shots[1] = f.shots[0].clone())),
            (
                "agent 3 started",
                Box::new(|f| f.timeline.started_ms[2] = Some(2000)),
            ),
            (
                "agent 1 files",
                Box::new(|f| f.files[0] = vec!["ws/agent-2/result.md".into()]),
            ),
            (
                "first frame",
                Box::new(|f| f.first_frame = Some("Target.getTargets".into())),
            ),
            (
                "single snapshot",
                Box::new(|f| f.timeline.all_running_count = 2),
            ),
            (
                "screenshot 2 is blank",
                Box::new(|f| f.shots[1] = b"\x89PNG\r\n\x1a\nx".to_vec()),
            ),
            ("operator profile", Box::new(|f| f.operator_touched = true)),
            ("agent 2 view", Box::new(|f| f.tasks[1] = None)),
            (
                "approval prompt",
                Box::new(|f| f.timeline.approval_ms = Some(900)),
            ),
        ];
        for (needle, plant) in cases {
            let mut f = green();
            plant(&mut f);
            let d = defects(&f);
            assert!(d.iter().any(|x| x.contains(needle)), "{needle}: {d:?}");
        }
    }
}
