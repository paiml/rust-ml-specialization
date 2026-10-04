//! The real tools on this host: `PATH` lookup and version probes. Gated on
//! the `process` feature, because it spawns programs; a confined crate
//! (d20, d21, agy-cdp) builds demo-kit without it.

use super::{app_from_launcher, AppFound, Probe, DEFAULT_DECLARED_APR};
use crate::pin;
use crate::verdict::NotRunReason;
use std::path::PathBuf;

/// The first `tool` on `path_env` (or this process's `PATH`).
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
#[derive(Debug, Clone, Default)]
pub struct SystemProbe {
    pub path: Option<std::ffi::OsString>,
    pub agy_bin: Option<PathBuf>,
}

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
