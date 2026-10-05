//! D19's mutant matrix (spec §5.2, M7), run against D18's contract with pv's
//! shapes gate. Ported from the ph1 `verify-mutants.sh`.
//!
//! - **identity**: the unpatched golden record passes: exit 0, verdict Pass, no findings.
//! - **kill**: every kill row is rejected (exit 1) with EXACTLY `.findings` findings, and every
//!   finding starts "<node><focus> violates shape `<target>` (<component>)". A mutant that also
//!   trips a second constraint, or the right one on the wrong node, is NOT killed: it is impure.
//!   Each finding must also name the row's `.properties`: the first "<ns><prop>" token after the
//!   prefix (the constraint's path), or "-" where pv 0.70.1 names none (datatype). And the sorted
//!   findings must equal the row's `.messages` byte for byte.
//! - **survive**: every survive row passes. The shapes cannot see it; the row names the Rust
//!   assert that does. A `rust_assert` that names nothing fails the row. A survivor that starts
//!   failing is a change in pv's semantics, and it is reported, never absorbed.
//!
//! Each judge run is materialised in a fresh directory outside any git work tree (PV-ONT-014).

use crate::proofs::{judge, Judged, Pv};
use serde_json::Value;
use std::path::Path;

/// What one run of the matrix printed, and how it ended: 0 every row held,
/// 1 a row failed, 2 the table or its inputs were refused.
pub struct MutantsOut {
    pub lines: Vec<String>,
    pub exit: i32,
    pub killed: usize,
    pub n_kill: usize,
    pub survived: usize,
    pub n_surv: usize,
}

impl MutantsOut {
    fn refused(line: String) -> MutantsOut {
        MutantsOut {
            lines: vec![line],
            exit: 2,
            killed: 0,
            n_kill: 0,
            survived: 0,
            n_surv: 0,
        }
    }
}

// ---- RFC 6902, the subset the table uses: add (incl. append with "-"), remove, replace.

fn pointer(path: &str) -> Result<Vec<String>, String> {
    match path.strip_prefix('/') {
        Some(rest) => Ok(rest.split('/').map(String::from).collect()),
        None if path.is_empty() => Ok(Vec::new()),
        None => Err(format!("bad pointer {path:?}")),
    }
}

fn step<'a>(v: &'a mut Value, seg: &str) -> Result<&'a mut Value, String> {
    match v {
        Value::Array(a) => {
            let i: usize = seg.parse().map_err(|_| format!("index {seg:?}"))?;
            a.get_mut(i)
                .ok_or_else(|| format!("index {i} out of range"))
        }
        Value::Object(o) => o.get_mut(seg).ok_or_else(|| format!("no key {seg:?}")),
        _ => Err(format!("cannot step into a scalar at {seg:?}")),
    }
}

fn parent<'a>(doc: &'a mut Value, p: &'a [String]) -> Result<(&'a mut Value, &'a str), String> {
    let (last, init) = p.split_last().ok_or("the root cannot be patched")?;
    let mut cur = doc;
    for seg in init {
        cur = step(cur, seg)?;
    }
    Ok((cur, last))
}

fn set(doc: &mut Value, p: &[String], value: Value) -> Result<(), String> {
    let (par, last) = parent(doc, p)?;
    match par {
        Value::Array(a) => {
            let i: usize = last.parse().map_err(|_| format!("index {last:?}"))?;
            let slot = a
                .get_mut(i)
                .ok_or_else(|| format!("index {i} out of range"))?;
            *slot = value;
        }
        Value::Object(o) => {
            o.insert(last.to_string(), value);
        }
        _ => return Err("cannot set into a scalar".into()),
    }
    Ok(())
}

fn remove(doc: &mut Value, p: &[String]) -> Result<(), String> {
    let (par, last) = parent(doc, p)?;
    match par {
        Value::Array(a) => {
            let i: usize = last.parse().map_err(|_| format!("index {last:?}"))?;
            if i >= a.len() {
                return Err(format!("index {i} out of range"));
            }
            a.remove(i);
        }
        Value::Object(o) => {
            o.remove(last);
        }
        _ => return Err("cannot remove from a scalar".into()),
    }
    Ok(())
}

fn append(doc: &mut Value, p: &[String], value: Value) -> Result<(), String> {
    let mut cur = doc;
    for seg in &p[..p.len() - 1] {
        cur = step(cur, seg)?;
    }
    cur.as_array_mut()
        .ok_or("\"-\" appends only to an array")?
        .push(value);
    Ok(())
}

fn apply_op(doc: &mut Value, op: &Value) -> Result<(), String> {
    let kind = op
        .get("op")
        .and_then(Value::as_str)
        .ok_or("op has no .op")?;
    let p = pointer(
        op.get("path")
            .and_then(Value::as_str)
            .ok_or("op has no .path")?,
    )?;
    let value = || op.get("value").cloned().ok_or("op has no .value");
    match kind {
        "remove" => remove(doc, &p),
        "add" if p.last().is_some_and(|s| s == "-") => append(doc, &p, value()?),
        "add" | "replace" => set(doc, &p, value()?),
        other => Err(format!("unsupported op {other}")),
    }
}

/// Apply `ops` to `golden`; a patch that errors, or leaves the record
/// unchanged, is refused (`None`).
pub fn patched(golden: &Value, ops: &Value) -> Option<Value> {
    let mut doc = golden.clone();
    for op in ops.as_array()? {
        apply_op(&mut doc, op).ok()?;
    }
    (doc != *golden).then_some(doc)
}

// ---- the matrix

fn text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "null".into(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

fn str_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().map(|x| text(Some(x))).collect())
        .unwrap_or_default()
}

/// The constraint path a finding names: the first `<ns><prop>` token after the
/// prefix, without a trailing `:`, or "-" when the finding names none.
fn property(line: &str, prefix: &str, ns: &str) -> String {
    let rest = line.get(prefix.len()..).unwrap_or("");
    let Some(i) = rest.find(ns) else {
        return "-".into();
    };
    let tail = &rest[i + ns.len()..];
    let end = tail
        .find(|c: char| !(c.is_ascii_alphanumeric() || "_:.-".contains(c)))
        .unwrap_or(tail.len());
    let tok = &tail[..end];
    tok.strip_suffix(':').unwrap_or(tok).to_string()
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

struct Ctx<'a> {
    pv: &'a dyn Pv,
    spec: &'a Path,
    golden: &'a Value,
    work: &'a Path,
    node: String,
    ns: String,
}

impl Ctx<'_> {
    fn run(&self, label: &str, record: &Value) -> Judged {
        let rec = self.work.join(format!("{label}.json"));
        let bytes = serde_json::to_vec_pretty(record).unwrap_or_default();
        if std::fs::write(&rec, bytes).is_err() {
            return Judged::unmeasured();
        }
        judge(self.pv, self.spec, &rec, &self.work.join(label))
    }
}

struct KillRow {
    id: String,
    comp: String,
    tgt: String,
    foc: String,
    want: u64,
    wantp: String,
    wantm: Vec<String>,
    nm: usize,
}

impl KillRow {
    fn parse(row: &Value) -> KillRow {
        let g = |k: &str| text(row.get(k));
        let mut props = str_list(row.get("properties"));
        props.sort();
        let wantm = str_list(row.get("messages"));
        KillRow {
            id: g("id"),
            comp: g("component"),
            tgt: g("target"),
            foc: g("focus"),
            want: row
                .get("findings")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX),
            wantp: props.join(" "),
            nm: wantm.len(),
            wantm: sorted(
                wantm
                    .iter()
                    .flat_map(|m| m.lines().map(String::from))
                    .collect(),
            ),
        }
    }

    fn verdict(&self, j: &Judged, prefix: &str, ns: &str) -> (bool, String) {
        let lines = j.message_lines();
        let off = lines.iter().filter(|l| !l.starts_with(prefix)).count();
        let gotp = sorted(lines.iter().map(|l| property(l, prefix, ns)).collect()).join(" ");
        let got = j.count() as u64;
        let pure = j.exit == 1 && got == self.want && off == 0;
        let named = !self.wantp.is_empty() && gotp == self.wantp;
        let exact = self.nm as u64 == self.want && sorted(lines) == self.wantm;
        if pure && named && exact {
            return (true, "killed".into());
        }
        let v = if j.exit != 1 {
            format!("NOT-KILLED(exit {})", j.exit)
        } else if off != 0 {
            format!("IMPURE({off} finding(s) not \"{prefix}\")")
        } else if got != self.want {
            format!("WRONG-COUNT(want {})", self.want)
        } else if !named {
            format!("WRONG-PROPERTY(want [{}] got [{gotp}])", self.wantp)
        } else {
            "WRONG-MESSAGE(the findings are not the row's .messages, byte for byte)".into()
        };
        (false, v)
    }
}

const VACUOUS: &str =
    "PATCH-FAILED-OR-VACUOUS: the patch errors or leaves the golden record unchanged";

fn kill_row(cx: &Ctx, row: &Value, lines: &mut Vec<String>) -> bool {
    let r = KillRow::parse(row);
    let Some(rec) = row.get("patch").and_then(|p| patched(cx.golden, p)) else {
        lines.push(format!("{} {VACUOUS}", r.id));
        return false;
    };
    let j = cx.run(&r.id, &rec);
    let prefix = format!(
        "{}{} violates shape `{}` ({})",
        cx.node, r.foc, r.tgt, r.comp
    );
    let (ok, verdict) = r.verdict(&j, &prefix, &cx.ns);
    let foc = if r.foc.is_empty() { "." } else { &r.foc };
    lines.push(format!(
        "{} {:<16} {:<8} {:<4} exit={} findings={} {verdict}",
        r.id,
        r.comp,
        r.tgt,
        foc,
        j.exit,
        j.count()
    ));
    ok
}

fn names_nothing(ra: &str) -> bool {
    let k: String = ra
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    matches!(
        k.as_str(),
        "" | "none" | "null" | "-" | "n/a" | "tbd" | "todo"
    )
}

fn survive_row(cx: &Ctx, row: &Value, lines: &mut Vec<String>) -> bool {
    let id = text(row.get("id"));
    let ra = row
        .get("rust_assert")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if names_nothing(&ra) {
        lines.push(format!(
            "{id} NO-RUST-ASSERT: a survive row must name the Rust assert that catches it"
        ));
        return false;
    }
    let Some(rec) = row.get("patch").and_then(|p| patched(cx.golden, p)) else {
        lines.push(format!("{id} {VACUOUS}"));
        return false;
    };
    let j = cx.run(&id, &rec);
    if j.passes() {
        lines.push(format!("{id} exit=0 findings=0 survived; caught by: {ra}"));
        return true;
    }
    lines.push(format!(
        "{id} exit={} findings={} SURVIVOR-KILLED: the shapes now see what this row says they cannot",
        j.exit,
        j.count_text()
    ));
    false
}

fn read_json(p: &Path) -> Result<Value, String> {
    let b =
        std::fs::read(p).map_err(|_| format!("verify-mutants: {}: no such file", p.display()))?;
    serde_json::from_slice(&b)
        .map_err(|e| format!("verify-mutants: {}: not JSON: {e}", p.display()))
}

fn rows(table: &Value, k: &str) -> Vec<Value> {
    table
        .get(k)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn has_duplicate_id(kill: &[Value], surv: &[Value]) -> bool {
    let ids: Vec<String> = kill.iter().chain(surv).map(|r| text(r.get("id"))).collect();
    let uniq: std::collections::BTreeSet<&String> = ids.iter().collect();
    uniq.len() != ids.len()
}

/// Run the matrix in `table` against the contract in `<d18>/spec` and the
/// record `<d18>/fixtures/receipt.golden.json`, materialising every judge run
/// under `work` (which must be outside any git work tree).
pub fn verify_mutants(pv: &dyn Pv, d18: &Path, table_path: &Path, work: &Path) -> MutantsOut {
    let golden = match read_json(&d18.join("fixtures/receipt.golden.json")) {
        Ok(v) => v,
        Err(e) => return MutantsOut::refused(e),
    };
    let table = match read_json(table_path) {
        Ok(v) => v,
        Err(e) => return MutantsOut::refused(e),
    };
    let Some(node) = table.get("node").and_then(Value::as_str) else {
        return MutantsOut::refused("verify-mutants: the table has no .node".into());
    };
    let (kill, surv) = (rows(&table, "kill"), rows(&table, "survive"));
    if has_duplicate_id(&kill, &surv) {
        return MutantsOut::refused("verify-mutants: the table has a duplicate row id".into());
    }
    if std::fs::create_dir_all(work).is_err() || demo_kit::shapes::inside_git(work) {
        return MutantsOut::refused(
            "verify-mutants: the work dir is inside a git work tree (PV-ONT-014); refusing".into(),
        );
    }
    let ns = match node.rfind('/') {
        Some(i) => node[..=i].to_string(),
        None => format!("{node}/"),
    };
    let spec = d18.join("spec");
    let cx = Ctx {
        pv,
        spec: &spec,
        golden: &golden,
        work,
        node: node.to_string(),
        ns,
    };
    let mut lines = Vec::new();
    let id = cx.run("identity", &golden);
    let identity = id.passes();
    lines.push(if identity {
        "identity exit=0 findings=0 passes".into()
    } else {
        format!(
            "identity exit={} findings={} IDENTITY-FAILED: the unpatched golden record must pass",
            id.exit,
            id.count_text()
        )
    });
    let killed = kill.iter().filter(|r| kill_row(&cx, r, &mut lines)).count();
    let survived = surv
        .iter()
        .filter(|r| survive_row(&cx, r, &mut lines))
        .count();
    lines.push(format!("killed+named: {killed}/{}", kill.len()));
    lines.push(format!("survived: {survived}/{}", surv.len()));
    // An empty table is "did not look", not "nothing wrong".
    let all = identity
        && !kill.is_empty()
        && killed == kill.len()
        && !surv.is_empty()
        && survived == surv.len();
    if !all {
        lines.push("verify-mutants: FAILED".into());
    }
    MutantsOut {
        lines,
        exit: if all { 0 } else { 1 },
        killed,
        n_kill: kill.len(),
        survived,
        n_surv: surv.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn patch_subset_add_append_remove_replace() {
        let g = json!({"a": [1, 2], "b": {"c": 1}});
        let ops = json!([
            {"op": "add", "path": "/a/-", "value": 3},
            {"op": "replace", "path": "/a/0", "value": 9},
            {"op": "remove", "path": "/b/c"},
            {"op": "add", "path": "/d", "value": "x"}
        ]);
        assert_eq!(
            patched(&g, &ops).unwrap(),
            json!({"a": [9, 2, 3], "b": {}, "d": "x"})
        );
    }

    #[test]
    fn vacuous_or_failing_patches_are_refused() {
        let g = json!({"a": 1});
        assert!(patched(&g, &json!([{"op": "replace", "path": "/a", "value": 1}])).is_none());
        assert!(patched(&g, &json!([{"op": "move", "path": "/a"}])).is_none());
        assert!(patched(&g, &json!([{"op": "replace", "path": "/x/y", "value": 1}])).is_none());
        assert!(patched(&g, &json!([{"op": "add", "path": "/a/-", "value": 1}])).is_none());
    }

    #[test]
    fn property_is_the_first_namespaced_token_after_the_prefix() {
        let p = "ont:d18/d18-run-v1 violates shape `d18-run` (in)";
        let line = format!("{p}: ont:d18/server_spawns: \"2\" is not one of");
        assert_eq!(property(&line, p, "ont:d18/"), "server_spawns");
        assert_eq!(property(&format!("{p}: \"x\" is not"), p, "ont:d18/"), "-");
    }

    #[test]
    fn a_survivor_must_name_its_assert() {
        for s in ["", " None ", "NULL", "-", "N/A", "tbd", "todo"] {
            assert!(names_nothing(s), "{s:?}");
        }
        assert!(!names_nothing("d18::tests::server_pid_is_stable"));
    }
}
