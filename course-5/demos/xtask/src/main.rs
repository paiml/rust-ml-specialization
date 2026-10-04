//! `cargo run -p xtask -- verify [--only <id>]` — every demo's `demo.toml` parses
//! and pins exactly (apr may pin a series), its `src/main.rs` carries a
//! provable contract, every demo with a `spec/` passes the pv shapes proofs
//! (`proofs`, `mutants`), and the workspace passes the confinement lints.
//! `cargo run -p xtask -- card <demo>` — print the demo's recording card.
//! `cargo run -p xtask -- promote-fixture <gate> [--check|--replace]` — copy an
//! entry gate's measurement to its committed fixture.

mod card;
mod confine;
mod lint;
mod mutants;
mod promote;
mod proofs;
mod shell;

use demo_kit::{pin, DemoManifest};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The PAIML ceiling for one video (MEGA-001 §4.1): 6 minutes.
const MAX_DURATION_S: u32 = 360;

/// Runtime `CARGO_MANIFEST_DIR` (set by `cargo run`) wins over the compile-time
/// one, so a binary reused from a shared target dir verifies THIS checkout.
fn demos_root() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_string());
    Path::new(&manifest)
        .parent()
        .expect("xtask lives in the demos workspace")
        .to_path_buf()
}

/// Every workspace member directory that carries a `demo.toml`.
pub fn demo_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    dirs.retain(|d| d.join("demo.toml").is_file());
    dirs.sort();
    dirs
}

/// demo-pin-v1: `apr` is exact or a series (`0.70.*`); `agy`, `pv`,
/// `antigravity` and `xdotool` are exact; an app pin carries its asar sha256.
fn pin_findings(m: &DemoManifest) -> Vec<String> {
    let mut findings = Vec::new();
    if m.uses_apr() {
        if let Err(bad) = pin::parse_exact_or_series(&m.apr) {
            findings.push(format!(
                "demo-pin-v1: apr pin {bad:?} is neither exact nor a series"
            ));
        }
    }
    for (tool, spec, used) in [
        ("agy", &m.agy, m.uses_agy()),
        ("pv", &m.pv, m.uses_pv()),
        ("antigravity", &m.antigravity, m.uses_antigravity()),
        ("xdotool", &m.xdotool, m.uses_xdotool()),
    ] {
        if used {
            if let Err(bad) = pin::parse_exact(spec) {
                findings.push(format!("demo-pin-v1: {tool} pin {bad:?} is not exact"));
            }
        }
    }
    let hex64 = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    match (&m.antigravity_asar_sha256, m.uses_antigravity()) {
        (Some(s), true) if !hex64(s) => findings.push(format!(
            "demo-pin-v1: antigravity_asar_sha256 {s:?} is not 64 hex"
        )),
        (None, true) => findings
            .push("demo-pin-v1: antigravity is pinned without antigravity_asar_sha256".into()),
        _ => {}
    }
    findings
}

/// All findings for one demo directory; empty means it passes.
pub fn verify_demo(dir: &Path) -> Vec<String> {
    let mut findings = Vec::new();
    let manifest = match DemoManifest::load(&dir.join("demo.toml")) {
        Ok(m) => m,
        Err(e) => return vec![e],
    };
    let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if manifest.id != dir_name {
        findings.push(format!(
            "id {:?} does not match directory {dir_name:?}",
            manifest.id
        ));
    }
    findings.extend(pin_findings(&manifest));
    if let Some(m) = &manifest.model {
        if m.sha256.len() != 64 || !m.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            findings.push(format!("model sha256 {:?} is not 64 hex", m.sha256));
        }
    }
    if manifest.assert.is_empty() {
        findings.push("no [assert] entries: a demo that checks nothing cannot be Green".into());
    }
    if manifest.record.target_duration_s > MAX_DURATION_S {
        findings.push(format!(
            "record.target_duration_s {} > {MAX_DURATION_S}",
            manifest.record.target_duration_s
        ));
    }
    // A bash demo has no src/main.rs; its script is named by a `bash X.sh` step.
    let script = manifest
        .step
        .iter()
        .find_map(|s| s.cmd.strip_prefix("bash ").map(str::trim));
    if let Some(script) = script.filter(|_| !dir.join("src/main.rs").is_file()) {
        match std::fs::read_to_string(dir.join(script)) {
            Ok(src) => findings.extend(lint::lint_shell_contract(&src).err().unwrap_or_default()),
            Err(e) => findings.push(format!("{script}: {e}")),
        }
    } else {
        match std::fs::read_to_string(dir.join("src/main.rs")) {
            Ok(src) => findings.extend(lint::lint_contract(&src).err().unwrap_or_default()),
            Err(e) => findings.push(format!("src/main.rs: {e}")),
        }
    }
    // every bash script in the demo goes through bashrs + shellcheck
    let mut scripts: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    scripts.retain(|p| p.extension().is_some_and(|e| e == "sh"));
    scripts.sort();
    for script in scripts {
        findings.extend(shell::lint_script(&script));
    }
    findings
}

/// `verify [--only <id>]`: every demo (or the one named), then the workspace
/// confinement lints, which hold for every phase.
fn verify(root: &Path, only: Option<&str>) -> ExitCode {
    let mut dirs = demo_dirs(root);
    if let Some(id) = only {
        dirs.retain(|d| d.file_name().is_some_and(|n| n == id));
        if dirs.is_empty() {
            println!("FAIL --only {id}: no such demo");
            return ExitCode::FAILURE;
        }
    }
    let mut failed = 0;
    for d in &dirs {
        let f = verify_demo(d);
        let name = d.file_name().unwrap_or_default().to_string_lossy();
        if f.is_empty() {
            println!("PASS {name}");
        } else {
            failed += 1;
            for line in f {
                println!("FAIL {name}: {line}");
            }
        }
    }
    println!("verify: {} demos, {failed} failing", dirs.len());
    if dirs.is_empty() {
        println!("FAIL no demos found: a gate over nothing is not a gate");
        return ExitCode::FAILURE;
    }
    let shapes = shapes_arm(root, &dirs, only);
    let lints = confine::lint_workspace(root);
    for l in &lints {
        println!("FAIL confine: {l}");
    }
    println!("confine: {} findings", lints.len());
    match (failed == 0 && lints.is_empty(), shapes) {
        (true, Shapes::Held) => ExitCode::SUCCESS,
        (true, Shapes::Refused) => ExitCode::from(2),
        _ => ExitCode::FAILURE,
    }
}

enum Shapes {
    Held,
    Failed,
    /// Not measured (pv absent or not the pinned version).
    Refused,
}

/// The pv shapes proofs over the demos that carry a contract. A demo with a
/// `spec/` the arm does not know is a failure: it would never be judged.
fn shapes_arm(root: &Path, dirs: &[PathBuf], only: Option<&str>) -> Shapes {
    let mut held = true;
    for d in dirs.iter().filter(|d| d.join("spec").is_dir()) {
        let name = d.file_name().unwrap_or_default().to_string_lossy();
        if !proofs::DEMOS.contains(&name.as_ref()) {
            println!("FAIL {name}: has spec/ but no shapes proofs");
            held = false;
        }
    }
    let pv: Box<dyn proofs::Pv> = match proofs::RealPv::find() {
        Some(p) => Box::new(p),
        None => Box::new(proofs::RealPv(PathBuf::from("pv"))),
    };
    let Some(report) = proofs::run(pv.as_ref(), root, only, &std::env::temp_dir()) else {
        return if held { Shapes::Held } else { Shapes::Failed };
    };
    for l in report.lines() {
        println!("{l}");
    }
    match report {
        proofs::Report::Refused(_) if held => Shapes::Refused,
        r if r.passed() && held => Shapes::Held,
        _ => Shapes::Failed,
    }
}

fn promote_fixture(root: &Path, args: &[String]) -> ExitCode {
    let usage = "usage: xtask promote-fixture <e3-probe|measure-stop-latency> [--check|--replace]";
    let Some((gate, mode)) = parse_promote_args(args) else {
        eprintln!("{usage}");
        return ExitCode::from(2);
    };
    let receipts = match std::env::var_os("RFML5_RECEIPTS") {
        Some(r) => PathBuf::from(r),
        None if mode == promote::Mode::Check => PathBuf::new(),
        None => {
            eprintln!("promote-fixture: RFML5_RECEIPTS is not set");
            return ExitCode::FAILURE;
        }
    };
    match promote::promote(&receipts, root, gate, mode) {
        Ok(msg) if mode == promote::Mode::Check => {
            println!("promote-fixture --check: {msg}");
            ExitCode::SUCCESS
        }
        Ok(sha) => {
            println!("promote-fixture: wrote {} sha256 {sha}", gate.destination());
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("FAIL promote-fixture: {e}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = demos_root();
    match args.first().map(String::as_str) {
        Some("verify") => match (args.get(1).map(String::as_str), args.get(2)) {
            (None, _) => verify(&root, None),
            (Some("--only"), Some(id)) => verify(&root, Some(id)),
            _ => {
                eprintln!("usage: xtask verify [--only <id>]");
                ExitCode::from(2)
            }
        },
        Some("promote-fixture") => promote_fixture(&root, &args[1..]),
        Some("card") => {
            let Some(name) = args.get(1) else {
                eprintln!("usage: xtask card <demo-dir>");
                return ExitCode::from(2);
            };
            match DemoManifest::load(&root.join(name).join("demo.toml")) {
                Ok(m) => {
                    print!("{}", card::render(&m));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: xtask verify [--only <id>] | xtask card <demo-dir> | xtask promote-fixture <gate> [--check|--replace]");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN_OK: &str = "//! Demo.\n//!\n//! Provable contract: reduce-deterministic-v1 — same inputs, same bytes.\n\nfn main() {\n    let a = 1;\n    assert_eq!(a, 1);\n    println!(\"contract: reduce-deterministic-v1 OK\");\n}\n";

    fn demo_fixture(name: &str, toml_body: &str, main_rs: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
        let d = root.join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("demo.toml"), toml_body).unwrap();
        std::fs::write(d.join("src/main.rs"), main_rs).unwrap();
        d
    }

    fn toml_for(id: &str, apr: &str) -> String {
        format!(
            "id = \"{id}\"\ntitle = \"t\"\nlesson = \"rfml5/2.2\"\napr = \"{apr}\"\nhost_class = \"any\"\n[assert]\nexit = 0\n[record]\ntarget_duration_s = 300\nresolution = \"1920x1080\"\n"
        )
    }

    #[test]
    fn good_demo_passes() {
        let d = demo_fixture("d99-ok", &toml_for("d99-ok", "none"), MAIN_OK);
        assert_eq!(verify_demo(&d), Vec::<String>::new());
    }

    /// demo-contract-docstring-v1: strip the docstring → RED.
    #[test]
    fn demo_contract_docstring_v1_strip_is_red() {
        let stripped: String = MAIN_OK
            .lines()
            .filter(|l| !l.starts_with("//!"))
            .map(|l| format!("{l}\n"))
            .collect();
        let d = demo_fixture("d98-stripped", &toml_for("d98-stripped", "none"), &stripped);
        let f = verify_demo(&d);
        assert!(f.iter().any(|l| l.contains("Provable contract")), "{f:?}");
    }

    /// demo-pin-v1 (floor arm, static): verify refuses a floor pin.
    #[test]
    fn demo_pin_v1_floor_in_toml_is_red() {
        let d = demo_fixture("d97-floor", &toml_for("d97-floor", ">=0.69.3"), MAIN_OK);
        let f = verify_demo(&d);
        assert!(f.iter().any(|l| l.contains("demo-pin-v1")), "{f:?}");
    }

    #[test]
    fn no_assertions_is_red() {
        let body = toml_for("d96-noassert", "none").replace("[assert]\nexit = 0\n", "");
        let d = demo_fixture("d96-noassert", &body, MAIN_OK);
        assert!(!verify_demo(&d).is_empty());
    }

    #[test]
    fn over_six_minutes_is_red() {
        let body = toml_for("d95-long", "none").replace("= 300", "= 361");
        let d = demo_fixture("d95-long", &body, MAIN_OK);
        assert!(verify_demo(&d)
            .iter()
            .any(|l| l.contains("target_duration_s")));
    }
}

/// Falsifier for D17's reduction rules: `quorum.sh` runs against a stub `agy` on PATH
/// (bash fixture, no network) and each arm must produce the stated quorum verdicts.
#[cfg(test)]
mod d17_falsifier {
    use std::process::Command;

    /// (stub modes, homogeneous quorum, heterogeneous quorum)
    const ARMS: &[(&str, &str, &str)] = &[
        ("", "PASS", "PASS"),
        ("claude-sonnet-4-6=fail", "PASS", "FAIL"),
        ("gemini-3.8-flash-high=fail", "FAIL", "FAIL"),
        ("gpt-oss-120b-medium=hang", "PASS", "NotRun"),
        ("gpt-oss-120b-medium=garbage", "PASS", "NotRun"),
        ("gpt-oss-120b-medium=vanish", "PASS", "NotRun"),
        ("gemini-3.8-flash-high=hang", "NotRun", "NotRun"),
        (
            "claude-sonnet-4-6=fail,gpt-oss-120b-medium=hang",
            "PASS",
            "FAIL",
        ),
    ];

    fn run(modes: &str) -> String {
        let demo = crate::demos_root().join("d17-agy-quorum");
        let bin = std::env::temp_dir().join(format!("d17-stub-{}", std::process::id()));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::copy(demo.join("fixtures/stub-agy.sh"), bin.join("agy")).unwrap();
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let out = Command::new("bash")
            .arg(demo.join("quorum.sh"))
            .env("PATH", path)
            .env("STUB_MODES", modes)
            .env("LANE_TIMEOUT_S", "1")
            .env("LANE_GRACE_S", "0")
            .env("LOAD_MAX", "1000000")
            .output()
            .unwrap();
        std::fs::remove_dir_all(&bin).ok();
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    #[test]
    fn reduction_rules_hold_against_a_stub_agy() {
        for (modes, homo, hetero) in ARMS {
            let out = run(modes);
            assert!(
                out.contains("contract: quorum-one-fail-blocks-v1 + notrun-never-green-v1 OK"),
                "[{modes}] {out}"
            );
            let want = format!("quorum: homogeneous={homo} heterogeneous={hetero}");
            assert!(out.contains(&want), "[{modes}] want `{want}`, got:\n{out}");
        }
    }
}

/// One gate name and at most one mode flag, in either order: the spec writes
/// `promote-fixture --check e3-probe`, the usage line the other way round.
fn parse_promote_args(args: &[String]) -> Option<(promote::Gate, promote::Mode)> {
    let (mut gate, mut mode) = (None, None);
    for a in args {
        let m = match a.as_str() {
            "--check" => Some(promote::Mode::Check),
            "--replace" => Some(promote::Mode::Replace),
            _ => None,
        };
        match (m, &gate, &mode) {
            (Some(m), _, None) => mode = Some(m),
            (None, None, _) => gate = Some(promote::Gate::parse(a)?),
            _ => return None,
        }
    }
    Some((gate?, mode.unwrap_or(promote::Mode::Promote)))
}

#[cfg(test)]
mod promote_args_tests {
    use super::*;

    fn p(a: &[&str]) -> Option<(promote::Gate, promote::Mode)> {
        parse_promote_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn flag_before_or_after_the_gate() {
        assert_eq!(
            p(&["--check", "e3-probe"]).map(|x| x.1),
            Some(promote::Mode::Check)
        );
        assert_eq!(
            p(&["e3-probe", "--check"]).map(|x| x.1),
            Some(promote::Mode::Check)
        );
        assert_eq!(p(&["e3-probe"]).map(|x| x.1), Some(promote::Mode::Promote));
        assert!(p(&["--check"]).is_none());
        assert!(p(&["--check", "--replace", "e3-probe"]).is_none());
        assert!(p(&["e3-probe", "e3-probe"]).is_none());
        assert!(p(&["nope", "--check"]).is_none());
    }
}
