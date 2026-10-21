//! W-1 gap fill (Refs #771): `xtask verify` walks EVERY demo dir of the real tree,
//! rust and bash, and goes RED on a planted docstring-less demo. The unit tests in
//! `main.rs` plant into a one-demo fixture; this one plants into a copy of the tree.

use std::path::{Path, PathBuf};
use std::process::Command;

fn demos() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Copy `from` to `to`, skipping build output.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().map(Result::unwrap) {
        let name = e.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        let dst = to.join(&name);
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &dst);
        } else {
            std::fs::copy(e.path(), &dst).unwrap();
        }
    }
}

/// `xtask verify` against the copy at `root`; `(exit code, stdout+stderr)`.
fn verify(root: &Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("verify")
        .env("CARGO_MANIFEST_DIR", root.join("xtask"))
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
}

/// Drop every line holding `Provable contract:` from `file`; returns how many went.
fn strip_contract_lines(file: &Path) -> usize {
    let src = std::fs::read_to_string(file).unwrap();
    let kept: String = src
        .lines()
        .filter(|l| !l.contains("Provable contract:"))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(file, &kept).unwrap();
    src.lines().count() - kept.lines().count()
}

fn plant(arm: &str, demo: &str, rel: &str) {
    let root = std::env::temp_dir().join(format!("w1-tree-{}-{arm}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    copy_tree(&demos(), &root);

    let (code, text) = verify(&root);
    assert_eq!(code, 0, "control: the unplanted tree must verify\n{text}");

    let dropped = strip_contract_lines(&root.join(demo).join(rel));
    assert!(dropped > 0, "{demo}/{rel} had no `Provable contract:` line to plant against");
    let (code, text) = verify(&root);
    assert_ne!(code, 0, "planted docstring-less {demo} must be RED\n{text}");
    assert!(text.contains("demo-contract-docstring-v1"), "{text}");
    assert!(text.contains(demo), "the finding must name {demo}\n{text}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_rust_demo_without_its_docstring_turns_the_tree_red() {
    plant("rust", "d01-serve-hello", "src/main.rs");
}

#[test]
fn a_bash_demo_without_its_contract_comment_turns_the_tree_red() {
    plant("bash", "d14-fanout", "fanout.sh");
}
