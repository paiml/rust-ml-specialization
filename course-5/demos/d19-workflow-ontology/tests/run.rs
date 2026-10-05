//! The D19 bin end to end, against the real pv. pv is part of the demo: a
//! machine without it fails these tests, it never skips them.
//!
//! The falsifiers run the bin on a scratch copy of D19 and D18's frozen
//! inputs (outside any git tree), with `CARGO_MANIFEST_DIR` pointed at the
//! copy, and break one input each. Every one must turn the run Red: a run
//! that stays Green with a broken table proves nothing.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_d19-workflow-ontology");
const HERE: &str = env!("CARGO_MANIFEST_DIR");

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "d19-test-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(manifest_dir: &Path, receipts: Option<&Path>) -> Output {
    let mut c = Command::new(BIN);
    c.env_remove("RFML5_BEATS")
        .env_remove("RFML5_RECEIPTS")
        .env("CARGO_MANIFEST_DIR", manifest_dir);
    if let Some(r) = receipts {
        c.env("RFML5_RECEIPTS", r);
    }
    c.output().expect("the d19 bin runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let dest = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_tree(&p, &dest);
        } else {
            std::fs::copy(&p, &dest).unwrap();
        }
    }
}

/// `<root>/a/b/c/{d19,d18}`: the copy sits three levels down, as in the repo,
/// so the receipts root resolves outside it.
fn copy_demo(root: &Path) -> PathBuf {
    let demos = root.join("a/b/c");
    let here = Path::new(HERE);
    let d19 = demos.join("d19-workflow-ontology");
    std::fs::create_dir_all(&d19).unwrap();
    std::fs::copy(here.join("demo.toml"), d19.join("demo.toml")).unwrap();
    copy_tree(&here.join("spec"), &d19.join("spec"));
    copy_tree(&here.join("fixtures"), &d19.join("fixtures"));
    let d18_src = here.join("../d18-two-agents-one-server");
    let d18 = demos.join("d18-two-agents-one-server");
    copy_tree(&d18_src.join("spec"), &d18.join("spec"));
    copy_tree(&d18_src.join("fixtures"), &d18.join("fixtures"));
    d19
}

fn edit_table(d19: &Path, f: impl FnOnce(&mut Value)) {
    let p = d19.join("fixtures/mutants.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    f(&mut v);
    std::fs::write(&p, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}

/// Run a sabotaged copy; it must be Red (exit 1) with no contract line.
fn assert_red(tag: &str, sabotage: impl FnOnce(&Path)) -> String {
    let root = scratch(tag);
    let d19 = copy_demo(&root.join("tree"));
    sabotage(&d19);
    let o = run(&d19, Some(&root.join("receipts")));
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(1), "{tag}: not Red\n{out}");
    assert!(!out.contains("contract: d19-run-v1 OK"), "{tag}: {out}");
    out
}

#[test]
fn the_real_run_is_green_and_prints_the_contract_line() {
    let r = scratch("green");
    let o = run(Path::new(HERE), Some(&r));
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    for want in [
        "== receipt.planted.expect: byte for byte",
        "killed 18/18",
        "components 7/7",
        "survived 6/6",
        "exit 1  reject: spec/contracts.nt differs",
        "W3C conformance cases: 19/19 on 27 of 27 judge calls",
        "cross-checked against .expect and the mutant table: agrees",
        "verdict: Green",
    ] {
        assert!(out.contains(want), "missing {want:?}\n{out}");
    }
    assert!(out.trim_end().ends_with("contract: d19-run-v1 OK"), "{out}");
}

#[test]
fn an_unset_receipts_root_is_not_run() {
    let o = run(Path::new(HERE), None);
    assert_eq!(o.status.code(), Some(2));
    assert!(stdout(&o).contains("EnvUnset(RFML5_RECEIPTS)"));
}

#[test]
fn an_untouched_copy_is_green() {
    let root = scratch("copy");
    let d19 = copy_demo(&root.join("tree"));
    let o = run(&d19, Some(&root.join("receipts")));
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
}

#[test]
fn a_kill_row_expecting_another_message_is_red() {
    let out = assert_red("message", |d| {
        edit_table(d, |v| {
            let m = v["kill"][0]["messages"][0].as_str().unwrap().to_string();
            v["kill"][0]["messages"][0] = Value::from(format!("{m}."));
        })
    });
    assert!(out.contains("WRONG-MESSAGE"), "{out}");
    assert!(out.contains("FAIL: m01 killed"), "{out}");
}

#[test]
fn a_survivor_with_no_owner_is_red() {
    let out = assert_red("owner", |d| {
        edit_table(d, |v| v["survive"][0]["rust_assert"] = Value::from("none"))
    });
    assert!(out.contains("NO-RUST-ASSERT"), "{out}");
    assert!(out.contains("FAIL: s01 survives"), "{out}");
}

#[test]
fn a_survivor_the_shapes_can_see_is_red() {
    let out = assert_red("stale", |d| {
        edit_table(d, |v| {
            v["survive"][0]["patch"] = v["kill"][0]["patch"].clone()
        })
    });
    assert!(out.contains("SURVIVOR-KILLED"), "{out}");
    assert!(out.contains("FAIL: s01 survives"), "{out}");
}

#[test]
fn a_vacuous_patch_is_red_not_skipped() {
    let out = assert_red("vacuous", |d| {
        edit_table(d, |v| {
            let path = "/server_spawns";
            v["kill"][0]["patch"] =
                serde_json::json!([{"op": "replace", "path": path, "value": 1}]);
        })
    });
    assert!(out.contains("vacuous"), "{out}");
    assert!(out.contains("FAIL: m01: patch"), "{out}");
}

#[test]
fn an_expect_file_one_line_short_is_red() {
    let out = assert_red("expect", |d| {
        let p = d.join("../d18-two-agents-one-server/fixtures/receipt.planted.expect");
        let t = std::fs::read_to_string(&p).unwrap();
        let keep: Vec<&str> = t.lines().skip(1).collect();
        std::fs::write(&p, format!("{}\n", keep.join("\n"))).unwrap();
    });
    assert!(out.contains("receipt.planted.expect: DIFFERS"), "{out}");
    assert!(
        out.contains("FAIL: planted is refused with exactly .expect"),
        "{out}"
    );
    assert!(out.contains("planted_findings 5 != 4 lines"), "{out}");
}
