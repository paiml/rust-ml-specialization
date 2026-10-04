//! The shapes arm of `xtask verify` (spec §4 item 5): the pv contract proofs
//! for the D18..D21 run records, exactly 17 checks over the four demos.
//! Ported from the ph1 `spec-proofs.sh`.
//!
//! - **validate** x4: `spec/` holds exactly one contract, `pv validate` accepts it, and refuses
//!   the same contract with a key declared twice (the negative control: `pv validate` is lenient,
//!   and a validate that cannot fail proves nothing by passing).
//! - **golden** x4: the golden record is Green: exit 0, verdict Pass, zero findings and
//!   violations, the positive control fired, no shape left unarmed, no unarmed violation, every
//!   W3C SHACL-Core case passed, and `focus_nodes_n`, `shapes_n` and `plant_violations` equal the
//!   spec's §3 table. An empty or unparseable pv output is not Green. D20 and D21 also hold
//!   `app_evidence = EVIDENCE ∩ {m.method : m ∈ cdp_methods}` (M8): pv cannot relate one
//!   property's values to another node's, so the cross-check is Rust's.
//! - **planted** x4: the planted record exits 1, and its sorted findings are byte-equal to
//!   `fixtures/receipt.planted.expect`.
//! - **d19** x4: D19's golden record agrees with what it summarises.
//! - **mutants** x1: [`crate::mutants`], 18/18 killed and named, 6/6 survived.
//!
//! Any other count fails: a gate over a different set of checks is not this gate. The fixtures
//! test the contracts, not a run. Every judge run is materialised outside any git work tree
//! (PV-ONT-014). pv must be 0.70.1, or the arm refuses (not measured).

use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const DEMOS: [&str; 4] = [
    "d18-two-agents-one-server",
    "d19-workflow-ontology",
    "d20-agy-app-fanout",
    "d21-agy-app-fanin",
];
const D18: &str = "d18-two-agents-one-server";
const D19: &str = "d19-workflow-ontology";
pub const PV_PIN: &str = "0.70.1";
pub const EXPECTED_CHECKS: usize = 17;
const KILL_N: usize = 18;
const SURVIVE_N: usize = 6;
/// The longest check name: the column the report is aligned on.
pub const NAME_MAX: usize = 52;
/// The five CDP methods only the app driver sends (§5.3, `app_driven`).
pub const EVIDENCE: [&str; 5] = [
    "Accessibility.getFullAXTree",
    "DOM.getBoxModel",
    "Input.dispatchMouseEvent",
    "Input.insertText",
    "Target.setDiscoverTargets",
];

pub fn mutants_check() -> String {
    format!("d19 mutants {KILL_N}/{KILL_N} killed+named, {SURVIVE_N}/{SURVIVE_N} survived")
}

/// `focus_nodes_n`, `shapes_n` and `plant_violations` of the golden run, as the
/// spec's §3 table states them.
fn counts(d: &str) -> Option<(u64, u64, u64)> {
    match d {
        "d18-two-agents-one-server" => Some((5, 2, 25)),
        "d19-workflow-ontology" => Some((1, 1, 17)),
        "d20-agy-app-fanout" => Some((10, 2, 28)),
        "d21-agy-app-fanin" => Some((10, 2, 25)),
        _ => None,
    }
}

// ---- pv

pub struct PvRun {
    pub exit: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// One pv invocation. The real one spawns the binary; tests wrap it to plant a
/// pv that misreports itself, without touching `PATH`.
pub trait Pv {
    fn call(&self, args: &[OsString]) -> PvRun;
}

pub struct RealPv(pub PathBuf);

impl RealPv {
    /// pv resolved on this process's `PATH`.
    pub fn find() -> Option<RealPv> {
        demo_kit::preflight::which_in(None, "pv").map(RealPv)
    }
}

impl Pv for RealPv {
    fn call(&self, args: &[OsString]) -> PvRun {
        match std::process::Command::new(&self.0).args(args).output() {
            Ok(o) => PvRun {
                exit: o.status.code(),
                stdout: o.stdout,
                stderr: o.stderr,
            },
            Err(e) => PvRun {
                exit: None,
                stdout: Vec::new(),
                stderr: e.to_string().into_bytes(),
            },
        }
    }
}

fn args(a: &[&dyn AsRef<std::ffi::OsStr>]) -> Vec<OsString> {
    a.iter().map(|x| x.as_ref().to_os_string()).collect()
}

/// The second token of `pv --version`'s first line.
pub fn pv_version(pv: &dyn Pv) -> Option<String> {
    let r = pv.call(&args(&[&"--version"]));
    let out = String::from_utf8_lossy(&r.stdout).into_owned();
    out.lines()
        .next()?
        .split_whitespace()
        .nth(1)
        .map(String::from)
}

// ---- one judge run

/// A pv lint run. `exit` is pv's status, 3 when the inputs could not be
/// materialised or pv printed no JSON object (no JSON is not "no findings":
/// it is an unmeasured run), 128 when pv died by signal.
pub struct Judged {
    pub exit: i32,
    pub json: Option<Value>,
}

impl Judged {
    pub fn unmeasured() -> Judged {
        Judged {
            exit: 3,
            json: None,
        }
    }

    fn findings(&self) -> Option<&Vec<Value>> {
        self.json.as_ref()?.get("findings")?.as_array()
    }

    /// `findings | length`, 0 without JSON.
    pub fn count(&self) -> usize {
        self.findings().map_or(0, Vec::len)
    }

    /// As [`Judged::count`], but "missing" without JSON.
    pub fn count_text(&self) -> String {
        match self.json {
            Some(_) => self.count().to_string(),
            None => "missing".into(),
        }
    }

    /// Every finding's message, one line each (a message holding a newline is
    /// several lines, as `jq -r` prints it), in pv's order.
    pub fn message_lines(&self) -> Vec<String> {
        let msgs = self.findings().cloned().unwrap_or_default();
        msgs.iter()
            .map(|f| match f.get("message") {
                Some(Value::String(s)) => s.clone(),
                other => other.map_or("null".into(), Value::to_string),
            })
            .flat_map(|m| m.split('\n').map(String::from).collect::<Vec<_>>())
            .collect()
    }

    /// exit 0, verdict Pass, and no findings.
    pub fn passes(&self) -> bool {
        let pass = self.json.as_ref().and_then(|j| j.get("verdict")) == Some(&Value::from("Pass"));
        self.exit == 0 && pass && self.findings().is_some_and(Vec::is_empty)
    }
}

fn yamls(spec: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(spec)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "yaml"))
                .filter(|p| {
                    !p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn materialise(spec: &Path, record: &Path, run: &Path) -> bool {
    let ys = yamls(spec);
    if ys.is_empty() || std::fs::create_dir_all(run.join("spec")).is_err() {
        return false;
    }
    let copied = ys.iter().all(|y| {
        y.file_name()
            .is_some_and(|n| std::fs::copy(y, run.join("spec").join(n)).is_ok())
    });
    copied && std::fs::copy(record, run.join("receipt.json")).is_ok()
}

/// `pv lint <run>/spec --gate shapes --format json` over a copy of every
/// `*.yaml` in `spec` and `record`, materialised in `run`.
pub fn judge(pv: &dyn Pv, spec: &Path, record: &Path, run: &Path) -> Judged {
    if !materialise(spec, record, run) {
        return Judged::unmeasured();
    }
    let r = pv.call(&args(&[
        &"lint",
        &run.join("spec"),
        &"--gate",
        &"shapes",
        &"--format",
        &"json",
    ]));
    let _ = std::fs::write(run.join("out.json"), &r.stdout);
    let _ = std::fs::write(run.join("err.txt"), &r.stderr);
    let json = serde_json::from_slice::<Value>(&r.stdout)
        .ok()
        .filter(Value::is_object);
    match json {
        None => Judged::unmeasured(),
        Some(j) => Judged {
            exit: r.exit.unwrap_or(128),
            json: Some(j),
        },
    }
}

// ---- the checks

pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

impl Check {
    fn new(name: impl Into<String>, r: Result<String, String>) -> Check {
        let name = name.into();
        assert!(name.len() <= NAME_MAX, "check name over {NAME_MAX}: {name}");
        match r {
            Ok(detail) => Check {
                name,
                ok: true,
                detail,
            },
            Err(detail) => Check {
                name,
                ok: false,
                detail,
            },
        }
    }

    pub fn line(&self) -> String {
        match (self.ok, self.detail.is_empty()) {
            (true, true) => format!("PROOF {:<NAME_MAX$} ok", self.name),
            (true, false) => format!("PROOF {:<NAME_MAX$} ok ({})", self.name, self.detail),
            (false, _) => format!("PROOF {:<NAME_MAX$} FAIL {}", self.name, self.detail),
        }
    }
}

fn output_text(r: &PvRun) -> String {
    let mut s = String::from_utf8_lossy(&r.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&r.stderr));
    s
}

fn code(e: Option<i32>) -> i32 {
    e.unwrap_or(128)
}

/// Exactly one contract; pv validate accepts it and refuses it with a key
/// declared twice, for that reason.
fn validate(pv: &dyn Pv, spec: &Path, root: &Path, d: &str) -> Result<String, String> {
    let ys = yamls(spec);
    let [y] = ys.as_slice() else {
        return Err("spec/ must hold exactly one contract".into());
    };
    // The key goes after a guaranteed newline: appended to a last line with
    // none, it would join that line, the YAML would not parse, and a parse
    // error would pass for the refusal (round 4r2).
    let mut neg = std::fs::read(y).map_err(|e| format!("read {e}"))?;
    neg.extend_from_slice(b"\nfalsification_tests: []\n");
    let negative = root.join(format!("negative-{d}.yaml"));
    std::fs::write(&negative, neg).map_err(|e| format!("write {e}"))?;
    let r = pv.call(&args(&[&"validate", y]));
    let _ = std::fs::write(root.join(format!("validate-{d}.txt")), output_text(&r));
    if r.exit != Some(0) {
        return Err(format!("pv validate exit {}", code(r.exit)));
    }
    let n = pv.call(&args(&[&"validate", &negative]));
    let text = output_text(&n);
    let _ = std::fs::write(root.join(format!("negative-{d}.txt")), &text);
    if n.exit == Some(0) {
        return Err("pv validate accepted a contract with a duplicate key: it cannot fail".into());
    }
    if !text.contains("duplicate field") {
        return Err("pv validate refused the negative control, but not for a duplicate key".into());
    }
    Ok(String::new())
}

fn show(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "null".into(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

fn len_of(v: Option<&Value>) -> usize {
    match v {
        Some(Value::Array(a)) => a.len(),
        Some(Value::Object(o)) => o.len(),
        Some(Value::String(s)) => s.chars().count(),
        _ => 0,
    }
}

fn empty_array(v: Option<&Value>) -> bool {
    v.and_then(Value::as_array).is_some_and(Vec::is_empty)
}

/// Why pv's report of a golden run is not Green, or "" when it is. An absent
/// field is not a pass: every criterion must be present and hold.
pub fn golden_why(j: &Value, (fn_, sn, pc): (u64, u64, u64)) -> String {
    let g = |k: &str| j.get(k);
    let zero = Value::from(0);
    if show(g("verdict")) != "Pass" {
        return format!("verdict {}", show(g("verdict")));
    }
    if !empty_array(g("findings")) {
        return format!("findings {}", len_of(g("findings")));
    }
    if g("violations") != Some(&zero) {
        return format!("violations {}", show(g("violations")));
    }
    if show(g("pc_shape")) != "fired" {
        return format!("pc_shape {}", show(g("pc_shape")));
    }
    if !empty_array(g("not_armed_shapes")) {
        return format!("not armed {}", show(g("not_armed_shapes")));
    }
    if g("unarmed_violations") != Some(&zero) {
        return format!("unarmed {}", show(g("unarmed_violations")));
    }
    count_why(j, (fn_, sn, pc))
}

/// The W3C cases and the section 3 counts, after the verdict fields hold.
fn count_why(j: &Value, (fn_, sn, pc): (u64, u64, u64)) -> String {
    let g = |k: &str| j.get(k);
    let n = g("w3c_cases_n").and_then(Value::as_u64).unwrap_or(0);
    if n == 0 || g("w3c_cases_passed").and_then(Value::as_u64) != Some(n) {
        return format!(
            "w3c {}/{}",
            show(g("w3c_cases_passed")),
            show(g("w3c_cases_n"))
        );
    }
    for (k, want) in [
        ("focus_nodes_n", fn_),
        ("shapes_n", sn),
        ("plant_violations", pc),
    ] {
        if g(k).and_then(Value::as_u64) != Some(want) {
            return format!("{k} {} want {want}", show(g(k)));
        }
    }
    String::new()
}

/// `app_evidence = EVIDENCE ∩ {m.method : m ∈ cdp_methods}`, as sorted lists
/// (a duplicate in `app_evidence` is not the set).
pub fn app_evidence_why(record: &Value) -> String {
    let Some(ev) = record.get("app_evidence").and_then(Value::as_array) else {
        return "app_evidence absent".into();
    };
    let methods: Vec<&str> = record
        .get("cdp_methods")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| m.get("method").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let mut got: Vec<String> = ev.iter().map(|v| show(Some(v))).collect();
    got.sort();
    let want: Vec<String> = EVIDENCE
        .iter()
        .filter(|e| methods.contains(e))
        .map(|e| e.to_string())
        .collect();
    if got == want {
        String::new()
    } else {
        format!("app_evidence {got:?} != EVIDENCE ∩ cdp_methods {want:?}")
    }
}

fn golden(pv: &dyn Pv, dir: &Path, d: &str, root: &Path) -> Result<String, String> {
    let rec = dir.join("fixtures/receipt.golden.json");
    let j = judge(
        pv,
        &dir.join("spec"),
        &rec,
        &root.join(format!("{d}-golden")),
    );
    let why = match (&j.json, counts(d)) {
        (None, _) => "no json".to_string(),
        (Some(_), None) => "no count row for this demo".to_string(),
        (Some(v), Some(c)) => golden_why(v, c),
    };
    let why = if why.is_empty() && matches!(d, "d20-agy-app-fanout" | "d21-agy-app-fanin") {
        std::fs::read(&rec)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map_or("record is not JSON".into(), |r| app_evidence_why(&r))
    } else {
        why
    };
    if j.exit == 0 && why.is_empty() {
        Ok(String::new())
    } else {
        Err(format!("exit {} {why}", j.exit))
    }
}

fn newlines(b: &[u8]) -> usize {
    b.iter().filter(|&&c| c == b'\n').count()
}

fn planted(pv: &dyn Pv, dir: &Path, d: &str, root: &Path) -> Result<String, String> {
    let run = root.join(format!("{d}-planted"));
    let j = judge(
        pv,
        &dir.join("spec"),
        &dir.join("fixtures/receipt.planted.json"),
        &run,
    );
    let mut lines = j.message_lines();
    lines.sort();
    let got: Vec<u8> = lines
        .iter()
        .flat_map(|l| format!("{l}\n").into_bytes())
        .collect();
    let _ = std::fs::write(run.join("messages.txt"), &got);
    let expect_path = dir.join("fixtures/receipt.planted.expect");
    if j.exit != 1 {
        return Err(format!("exit {}, want 1", j.exit));
    }
    if lines.is_empty() {
        return Err("exit 1 with no findings".into());
    }
    match std::fs::read(&expect_path) {
        Ok(expect) if expect == got => Ok(format!("{} findings", newlines(&expect))),
        _ => Err(format!(
            "diff {} {}",
            run.join("messages.txt").display(),
            expect_path.display()
        )),
    }
}

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

fn strings(v: Option<&Value>) -> Option<Vec<String>> {
    v?.as_array()?
        .iter()
        .map(|x| x.as_str().map(String::from))
        .collect()
}

fn ids(t: &Value, k: &str, field: &str) -> Option<Vec<String>> {
    t.get(k)?
        .as_array()?
        .iter()
        .map(|r| r.get(field).and_then(Value::as_str).map(String::from))
        .collect()
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// D19's golden record summarises runs anyone can repeat; it must agree with
/// what it summarises. Four checks.
fn cross(demos: &Path) -> Vec<Check> {
    let g = read_json(&demos.join(D19).join("fixtures/receipt.golden.json"));
    let t = read_json(&demos.join(D19).join("fixtures/mutants.json"));
    let n18 = std::fs::read(demos.join(D18).join("fixtures/receipt.planted.expect"))
        .map_or(0, |b| newlines(&b)) as u64;
    let bad = || "disagrees, or a file is missing".to_string();
    let res = |ok: Option<bool>| {
        if ok == Some(true) {
            Ok(String::new())
        } else {
            Err(bad())
        }
    };
    let (g, t) = (g.as_ref(), t.as_ref());
    let pf = g.and_then(|g| g.get("planted_findings")?.as_u64());
    let same = |gk: &str, tk: &str, f: &str, uniq: bool| -> Option<bool> {
        let mut want = sorted(ids(t?, tk, f)?);
        if uniq {
            want.dedup();
        }
        Some((uniq || !want.is_empty()) && sorted(strings(g?.get(gk))?) == want)
    };
    vec![
        Check::new(
            "d19 planted_findings == d18 .expect lines",
            res(Some(n18 > 0 && pf == Some(n18))),
        ),
        Check::new(
            "d19 killed == mutants.json kill ids",
            res(same("killed", "kill", "id", false)),
        ),
        Check::new(
            "d19 survived == mutants.json survive ids",
            res(same("survived", "survive", "id", false)),
        ),
        Check::new(
            "d19 components_killed == mutants.json components",
            res(same("components_killed", "kill", "component", true)),
        ),
    ]
}

const BAD_ROW: [&str; 8] = [
    "NOT-KILLED",
    "IMPURE",
    "WRONG-",
    "NO-RUST-ASSERT",
    "PATCH-FAILED",
    "SURVIVOR-KILLED",
    "IDENTITY-FAILED",
    "verify-mutants:",
];

fn mutants(pv: &dyn Pv, demos: &Path, root: &Path) -> Check {
    let out = crate::mutants::verify_mutants(
        pv,
        &demos.join(D18),
        &demos.join(D19).join("fixtures/mutants.json"),
        &root.join("mutants"),
    );
    let holds = out.exit == 0
        && (out.killed, out.n_kill) == (KILL_N, KILL_N)
        && (out.survived, out.n_surv) == (SURVIVE_N, SURVIVE_N);
    if holds {
        return Check::new(mutants_check(), Ok(String::new()));
    }
    // The first row that is not ok says WHY; the summary lines alone say only how many.
    let first = out.lines.iter().find(|l| {
        BAD_ROW
            .iter()
            .any(|b| l.contains(b) && (*b != "verify-mutants:" || l.starts_with(b)))
    });
    let tail: Vec<&str> = out
        .lines
        .iter()
        .rev()
        .take(2)
        .rev()
        .map(String::as_str)
        .collect();
    let first = first.map_or(String::new(), |f| format!("{f} | "));
    Check::new(
        mutants_check(),
        Err(format!("exit {}: {first}{}", out.exit, tail.join(" "))),
    )
}

// ---- the arm

pub enum Report {
    /// pv is not the pinned version, or the run root is unusable: not measured.
    Refused(String),
    Ran {
        checks: Vec<Check>,
        expected: usize,
    },
}

impl Report {
    pub fn lines(&self) -> Vec<String> {
        match self {
            Report::Refused(r) => vec![r.clone()],
            Report::Ran { checks, expected } => {
                let failed = checks.iter().filter(|c| !c.ok).count();
                let mut v: Vec<String> = checks.iter().map(Check::line).collect();
                v.push(format!("shapes: {} checks, {failed} failing", checks.len()));
                if checks.len() != *expected {
                    v.push(format!(
                        "shapes: {} checks, expected {expected}: a gate over a different set is not this gate",
                        checks.len()
                    ));
                }
                v
            }
        }
    }

    pub fn passed(&self) -> bool {
        matches!(self, Report::Ran { checks, expected }
            if checks.len() == *expected && checks.iter().all(|c| c.ok))
    }

    /// The sorted names of every failing check.
    #[cfg(test)]
    pub fn fail_set(&self) -> Vec<String> {
        match self {
            Report::Refused(_) => Vec::new(),
            Report::Ran { checks, .. } => sorted(
                checks
                    .iter()
                    .filter(|c| !c.ok)
                    .map(|c| c.name.clone())
                    .collect(),
            ),
        }
    }
}

/// A fresh directory under `parent`, named `<prefix>-<pid>-<nanos>-<n>`.
pub fn fresh_dir(parent: &Path, prefix: &str) -> Result<PathBuf, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    for _ in 0..1000 {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = parent.join(format!("{prefix}-{}-{nanos}-{n}", std::process::id()));
        if std::fs::create_dir(&p).is_ok() {
            return Ok(p);
        }
    }
    Err(format!("no fresh directory under {}", parent.display()))
}

fn demo_checks(pv: &dyn Pv, demos: &Path, d: &str, root: &Path) -> Vec<Check> {
    let dir = demos.join(d);
    if !dir.join("spec").is_dir() || !dir.join("fixtures").is_dir() {
        return vec![Check::new(
            format!("{d} present"),
            Err("no spec or fixtures dir".into()),
        )];
    }
    vec![
        Check::new(
            format!("{d} validate"),
            validate(pv, &dir.join("spec"), root, d),
        ),
        Check::new(format!("{d} golden Green"), golden(pv, &dir, d, root)),
        Check::new(
            format!("{d} planted == .expect"),
            planted(pv, &dir, d, root),
        ),
    ]
}

/// Run the arm over `demos`, all four demos or the one `only` names (`None`
/// when `only` names a demo without a contract). Judge runs go under a fresh
/// directory in `scratch`, kept when a check fails and removed otherwise.
pub fn run(pv: &dyn Pv, demos: &Path, only: Option<&str>, scratch: &Path) -> Option<Report> {
    let set: Vec<&str> = DEMOS
        .iter()
        .copied()
        .filter(|d| only.is_none_or(|o| o == *d))
        .collect();
    if set.is_empty() {
        return None;
    }
    let version = pv_version(pv);
    if version.as_deref() != Some(PV_PIN) {
        return Some(Report::Refused(format!(
            "shapes: pv {}, pinned {PV_PIN}; refusing (not measured)",
            version.as_deref().unwrap_or("absent")
        )));
    }
    let root = match fresh_dir(scratch, "xtask-shapes") {
        Ok(r) if !demo_kit::shapes::inside_git(&r) => r,
        Ok(r) => {
            return Some(Report::Refused(format!(
                "shapes: {} is inside a git work tree (PV-ONT-014); refusing",
                r.display()
            )))
        }
        Err(e) => return Some(Report::Refused(format!("shapes: {e}"))),
    };
    let with_d19 = set.contains(&D19);
    let mut checks: Vec<Check> = set
        .iter()
        .flat_map(|d| demo_checks(pv, demos, d, &root))
        .collect();
    if with_d19 {
        checks.extend(cross(demos));
        checks.push(mutants(pv, demos, &root));
    }
    // The full arm is exactly EXPECTED_CHECKS; `--only` narrows it to that
    // demo's three, plus the five D19 owns.
    let expected = match only {
        None => EXPECTED_CHECKS,
        Some(_) => 3 * set.len() + if with_d19 { 5 } else { 0 },
    };
    let report = Report::Ran { checks, expected };
    if report.passed() {
        let _ = std::fs::remove_dir_all(&root);
    }
    Some(report)
}

#[cfg(test)]
#[path = "proofs_selftest.rs"]
mod selftest;
