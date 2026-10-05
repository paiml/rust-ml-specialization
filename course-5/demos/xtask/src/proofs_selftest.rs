//! The shapes arm's self-test: the twenty-nine sabotages of the ph1
//! `spec-proofs.sh --self-test`, one test each.
//!
//! Each test copies the four demos' `spec/` and `fixtures/` to a scratch tree
//! outside git, plants one defect, and requires the arm to refuse EXACTLY as the
//! row states: exactly the named set of failing checks, no more and no fewer
//! (or the named refusal), and, where a row names one, the reason in the output.
//! A file sabotage that leaves the copy unchanged fails the test. A pv sabotage
//! wraps the real pv, which is how a pv that misreports itself is planted.
//! pv must be on `PATH`: a missing tool fails, never skips.

use super::*;
use serde_json::{json, Map};
use std::collections::BTreeMap;

fn real() -> RealPv {
    RealPv::find().expect("pv on PATH: a missing tool fails, never skips")
}

fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let p = e.unwrap().path();
        let to = dst.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_tree(&p, &to);
        } else {
            std::fs::copy(&p, &to).unwrap();
        }
    }
}

fn tree(dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd {
        let p = e.unwrap().path();
        if p.is_dir() {
            tree(&p, out);
        } else {
            out.insert(p.clone(), std::fs::read(&p).unwrap());
        }
    }
}

fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut m = BTreeMap::new();
    tree(dir, &mut m);
    m
}

/// `(scratch, copy)`: the four demos' spec and fixtures copied outside git.
fn setup(name: &str) -> (PathBuf, PathBuf) {
    let st = fresh_dir(&std::env::temp_dir(), &format!("xtask-selftest-{name}")).unwrap();
    assert!(!demo_kit::shapes::inside_git(&st), "scratch inside git");
    let demos = crate::demos_root();
    let c = st.join("copy");
    for d in DEMOS {
        for sub in ["spec", "fixtures"] {
            copy_tree(&demos.join(d).join(sub), &c.join(d).join(sub));
        }
    }
    (st, c)
}

fn edit(path: &Path, f: impl FnOnce(&mut Value)) {
    let mut v: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let before = v.clone();
    f(&mut v);
    assert_ne!(v, before, "{}: the edit changed nothing", path.display());
    std::fs::write(path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}

fn table(c: &Path) -> PathBuf {
    c.join(D19).join("fixtures/mutants.json")
}

fn row<'a>(t: &'a mut Value, k: &str, id: &str) -> &'a mut Value {
    t[k].as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["id"] == id)
        .unwrap()
}

fn each(suffix: &str) -> Vec<String> {
    DEMOS.iter().map(|d| format!("{d} {suffix}")).collect()
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

struct Shim {
    real: RealPv,
    f: fn(&[OsString], &RealPv) -> PvRun,
}

impl Pv for Shim {
    fn call(&self, a: &[OsString]) -> PvRun {
        (self.f)(a, &self.real)
    }
}

fn is(a: &[OsString], verb: &str) -> bool {
    a.first().is_some_and(|x| x == verb)
}

fn ran(exit: i32, stdout: &str) -> PvRun {
    PvRun {
        exit: Some(exit),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

/// pv lint runs for real; when its verdict is Pass, `f` rewrites its JSON.
fn lint_rewrite(a: &[OsString], real: &RealPv, f: fn(&mut Map<String, Value>)) -> PvRun {
    let mut r = real.call(a);
    if !is(a, "lint") {
        return r;
    }
    if let Ok(Value::Object(mut o)) = serde_json::from_slice::<Value>(&r.stdout) {
        if o.get("verdict") == Some(&json!("Pass")) {
            f(&mut o);
            r.stdout = serde_json::to_vec_pretty(&Value::Object(o)).unwrap();
        }
    }
    r
}

/// pv lint runs for real; a reject (exit 1) is reported as `to`.
fn lint_exit(a: &[OsString], real: &RealPv, to: i32) -> PvRun {
    let mut r = real.call(a);
    if is(a, "lint") && r.exit == Some(1) {
        r.exit = Some(to);
    }
    r
}

enum Plant {
    Files(fn(&Path)),
    Pv(fn(&[OsString], &RealPv) -> PvRun),
}

enum Want {
    Fails(Vec<String>),
    Refusal(&'static str),
}

fn expect(name: &str, plant: Plant, want: Want, reason: &str) {
    let (st, c) = setup(name);
    let pv: Box<dyn Pv> = match plant {
        Plant::Files(f) => {
            let before = snapshot(&c);
            f(&c);
            assert_ne!(snapshot(&c), before, "{name}: SABOTAGE-NOT-APPLIED");
            Box::new(real())
        }
        Plant::Pv(f) => Box::new(Shim { real: real(), f }),
    };
    let rep = run(pv.as_ref(), &c, None, &st.join("tmp")).expect("the arm runs");
    let out = rep.lines().join("\n");
    assert!(!rep.passed(), "{name}: PASSED-WRONGLY\n{out}");
    match (&rep, want) {
        (Report::Refused(got), Want::Refusal(w)) => assert_eq!(got, w, "{name}"),
        (Report::Ran { .. }, Want::Fails(w)) => {
            assert_eq!(rep.fail_set(), sorted(w), "{name}: REFUSED-WRONGLY\n{out}")
        }
        _ => panic!("{name}: REFUSED-WRONGLY\n{out}"),
    }
    assert!(
        out.contains(reason),
        "{name}: reason [{reason}] ABSENT\n{out}"
    );
    let _ = std::fs::remove_dir_all(&st);
}

fn fails(v: Vec<String>) -> Want {
    Want::Fails(v)
}

fn with_mutants(mut v: Vec<String>) -> Want {
    v.push(mutants_check());
    Want::Fails(v)
}

#[test]
fn baseline_copy_passes_with_exactly_17_checks() {
    let (st, c) = setup("baseline");
    let rep = run(&real(), &c, None, &st.join("tmp")).unwrap();
    let out = rep.lines().join("\n");
    assert!(rep.passed(), "BASELINE-FAILED\n{out}");
    match rep {
        Report::Ran { checks, .. } => assert_eq!(checks.len(), EXPECTED_CHECKS),
        Report::Refused(r) => panic!("{r}"),
    }
    let _ = std::fs::remove_dir_all(&st);
}

#[test]
fn only_narrows_the_arm_and_skips_demos_without_a_contract() {
    let (st, c) = setup("only");
    let rep = run(&real(), &c, Some("d20-agy-app-fanout"), &st.join("tmp")).unwrap();
    assert!(rep.passed(), "{}", rep.lines().join("\n"));
    let d19 = run(&real(), &c, Some(D19), &st.join("tmp")).unwrap();
    assert!(d19.passed(), "{}", d19.lines().join("\n"));
    assert!(run(&real(), &c, Some("d01-anything"), &st.join("tmp")).is_none());
    let _ = std::fs::remove_dir_all(&st);
}

#[test]
fn app_evidence_is_the_evidence_methods_the_driver_sent() {
    let r = json!({"app_evidence": ["DOM.getBoxModel"],
                   "cdp_methods": [{"method": "DOM.getBoxModel", "count": 1},
                                   {"method": "Page.captureScreenshot", "count": 1}]});
    assert_eq!(app_evidence_why(&r), "");
    let r2 = json!({"app_evidence": ["DOM.getBoxModel", "Input.insertText"],
                    "cdp_methods": [{"method": "DOM.getBoxModel", "count": 1}]});
    assert!(app_evidence_why(&r2).starts_with("app_evidence"));
    assert_eq!(app_evidence_why(&json!({})), "app_evidence absent");
}

// ---- the twenty-nine sabotages

#[test]
fn swap_d18_golden_planted() {
    expect(
        "swap-d18-golden-planted",
        Plant::Files(|c| {
            let f = c.join(D18).join("fixtures");
            std::fs::rename(f.join("receipt.golden.json"), f.join("swap.json")).unwrap();
            std::fs::rename(
                f.join("receipt.planted.json"),
                f.join("receipt.golden.json"),
            )
            .unwrap();
            std::fs::rename(f.join("swap.json"), f.join("receipt.planted.json")).unwrap();
        }),
        with_mutants(names(&[
            "d18-two-agents-one-server golden Green",
            "d18-two-agents-one-server planted == .expect",
        ])),
        "",
    );
}

#[test]
fn d18_expect_empty() {
    expect(
        "d18-expect-empty",
        Plant::Files(|c| {
            std::fs::write(c.join(D18).join("fixtures/receipt.planted.expect"), "").unwrap()
        }),
        fails(names(&[
            "d18-two-agents-one-server planted == .expect",
            "d19 planted_findings == d18 .expect lines",
        ])),
        "",
    );
}

#[test]
fn d18_checker_files_unpinned() {
    expect(
        "d18-checker-files-unpinned",
        Plant::Files(|c| {
            let f = c.join(D18).join("spec/d18-run-v1.yaml");
            let s = std::fs::read_to_string(&f).unwrap().replace(
                "{path: d18:checker_files, minCount: 1, in: [out/verdicts.json]}",
                "{path: d18:checker_files, minCount: 1}",
            );
            std::fs::write(&f, s).unwrap();
        }),
        with_mutants(names(&["d18-two-agents-one-server planted == .expect"])),
        "",
    );
}

#[test]
fn d20_expect_extra_line() {
    expect(
        "d20-expect-extra-line",
        Plant::Files(|c| {
            let f = c.join("d20-agy-app-fanout/fixtures/receipt.planted.expect");
            let mut b = std::fs::read(&f).unwrap();
            b.extend_from_slice(b"an extra line\n");
            std::fs::write(&f, b).unwrap();
        }),
        fails(names(&["d20-agy-app-fanout planted == .expect"])),
        "",
    );
}

#[test]
fn d20_golden_no_insert_text() {
    expect(
        "d20-golden-no-insertText",
        Plant::Files(|c| {
            edit(
                &c.join("d20-agy-app-fanout/fixtures/receipt.golden.json"),
                |v| {
                    v["app_evidence"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|m| m != "Input.insertText")
                },
            )
        }),
        fails(names(&["d20-agy-app-fanout golden Green"])),
        "",
    );
}

#[test]
fn d21_golden_sends_f12() {
    expect(
        "d21-golden-sends-F12",
        Plant::Files(|c| {
            edit(
                &c.join("d21-agy-app-fanin/fixtures/receipt.golden.json"),
                |v| v["keys_sent"].as_array_mut().unwrap().push(json!("F12")),
            )
        }),
        fails(names(&["d21-agy-app-fanin golden Green"])),
        "",
    );
}

#[test]
fn d21_spec_deleted() {
    expect(
        "d21-spec-deleted",
        Plant::Files(|c| {
            std::fs::remove_file(c.join("d21-agy-app-fanin/spec/d21-run-v1.yaml")).unwrap()
        }),
        fails(names(&[
            "d21-agy-app-fanin golden Green",
            "d21-agy-app-fanin planted == .expect",
            "d21-agy-app-fanin validate",
        ])),
        "spec/ must hold exactly one contract",
    );
}

#[test]
fn empty_demos_dir() {
    let mut want = each("present");
    want.extend(names(&[
        "d19 components_killed == mutants.json components",
        "d19 killed == mutants.json kill ids",
        "d19 planted_findings == d18 .expect lines",
        "d19 survived == mutants.json survive ids",
    ]));
    expect(
        "empty-demos-dir",
        Plant::Files(|c| {
            for d in DEMOS {
                std::fs::remove_dir_all(c.join(d)).unwrap();
            }
        }),
        with_mutants(want),
        "",
    );
}

#[test]
fn m01_moved_to_survive() {
    expect(
        "m01-moved-to-survive",
        Plant::Files(|c| {
            edit(&table(c), |t| {
                let m = row(t, "kill", "m01").clone();
                t["survive"].as_array_mut().unwrap().push(json!({
                    "id": m["id"], "why": "sabotage", "rust_assert": "sabotage", "patch": m["patch"]
                }));
                t["kill"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|r| r["id"] != "m01");
            })
        }),
        with_mutants(names(&[
            "d19 killed == mutants.json kill ids",
            "d19 survived == mutants.json survive ids",
        ])),
        "",
    );
}

#[test]
fn m03_vacuous_patch() {
    expect(
        "m03-vacuous-patch",
        Plant::Files(|c| {
            edit(
                &table(c),
                |t| {
                    row(t, "kill", "m03")["patch"] =
                        json!([{"op": "replace", "path": "/q1/decision", "value": "accept"}])
                },
            )
        }),
        with_mutants(vec![]),
        "m03 PATCH-FAILED-OR-VACUOUS",
    );
}

#[test]
fn m07_wrong_component() {
    expect(
        "m07-wrong-component",
        Plant::Files(|c| {
            edit(&table(c), |t| {
                row(t, "kill", "m07")["component"] = json!("in")
            })
        }),
        with_mutants(vec![]),
        "IMPURE(1 finding(s) not",
    );
}

#[test]
fn m07_wrong_property() {
    expect(
        "m07-wrong-property",
        Plant::Files(|c| {
            edit(&table(c), |t| {
                row(t, "kill", "m07")["properties"] = json!(["server_pid_start"])
            })
        }),
        with_mutants(vec![]),
        "WRONG-PROPERTY(want [server_pid_start] got [server_pid_end])",
    );
}

#[test]
fn m07_duplicate_row() {
    expect(
        "m07-duplicate-row",
        Plant::Files(|c| {
            edit(&table(c), |t| {
                let m = row(t, "kill", "m07").clone();
                t["kill"].as_array_mut().unwrap().push(m);
            })
        }),
        with_mutants(names(&["d19 killed == mutants.json kill ids"])),
        "has a duplicate row id",
    );
}

#[test]
fn no_survivors() {
    expect(
        "no-survivors",
        Plant::Files(|c| edit(&table(c), |t| t["survive"] = json!([]))),
        with_mutants(names(&["d19 survived == mutants.json survive ids"])),
        "",
    );
}

#[test]
fn pv_version_0_70_2() {
    expect(
        "pv-version-0.70.2",
        Plant::Pv(|a, real| {
            if is(a, "--version") {
                ran(0, "pv 0.70.2\n")
            } else {
                real.call(a)
            }
        }),
        Want::Refusal("shapes: pv 0.70.2, pinned 0.70.1; refusing (not measured)"),
        "",
    );
}

#[test]
fn golden_pc_shape_not_fired() {
    expect(
        "golden-pc-shape-not-fired",
        Plant::Pv(|a, r| {
            lint_rewrite(a, r, |o| {
                o.insert("pc_shape".into(), json!("not-fired"));
            })
        }),
        fails(each("golden Green")),
        "pc_shape not-fired",
    );
}

#[test]
fn golden_not_armed() {
    expect(
        "golden-not-armed",
        Plant::Pv(|a, r| {
            lint_rewrite(a, r, |o| {
                o.insert("not_armed_shapes".into(), json!(["sabotage"]));
            })
        }),
        fails(each("golden Green")),
        "not armed",
    );
}

#[test]
fn golden_unarmed_violation() {
    expect(
        "golden-unarmed-violation",
        Plant::Pv(|a, r| {
            lint_rewrite(a, r, |o| {
                o.insert("unarmed_violations".into(), json!(1));
            })
        }),
        fails(each("golden Green")),
        "unarmed 1",
    );
}

#[test]
fn golden_w3c_short() {
    expect(
        "golden-w3c-short",
        Plant::Pv(|a, r| {
            lint_rewrite(a, r, |o| {
                let n = o["w3c_cases_n"].as_u64().unwrap();
                o.insert("w3c_cases_passed".into(), json!(n - 1));
            })
        }),
        fails(each("golden Green")),
        "w3c 18/19",
    );
}

#[test]
fn golden_focus_count_off() {
    expect(
        "golden-focus-count-off",
        Plant::Pv(|a, r| {
            lint_rewrite(a, r, |o| {
                let n = o["focus_nodes_n"].as_u64().unwrap();
                o.insert("focus_nodes_n".into(), json!(n + 1));
            })
        }),
        fails(each("golden Green")),
        "focus_nodes_n 6 want 5",
    );
}

#[test]
fn pv_lint_pass_with_finding() {
    expect(
        "pv-lint-pass-with-finding",
        Plant::Pv(|a, r| {
            lint_rewrite(a, r, |o| {
                o["findings"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"message": "sabotage"}));
            })
        }),
        with_mutants(each("golden Green")),
        "IDENTITY-FAILED",
    );
}

#[test]
fn pv_lint_reject_exits_2() {
    expect(
        "pv-lint-reject-exits-2",
        Plant::Pv(|a, r| lint_exit(a, r, 2)),
        with_mutants(each("planted == .expect")),
        "NOT-KILLED(exit 2)",
    );
}

#[test]
fn pv_lint_reject_exits_0() {
    expect(
        "pv-lint-reject-exits-0",
        Plant::Pv(|a, r| lint_exit(a, r, 0)),
        with_mutants(each("planted == .expect")),
        "NOT-KILLED(exit 0)",
    );
}

#[test]
fn pv_validate_always_0() {
    expect(
        "pv-validate-always-0",
        Plant::Pv(|a, real| {
            if is(a, "validate") {
                ran(0, "")
            } else {
                real.call(a)
            }
        }),
        fails(each("validate")),
        "accepted a contract with a duplicate key",
    );
}

#[test]
fn pv_validate_refuses_unparsed() {
    expect(
        "pv-validate-refuses-unparsed",
        Plant::Pv(|a, real| {
            let negative = a
                .get(1)
                .is_some_and(|p| p.to_string_lossy().contains("/negative-"));
            if is(a, "validate") && negative {
                ran(1, "mapping values are not allowed in this context\n")
            } else {
                real.call(a)
            }
        }),
        fails(each("validate")),
        "but not for a duplicate key",
    );
}

#[test]
fn second_yaml_in_spec() {
    expect(
        "second-yaml-in-spec",
        Plant::Files(|c| {
            let s = c.join("d20-agy-app-fanout/spec");
            std::fs::copy(s.join("d20-run-v1.yaml"), s.join("d20-run-v2.yaml")).unwrap();
        }),
        fails(names(&[
            "d20-agy-app-fanout golden Green",
            "d20-agy-app-fanout planted == .expect",
            "d20-agy-app-fanout validate",
        ])),
        "must hold exactly one contract",
    );
}

#[test]
fn s05_rust_assert_none() {
    expect(
        "s05-rust-assert-none",
        Plant::Files(|c| {
            edit(&table(c), |t| {
                row(t, "survive", "s05")["rust_assert"] = json!("None")
            })
        }),
        with_mutants(vec![]),
        "s05 NO-RUST-ASSERT",
    );
}

#[test]
fn m07_wrong_message() {
    expect(
        "m07-wrong-message",
        Plant::Files(|c| {
            edit(&table(c), |t| {
                let m = &mut row(t, "kill", "m07")["messages"][0];
                *m = json!(m.as_str().unwrap().replacen("41388", "41389", 1));
            })
        }),
        with_mutants(vec![]),
        "WRONG-MESSAGE(the findings are not",
    );
}

#[test]
fn pv_lint_prints_nothing() {
    let mut want = each("golden Green");
    want.extend(each("planted == .expect"));
    expect(
        "pv-lint-prints-nothing",
        Plant::Pv(|a, real| {
            if is(a, "lint") {
                ran(0, "")
            } else {
                real.call(a)
            }
        }),
        with_mutants(want),
        "",
    );
}
