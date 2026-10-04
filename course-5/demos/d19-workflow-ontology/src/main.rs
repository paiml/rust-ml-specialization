//! D19: the workflow as an ontology. D18's frozen contract is validated, its
//! golden record judged Green and its planted record refused with exactly the
//! expected findings; a mutant table then proves the shapes bite (every kill
//! row refused for its own constraint, every survivor passing and owned by a
//! named Rust assert); `pv extract spec --check` catches a one-byte record
//! edit; and D19's own record is judged with D19's shapes and cross-checked
//! against its sources.
//!
//! Provable contract: d19-run-v1 — the run record conforms to spec/d19-run-v1.yaml, and every mutant in fixtures/mutants.json is killed or survives as its row states.
//!
//! D18 is only read: its `spec/` and `fixtures/`, never its live output.
//! Environment: `RFML5_RECEIPTS` (receipts and this run's working files,
//! outside this repo), and optionally `RFML5_BEATS` (beat times; unset means
//! no pacing). Exit codes: 0 Green, 1 Red, 2 NotRun.

use d19_workflow_ontology::matrix::{Kill, KillRow, Survive, SurviveRow, Table};
use d19_workflow_ontology::record::{Record, DEMO};
use d19_workflow_ontology::screen::Screen;
use d19_workflow_ontology::{patch, steps};
use demo_kit::harness::Harness;
use demo_kit::shapes::ShapesOutcome;
use demo_kit::{NotRunReason, Verdict};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The record key whose digit the drift step changes.
const DRIFT_KEY: &str = "server_spawns";
const D18: &str = "../d18-two-agents-one-server";

/// Everything D19 reads, loaded before the first pv call.
struct Inputs {
    d18_spec: PathBuf,
    d18_contract: PathBuf,
    golden: Vec<u8>,
    golden_json: Value,
    planted: Vec<u8>,
    expect: String,
    table: Table,
    d19_spec: PathBuf,
}

fn read(p: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))
}

fn load_inputs(dir: &Path) -> Result<Inputs, String> {
    let d18 = dir.join(D18);
    let d18_spec = d18.join("spec");
    let fx = d18.join("fixtures");
    let golden = read(&fx.join("receipt.golden.json"))?;
    let golden_json = serde_json::from_slice(&golden).map_err(|e| format!("golden: {e}"))?;
    let expect = String::from_utf8(read(&fx.join("receipt.planted.expect"))?)
        .map_err(|e| format!("expect: {e}"))?;
    let tv: Value = serde_json::from_slice(&read(&dir.join("fixtures/mutants.json"))?)
        .map_err(|e| format!("mutants.json: {e}"))?;
    Ok(Inputs {
        d18_contract: steps::contract(&d18_spec)?,
        d18_spec,
        golden,
        golden_json,
        planted: read(&fx.join("receipt.planted.json"))?,
        expect,
        table: Table::parse(&tv).map_err(|e| format!("mutants.json: {e}"))?,
        d19_spec: dir.join("spec"),
    })
}

/// What the run measured, filled beat by beat.
#[derive(Default)]
struct Measured {
    pv_version: String,
    validate_exit: i32,
    extract_exit: i32,
    extract_check_exit: i32,
    extract_drift_exit: i32,
    golden_exit: i32,
    golden_pc: String,
    planted_exit: i32,
    planted_findings: usize,
    planted_matches_expect: bool,
    killed: Vec<String>,
    survived: Vec<String>,
    components_killed: Vec<String>,
    /// (passed, total) W3C cases, one entry per judge call.
    w3c: Vec<(u64, u64)>,
    /// (kill rows, survive rows) in the table.
    expect: (usize, usize),
}

struct Demo<'a> {
    h: Harness,
    s: Screen<'a>,
    work: PathBuf,
    m: Measured,
    /// Rust-side asserts that did not hold; any one makes the run Red.
    broken: Vec<String>,
}

impl Demo<'_> {
    fn check(&mut self, ok: bool, what: &str) {
        if !ok {
            self.s.line(&format!("  FAIL: {what}"));
            self.broken.push(what.to_string());
        }
    }

    /// One shapes judge call. A judge that gives no verdict stops the run.
    fn judge(&mut self, spec: &Path, record: &[u8]) -> Result<ShapesOutcome, String> {
        let o = demo_kit::shapes::judge(spec, record, &self.work.join("judges"));
        if let Verdict::NotRun { .. } = o.verdict {
            return Err(format!("shapes judge: {}", o.why));
        }
        let (p, n) = (o.w3c_cases_passed.unwrap_or(0), o.w3c_cases_n.unwrap_or(0));
        self.m.w3c.push((p, n));
        let pc = steps::pc_shape(&o);
        self.check(pc == "fired", &format!("pc_shape {pc} on a judge call"));
        Ok(o)
    }

    /// Record a judge call as a receipt step, with pv's JSON as its stdout.
    fn step_of(&mut self, cmd: &str, o: &ShapesOutcome) {
        let out = o
            .run_dir
            .as_ref()
            .and_then(|d| std::fs::read(d.join("out.json")).ok())
            .unwrap_or_default();
        self.h.receipt.step(cmd, o.exit.unwrap_or(-1), &out);
    }

    /// `pv args…` in `cwd`, shown and recorded as a receipt step.
    fn pv(&mut self, cwd: &Path, args: &[&str]) -> steps::Ran {
        let r = steps::pv(cwd, args);
        self.s.line(&format!("$ {}", r.shown));
        self.h.receipt.step(&r.shown, r.code(), &r.stdout);
        r
    }

    fn validate(&mut self, inp: &Inputs) -> Result<(), String> {
        self.s.cue("D19-B04");
        let dir = self.work.join("validate");
        steps::materialise(&dir, &inp.d18_contract, &inp.golden)?;
        let name = file_name(&inp.d18_contract);
        let r = self.pv(&dir, &["validate", &format!("spec/{name}")]);
        self.s
            .line(&format!("  {} -> exit {}", last_line(&r.text()), r.code()));
        self.m.validate_exit = r.code();
        self.check(r.code() == 0, "pv validate accepts D18's contract");
        self.validate_negative(&dir, &inp.d18_contract)?;
        self.s.cue("D19-B05");
        let sha = demo_kit::sha::sha256_bytes(&read(&inp.d18_contract)?);
        self.s
            .line(&format!("  {D18}/spec/{name}  sha256 {}", &sha[..16]));
        self.s
            .line("  the contract D18 ran under, read in place: nothing new invented");
        Ok(())
    }

    /// The control: the same contract with a key declared twice is refused.
    fn validate_negative(&mut self, dir: &Path, contract: &Path) -> Result<(), String> {
        let neg = dir.join("neg");
        std::fs::create_dir_all(&neg).map_err(|e| e.to_string())?;
        let path = neg.join(file_name(contract));
        std::fs::write(&path, steps::duplicate_key(&read(contract)?)).map_err(|e| e.to_string())?;
        let r = steps::pv(&neg, &["validate", &file_name(contract)]);
        let named = r.text().contains("falsification_tests");
        self.s.line(&format!(
            "  control: a duplicate key -> exit {} ({})",
            r.code(),
            if named {
                "names the field"
            } else {
                "names nothing"
            }
        ));
        self.check(
            r.code() != 0 && named,
            "pv validate refuses a duplicate key",
        );
        Ok(())
    }

    fn golden(&mut self, inp: &Inputs) -> Result<(), String> {
        self.s.cue("D19-B06");
        self.s
            .line("$ pv lint spec --gate shapes --format json   # D18's golden record");
        let o = self.judge(&inp.d18_spec, &inp.golden)?;
        self.step_of("pv lint spec --gate shapes --format json  # golden", &o);
        self.s.cue("D19-B07");
        self.s.line(&format!(
            "  exit {}  verdict {}  findings {}",
            matrix_code(o.exit),
            o.verdict,
            o.findings.len()
        ));
        self.m.golden_exit = o.exit.unwrap_or(-1);
        self.check(
            o.is_green() && o.findings.is_empty(),
            "golden is Green and silent",
        );
        self.s.cue("D19-B08");
        self.m.golden_pc = steps::pc_shape(&o);
        self.s.line(&format!(
            "  pc_shape {}  plant_violations {}  focus nodes {}  shapes {}",
            self.m.golden_pc,
            opt(o.plant_violations),
            opt(o.focus_nodes_n),
            opt(o.shapes_n)
        ));
        Ok(())
    }

    fn planted(&mut self, inp: &Inputs) -> Result<(), String> {
        self.s.cue("D19-B09");
        self.s
            .line("$ pv lint spec --gate shapes --format json   # D18's planted record");
        let o = self.judge(&inp.d18_spec, &inp.planted)?;
        self.step_of("pv lint spec --gate shapes --format json  # planted", &o);
        self.s.line(&format!(
            "  exit {}  findings {}",
            matrix_code(o.exit),
            o.findings.len()
        ));
        self.s.cue("D19-B10");
        let got = steps::sorted_lines(&o.findings);
        self.m.planted_exit = o.exit.unwrap_or(-1);
        self.m.planted_findings = o.findings.len();
        self.m.planted_matches_expect = got == inp.expect;
        for l in got.lines() {
            self.s.line(&format!("  {l}"));
        }
        self.s.line(&format!(
            "  == receipt.planted.expect: {}",
            if self.m.planted_matches_expect {
                "byte for byte"
            } else {
                "DIFFERS"
            }
        ));
        self.check(
            o.exit == Some(1) && self.m.planted_matches_expect,
            "planted is refused with exactly .expect",
        );
        Ok(())
    }

    /// The identity arm and the non-vacuity of every patch.
    fn table_start(&mut self, inp: &Inputs) -> Result<(), String> {
        self.s.cue("D19-B11");
        let t = &inp.table;
        self.s.line(&format!(
            "mutant table: fixtures/mutants.json  subject {}  {} kill, {} survive",
            t.subject,
            t.kill.len(),
            t.survive.len()
        ));
        let id = patch::patched(&inp.golden_json, &json!([]))?;
        let o = self.judge(&inp.d18_spec, &to_bytes(&id)?)?;
        self.s
            .line(&format!("  identity (empty patch): {}", o.verdict));
        self.check(o.is_green(), "the empty patch leaves the golden Green");
        self.s.cue("D19-B12");
        let patches = t
            .kill
            .iter()
            .map(|r| &r.patch)
            .chain(t.survive.iter().map(|r| &r.patch));
        let bad: Vec<String> = patches
            .filter_map(|p| patch::patched(&inp.golden_json, p).err())
            .collect();
        let n = t.kill.len() + t.survive.len();
        self.s.line(&format!(
            "  {}/{n} patches change the record",
            n - bad.len()
        ));
        self.check(bad.is_empty(), "every patch changes the record");
        Ok(())
    }

    /// Judge the golden record with `ops` applied. A patch that does not
    /// apply, or changes nothing, is a broken row: Red, never NotRun.
    fn mutant(
        &mut self,
        inp: &Inputs,
        id: &str,
        ops: &Value,
    ) -> Result<Option<ShapesOutcome>, String> {
        match patch::patched(&inp.golden_json, ops) {
            Ok(doc) => self.judge(&inp.d18_spec, &to_bytes(&doc)?).map(Some),
            Err(e) => {
                self.check(false, &format!("{id}: patch {e}"));
                Ok(None)
            }
        }
    }

    fn kill_row(&mut self, inp: &Inputs, row: &KillRow) -> Result<(), String> {
        let t = &inp.table;
        let Some(o) = self.mutant(inp, &row.id, &row.patch)? else {
            return Ok(());
        };
        let k = row.judge(&o, &t.node, &t.ns());
        self.s.line(&format!(
            "  {}  {:<16} {:<34} {}",
            row.id,
            row.component,
            row.properties.join(","),
            k
        ));
        if k == Kill::Killed {
            self.m.killed.push(row.id.clone());
        }
        self.check(k == Kill::Killed, &format!("{} killed", row.id));
        Ok(())
    }

    fn survive_row(&mut self, inp: &Inputs, row: &SurviveRow) -> Result<(), String> {
        let Some(o) = self.mutant(inp, &row.id, &row.patch)? else {
            return Ok(());
        };
        let v = row.judge(&o);
        self.s.line(&format!(
            "  {}  {:<16} {}",
            row.id,
            v.to_string(),
            row.short_why()
        ));
        if v == Survive::Survived {
            self.m.survived.push(row.id.clone());
        }
        self.check(v == Survive::Survived, &format!("{} survives", row.id));
        Ok(())
    }

    fn kill_rows(&mut self, inp: &Inputs) -> Result<(), String> {
        let tags = ["D19-B13", "D19-B14", "D19-B15"];
        let per = inp.table.kill.len().div_ceil(tags.len()).max(1);
        for (i, rows) in inp.table.kill.chunks(per).enumerate() {
            self.s.cue(tags.get(i).copied().unwrap_or("D19-B15"));
            for row in rows {
                self.kill_row(inp, row)?;
            }
        }
        let n = inp.table.kill.len();
        self.s
            .line(&format!("  killed {}/{n}", self.m.killed.len()));
        self.s.cue("D19-B16");
        let killed: Vec<&KillRow> = inp
            .table
            .kill
            .iter()
            .filter(|r| self.m.killed.contains(&r.id))
            .collect();
        let mut comps: Vec<String> = killed.iter().map(|r| r.component.clone()).collect();
        comps.sort();
        comps.dedup();
        let all = inp.table.components();
        self.s.line(&format!(
            "  components {}/{}: {}",
            comps.len(),
            all.len(),
            comps.join(" ")
        ));
        self.check(comps == all, "the kill rows cover every component");
        self.m.components_killed = comps;
        Ok(())
    }

    fn survive_rows(&mut self, inp: &Inputs) -> Result<(), String> {
        self.s.cue("D19-B17");
        self.s.line("survivors: breaks the shapes cannot see");
        let rows = &inp.table.survive;
        let half = rows.len().div_ceil(2);
        for (i, row) in rows.iter().enumerate() {
            if i == 0 {
                self.s.cue("D19-B18");
            } else if i == half {
                self.s.cue("D19-B19");
            }
            self.survive_row(inp, row)?;
        }
        self.s.line(&format!(
            "  survived {}/{}",
            self.m.survived.len(),
            rows.len()
        ));
        self.s.cue("D19-B20");
        for row in rows {
            self.s
                .line(&format!("  {}  rust_assert: {}", row.id, row.rust_assert));
        }
        Ok(())
    }

    fn extract(&mut self, inp: &Inputs) -> Result<(), String> {
        self.s.cue("D19-B21");
        let dir = self.work.join("extract");
        steps::materialise(&dir, &inp.d18_contract, &inp.golden)?;
        let e = self.pv(&dir, &["extract", "spec"]);
        self.s.line(&format!("  exit {}", e.code()));
        let c = self.pv(&dir, &["extract", "spec", "--check"]);
        self.s.line(&format!("  exit {}  (fresh)", c.code()));
        (self.m.extract_exit, self.m.extract_check_exit) = (e.code(), c.code());
        self.check(
            e.code() == 0 && c.code() == 0,
            "extract, then --check exits 0",
        );
        self.s.cue("D19-B22");
        let (edited, at, old, new) = steps::one_byte_edit(&inp.golden, DRIFT_KEY)?;
        std::fs::write(dir.join("receipt.json"), &edited).map_err(|e| e.to_string())?;
        self.s.line(&format!(
            "  receipt.json byte {at}: '{}' -> '{}'  ({DRIFT_KEY})",
            old as char, new as char
        ));
        let d = self.pv(&dir, &["extract", "spec", "--check"]);
        self.s.cue("D19-B23");
        self.s
            .line(&format!("  exit {}  {}", d.code(), last_line(&d.text())));
        self.m.extract_drift_exit = d.code();
        self.check(d.code() == 1, "--check exits 1 after a one-byte edit");
        Ok(())
    }

    fn w3c(&mut self) {
        self.s.cue("D19-B24");
        let calls = self.m.w3c.len();
        let all_pass = calls > 0 && self.m.w3c.iter().all(|&(p, n)| n > 0 && p == n);
        let (p, n) = self.m.w3c.first().copied().unwrap_or((0, 0));
        self.s.line(&format!(
            "W3C conformance cases: {p}/{n} on {} of {calls} judge calls",
            self.m.w3c.iter().filter(|&&(a, b)| b > 0 && a == b).count()
        ));
        self.check(all_pass, "pv passes its W3C cases on every judge call");
    }

    fn record(&self, inp: &Inputs) -> Record {
        let (p, n) = self.m.w3c.first().copied().unwrap_or((0, 0));
        let m = &self.m;
        Record {
            pv_version: m.pv_version.clone(),
            subject: inp.table.subject.clone(),
            validate_exit: m.validate_exit,
            extract_exit: m.extract_exit,
            extract_check_exit: m.extract_check_exit,
            extract_drift_exit: m.extract_drift_exit,
            golden_exit: m.golden_exit,
            golden_pc: m.golden_pc.clone(),
            w3c_passed: p,
            w3c_total: n,
            planted_exit: m.planted_exit,
            planted_findings: m.planted_findings,
            planted_matches_expect: m.planted_matches_expect,
            killed: m.killed.clone(),
            survived: m.survived.clone(),
            components_killed: m.components_killed.clone(),
        }
    }

    fn own_record(&mut self, inp: &Inputs) -> Result<(), String> {
        self.s.cue("D19-B25");
        let rec = self.record(inp);
        let bytes = to_bytes(&rec.to_json())?;
        let o = self.judge(&inp.d19_spec, &bytes)?;
        self.s.line(&format!(
            "D19 record: {} fields, judged with spec/{}: {}  findings {}",
            rec.to_json().as_object().map_or(0, |m| m.len()),
            file_name(&steps::contract(&inp.d19_spec)?),
            o.verdict,
            o.findings.len()
        ));
        let bad = rec.cross_check(&inp.table, &inp.expect);
        self.s.line(&format!(
            "  cross-checked against .expect and the mutant table: {}",
            if bad.is_empty() {
                "agrees".to_string()
            } else {
                bad.join("; ")
            }
        ));
        self.check(o.is_green(), "D19's record is Green under D19's shapes");
        self.check(bad.is_empty(), "D19's record agrees with its sources");
        self.h.set_shapes(o, &bytes);
        Ok(())
    }

    fn body(&mut self, inp: &Inputs) -> Result<(), String> {
        self.m.expect = (inp.table.kill.len(), inp.table.survive.len());
        self.validate(inp)?;
        self.golden(inp)?;
        self.planted(inp)?;
        self.table_start(inp)?;
        self.kill_rows(inp)?;
        self.survive_rows(inp)?;
        self.extract(inp)?;
        self.w3c();
        self.own_record(inp)?;
        self.s.cue("D19-B26");
        self.s
            .line("rerun: cargo run -q -p xtask -- verify --only d19-workflow-ontology");
        Ok(())
    }

    fn measured(&self) -> BTreeMap<String, Value> {
        let shapes = self
            .h
            .receipt
            .shapes
            .as_ref()
            .map_or("absent".to_string(), |o| o.verdict.to_string());
        BTreeMap::from([
            ("shapes".to_string(), json!(shapes)),
            ("mutants_killed".to_string(), json!(self.m.killed.len())),
            ("mutants_survived".to_string(), json!(self.m.survived.len())),
            (
                "exit".to_string(),
                json!(i32::from(!self.broken.is_empty())),
            ),
        ])
    }
}

/// The title card: the first thing on screen, before any tool runs.
fn title(s: &mut Screen) {
    s.cue("D19-B01");
    s.line("D19 - the workflow as an ontology");
    s.cue("D19-B02");
    s.line("  D18's two-agent run, written down as a contract with shapes");
    s.cue("D19-B03");
    s.line("  then attacked on purpose: what the shapes catch, and what they cannot");
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn last_line(t: &str) -> String {
    t.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn opt(v: Option<u64>) -> String {
    v.map_or_else(|| "absent".into(), |n| n.to_string())
}

fn matrix_code(e: Option<i32>) -> String {
    d19_workflow_ontology::matrix::code(e)
}

fn to_bytes(v: &Value) -> Result<Vec<u8>, String> {
    let mut b = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    b.push(b'\n');
    Ok(b)
}

fn pv_version() -> String {
    std::process::Command::new("pv")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| demo_kit::pin::version_from_output("pv", &String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_else(|| "absent".into())
}

/// `$RFML5_RECEIPTS/<demo>/work/<run_id>`, refused inside the repo.
fn work_dir(h: &Harness) -> Result<PathBuf, NotRunReason> {
    if std::env::var_os("RFML5_RECEIPTS").is_none() {
        return Err(NotRunReason::EnvUnset("RFML5_RECEIPTS".into()));
    }
    let root =
        demo_kit::receipt::receipts_root(&h.dir.join("../../..")).map_err(NotRunReason::Refused)?;
    let w = root.join(DEMO).join("work").join(&h.receipt.run_id);
    std::fs::create_dir_all(&w).map_err(|e| NotRunReason::Refused(e.to_string()))?;
    Ok(w)
}

fn exit_code(v: &Verdict) -> u8 {
    match v {
        Verdict::Green => 0,
        Verdict::Red { .. } => 1,
        Verdict::NotRun { .. } => 2,
    }
}

/// Load, run every beat, finish. A run that cannot judge finishes NotRun.
fn run(d: &mut Demo) -> Verdict {
    if !d.h.not_run().is_empty() {
        return d.h.finish(BTreeMap::new());
    }
    match work_dir(&d.h) {
        Ok(w) => d.work = w,
        Err(r) => {
            d.h.refuse(r);
            return d.h.finish(BTreeMap::new());
        }
    }
    d.m.pv_version = pv_version();
    d.h.receipt
        .tools
        .insert("pv".into(), d.m.pv_version.clone());
    let res = load_inputs(&d.h.dir).and_then(|inp| d.body(&inp));
    if let Err(e) = res {
        d.s.line(&format!("NotRun: {e}"));
        d.h.refuse(NotRunReason::ShapesNotRun(e));
    }
    d.s.cue("D19-B27");
    let measured = d.measured();
    d.h.finish(measured)
}

fn main() -> Result<ExitCode, String> {
    let pacer = demo_kit::pace::Pacer::from_env()?;
    let mut s = Screen::new(pacer.as_ref());
    title(&mut s);
    let mut d = Demo {
        h: Harness::load(env!("CARGO_MANIFEST_DIR")),
        s,
        work: PathBuf::new(),
        m: Measured::default(),
        broken: Vec::new(),
    };
    let verdict = run(&mut d);
    let code = exit_code(&verdict);
    if code != 0 {
        d.s.report();
        return Ok(ExitCode::from(code));
    }
    assert!(
        verdict.is_green()
            && d.broken.is_empty()
            && d.m.expect.0 > 0
            && d.m.killed.len() == d.m.expect.0
            && d.m.survived.len() == d.m.expect.1,
        "{DEMO}: exit 0 only on Green with every mutant as its row states"
    );
    println!("contract: d19-run-v1 OK");
    d.s.report();
    Ok(ExitCode::SUCCESS)
}
