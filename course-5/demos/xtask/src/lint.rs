//! Provable-contract lint (MEGA-001 §5.4, rmedia CLAUDE.md): every demo
//! `src/main.rs`
//! 1. opens with a `//!` docstring carrying a `Provable contract:` line,
//! 2. has `main` assert the contract (`assert!` / `assert_eq!` / `assert_ne!`),
//! 3. prints `contract: <name> OK` after the last assertion in `main`.

pub fn lint_contract(src: &str) -> Result<(), Vec<String>> {
    let mut findings = Vec::new();

    let doc: Vec<&str> = src
        .lines()
        .take_while(|l| l.starts_with("//!") || l.trim().is_empty())
        .filter(|l| l.starts_with("//!"))
        .collect();
    let named = doc.iter().any(|l| {
        l.trim_start_matches("//!")
            .trim()
            .strip_prefix("Provable contract:")
            .is_some_and(|rest| !rest.trim().is_empty())
    });
    if !named {
        findings.push("demo-contract-docstring-v1: no leading `//!` docstring with a non-empty `Provable contract:` line".into());
    }

    match main_body(src) {
        None => findings.push("no `fn main` found".into()),
        Some(body) => {
            let last_assert = ["assert!(", "assert_eq!(", "assert_ne!("]
                .iter()
                .filter_map(|m| body.rfind(m))
                .max();
            match last_assert {
                None => findings.push("`main` asserts nothing".into()),
                Some(pos) => {
                    let after = &body[pos..];
                    let prints = after.match_indices("println!(").any(|(i, _)| {
                        let call = &after[i..];
                        let end = call.find(");").unwrap_or(call.len());
                        let call = &call[..end];
                        call.contains("contract: ") && call.contains(" OK")
                    });
                    if !prints {
                        findings.push(
                            "no `println!(\"contract: … OK\")` after the last assertion in `main`"
                                .into(),
                        );
                    }
                }
            }
        }
    }

    if findings.is_empty() {
        Ok(())
    } else {
        Err(findings)
    }
}

/// The text of `fn main`'s body, by brace matching from its opening brace.
fn main_body(src: &str) -> Option<&str> {
    let start = src.find("fn main(")?;
    let open = start + src[start..].find('{')?;
    let mut depth = 0usize;
    for (i, c) in src[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[open..=open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Bash twin of [`lint_contract`] for demos that ship `quorum.sh`-style scripts instead of
/// `src/main.rs`: a leading `# Provable contract:` comment, at least one
/// `[[ … ]] || { … exit 1; }` assertion, and an `echo "contract: … OK"` after the last one.
pub fn lint_contract_sh(src: &str) -> Result<(), Vec<String>> {
    let mut findings = Vec::new();
    let named = src.lines().take_while(|l| l.starts_with('#')).any(|l| {
        l.trim_start_matches('#')
            .trim()
            .strip_prefix("Provable contract:")
            .is_some_and(|rest| !rest.trim().is_empty())
    });
    if !named {
        findings.push("demo-contract-docstring-v1: no leading `#` comment with a non-empty `Provable contract:` line".into());
    }
    let last_assert = src
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("[[") && l.contains("|| {") && l.contains("exit 1"))
        .map(|(i, _)| i)
        .max();
    match last_assert {
        None => findings.push("script asserts nothing (`[[ … ]] || { …; exit 1; }`)".into()),
        Some(n) => {
            if !src
                .lines()
                .skip(n + 1)
                .any(|l| l.contains("echo \"contract: ") && l.contains(" OK"))
            {
                findings.push("no `echo \"contract: … OK\"` after the last assertion".into());
            }
        }
    }
    if findings.is_empty() {
        Ok(())
    } else {
        Err(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = "//! D13.\n//!\n//! Provable contract: fanout-independent-v1 — order never changes the result.\n\nfn helper() -> u32 { 1 }\n\nfn main() {\n    let r = helper();\n    assert_eq!(r, 1, \"reduce\");\n    println!(\"contract: fanout-independent-v1 OK\");\n}\n";

    #[test]
    fn accepts_a_proper_demo() {
        assert_eq!(lint_contract(OK), Ok(()));
    }

    #[test]
    fn empty_contract_name_is_refused() {
        let src = OK.replace(
            "fanout-independent-v1 — order never changes the result.",
            "",
        );
        assert!(lint_contract(&src).is_err());
    }

    #[test]
    fn assert_outside_main_does_not_count() {
        let src = OK.replace("    assert_eq!(r, 1, \"reduce\");\n", "");
        let src = src.replace(
            "fn helper() -> u32 { 1 }",
            "fn helper() -> u32 { assert!(true); 1 }",
        );
        assert!(lint_contract(&src).is_err());
    }

    #[test]
    fn print_before_assert_is_refused() {
        let src = "//! Provable contract: x-v1 — y\nfn main() {\n    println!(\"contract: x-v1 OK\");\n    assert!(1 == 1);\n}\n";
        assert!(lint_contract(src).is_err());
    }

    #[test]
    fn empty_file_is_refused() {
        assert!(lint_contract("").is_err());
    }

    const SH: &str = "#!/usr/bin/env bash\n# Provable contract: x-v1 + y-v1\n[[ 1 == 1 ]] || { echo \"contract: FAIL x\"; exit 1; }\necho \"contract: x-v1 + y-v1 OK\"\n";

    #[test]
    fn bash_demo_with_contract_passes() {
        assert!(lint_contract_sh(SH).is_ok());
    }

    #[test]
    fn bash_demo_without_assertion_or_ok_line_is_refused() {
        assert!(lint_contract_sh(&SH.replace("[[ 1 == 1 ]]", "true")).is_err());
        assert!(lint_contract_sh(&SH.replace("contract: x-v1 + y-v1 OK", "done")).is_err());
        assert!(lint_contract_sh(&SH.replace("Provable contract:", "Note:")).is_err());
    }
}
