//! The mutant table (`fixtures/mutants.json`) and how one judged row is read.
//!
//! - A **kill** row is killed only *purely*: pv exits 1, with exactly the
//!   row's finding count; every finding begins
//!   ``<node><focus> violates shape `<target>` (<component>)``; the findings
//!   name exactly the row's `properties`; and the sorted findings equal the
//!   row's `messages` byte for byte.
//! - A **survive** row must pass: exit 0, Pass, no findings. It is a defect
//!   the shapes cannot see, so it must also name the Rust assert that does.

use demo_kit::shapes::ShapesOutcome;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct KillRow {
    pub id: String,
    pub component: String,
    pub target: String,
    /// `""` for the run node, `.q1` … for an item node.
    pub focus: String,
    pub findings: usize,
    pub properties: Vec<String>,
    pub patch: Value,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurviveRow {
    pub id: String,
    pub patch: Value,
    pub rust_assert: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub subject: String,
    /// The run node, e.g. `ont:d18/d18-run-v1`.
    pub node: String,
    pub kill: Vec<KillRow>,
    pub survive: Vec<SurviveRow>,
}

fn s(row: &Value, k: &str) -> Result<String, String> {
    row.get(k)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("row {}: no string .{k}", show_id(row)))
}

fn strs(row: &Value, k: &str) -> Result<Vec<String>, String> {
    row.get(k)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("row {}: no array .{k}", show_id(row)))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("row {}: .{k} holds a non-string", show_id(row)))
        })
        .collect()
}

fn show_id(row: &Value) -> String {
    row.get("id")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string()
}

fn patch(row: &Value) -> Result<Value, String> {
    match row.get("patch") {
        Some(p @ Value::Array(_)) => Ok(p.clone()),
        _ => Err(format!("row {}: no array .patch", show_id(row))),
    }
}

fn kill_row(row: &Value) -> Result<KillRow, String> {
    let findings = row
        .get("findings")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("row {}: no integer .findings", show_id(row)))?;
    Ok(KillRow {
        id: s(row, "id")?,
        component: s(row, "component")?,
        target: s(row, "target")?,
        focus: s(row, "focus")?,
        findings: usize::try_from(findings).map_err(|e| e.to_string())?,
        properties: strs(row, "properties")?,
        patch: patch(row)?,
        messages: strs(row, "messages")?,
    })
}

fn survive_row(row: &Value) -> Result<SurviveRow, String> {
    Ok(SurviveRow {
        id: s(row, "id")?,
        patch: patch(row)?,
        rust_assert: s(row, "rust_assert")?,
        why: s(row, "why")?,
    })
}

fn rows<T>(table: &Value, k: &str, f: fn(&Value) -> Result<T, String>) -> Result<Vec<T>, String> {
    let a = table
        .get(k)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("the table has no array .{k}"))?;
    if a.is_empty() {
        // An empty table is "did not look", never "nothing wrong".
        return Err(format!("the table's .{k} is empty"));
    }
    a.iter().map(f).collect()
}

impl Table {
    /// Parse the table strictly: a missing field, an empty side or a
    /// duplicate id is an error, never a default.
    pub fn parse(v: &Value) -> Result<Table, String> {
        let t = Table {
            subject: s(v, "subject")?,
            node: s(v, "node")?,
            kill: rows(v, "kill", kill_row)?,
            survive: rows(v, "survive", survive_row)?,
        };
        let ids: Vec<&str> = t
            .kill
            .iter()
            .map(|r| r.id.as_str())
            .chain(t.survive.iter().map(|r| r.id.as_str()))
            .collect();
        if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err("the table has a duplicate row id".into());
        }
        Ok(t)
    }

    /// The namespace findings name properties in: `ont:d18/` for
    /// `ont:d18/d18-run-v1`.
    pub fn ns(&self) -> String {
        match self.node.rfind('/') {
            Some(i) => self.node[..=i].to_string(),
            None => format!("{}/", self.node),
        }
    }

    pub fn kill_ids(&self) -> Vec<String> {
        self.kill.iter().map(|r| r.id.clone()).collect()
    }

    pub fn survive_ids(&self) -> Vec<String> {
        self.survive.iter().map(|r| r.id.clone()).collect()
    }

    /// The set of components the kill rows target, sorted.
    pub fn components(&self) -> Vec<String> {
        let set: BTreeSet<&str> = self.kill.iter().map(|r| r.component.as_str()).collect();
        set.into_iter().map(str::to_string).collect()
    }
}

/// The constraint path a finding names: the first `<ns><prop>` token after
/// `prefix`, without a trailing `:`, or `-` when it names none (pv 0.70.1's
/// `datatype` findings name only the value).
pub fn property(line: &str, prefix: &str, ns: &str) -> String {
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

/// How one kill row came out. Only `Killed` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kill {
    Killed,
    NotKilled(Option<i32>),
    /// Findings that do not start with the row's node, shape and component.
    Impure(usize),
    WrongCount(usize),
    WrongProperty(String),
    WrongMessage,
}

impl std::fmt::Display for Kill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kill::Killed => write!(f, "killed"),
            Kill::NotKilled(e) => write!(f, "NOT-KILLED(exit {})", code(*e)),
            Kill::Impure(n) => write!(f, "IMPURE({n} finding(s) off the row's prefix)"),
            Kill::WrongCount(n) => write!(f, "WRONG-COUNT({n})"),
            Kill::WrongProperty(got) => write!(f, "WRONG-PROPERTY({got})"),
            Kill::WrongMessage => write!(f, "WRONG-MESSAGE"),
        }
    }
}

pub fn code(e: Option<i32>) -> String {
    e.map_or_else(|| "none".into(), |c| c.to_string())
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

impl KillRow {
    /// ``<node><focus> violates shape `<target>` (<component>)``.
    pub fn prefix(&self, node: &str) -> String {
        format!(
            "{node}{} violates shape `{}` ({})",
            self.focus, self.target, self.component
        )
    }

    /// Read a judged mutant against this row.
    pub fn judge(&self, o: &ShapesOutcome, node: &str, ns: &str) -> Kill {
        if o.exit != Some(1) {
            return Kill::NotKilled(o.exit);
        }
        let prefix = self.prefix(node);
        let off = o
            .findings
            .iter()
            .filter(|l| !l.starts_with(&prefix))
            .count();
        if off != 0 {
            return Kill::Impure(off);
        }
        if o.findings.len() != self.findings {
            return Kill::WrongCount(o.findings.len());
        }
        let got = sorted(
            o.findings
                .iter()
                .map(|l| property(l, &prefix, ns))
                .collect(),
        );
        if self.properties.is_empty() || got != sorted(self.properties.clone()) {
            return Kill::WrongProperty(got.join(","));
        }
        if self.messages.len() != self.findings
            || sorted(o.findings.clone()) != sorted(self.messages.clone())
        {
            return Kill::WrongMessage;
        }
        Kill::Killed
    }
}

/// A `rust_assert` that names nothing does not own the blind spot.
pub fn names_nothing(ra: &str) -> bool {
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

/// How one survive row came out. Only `Survived` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Survive {
    Survived,
    NoRustAssert,
    /// The shapes now see what this row says they cannot: the list is stale.
    Killed(Option<i32>, usize),
}

impl std::fmt::Display for Survive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Survive::Survived => write!(f, "survived"),
            Survive::NoRustAssert => write!(f, "NO-RUST-ASSERT"),
            Survive::Killed(e, n) => write!(f, "SURVIVOR-KILLED(exit {}, {n} findings)", code(*e)),
        }
    }
}

impl SurviveRow {
    pub fn judge(&self, o: &ShapesOutcome) -> Survive {
        if names_nothing(&self.rust_assert) {
            return Survive::NoRustAssert;
        }
        if o.exit == Some(0) && o.findings.is_empty() && o.is_green() {
            Survive::Survived
        } else {
            Survive::Killed(o.exit, o.findings.len())
        }
    }

    /// The first clause of `why`, for one screen line.
    pub fn short_why(&self) -> &str {
        let w = self.why.as_str();
        let cut = w.find(": ").or_else(|| w.find(", ")).unwrap_or(w.len());
        &w[..cut]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use demo_kit::verdict::Verdict;
    use serde_json::json;

    fn row() -> KillRow {
        KillRow {
            id: "m01".into(),
            component: "in".into(),
            target: "d18-run".into(),
            focus: String::new(),
            findings: 1,
            properties: vec!["server_spawns".into()],
            patch: json!([]),
            messages: vec![MSG.into()],
        }
    }

    const NODE: &str = "ont:d18/d18-run-v1";
    const MSG: &str = "ont:d18/d18-run-v1 violates shape `d18-run` (in): ont:d18/server_spawns: \"2\"^^xsd:integer is not one of [\"1\"^^xsd:integer]";

    fn out(exit: i32, findings: &[&str]) -> ShapesOutcome {
        let mut o = ShapesOutcome::from_verdict(if exit == 0 {
            Verdict::Green
        } else {
            Verdict::Red { failed: vec![] }
        });
        o.exit = Some(exit);
        o.findings = findings.iter().map(|s| s.to_string()).collect();
        o
    }

    #[test]
    fn a_pure_kill_is_killed() {
        assert_eq!(row().judge(&out(1, &[MSG]), NODE, "ont:d18/"), Kill::Killed);
    }

    /// Every way a kill can be impure is reported for its own reason.
    #[test]
    fn impure_kills_are_named() {
        let r = row();
        let j = |o| r.judge(&o, NODE, "ont:d18/");
        assert_eq!(j(out(0, &[])), Kill::NotKilled(Some(0)));
        let other = MSG.replace("(in)", "(maxCount)");
        assert_eq!(j(out(1, &[MSG, &other])), Kill::Impure(1));
        assert_eq!(j(out(1, &[MSG, MSG])), Kill::WrongCount(2));
        let wrong_prop = MSG.replace("server_spawns", "server_pid_end");
        assert_eq!(
            j(out(1, &[&wrong_prop])),
            Kill::WrongProperty("server_pid_end".into())
        );
        let wrong_value = MSG.replace("\"2\"", "\"3\"");
        assert_eq!(j(out(1, &[&wrong_value])), Kill::WrongMessage);
        let item = MSG.replace(NODE, &format!("{NODE}.q1"));
        assert_eq!(j(out(1, &[&item])), Kill::Impure(1));
    }

    #[test]
    fn property_is_the_first_namespaced_token_after_the_prefix() {
        let p = "ont:d18/d18-run-v1 violates shape `d18-run` (in)";
        assert_eq!(property(MSG, p, "ont:d18/"), "server_spawns");
        assert_eq!(property(&format!("{p}: \"x\" is not"), p, "ont:d18/"), "-");
        let closed = format!("{p}: ont:d18/rdf:type: is not allowed");
        assert_eq!(property(&closed, p, "ont:d18/"), "rdf:type");
    }

    #[test]
    fn a_survivor_must_pass_and_name_its_assert() {
        let mut r = SurviveRow {
            id: "s01".into(),
            patch: json!([]),
            rust_assert: "the program recomputes sha256".into(),
            why: "two equal digests: more".into(),
        };
        assert_eq!(r.judge(&out(0, &[])), Survive::Survived);
        assert_eq!(r.judge(&out(1, &[MSG])), Survive::Killed(Some(1), 1));
        assert_eq!(r.short_why(), "two equal digests");
        for s in ["", " None ", "NULL", "-", "N/A", "tbd", "todo"] {
            r.rust_assert = s.into();
            assert_eq!(r.judge(&out(0, &[])), Survive::NoRustAssert, "{s:?}");
        }
    }

    #[test]
    fn the_table_is_parsed_strictly() {
        let k = json!({"id":"m01","component":"in","target":"d18-run","focus":"","findings":1,
                       "properties":["p"],"patch":[],"messages":["x"]});
        let s = json!({"id":"s01","patch":[],"rust_assert":"a","why":"w"});
        let ok = json!({"subject":"d18-run-v1","node":NODE,"kill":[k],"survive":[s]});
        let t = Table::parse(&ok).unwrap();
        assert_eq!(
            (t.ns(), t.components()),
            ("ont:d18/".into(), vec!["in".into()])
        );
        let mut dup = ok.clone();
        dup["survive"][0]["id"] = json!("m01");
        assert!(Table::parse(&dup).unwrap_err().contains("duplicate"));
        let mut empty = ok.clone();
        empty["kill"] = json!([]);
        assert!(Table::parse(&empty).unwrap_err().contains("empty"));
        let mut missing = ok;
        missing["kill"][0]
            .as_object_mut()
            .unwrap()
            .remove("findings");
        assert!(Table::parse(&missing).unwrap_err().contains(".findings"));
    }
}
