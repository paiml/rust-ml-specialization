//! Preflight: every reason a demo must not run, collected before any step.
//!
//! The probe is a trait so the falsifiers can drive it with a fake tool set;
//! [`SystemProbe`] asks the real binaries.

use crate::manifest::DemoManifest;
use crate::pin;
use crate::sha;
use crate::verdict::NotRunReason;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a forjar-declared apr may live. Overridable per host with
/// `RFML5_DECLARED_APR` (colon-separated absolute paths).
pub const DEFAULT_DECLARED_APR: &str = "/opt/course-bin/bin/apr";

pub trait Probe {
    /// Absolute path `apr` resolves to on PATH, if any.
    fn which(&self, tool: &str) -> Option<PathBuf>;
    /// `<tool> --version`, parsed.
    fn version(&self, tool: &str) -> Option<String>;
    /// Is this `needs` entry available (not refused) in the installed tools?
    fn available(&self, need: &str) -> bool;
    /// Does any job hold a GPU reservation?
    fn gpu_reserved(&self) -> bool;
    fn declared_apr(&self) -> Vec<PathBuf>;
}

pub fn preflight(m: &DemoManifest, base: &Path, probe: &dyn Probe) -> Vec<NotRunReason> {
    let mut out = Vec::new();

    for (tool, spec, used) in [("apr", &m.apr, m.uses_apr()), ("agy", &m.agy, m.uses_agy())] {
        if !used {
            continue;
        }
        match pin::parse_exact(spec) {
            Err(bad) => out.push(NotRunReason::PinRefused(format!("{tool} {bad}"))),
            Ok(p) => match probe.which(tool) {
                None => out.push(NotRunReason::MissingTool(tool.into())),
                Some(path) => {
                    if tool == "apr" && !probe.declared_apr().contains(&path) {
                        out.push(NotRunReason::UndeclaredApr(path.display().to_string()));
                    }
                    let found = probe.version(tool).unwrap_or_else(|| "unknown".into());
                    if found != p.0 {
                        out.push(NotRunReason::VersionMismatch {
                            tool: tool.into(),
                            pinned: p.0,
                            found,
                        });
                    }
                }
            },
        }
    }

    if let Some(model) = &m.model {
        let found = sha::sha256_file(&base.join(&model.path)).unwrap_or_else(|_| "missing".into());
        if found != model.sha256 {
            out.push(NotRunReason::WeightsMismatch {
                expected: model.sha256.clone(),
                found,
            });
        }
    }

    if let Some(fx) = &m.fixtures {
        let found = sha::tree_hash(&base.join(&fx.dir)).unwrap_or_else(|_| "missing".into());
        if found != fx.sha256 {
            out.push(NotRunReason::FixturesMismatch {
                expected: fx.sha256.clone(),
                found,
            });
        }
    }

    for need in &m.needs {
        if !probe.available(need) {
            out.push(NotRunReason::Refused(need.clone()));
        }
    }

    if m.host_class.contains("cuda") && probe.gpu_reserved() {
        out.push(NotRunReason::GpuLockHeld);
    }
    out
}

/// The real tools on this host.
pub struct SystemProbe;

fn run(tool: &str, args: &[&str]) -> Option<(bool, String)> {
    let o = Command::new(tool).args(args).output().ok()?;
    let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&o.stderr));
    Some((o.status.success(), text))
}

impl SystemProbe {
    fn help(&self, tool: &str, subs: &[&str]) -> Option<String> {
        let mut args: Vec<&str> = subs.to_vec();
        args.push("--help");
        match run(tool, &args)? {
            (true, text) if !text.contains("unavailable in this build") => Some(text),
            _ => None,
        }
    }
}

impl Probe for SystemProbe {
    fn which(&self, tool: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|d| d.join(tool))
            .find(|p| p.is_file())
    }

    fn version(&self, tool: &str) -> Option<String> {
        let (ok, text) = run(tool, &["--version"])?;
        ok.then(|| pin::version_from_output(tool, &text)).flatten()
    }

    /// `"apr serve run"` → `apr serve run --help` exits 0.
    /// `"apr run --json"` → and `--json` appears in that help.
    /// `"--json-schema"` → appears in `apr run --help` or `apr serve run --help`.
    /// `"agy"` / `"agy ..."` → `agy --version` exits 0.
    fn available(&self, need: &str) -> bool {
        let words: Vec<&str> = need.split_whitespace().collect();
        match words.first() {
            Some(&"agy") => run("agy", &["--version"]).is_some_and(|(ok, _)| ok),
            Some(&"apr") => {
                let subs: Vec<&str> = words[1..]
                    .iter()
                    .copied()
                    .filter(|w| !w.starts_with('-'))
                    .collect();
                let flags: Vec<&str> = words[1..]
                    .iter()
                    .copied()
                    .filter(|w| w.starts_with('-'))
                    .collect();
                self.help("apr", &subs)
                    .is_some_and(|h| flags.iter().all(|f| h.contains(f)))
            }
            Some(flag) if flag.starts_with("--") => [&["run"][..], &["serve", "run"][..]]
                .iter()
                .filter_map(|subs| self.help("apr", subs))
                .any(|h| h.contains(flag)),
            Some(tool) => run(tool, &["--version"]).is_some_and(|(ok, _)| ok),
            None => false,
        }
    }

    fn gpu_reserved(&self) -> bool {
        let Some((true, text)) = run("apr", &["gpu", "--json"]) else {
            return false;
        };
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("reservations")
                    .and_then(|r| r.as_array())
                    .map(|a| !a.is_empty())
            })
            .unwrap_or(false)
    }

    fn declared_apr(&self) -> Vec<PathBuf> {
        std::env::var("RFML5_DECLARED_APR")
            .unwrap_or_else(|_| DEFAULT_DECLARED_APR.into())
            .split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// A tool set the falsifiers control.
    pub struct FakeProbe {
        pub apr: Option<(PathBuf, String)>,
        pub agy: Option<String>,
        pub refused: BTreeSet<String>,
        pub gpu_reserved: bool,
    }

    impl Default for FakeProbe {
        fn default() -> Self {
            Self {
                apr: Some((PathBuf::from(DEFAULT_DECLARED_APR), "0.69.3".into())),
                agy: Some("1.2.11".into()),
                refused: BTreeSet::new(),
                gpu_reserved: false,
            }
        }
    }

    impl Probe for FakeProbe {
        fn which(&self, tool: &str) -> Option<PathBuf> {
            match tool {
                "apr" => self.apr.as_ref().map(|(p, _)| p.clone()),
                "agy" => self
                    .agy
                    .as_ref()
                    .map(|_| PathBuf::from("/opt/course-bin/bin/agy")),
                _ => None,
            }
        }
        fn version(&self, tool: &str) -> Option<String> {
            match tool {
                "apr" => self.apr.as_ref().map(|(_, v)| v.clone()),
                "agy" => self.agy.clone(),
                _ => None,
            }
        }
        fn available(&self, need: &str) -> bool {
            !self.refused.contains(need)
        }
        fn gpu_reserved(&self) -> bool {
            self.gpu_reserved
        }
        fn declared_apr(&self) -> Vec<PathBuf> {
            vec![PathBuf::from(DEFAULT_DECLARED_APR)]
        }
    }

    fn manifest(extra: &str) -> DemoManifest {
        DemoManifest::parse(&format!(
            r#"
id = "d04-json-verdict"
title = "t"
lesson = "rfml5/3.2"
apr = "=0.69.3"
host_class = "gx10-cuda"
needs = ["apr serve run", "--json-schema"]
{extra}
[assert]
exit = 0
[record]
target_duration_s = 300
resolution = "1920x1080"
"#
        ))
        .unwrap()
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn clean_preflight_is_empty() {
        assert!(preflight(&manifest(""), Path::new("."), &FakeProbe::default()).is_empty());
    }

    /// demo-pin-v1 (floor arm): a floor pin never runs.
    #[test]
    fn demo_pin_v1_floor_pin_refused() {
        let m = manifest("").clone();
        let m = DemoManifest {
            apr: ">=0.69.3".into(),
            ..m
        };
        let r = preflight(&m, Path::new("."), &FakeProbe::default());
        assert_eq!(r, vec![NotRunReason::PinRefused("apr >=0.69.3".into())]);
    }

    #[test]
    fn version_mismatch_is_notrun() {
        let probe = FakeProbe {
            apr: Some((PathBuf::from(DEFAULT_DECLARED_APR), "0.69.1".into())),
            ..Default::default()
        };
        let r = preflight(&manifest(""), Path::new("."), &probe);
        assert!(matches!(r[0], NotRunReason::VersionMismatch { .. }));
    }

    /// demo-pin-v1 (sha arm): wrong weights sha → NotRun{WeightsMismatch}.
    #[test]
    fn demo_pin_v1_wrong_sha_is_weights_mismatch() {
        let d = tmp("weights");
        std::fs::write(d.join("model.gguf"), b"not the declared bytes").unwrap();
        let m = manifest(&format!(
            "model = {{ name = \"Qwen3.5-4B-Q4_K_M\", sha256 = \"{}\", path = \"model.gguf\" }}",
            "0".repeat(64)
        ));
        let r = preflight(&m, &d, &FakeProbe::default());
        assert!(
            matches!(r.as_slice(), [NotRunReason::WeightsMismatch { .. }]),
            "{r:?}"
        );
        let good = crate::sha::sha256_file(&d.join("model.gguf")).unwrap();
        let m = manifest(&format!(
            "model = {{ name = \"Qwen3.5-4B-Q4_K_M\", sha256 = \"{good}\", path = \"model.gguf\" }}"
        ));
        assert!(preflight(&m, &d, &FakeProbe::default()).is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn missing_weights_is_weights_mismatch() {
        let m = manifest(&format!(
            "model = {{ name = \"m\", sha256 = \"{}\", path = \"/nonexistent/m.gguf\" }}",
            "0".repeat(64)
        ));
        let r = preflight(&m, Path::new("."), &FakeProbe::default());
        assert!(matches!(&r[0], NotRunReason::WeightsMismatch { found, .. } if found == "missing"));
    }

    #[test]
    fn refused_need_is_named() {
        let probe = FakeProbe {
            refused: ["--json-schema".to_string()].into(),
            ..Default::default()
        };
        let r = preflight(&manifest(""), Path::new("."), &probe);
        assert_eq!(r, vec![NotRunReason::Refused("--json-schema".into())]);
    }

    #[test]
    fn undeclared_apr_is_notrun() {
        let probe = FakeProbe {
            apr: Some((
                PathBuf::from("/home/someone/.cargo/bin/apr"),
                "0.69.3".into(),
            )),
            ..Default::default()
        };
        let r = preflight(&manifest(""), Path::new("."), &probe);
        assert_eq!(
            r,
            vec![NotRunReason::UndeclaredApr(
                "/home/someone/.cargo/bin/apr".into()
            )]
        );
    }

    #[test]
    fn gpu_lock_held_is_notrun_never_wait() {
        let probe = FakeProbe {
            gpu_reserved: true,
            ..Default::default()
        };
        assert_eq!(
            preflight(&manifest(""), Path::new("."), &probe),
            vec![NotRunReason::GpuLockHeld]
        );
    }

    #[test]
    fn fixtures_hash_is_checked() {
        let d = tmp("fixtures");
        std::fs::create_dir_all(d.join("fx")).unwrap();
        std::fs::write(d.join("fx/a.diff"), "x").unwrap();
        let h = crate::sha::tree_hash(&d.join("fx")).unwrap();
        let ok = manifest(&format!("fixtures = {{ dir = \"fx\", sha256 = \"{h}\" }}"));
        assert!(preflight(&ok, &d, &FakeProbe::default()).is_empty());
        std::fs::write(d.join("fx/a.diff"), "y").unwrap();
        assert!(matches!(
            preflight(&ok, &d, &FakeProbe::default()).as_slice(),
            [NotRunReason::FixturesMismatch { .. }]
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
