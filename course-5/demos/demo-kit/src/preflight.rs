//! Preflight: every reason a demo must not run, collected before any step.
//!
//! The probe is a trait so the falsifiers can drive it with a fake tool set;
//! [`SystemProbe`] (feature `process`) asks the real binaries, on this
//! process's `PATH` or on an explicit one, so tests can point it at a
//! directory of fake tools without touching the environment.
//!
//! Pins checked here: `apr` (exact or series), `agy`, `pv`, `xdotool` (exact),
//! and the Antigravity app (exact version plus the sha256 of its `app.asar`).
//! The app's version is read from `package.json` inside `app.asar`; the app is
//! never launched to ask, because a second launch is forwarded to any instance
//! already running. `Xvfb` in `needs` is found on `PATH` and its version text
//! is recorded, not pinned.

use crate::manifest::DemoManifest;
use crate::pin;
use crate::sha;
use crate::verdict::NotRunReason;
use std::path::{Path, PathBuf};

/// Where a forjar-declared apr may live. Overridable per host with
/// `RFML5_DECLARED_APR` (colon-separated absolute paths).
pub const DEFAULT_DECLARED_APR: &str = "/opt/course-bin/bin/apr";

/// What preflight found for the Antigravity app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppFound {
    pub version: String,
    pub asar_sha256: String,
}

pub trait Probe {
    /// Absolute path `apr` resolves to on PATH, if any.
    fn which(&self, tool: &str) -> Option<PathBuf>;
    /// `<tool> --version` (`xdotool version`), parsed.
    fn version(&self, tool: &str) -> Option<String>;
    /// Is this `needs` entry available (not refused) in the installed tools?
    fn available(&self, need: &str) -> bool;
    /// Does any job hold a GPU reservation?
    fn gpu_reserved(&self) -> bool;
    fn declared_apr(&self) -> Vec<PathBuf>;
    /// The Antigravity app named by `RFML5_AGY_BIN`: its version and asar sha.
    fn antigravity(&self) -> Result<AppFound, NotRunReason> {
        Err(NotRunReason::MissingTool("antigravity".into()))
    }
}

fn mismatch(out: &mut Vec<NotRunReason>, tool: &str, pinned: String, found: String) {
    out.push(NotRunReason::VersionMismatch {
        tool: tool.into(),
        pinned,
        found,
    });
}

pub fn preflight(m: &DemoManifest, base: &Path, probe: &dyn Probe) -> Vec<NotRunReason> {
    let mut out = Vec::new();
    if m.uses_apr() {
        check_apr(&mut out, probe, &m.apr);
    }
    for (tool, spec, used) in [
        ("agy", &m.agy, m.uses_agy()),
        ("pv", &m.pv, m.uses_pv()),
        ("xdotool", &m.xdotool, m.uses_xdotool()),
    ] {
        if used {
            check_exact(&mut out, probe, tool, spec);
        }
    }
    if m.uses_antigravity() {
        check_app(&mut out, probe, m);
    }
    check_files(&mut out, m, base);
    for need in &m.needs {
        if need == "Xvfb" {
            // Recorded, not pinned: it only supplies a blank framebuffer.
            if probe.which("Xvfb").is_none() {
                out.push(NotRunReason::MissingTool("Xvfb".into()));
            }
        } else if !probe.available(need) {
            out.push(NotRunReason::Refused(need.clone()));
        }
    }
    if m.host_class.contains("cuda") && probe.gpu_reserved() {
        out.push(NotRunReason::GpuLockHeld);
    }
    out
}

fn check_apr(out: &mut Vec<NotRunReason>, probe: &dyn Probe, spec: &str) {
    let p = match pin::parse_exact_or_series(spec) {
        Err(bad) => return out.push(NotRunReason::PinRefused(format!("apr {bad}"))),
        Ok(p) => p,
    };
    let Some(path) = probe.which("apr") else {
        return out.push(NotRunReason::MissingTool("apr".into()));
    };
    if !probe.declared_apr().contains(&path) {
        out.push(NotRunReason::UndeclaredApr(path.display().to_string()));
    }
    let found = probe.version("apr").unwrap_or_else(|| "unknown".into());
    if !p.admits(&found) {
        mismatch(out, "apr", p.to_string(), found);
    }
}

fn check_exact(out: &mut Vec<NotRunReason>, probe: &dyn Probe, tool: &str, spec: &str) {
    let p = match pin::parse_exact(spec) {
        Err(bad) => return out.push(NotRunReason::PinRefused(format!("{tool} {bad}"))),
        Ok(p) => p,
    };
    if probe.which(tool).is_none() {
        return out.push(NotRunReason::MissingTool(tool.into()));
    }
    let found = probe.version(tool).unwrap_or_else(|| "unknown".into());
    if found != p.0 {
        mismatch(out, tool, p.0, found);
    }
}

fn check_app(out: &mut Vec<NotRunReason>, probe: &dyn Probe, m: &DemoManifest) {
    let p = match pin::parse_exact(&m.antigravity) {
        Err(bad) => return out.push(NotRunReason::PinRefused(format!("antigravity {bad}"))),
        Ok(p) => p,
    };
    let Some(pinned_sha) = m.antigravity_asar_sha256.as_deref() else {
        return out.push(NotRunReason::PinRefused(
            "antigravity_asar_sha256 is not set".into(),
        ));
    };
    let found = match probe.antigravity() {
        Err(r) => return out.push(r),
        Ok(f) => f,
    };
    if found.version != p.0 {
        mismatch(out, "antigravity", p.0, found.version);
    }
    if found.asar_sha256 != pinned_sha {
        out.push(NotRunReason::AppMismatch {
            pinned_sha: pinned_sha.to_string(),
            found_sha: found.asar_sha256,
        });
    }
}

fn check_files(out: &mut Vec<NotRunReason>, m: &DemoManifest, base: &Path) {
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
}

/// One file's bytes inside an Electron `app.asar`: a 16-byte pickle prefix
/// (`4`, header pickle size, payload size, JSON length), the header JSON, then
/// the file data at `8 + header_pickle_size + offset`.
pub fn asar_file<'a>(asar: &'a [u8], name: &str) -> Result<&'a [u8], String> {
    let u32_at = |i: usize| {
        asar.get(i..i + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
            .ok_or_else(|| "asar: short prefix".to_string())
    };
    let header_size = u32_at(4)?;
    let json_len = u32_at(12)?;
    let json = asar.get(16..16 + json_len).ok_or("asar: short header")?;
    let h: serde_json::Value =
        serde_json::from_slice(json).map_err(|e| format!("asar header: {e}"))?;
    let f = &h["files"][name];
    let size = f["size"].as_u64().ok_or("asar: no size")? as usize;
    let offset: usize = f["offset"]
        .as_str()
        .and_then(|o| o.parse().ok())
        .ok_or("asar: no offset")?;
    let start = 8 + header_size + offset;
    asar.get(start..start + size)
        .ok_or_else(|| format!("asar: {name} out of range"))
}

/// The app beside `launcher` (followed through symlinks): the version in
/// `resources/app.asar`'s `package.json` and the sha256 of that asar.
pub fn app_from_launcher(launcher: &Path) -> Result<AppFound, String> {
    let real = std::fs::canonicalize(launcher).map_err(|e| format!("launcher: {e}"))?;
    let asar_path = real
        .parent()
        .ok_or("launcher has no directory")?
        .join("resources/app.asar");
    let bytes = std::fs::read(&asar_path).map_err(|e| format!("app.asar: {e}"))?;
    let pkg: serde_json::Value = serde_json::from_slice(asar_file(&bytes, "package.json")?)
        .map_err(|e| format!("package.json: {e}"))?;
    let version = pkg["version"]
        .as_str()
        .ok_or("package.json has no version")?
        .to_string();
    Ok(AppFound {
        version,
        asar_sha256: sha::sha256_bytes(&bytes),
    })
}

/// The first `tool` on `path_env` (or this process's `PATH`).
#[cfg(feature = "process")]
pub fn which_in(path_env: Option<&std::ffi::OsStr>, tool: &str) -> Option<PathBuf> {
    let path = match path_env {
        Some(p) => p.to_os_string(),
        None => std::env::var_os("PATH")?,
    };
    std::env::split_paths(&path)
        .map(|d| d.join(tool))
        .find(|p| p.is_file())
}

/// The real tools on this host. `path` overrides `PATH` and `agy_bin`
/// overrides `RFML5_AGY_BIN`; both default to the environment.
#[cfg(feature = "process")]
#[derive(Debug, Clone, Default)]
pub struct SystemProbe {
    pub path: Option<std::ffi::OsString>,
    pub agy_bin: Option<PathBuf>,
}

#[cfg(feature = "process")]
impl SystemProbe {
    fn run(&self, tool: &str, args: &[&str]) -> Option<(bool, String)> {
        let bin = self.which(tool)?;
        let mut cmd = std::process::Command::new(bin);
        cmd.args(args);
        if let Some(p) = &self.path {
            cmd.env("PATH", p);
        }
        let o = cmd.output().ok()?;
        let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&o.stderr));
        Some((o.status.success(), text))
    }

    fn help(&self, tool: &str, subs: &[&str]) -> Option<String> {
        let mut args: Vec<&str> = subs.to_vec();
        args.push("--help");
        match self.run(tool, &args)? {
            (true, text) if !text.contains("unavailable in this build") => Some(text),
            _ => None,
        }
    }

    /// The raw version text of a recorded-not-pinned tool (`Xvfb -version`).
    pub fn version_text(&self, tool: &str) -> Option<String> {
        let args: &[&str] = if tool == "Xvfb" {
            &["-version"]
        } else {
            &["--version"]
        };
        self.run(tool, args).map(|(_, t)| t.trim().to_string())
    }
}

#[cfg(feature = "process")]
impl Probe for SystemProbe {
    fn which(&self, tool: &str) -> Option<PathBuf> {
        which_in(self.path.as_deref(), tool)
    }

    fn version(&self, tool: &str) -> Option<String> {
        let args: &[&str] = if tool == "xdotool" {
            &["version"]
        } else {
            &["--version"]
        };
        let (ok, text) = self.run(tool, args)?;
        ok.then(|| pin::version_from_output(tool, &text)).flatten()
    }

    /// `"apr serve run"` → `apr serve run --help` exits 0.
    /// `"apr run --json"` → and `--json` appears in that help.
    /// `"--json-schema"` → appears in `apr run --help` or `apr serve run --help`.
    /// `"agy"` / `"agy ..."` → `agy --version` exits 0.
    fn available(&self, need: &str) -> bool {
        let words: Vec<&str> = need.split_whitespace().collect();
        match words.first() {
            Some(&"agy") => self.run("agy", &["--version"]).is_some_and(|(ok, _)| ok),
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
            Some(tool) => self.run(tool, &["--version"]).is_some_and(|(ok, _)| ok),
            None => false,
        }
    }

    fn gpu_reserved(&self) -> bool {
        let Some((true, text)) = self.run("apr", &["gpu", "--json"]) else {
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

    fn antigravity(&self) -> Result<AppFound, NotRunReason> {
        let bin = self
            .agy_bin
            .clone()
            .or_else(|| std::env::var_os("RFML5_AGY_BIN").map(PathBuf::from))
            .ok_or_else(|| NotRunReason::EnvUnset("RFML5_AGY_BIN".into()))?;
        app_from_launcher(&bin).map_err(|_| NotRunReason::MissingTool("antigravity".into()))
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

/// Preflight against directories of fake tools: `SystemProbe` with an explicit
/// `PATH` and launcher, so nothing on this host is consulted.
#[cfg(all(test, feature = "process"))]
mod fake_tool_dirs {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const ASAR_SHA_UNSET: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dk-fake-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("bin")).unwrap();
        d
    }

    fn tool(dir: &Path, name: &str, prints: &str) {
        let p = dir.join("bin").join(name);
        std::fs::write(&p, format!("#!/bin/sh\necho '{prints}'\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A minimal `app.asar` holding one `package.json`, laid out as Electron's.
    pub fn asar(version: &str) -> Vec<u8> {
        let pkg = format!("{{\"name\":\"antigravity\",\"version\":\"{version}\"}}");
        let json = format!(
            "{{\"files\":{{\"package.json\":{{\"size\":{},\"offset\":\"0\"}}}}}}",
            pkg.len()
        );
        let aligned = json.len().div_ceil(4) * 4;
        let mut b = Vec::new();
        for n in [4, aligned + 8, aligned + 4, json.len()] {
            b.extend_from_slice(&(n as u32).to_le_bytes());
        }
        b.extend_from_slice(json.as_bytes());
        b.resize(16 + aligned, 0);
        b.extend_from_slice(pkg.as_bytes());
        b
    }

    fn app(dir: &Path, version: &str) -> (PathBuf, String) {
        let res = dir.join("app/resources");
        std::fs::create_dir_all(&res).unwrap();
        let bytes = asar(version);
        std::fs::write(res.join("app.asar"), &bytes).unwrap();
        let launcher = dir.join("app/antigravity");
        std::fs::write(&launcher, "#!/bin/sh\nexit 99\n").unwrap();
        std::os::unix::fs::symlink(&launcher, dir.join("bin/antigravity")).unwrap();
        (
            dir.join("bin/antigravity"),
            crate::sha::sha256_bytes(&bytes),
        )
    }

    fn manifest(sha: &str) -> DemoManifest {
        DemoManifest::parse(&format!(
            r#"
id = "d20-agy-app-fanout"
title = "t"
lesson = "rfml5/4.3"
apr = "none"
pv = "=0.70.1"
antigravity = "=2.8.1"
antigravity_asar_sha256 = "{sha}"
xdotool = "=3.20160805.1"
host_class = "any"
needs = ["Xvfb"]
[assert]
exit = 0
[record]
target_duration_s = 300
resolution = "1920x1080"
"#
        ))
        .unwrap()
    }

    fn probe(d: &Path, launcher: Option<PathBuf>) -> SystemProbe {
        SystemProbe {
            path: Some(d.join("bin").into_os_string()),
            agy_bin: launcher,
        }
    }

    fn all_tools(d: &Path, pv: &str) {
        tool(
            d,
            "pv",
            &format!("pv {pv} (04332f838) (aprender provable-contracts verifier)"),
        );
        tool(d, "xdotool", "xdotool version 3.20160805.1");
        tool(d, "Xvfb", "Unrecognized option: -version");
    }

    #[test]
    fn asar_package_version_is_read_without_launching() {
        let bytes = asar("2.8.1");
        let found = asar_file(&bytes, "package.json").unwrap();
        assert!(String::from_utf8_lossy(found).contains("\"2.8.1\""));
        assert!(asar_file(b"short", "package.json").is_err());
    }

    #[test]
    fn pinned_tools_present_is_clean() {
        let d = scratch("clean");
        all_tools(&d, "0.70.1");
        let (launcher, sha) = app(&d, "2.8.1");
        let r = preflight(&manifest(&sha), &d, &probe(&d, Some(launcher)));
        assert_eq!(r, vec![]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn pv_version_mismatch_is_notrun() {
        let d = scratch("pv");
        all_tools(&d, "0.70.2");
        let (launcher, sha) = app(&d, "2.8.1");
        let r = preflight(&manifest(&sha), &d, &probe(&d, Some(launcher)));
        assert_eq!(
            r,
            vec![NotRunReason::VersionMismatch {
                tool: "pv".into(),
                pinned: "0.70.1".into(),
                found: "0.70.2".into()
            }]
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn missing_tools_are_named() {
        let d = scratch("missing");
        let (launcher, sha) = app(&d, "2.8.1");
        let r = preflight(&manifest(&sha), &d, &probe(&d, Some(launcher)));
        assert_eq!(
            r,
            vec![
                NotRunReason::MissingTool("pv".into()),
                NotRunReason::MissingTool("xdotool".into()),
                NotRunReason::MissingTool("Xvfb".into()),
            ]
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn app_version_and_asar_are_pinned_separately() {
        let d = scratch("app");
        all_tools(&d, "0.70.1");
        let (launcher, sha) = app(&d, "2.8.2");
        let r = preflight(&manifest(&sha), &d, &probe(&d, Some(launcher.clone())));
        assert!(
            matches!(r.as_slice(), [NotRunReason::VersionMismatch { tool, .. }] if tool == "antigravity"),
            "{r:?}"
        );
        // Same version string, different asar: auto-update's likely failure.
        let r = preflight(&manifest(ASAR_SHA_UNSET), &d, &probe(&d, Some(launcher)));
        assert!(
            matches!(r.as_slice(), [NotRunReason::VersionMismatch { .. }, NotRunReason::AppMismatch { found_sha, .. }] if *found_sha == sha),
            "{r:?}"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn app_missing_is_missing_tool() {
        let d = scratch("noapp");
        all_tools(&d, "0.70.1");
        let r = preflight(
            &manifest(ASAR_SHA_UNSET),
            &d,
            &probe(&d, Some(d.join("bin/antigravity"))),
        );
        assert_eq!(r, vec![NotRunReason::MissingTool("antigravity".into())]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn app_pin_without_asar_sha_is_refused() {
        let d = scratch("nosha");
        all_tools(&d, "0.70.1");
        let mut m = manifest(ASAR_SHA_UNSET);
        m.antigravity_asar_sha256 = None;
        let r = preflight(&m, &d, &probe(&d, Some(d.join("x"))));
        assert!(
            matches!(r.as_slice(), [NotRunReason::PinRefused(_)]),
            "{r:?}"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn xvfb_version_text_is_recorded() {
        let d = scratch("xvfb");
        all_tools(&d, "0.70.1");
        let p = probe(&d, None);
        assert_eq!(
            p.version_text("Xvfb").as_deref(),
            Some("Unrecognized option: -version")
        );
        assert_eq!(p.version("xdotool").as_deref(), Some("3.20160805.1"));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
