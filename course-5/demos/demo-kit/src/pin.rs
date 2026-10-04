//! Exact pins (MEGA-001 H-2). A pin is `=MAJOR.MINOR.PATCH[-PRE]`. Anything
//! else — a floor, a caret, a tilde, a bare version, a wildcard — is refused,
//! because a floor lets the tool under test change without the receipt saying so.
//! The one exception is [`SeriesPin`] (`"0.70.*"`), admitted for `apr` only.

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

/// A series pin, `"MAJOR.MINOR.*"`: admits exactly `MAJOR.MINOR.<n>`. It is
/// not a floor — the upper bound is the minor version — and it admits no
/// suffix, so `0.70.1-dirty` is refused. Only `apr` may use one; the Receipt
/// records apr's full version string and binary sha256, so an admitted patch
/// change is still visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeriesPin {
    pub major: u64,
    pub minor: u64,
}

impl fmt::Display for SeriesPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.*", self.major, self.minor)
    }
}

fn digits(s: &str) -> Option<u64> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// Parse `"0.70.*"`.
pub fn parse_series(spec: &str) -> Result<SeriesPin, String> {
    let s = spec.trim();
    let parts: Vec<&str> = s.split('.').collect();
    match parts.as_slice() {
        [ma, mi, "*"] => match (digits(ma), digits(mi)) {
            (Some(major), Some(minor)) => Ok(SeriesPin { major, minor }),
            _ => Err(s.to_string()),
        },
        _ => Err(s.to_string()),
    }
}

impl SeriesPin {
    /// `0.70.<n>` and nothing else: not 0.69.x, 0.71.x, 0.701.x or 0.70.1-dirty.
    pub fn admits(&self, found: &str) -> bool {
        let parts: Vec<&str> = found.split('.').collect();
        match parts.as_slice() {
            [ma, mi, pa] => {
                digits(ma) == Some(self.major)
                    && digits(mi) == Some(self.minor)
                    && digits(pa).is_some()
            }
            _ => false,
        }
    }
}

/// apr's pin: exact, or (apr only) a series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AprPin {
    Exact(ExactPin),
    Series(SeriesPin),
}

impl AprPin {
    pub fn admits(&self, found: &str) -> bool {
        match self {
            AprPin::Exact(p) => p.0 == found,
            AprPin::Series(s) => s.admits(found),
        }
    }
}

impl fmt::Display for AprPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AprPin::Exact(p) => write!(f, "{}", p.0),
            AprPin::Series(s) => write!(f, "{s}"),
        }
    }
}

/// `"=X.Y.Z"` or `"X.Y.*"`; every other form is refused.
pub fn parse_exact_or_series(spec: &str) -> Result<AprPin, String> {
    parse_exact(spec)
        .map(AprPin::Exact)
        .or_else(|_| parse_series(spec).map(AprPin::Series))
}

/// Extract the version from `apr --version` output, e.g.
/// `apr 0.69.3 (v0.69.3+no-git)` → `0.69.3`.
pub fn version_from_output(tool: &str, out: &str) -> Option<String> {
    let line = out.lines().find(|l| !l.trim().is_empty())?.trim();
    let rest = line.strip_prefix(tool).map(str::trim).unwrap_or(line);
    // `xdotool version` prints `xdotool version 3.20160805.1`: skip that word.
    let rest = rest.strip_prefix("version").map(str::trim).unwrap_or(rest);
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
        assert_eq!(
            version_from_output(
                "pv",
                "pv 0.70.1 (04332f838) (aprender provable-contracts verifier)\nmore\n"
            )
            .as_deref(),
            Some("0.70.1")
        );
        assert_eq!(
            version_from_output("xdotool", "xdotool version 3.20160805.1\n").as_deref(),
            Some("3.20160805.1")
        );
        assert_eq!(version_from_output("apr", "garbage"), None);
    }

    /// demo-pin-v1 (series arm): `0.70.*` admits 0.70.<n> and nothing else.
    #[test]
    fn series_pin_admits_only_its_minor() {
        let s = parse_series("0.70.*").unwrap();
        assert_eq!(
            s,
            SeriesPin {
                major: 0,
                minor: 70
            }
        );
        for ok in ["0.70.0", "0.70.1", "0.70.42"] {
            assert!(s.admits(ok), "{ok}");
        }
        for bad in [
            "0.69.9",
            "0.71.0",
            "0.701.0",
            "0.70.1-dirty",
            "0.70",
            "0.70.x",
            "1.70.1",
            "",
        ] {
            assert!(!s.admits(bad), "admitted {bad:?}");
        }
        for spec in ["0.70", "0.70.**", ">=0.70.*", "0.*.*", "*", "0.70.1"] {
            assert!(parse_series(spec).is_err(), "series {spec:?}");
        }
    }

    #[test]
    fn apr_pin_is_exact_or_series() {
        assert!(matches!(
            parse_exact_or_series("=0.70.1"),
            Ok(AprPin::Exact(_))
        ));
        assert!(matches!(
            parse_exact_or_series("0.70.*"),
            Ok(AprPin::Series(_))
        ));
        assert!(parse_exact_or_series(">=0.70.1").is_err());
        assert!(parse_exact_or_series("0.70.1").is_err());
        let p = parse_exact_or_series("=0.70.1").unwrap();
        assert!(p.admits("0.70.1") && !p.admits("0.70.2"));
    }
    /// pv stays exact: a series spec is refused by parse_exact.
    #[test]
    fn pv_series_is_refused() {
        assert!(parse_exact("0.70.*").is_err());
    }
}
