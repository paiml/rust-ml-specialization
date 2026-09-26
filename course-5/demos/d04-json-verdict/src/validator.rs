//! The client-side validator D04 would run against a lane's structured
//! output, mirroring `schemas/verdict.json` by hand: stdlib + `serde_json`
//! only, no schema-validation crate, so the rules living in code are exactly
//! the rules a reader sees in the schema file.
//!
//! `{"verdict": "PASS"|"FAIL", "findings": [{"line": int, "class": string,
//! "why": string}]}`, no unknown top-level or per-finding fields.

use serde_json::Value;

/// Every way a candidate verdict object can fail the schema, in the order
/// they are found (top-level shape first, then each finding in array order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    NotAnObject,
    MissingField(&'static str),
    UnknownField(String),
    WrongVerdict(String),
    FindingsNotArray,
    FindingNotObject(usize),
    FindingMissingField(usize, &'static str),
    FindingUnknownField(usize, String),
    FindingLineNotPositiveInteger(usize),
    FindingFieldNotString(usize, &'static str),
    FindingFieldEmpty(usize, &'static str),
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::NotAnObject => write!(f, "verdict payload is not a JSON object"),
            Problem::MissingField(k) => write!(f, "missing field {k:?}"),
            Problem::UnknownField(k) => write!(f, "unknown field {k:?}"),
            Problem::WrongVerdict(v) => {
                write!(f, "verdict {v:?} is neither \"PASS\" nor \"FAIL\"")
            }
            Problem::FindingsNotArray => write!(f, "findings is not an array"),
            Problem::FindingNotObject(i) => write!(f, "findings[{i}] is not an object"),
            Problem::FindingMissingField(i, k) => write!(f, "findings[{i}] missing field {k:?}"),
            Problem::FindingUnknownField(i, k) => write!(f, "findings[{i}] unknown field {k:?}"),
            Problem::FindingLineNotPositiveInteger(i) => {
                write!(f, "findings[{i}].line is not a positive integer")
            }
            Problem::FindingFieldNotString(i, k) => {
                write!(f, "findings[{i}].{k} is not a string")
            }
            Problem::FindingFieldEmpty(i, k) => write!(f, "findings[{i}].{k} is an empty string"),
        }
    }
}

const TOP_FIELDS: [&str; 2] = ["verdict", "findings"];
const FINDING_FIELDS: [&str; 3] = ["line", "class", "why"];

/// Validate one candidate verdict object against `schemas/verdict.json`'s
/// rules. `Ok(())` means every rule held; otherwise every violation found.
pub fn validate_verdict(v: &Value) -> Result<(), Vec<Problem>> {
    let mut problems = Vec::new();
    let Some(obj) = v.as_object() else {
        return Err(vec![Problem::NotAnObject]);
    };

    for key in obj.keys() {
        if !TOP_FIELDS.contains(&key.as_str()) {
            problems.push(Problem::UnknownField(key.clone()));
        }
    }

    match obj.get("verdict") {
        None => problems.push(Problem::MissingField("verdict")),
        Some(Value::String(s)) if s == "PASS" || s == "FAIL" => {}
        Some(other) => problems.push(Problem::WrongVerdict(other.to_string())),
    }

    match obj.get("findings") {
        None => problems.push(Problem::MissingField("findings")),
        Some(Value::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                validate_finding(i, item, &mut problems);
            }
        }
        Some(_) => problems.push(Problem::FindingsNotArray),
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

fn validate_finding(i: usize, item: &Value, problems: &mut Vec<Problem>) {
    let Some(obj) = item.as_object() else {
        problems.push(Problem::FindingNotObject(i));
        return;
    };
    for key in obj.keys() {
        if !FINDING_FIELDS.contains(&key.as_str()) {
            problems.push(Problem::FindingUnknownField(i, key.clone()));
        }
    }
    match obj.get("line") {
        None => problems.push(Problem::FindingMissingField(i, "line")),
        Some(v) => {
            let ok = v.as_u64().is_some_and(|n| n >= 1) || v.as_i64().is_some_and(|n| n >= 1);
            if !ok {
                problems.push(Problem::FindingLineNotPositiveInteger(i));
            }
        }
    }
    for field in ["class", "why"] {
        match obj.get(field) {
            None => problems.push(Problem::FindingMissingField(i, field)),
            Some(Value::String(s)) if !s.is_empty() => {}
            Some(Value::String(_)) => problems.push(Problem::FindingFieldEmpty(i, field)),
            Some(_) => problems.push(Problem::FindingFieldNotString(i, field)),
        }
    }
}

/// digest-pinned copy check: the schema file on disk must hash to exactly
/// the pin in `demo.toml`, never "whatever is there now".
pub fn check_schema_copy(schema_path: &std::path::Path, pinned_sha256: &str) -> Result<(), String> {
    let found = demo_kit::sha::sha256_file(schema_path)
        .map_err(|e| format!("{}: {e}", schema_path.display()))?;
    if found == pinned_sha256 {
        Ok(())
    } else {
        Err(format!(
            "schema copy digest mismatch: pinned {pinned_sha256}, found {found}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_a_clean_pass() {
        let v = json!({"verdict": "PASS", "findings": []});
        assert_eq!(validate_verdict(&v), Ok(()));
    }

    #[test]
    fn accepts_a_fail_with_one_finding() {
        let v = json!({
            "verdict": "FAIL",
            "findings": [{"line": 29, "class": "off-by-one", "why": "0..=len over-reads by one"}]
        });
        assert_eq!(validate_verdict(&v), Ok(()));
    }

    #[test]
    fn rejects_missing_verdict() {
        let v = json!({"findings": []});
        assert_eq!(
            validate_verdict(&v),
            Err(vec![Problem::MissingField("verdict")])
        );
    }

    #[test]
    fn rejects_bad_verdict_value() {
        let v = json!({"verdict": "MAYBE", "findings": []});
        assert_eq!(
            validate_verdict(&v),
            Err(vec![Problem::WrongVerdict("\"MAYBE\"".to_string())])
        );
    }

    #[test]
    fn rejects_findings_not_array() {
        let v = json!({"verdict": "PASS", "findings": "none"});
        assert_eq!(validate_verdict(&v), Err(vec![Problem::FindingsNotArray]));
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let v = json!({"verdict": "PASS", "findings": [], "extra": 1});
        assert_eq!(
            validate_verdict(&v),
            Err(vec![Problem::UnknownField("extra".to_string())])
        );
    }

    #[test]
    fn rejects_finding_missing_line() {
        let v = json!({"verdict": "FAIL", "findings": [{"class": "x", "why": "y"}]});
        assert_eq!(
            validate_verdict(&v),
            Err(vec![Problem::FindingMissingField(0, "line")])
        );
    }

    #[test]
    fn rejects_finding_line_wrong_type() {
        let v = json!({"verdict": "FAIL", "findings": [{"line": "29", "class": "x", "why": "y"}]});
        assert_eq!(
            validate_verdict(&v),
            Err(vec![Problem::FindingLineNotPositiveInteger(0)])
        );
    }

    #[test]
    fn rejects_finding_line_zero_or_negative() {
        let v = json!({"verdict": "FAIL", "findings": [{"line": 0, "class": "x", "why": "y"}]});
        assert!(matches!(
            validate_verdict(&v),
            Err(p) if p == vec![Problem::FindingLineNotPositiveInteger(0)]
        ));
    }

    #[test]
    fn rejects_finding_unknown_field() {
        let v = json!({
            "verdict": "FAIL",
            "findings": [{"line": 1, "class": "x", "why": "y", "severity": "high"}]
        });
        assert_eq!(
            validate_verdict(&v),
            Err(vec![Problem::FindingUnknownField(
                0,
                "severity".to_string()
            )])
        );
    }

    #[test]
    fn rejects_empty_why() {
        let v = json!({"verdict": "FAIL", "findings": [{"line": 1, "class": "x", "why": ""}]});
        assert_eq!(
            validate_verdict(&v),
            Err(vec![Problem::FindingFieldEmpty(0, "why")])
        );
    }

    #[test]
    fn reports_every_problem_not_just_the_first() {
        let v = json!({"verdict": "NOPE", "findings": [{"class": "x"}]});
        let errs = validate_verdict(&v).unwrap_err();
        assert!(errs.contains(&Problem::WrongVerdict("\"NOPE\"".to_string())));
        assert!(errs.contains(&Problem::FindingMissingField(0, "line")));
        assert!(errs.contains(&Problem::FindingMissingField(0, "why")));
        assert_eq!(errs.len(), 3);
    }

    /// schema-copy-digest-v1 falsifier: mutate the schema copy on disk ->
    /// the digest check goes RED.
    #[test]
    fn mutated_schema_copy_fails_the_digest_check() {
        let real = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas/verdict.json");
        let pinned = super_pinned_sha256();
        check_schema_copy(&real, &pinned).expect("the committed copy must match its own pin");

        let tmp = std::env::temp_dir().join(format!(
            "d04-mutated-schema-{}-{}.json",
            std::process::id(),
            "mutation-falsifier"
        ));
        let mut bytes = std::fs::read(&real).unwrap();
        // Flip one byte in the schema text: still valid-ish JSON-shaped
        // bytes are not required, only that the digest moves.
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xff;
        std::fs::write(&tmp, &bytes).unwrap();

        let result = check_schema_copy(&tmp, &pinned);
        assert!(result.is_err(), "a mutated schema copy must fail its pin");
        std::fs::remove_file(&tmp).unwrap();
    }

    /// Reads the pin straight out of demo.toml so this test cannot drift
    /// from the manifest the harness actually checks.
    fn super_pinned_sha256() -> String {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let m = demo_kit::DemoManifest::load(&dir.join("demo.toml")).unwrap();
        m.schema_copy_sha256
            .expect("demo.toml pins schema_copy_sha256")
    }
}
