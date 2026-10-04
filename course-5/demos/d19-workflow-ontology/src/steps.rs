//! The pv steps D19 runs outside the shapes judge: `pv validate`, and
//! `pv extract spec` with `--check` before and after a one-byte record edit.
//!
//! Every step runs in D19's own run directory, which must be outside any git
//! work tree (M4). D18's tree is only ever read: its contract and golden
//! record are copied in, so ph3 and ph5 share no output path.

use demo_kit::shapes::ShapesOutcome;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One pv invocation as it ran.
#[derive(Debug, Clone)]
pub struct Ran {
    /// The command as shown on screen, relative to `cwd`.
    pub shown: String,
    pub exit: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Ran {
    /// stdout then stderr, as text.
    pub fn text(&self) -> String {
        let mut t = String::from_utf8_lossy(&self.stdout).into_owned();
        t.push_str(&String::from_utf8_lossy(&self.stderr));
        t
    }

    /// The exit code for a record; a signal or a spawn failure is -1.
    pub fn code(&self) -> i32 {
        self.exit.unwrap_or(-1)
    }
}

/// Run `pv args…` with `cwd` as the working directory.
pub fn pv(cwd: &Path, args: &[&str]) -> Ran {
    let shown = std::iter::once("pv")
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    match Command::new("pv").args(args).current_dir(cwd).output() {
        Ok(o) => Ran {
            shown,
            exit: o.status.code(),
            stdout: o.stdout,
            stderr: o.stderr,
        },
        Err(e) => Ran {
            shown,
            exit: None,
            stdout: Vec::new(),
            stderr: format!("spawn failed: {e}").into_bytes(),
        },
    }
}

/// The single `*.yaml` contract in `spec`; zero or several is an error.
pub fn contract(spec: &Path) -> Result<PathBuf, String> {
    let mut ys: Vec<PathBuf> = std::fs::read_dir(spec)
        .map_err(|e| format!("{}: {e}", spec.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.is_file())
        .collect();
    ys.sort();
    match ys.as_slice() {
        [y] => Ok(y.clone()),
        other => Err(format!("spec/ holds {} contracts, not one", other.len())),
    }
}

/// Copy `contract` and `record` into `<dir>/spec/<name>` and
/// `<dir>/receipt.json`, the layout `pv extract spec` resolves.
pub fn materialise(dir: &Path, contract: &Path, record: &[u8]) -> Result<(), String> {
    if demo_kit::shapes::inside_git(dir.parent().unwrap_or(dir)) {
        return Err("run dir is inside a git work tree (M4); refusing".into());
    }
    let name = contract.file_name().ok_or("contract has no file name")?;
    std::fs::create_dir_all(dir.join("spec")).map_err(|e| e.to_string())?;
    std::fs::copy(contract, dir.join("spec").join(name)).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("receipt.json"), record).map_err(|e| e.to_string())
}

/// The negative control for `pv validate`: the contract with a top-level key
/// declared twice. The key goes after a guaranteed newline, so a parse error
/// cannot pass for the refusal.
pub fn duplicate_key(contract: &[u8]) -> Vec<u8> {
    let mut neg = contract.to_vec();
    neg.extend_from_slice(b"\nfalsification_tests: []\n");
    neg
}

/// The one-byte edit: the digit after `"<key>": ` goes up by one (9 wraps to
/// 0). Returns the edited bytes, the byte offset, and the old and new byte.
pub fn one_byte_edit(record: &[u8], key: &str) -> Result<(Vec<u8>, usize, u8, u8), String> {
    let needle = format!("\"{key}\": ");
    let at = record
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())
        .ok_or_else(|| format!("no {needle:?} in the record"))?
        + needle.len();
    let old = *record.get(at).ok_or("record ends after the key")?;
    if !old.is_ascii_digit() {
        return Err(format!("{key} is not an integer"));
    }
    let new = if old == b'9' { b'0' } else { old + 1 };
    let mut out = record.to_vec();
    out[at] = new;
    Ok((out, at, old, new))
}

/// pv's own `pc_shape` for a judge call, read from the JSON it printed.
pub fn pc_shape(o: &ShapesOutcome) -> String {
    o.run_dir
        .as_ref()
        .and_then(|d| std::fs::read(d.join("out.json")).ok())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|j| j.get("pc_shape").and_then(|v| v.as_str()).map(String::from))
        .unwrap_or_else(|| "absent".into())
}

/// The planted findings sorted as `LC_ALL=C sort` would (bytewise), one per
/// line, each ending in a newline: the bytes `.expect` must equal.
pub fn sorted_lines(findings: &[String]) -> String {
    let mut f = findings.to_vec();
    f.sort();
    f.iter().map(|l| format!("{l}\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_byte_edit_changes_exactly_one_byte() {
        let r = b"{\n  \"a\": 9,\n  \"server_spawns\": 1\n}\n";
        let (e, at, old, new) = one_byte_edit(r, "server_spawns").unwrap();
        assert_eq!((old, new), (b'1', b'2'));
        assert_eq!(e.iter().zip(r).filter(|(a, b)| a != b).count(), 1);
        assert_eq!(e[at], b'2');
        let (e, _, _, new) = one_byte_edit(r, "a").unwrap();
        assert_eq!((new, e.len()), (b'0', r.len()));
        assert!(one_byte_edit(r, "missing").is_err());
        assert!(one_byte_edit(b"{\"s\": \"x\"}", "s").is_err());
    }

    #[test]
    fn sorted_lines_are_bytewise_and_newline_terminated() {
        let f = vec!["b".to_string(), "B".into(), "a".into()];
        assert_eq!(sorted_lines(&f), "B\na\nb\n");
    }

    #[test]
    fn the_negative_contract_appends_after_a_newline() {
        assert_eq!(
            duplicate_key(b"x: 1"),
            b"x: 1\nfalsification_tests: []\n".to_vec()
        );
    }
}
