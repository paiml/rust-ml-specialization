//! Exact pins (MEGA-001 H-2). A pin is `=MAJOR.MINOR.PATCH[-PRE]`. Anything
//! else — a floor, a caret, a tilde, a bare version, a wildcard — is refused,
//! because a floor lets the tool under test change without the receipt saying so.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactPin(pub String);

impl fmt::Display for ExactPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "={}", self.0)
    }
}

/// Parse `"=0.69.3"`. Returns the version text, or the refused spec.
pub fn parse_exact(spec: &str) -> Result<ExactPin, String> {
    let s = spec.trim();
    let Some(v) = s.strip_prefix('=') else {
        return Err(s.to_string());
    };
    if v.starts_with('=') || !is_semver(v) {
        return Err(s.to_string());
    }
    Ok(ExactPin(v.to_string()))
}

fn is_semver(v: &str) -> bool {
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (v, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && pre.is_none_or(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        })
}

/// Extract the version from `apr --version` output, e.g.
/// `apr 0.69.3 (v0.69.3+no-git)` → `0.69.3`.
pub fn version_from_output(tool: &str, out: &str) -> Option<String> {
    let line = out.lines().find(|l| !l.trim().is_empty())?.trim();
    let rest = line.strip_prefix(tool).map(str::trim).unwrap_or(line);
    let v = rest.split_whitespace().next()?.trim_start_matches('v');
    is_semver(v).then(|| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_is_accepted() {
        assert_eq!(parse_exact("=0.69.3").unwrap().0, "0.69.3");
        assert_eq!(parse_exact("=0.70.0-rc.1").unwrap().0, "0.70.0-rc.1");
    }

    /// demo-pin-v1 (floor arm): every non-exact spec is refused.
    #[test]
    fn demo_pin_v1_floors_are_refused() {
        for spec in [
            ">=0.69.3", "^0.69.3", "~0.69.3", "0.69.3", "*", "=0.69", "==0.69.3", "=0.69.x", "",
        ] {
            assert!(
                parse_exact(spec).is_err(),
                "accepted non-exact pin {spec:?}"
            );
        }
    }

    #[test]
    fn reads_tool_versions() {
        assert_eq!(
            version_from_output("apr", "apr 0.69.3 (v0.69.3+no-git)\n").as_deref(),
            Some("0.69.3")
        );
        assert_eq!(
            version_from_output("agy", "1.2.11\n").as_deref(),
            Some("1.2.11")
        );
        assert_eq!(version_from_output("apr", "garbage"), None);
    }
}
