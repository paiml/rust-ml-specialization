//! D19's own run record, and the cross-check that keeps it honest.
//!
//! The record is a summary, not a self-judgement: every field is copied from
//! pv's output, and D19's shapes can only see the record. So the program also
//! checks the record against its sources (spec §5.2): the planted finding
//! count against D18's `.expect`, and `killed`, `survived` and
//! `components_killed` against the mutant table.

use crate::matrix::Table;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub pv_version: String,
    pub subject: String,
    pub validate_exit: i32,
    pub extract_exit: i32,
    pub extract_check_exit: i32,
    pub extract_drift_exit: i32,
    pub golden_exit: i32,
    /// pv's `pc_shape` on the golden judge.
    pub golden_pc: String,
    pub w3c_passed: u64,
    pub w3c_total: u64,
    pub planted_exit: i32,
    pub planted_findings: usize,
    pub planted_matches_expect: bool,
    pub killed: Vec<String>,
    pub survived: Vec<String>,
    pub components_killed: Vec<String>,
}

pub const DEMO: &str = "d19-workflow-ontology";

impl Record {
    /// The run record as the D19 contract's entity (`receipt.json`).
    pub fn to_json(&self) -> Value {
        json!({
            "demo": DEMO,
            "pv_version": self.pv_version,
            "subject": self.subject,
            "validate_exit": self.validate_exit,
            "extract_exit": self.extract_exit,
            "extract_check_exit": self.extract_check_exit,
            "extract_drift_exit": self.extract_drift_exit,
            "golden_exit": self.golden_exit,
            "golden_pc": self.golden_pc,
            "w3c_passed": self.w3c_passed,
            "w3c_total": self.w3c_total,
            "planted_exit": self.planted_exit,
            "planted_findings": self.planted_findings,
            "planted_matches_expect": self.planted_matches_expect,
            "killed": self.killed,
            "survived": self.survived,
            "components_killed": self.components_killed,
        })
    }

    /// Every way the record disagrees with its sources; empty means it agrees.
    pub fn cross_check(&self, table: &Table, expect: &str) -> Vec<String> {
        let mut bad = Vec::new();
        let lines = expect.lines().count();
        if self.planted_findings != lines {
            bad.push(format!(
                "planted_findings {} != {lines} lines in D18's .expect",
                self.planted_findings
            ));
        }
        if self.killed != table.kill_ids() {
            bad.push("killed != the kill rows' ids".into());
        }
        if self.survived != table.survive_ids() {
            bad.push("survived != the survive rows' ids".into());
        }
        if self.components_killed != table.components() {
            bad.push("components_killed != the kill rows' components".into());
        }
        if self.subject != table.subject {
            bad.push(format!(
                "subject {} != the table's {}",
                self.subject, table.subject
            ));
        }
        bad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        let k = |id: &str, c: &str| {
            json!({"id":id,"component":c,"target":"t","focus":"","findings":1,
                   "properties":["p"],"patch":[],"messages":["x"]})
        };
        Table::parse(&json!({
            "subject":"d18-run-v1","node":"ont:d18/d18-run-v1",
            "kill":[k("m01","in"), k("m02","closed"), k("m03","in")],
            "survive":[{"id":"s01","patch":[],"rust_assert":"a","why":"w"}]
        }))
        .unwrap()
    }

    fn good() -> Record {
        Record {
            pv_version: "0.70.1".into(),
            subject: "d18-run-v1".into(),
            validate_exit: 0,
            extract_exit: 0,
            extract_check_exit: 0,
            extract_drift_exit: 1,
            golden_exit: 0,
            golden_pc: "fired".into(),
            w3c_passed: 19,
            w3c_total: 19,
            planted_exit: 1,
            planted_findings: 2,
            planted_matches_expect: true,
            killed: vec!["m01".into(), "m02".into(), "m03".into()],
            survived: vec!["s01".into()],
            components_killed: vec!["closed".into(), "in".into()],
        }
    }

    #[test]
    fn an_agreeing_record_passes_the_cross_check() {
        assert_eq!(good().cross_check(&table(), "a\nb\n"), Vec::<String>::new());
    }

    /// Each field the shapes cannot relate to its source is caught here.
    #[test]
    fn each_disagreement_is_named() {
        let t = table();
        let mut r = good();
        r.planted_findings = 3;
        assert!(r.cross_check(&t, "a\nb\n")[0].contains("planted_findings"));
        let mut r = good();
        r.killed.pop();
        assert_eq!(
            r.cross_check(&t, "a\nb\n"),
            ["killed != the kill rows' ids"]
        );
        let mut r = good();
        r.survived.clear();
        assert_eq!(
            r.cross_check(&t, "a\nb\n"),
            ["survived != the survive rows' ids"]
        );
        let mut r = good();
        r.components_killed.push("maxCount".into());
        assert_eq!(
            r.cross_check(&t, "a\nb\n"),
            ["components_killed != the kill rows' components"]
        );
        let mut r = good();
        r.subject = "d20-run-v1".into();
        assert!(r.cross_check(&t, "a\nb\n")[0].starts_with("subject"));
    }

    #[test]
    fn the_record_carries_exactly_the_contract_fields() {
        let v = good().to_json();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 17);
        assert_eq!(v["demo"], DEMO);
    }
}
