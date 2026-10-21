//! `xtask autofile` (#792 S3): one aprender issue per (demo id, dev-apr sha7)
//! when a demo is Red. The same Red again is a comment on that issue, never a
//! second issue; NotRun and Green file nothing. Dry-run prints the actions;
//! only `--apply` calls `gh`.
//!
//! Input is a receipts tree (`<root>/<id>/<host>/<apr>/<run>.json`). The newest
//! run per demo id decides. Host names never leave the receipt: the issue
//! carries the demo id, apr sha7, run id and failed assertions only.

use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// What one demo's newest run says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub id: String,
    pub run_id: String,
    pub apr_sha7: String,
    pub verdict: String,
    pub failed: Vec<String>,
}

/// An open issue already in the tracker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Existing {
    pub number: u64,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Create { title: String, body: String },
    Comment { number: u64, body: String },
}

pub fn title_for(id: &str, sha7: &str) -> String {
    format!("rfml5 demo {id} Red on dev apr {sha7}")
}

fn sha7(r: &Value) -> String {
    ["apr_sha256", "apr_tarball_sha"]
        .iter()
        .find_map(|k| r.get(*k).and_then(Value::as_str))
        .map_or_else(|| "unknown".to_string(), |s| s.chars().take(7).collect())
}

fn outcome_of(r: &Value) -> Option<Outcome> {
    let v = r.get("verdict")?;
    Some(Outcome {
        id: r.get("id")?.as_str()?.to_string(),
        run_id: r.get("run_id")?.as_str()?.to_string(),
        apr_sha7: sha7(r),
        verdict: v.get("verdict")?.as_str()?.to_string(),
        failed: v
            .get("failed")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn json_files(root: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            json_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

/// Newest run per demo id under `root`. Unparseable files are errors, not skips.
pub fn load_outcomes(root: &Path) -> Result<Vec<Outcome>, String> {
    let mut files = Vec::new();
    json_files(root, &mut files);
    files.sort();
    let mut newest: std::collections::BTreeMap<String, Outcome> = Default::default();
    for f in files {
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))?;
        let o = outcome_of(&v).ok_or_else(|| format!("{}: not a receipt", f.display()))?;
        if newest.get(&o.id).is_none_or(|n| n.run_id < o.run_id) {
            newest.insert(o.id.clone(), o);
        }
    }
    Ok(newest.into_values().collect())
}

fn body_for(o: &Outcome) -> String {
    format!(
        "Nightly demo `{}` was Red on dev apr `{}` (run `{}`).\n\nFailed assertions: {}\n\nFiled by `xtask autofile` (paiml/rust-ml-specialization #792).",
        o.id,
        o.apr_sha7,
        o.run_id,
        if o.failed.is_empty() { "(none named)".to_string() } else { o.failed.join(", ") }
    )
}

/// Pure planner: Red -> create, or a comment when an open issue has the title;
/// Green and NotRun -> nothing.
pub fn plan(outcomes: &[Outcome], existing: &[Existing]) -> Vec<Action> {
    outcomes
        .iter()
        .filter(|o| o.verdict == "Red")
        .map(|o| {
            let title = title_for(&o.id, &o.apr_sha7);
            match existing.iter().find(|e| e.title == title) {
                Some(e) => Action::Comment {
                    number: e.number,
                    body: body_for(o),
                },
                None => Action::Create {
                    title,
                    body: body_for(o),
                },
            }
        })
        .collect()
}

pub fn parse_existing(json: &str) -> Result<Vec<Existing>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    v.as_array()
        .ok_or("existing issues must be a JSON array")?
        .iter()
        .map(|i| {
            Ok(Existing {
                number: i
                    .get("number")
                    .and_then(Value::as_u64)
                    .ok_or("issue without number")?,
                title: i
                    .get("title")
                    .and_then(Value::as_str)
                    .ok_or("issue without title")?
                    .to_string(),
            })
        })
        .collect()
}

/// Open issues of `repo`, via `gh`.
pub fn gh_existing(repo: &str) -> Result<Vec<Existing>, String> {
    let out = Command::new("gh")
        .args([
            "issue", "list", "-R", repo, "--state", "open", "--limit", "200", "--search",
        ])
        .arg("rfml5 demo in:title")
        .args(["--json", "number,title"])
        .output()
        .map_err(|e| format!("gh not runnable: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "gh issue list: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    parse_existing(&String::from_utf8_lossy(&out.stdout))
}

/// Execute one action through `gh`.
pub fn gh_apply(repo: &str, a: &Action) -> Result<(), String> {
    let mut c = Command::new("gh");
    match a {
        Action::Create { title, body } => {
            c.args([
                "issue", "create", "-R", repo, "--title", title, "--body", body,
            ]);
        }
        Action::Comment { number, body } => {
            c.args([
                "issue",
                "comment",
                &number.to_string(),
                "-R",
                repo,
                "--body",
                body,
            ]);
        }
    }
    let out = c.output().map_err(|e| format!("gh not runnable: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(id: &str, verdict: &str, sha: &str) -> Outcome {
        Outcome {
            id: id.into(),
            run_id: "r1".into(),
            apr_sha7: sha.into(),
            verdict: verdict.into(),
            failed: vec!["exit".into()],
        }
    }

    #[test]
    fn red_creates_one_issue_titled_with_demo_id_and_sha7() {
        let a = plan(&[o("d04-json-verdict", "Red", "d13f869")], &[]);
        assert_eq!(a.len(), 1);
        let Action::Create { title, body } = &a[0] else {
            panic!("{a:?}")
        };
        assert!(
            title.contains("d04-json-verdict") && title.contains("d13f869"),
            "{title}"
        );
        assert!(body.contains("exit"));
    }

    #[test]
    fn the_same_red_again_is_a_comment_not_a_second_issue() {
        let ex = [Existing {
            number: 77,
            title: title_for("d04-json-verdict", "d13f869"),
        }];
        let a = plan(&[o("d04-json-verdict", "Red", "d13f869")], &ex);
        assert!(
            matches!(&a[..], [Action::Comment { number: 77, .. }]),
            "{a:?}"
        );
    }

    #[test]
    fn a_new_apr_sha_is_a_new_issue() {
        let ex = [Existing {
            number: 77,
            title: title_for("d04-json-verdict", "d13f869"),
        }];
        let a = plan(&[o("d04-json-verdict", "Red", "aaaaaaa")], &ex);
        assert!(matches!(&a[..], [Action::Create { .. }]), "{a:?}");
    }

    #[test]
    fn notrun_and_green_file_nothing() {
        let a = plan(
            &[o("d01", "NotRun", "d13f869"), o("d02", "Green", "d13f869")],
            &[],
        );
        assert!(a.is_empty(), "{a:?}");
    }

    #[test]
    fn receipts_tree_newest_run_decides_and_host_never_leaks() {
        let root = std::env::temp_dir().join(format!("xtask-autofile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("d04/secret-host/0.69.3");
        std::fs::create_dir_all(&dir).unwrap();
        let rec = |run: &str, verdict: &str| {
            format!(
                r#"{{"id":"d04","run_id":"{run}","host":"secret-host","apr_tarball_sha":"d13f86934aaaa","verdict":{verdict}}}"#
            )
        };
        std::fs::write(
            dir.join("a.json"),
            rec("2026-10-20", r#"{"verdict":"Red","failed":["exit"]}"#),
        )
        .unwrap();
        std::fs::write(
            dir.join("b.json"),
            rec("2026-10-21", r#"{"verdict":"Green"}"#),
        )
        .unwrap();
        let outs = load_outcomes(&root).unwrap();
        assert_eq!(outs.len(), 1);
        assert_eq!(
            outs[0].verdict, "Green",
            "the newer Green run must win over the older Red"
        );
        std::fs::write(
            dir.join("c.json"),
            rec("2026-10-22", r#"{"verdict":"Red","failed":["exit"]}"#),
        )
        .unwrap();
        let outs = load_outcomes(&root).unwrap();
        let a = plan(&outs, &[]);
        let Action::Create { title, body } = &a[0] else {
            panic!()
        };
        assert!(title.contains("d13f869"));
        assert!(!body.contains("secret-host") && !title.contains("secret-host"));
        std::fs::write(dir.join("bad.json"), "nope").unwrap();
        assert!(
            load_outcomes(&root).is_err(),
            "an unparseable receipt is an error, not a skip"
        );
    }
}
