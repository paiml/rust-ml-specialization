//! `cargo run -p xtask -- verify` — every demo's `demo.toml` parses, pins
//! exactly, and its `src/main.rs` carries a provable contract.
//! `cargo run -p xtask -- card <demo>` — print the demo's recording card.

mod card;
mod lint;

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
    for (tool, spec, used) in [
        ("apr", &manifest.apr, manifest.uses_apr()),
        ("agy", &manifest.agy, manifest.uses_agy()),
    ] {
        if used {
            if let Err(bad) = pin::parse_exact(spec) {
                findings.push(format!("demo-pin-v1: {tool} pin {bad:?} is not exact"));
            }
        }
    }
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
    match std::fs::read_to_string(dir.join("src/main.rs")) {
        Ok(src) => findings.extend(lint::lint_contract(&src).err().unwrap_or_default()),
        Err(e) => findings.push(format!("src/main.rs: {e}")),
    }
    findings
}

fn verify(root: &Path) -> ExitCode {
    let dirs = demo_dirs(root);
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
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = demos_root();
    match args.first().map(String::as_str) {
        Some("verify") => verify(&root),
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
            eprintln!("usage: xtask verify | xtask card <demo-dir>");
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
