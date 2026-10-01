//! External lint for bash demos: `bashrs lint` (any error fails) and
//! `shellcheck -S warning` (any finding fails). A missing tool is a failure,
//! never a skip: a gate that cannot run is not a gate.

use std::path::Path;
use std::process::Command;

/// Findings for one script; empty means it passes both linters.
pub fn lint_script(path: &Path) -> Vec<String> {
    let mut findings = Vec::new();
    let name = path.display();
    match Command::new("bashrs").arg("lint").arg(path).output() {
        Err(e) => findings.push(format!("{name}: bashrs not runnable: {e}")),
        Ok(out) => {
            let text = strip_ansi(&String::from_utf8_lossy(&out.stdout));
            match bashrs_errors(&text) {
                None => findings.push(format!("{name}: bashrs gave no summary line")),
                Some(0) => {}
                Some(n) => {
                    findings.push(format!("{name}: bashrs lint: {n} error(s)"));
                    findings.extend(
                        text.lines()
                            .filter(|l| l.contains("[error]"))
                            .map(|l| format!("  {}", l.trim())),
                    );
                }
            }
        }
    }
    match Command::new("shellcheck")
        .args(["-S", "warning"])
        .arg(path)
        .output()
    {
        Err(e) => findings.push(format!("{name}: shellcheck not runnable: {e}")),
        Ok(out) if !out.status.success() => {
            findings.push(format!("{name}: shellcheck -S warning failed"));
            findings.extend(
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .take(20)
                    .map(|l| format!("  {l}")),
            );
        }
        Ok(_) => {}
    }
    findings
}

/// `N` from bashrs's `Summary: N error(s), ...` line; 0 for its clean message.
fn bashrs_errors(text: &str) -> Option<u32> {
    if text.contains("No issues found") {
        return Some(0);
    }
    let line = text.lines().find(|l| l.starts_with("Summary:"))?;
    line.trim_start_matches("Summary:")
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(name: &str, body: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("xtask-shell-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(format!("{name}.sh"));
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn clean_script_passes() {
        let p = script(
            "clean",
            "#!/usr/bin/env bash\nset -euo pipefail\necho \"hello\"\n",
        );
        assert_eq!(lint_script(&p), Vec::<String>::new());
    }

    #[test]
    fn planted_bashrs_error_is_refused() {
        // SEC011: rm -rf on a variable that was never validated.
        let p = script(
            "sec011",
            "#!/usr/bin/env bash\nD=$(mktemp -d)\nrm -rf \"$D\"\n",
        );
        let f = lint_script(&p);
        assert!(f.iter().any(|l| l.contains("bashrs lint")), "{f:?}");
    }

    #[test]
    fn planted_shellcheck_warning_is_refused() {
        // SC2154: referenced but never assigned.
        let p = script("sc", "#!/usr/bin/env bash\necho \"$never_set\"\n");
        let f = lint_script(&p);
        assert!(f.iter().any(|l| l.contains("shellcheck")), "{f:?}");
    }

    #[test]
    fn parses_the_summary_line() {
        assert_eq!(
            bashrs_errors("x\nSummary: 3 error(s), 1 warning(s)\n"),
            Some(3)
        );
        assert_eq!(bashrs_errors("no summary"), None);
    }
}
