//! The only module that spawns a process, and only three programs: the pinned
//! app, `Xvfb` and `xdotool`. Each gets `env_clear()` and an argument vector of
//! fixed literals plus the display number and profile paths.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::os::fd::OwnedFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::open_under::Roots;
use crate::operator_listing::{CONFINED_HOME, CONFINED_XDG};
use crate::proc_probe::{self, AppPid};

/// The `PATH` every child gets.
pub const CHILD_PATH: &str = "/usr/bin:/bin";
/// The `LANG` the app gets.
pub const CHILD_LANG: &str = "C.UTF-8";

/// `RFML5_AGY_PROFILE`, canonical. The only read of that name.
pub fn profile_from_env() -> Result<PathBuf, String> {
    let p = std::env::var_os("RFML5_AGY_PROFILE").ok_or("RFML5_AGY_PROFILE")?;
    crate::open_under::canonical(Path::new(&p)).map_err(|_| "RFML5_AGY_PROFILE".to_string())
}

/// `RFML5_AGY_BIN`, the app launcher.
pub fn app_bin_from_env() -> Result<PathBuf, String> {
    std::env::var_os("RFML5_AGY_BIN")
        .map(PathBuf::from)
        .ok_or("RFML5_AGY_BIN".to_string())
}

/// `RFML5_DISPLAY`: an explicit display number for the scratch X server.
pub fn display_override() -> Result<Option<u32>, String> {
    match std::env::var("RFML5_DISPLAY") {
        Ok(s) => s
            .trim_start_matches(':')
            .parse()
            .map(Some)
            .map_err(|_| format!("RFML5_DISPLAY={s} is not a display number")),
        Err(_) => Ok(None),
    }
}

/// The scratch X server.
pub struct Xvfb {
    child: Child,
    pub num: u32,
}

impl Xvfb {
    /// Start `Xvfb` on a free display (or `want`), 1920x1080, no TCP; the
    /// number is read from `-displayfd`.
    pub fn start(want: Option<u32>) -> Result<Xvfb, String> {
        let mut c = Command::new("Xvfb");
        c.env_clear().env("PATH", CHILD_PATH);
        if let Some(n) = want {
            c.arg(format!(":{n}"));
        }
        c.args([
            "-displayfd",
            "1",
            "-screen",
            "0",
            "1920x1080x24",
            "-nolisten",
            "tcp",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
        let mut child = c.spawn().map_err(|e| format!("Xvfb: {e}"))?;
        let out = child.stdout.take().ok_or("Xvfb stdout")?;
        let mut line = String::new();
        BufReader::new(out)
            .read_line(&mut line)
            .map_err(|e| format!("Xvfb displayfd: {e}"))?;
        let num = line.trim().parse().map_err(|_| {
            let _ = child.kill();
            format!("Xvfb displayfd gave `{}`", line.trim())
        })?;
        Ok(Xvfb { child, num })
    }

    pub fn display(&self) -> String {
        format!(":{}", self.num)
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The two keys `xdotool` may press on the scratch display (E_4 only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XdotoolKey {
    F1,
    F12,
}

/// `xdotool key <F1|F12>` on `display`. Its whole environment is `PATH` and
/// `DISPLAY`.
pub fn xdotool_key(display: &Xvfb, key: XdotoolKey) -> Result<i32, String> {
    let k = match key {
        XdotoolKey::F1 => "F1",
        XdotoolKey::F12 => "F12",
    };
    let st = Command::new("xdotool")
        .env_clear()
        .env("PATH", CHILD_PATH)
        .env("DISPLAY", display.display())
        .args(["key", k])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("xdotool: {e}"))?;
    Ok(st.code().unwrap_or(-1))
}

/// `xdotool version`, first line.
pub fn xdotool_version(display: &Xvfb) -> Result<String, String> {
    let out = Command::new("xdotool")
        .env_clear()
        .env("PATH", CHILD_PATH)
        .env("DISPLAY", display.display())
        .arg("version")
        .output()
        .map_err(|e| format!("xdotool: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string())
}

/// The app, leader of its own process group.
pub struct App {
    child: Option<Child>,
    pub leader: AppPid,
    /// What the launcher set, by name.
    pub env: BTreeMap<String, String>,
    pub port: Option<u16>,
}

fn confined_env(profile: &Path, display: &str) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert("PATH".to_string(), CHILD_PATH.to_string());
    env.insert("LANG".to_string(), CHILD_LANG.to_string());
    env.insert("DISPLAY".to_string(), display.to_string());
    let (h, sub) = CONFINED_HOME;
    env.insert(h.to_string(), profile.join(sub).display().to_string());
    for (k, sub) in CONFINED_XDG {
        env.insert(k.to_string(), profile.join(sub).display().to_string());
    }
    env
}

/// Launch the app on `display` with the profile's user-data and extensions
/// directories; stdout and stderr go to `log` (a run-directory file).
pub fn launch_app(bin: &Path, profile: &Path, display: &Xvfb, log: OwnedFd) -> Result<App, String> {
    let env = confined_env(profile, &display.display());
    let err = log.try_clone().map_err(|e| e.to_string())?;
    let child = Command::new(bin)
        .env_clear()
        .envs(&env)
        .arg(format!(
            "--user-data-dir={}",
            profile.join("user-data").display()
        ))
        .arg(format!(
            "--extensions-dir={}",
            profile.join("extensions").display()
        ))
        .args([
            "--password-store=basic",
            "--remote-debugging-port=0",
            "--window-position=0,0",
            "--window-size=1920,1080",
            "--start-maximized",
        ])
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err))
        .spawn()
        .map_err(|e| format!("app: {e}"))?;
    let leader = AppPid::from_child_id(child.id());
    Ok(App {
        child: Some(child),
        leader,
        env,
        port: None,
    })
}

impl App {
    /// Has the leader exited?
    pub fn exited(&mut self) -> bool {
        self.child
            .as_mut()
            .is_none_or(|c| c.try_wait().ok().flatten().is_some())
    }

    /// Wait for `DevToolsActivePort` to name a port whose listener belongs
    /// to this group. Never looks for any other port.
    pub fn wait_port(&mut self, roots: &Roots, within: Duration) -> Result<u16, String> {
        let end = Instant::now() + within;
        while Instant::now() < end {
            if self.exited() {
                return Err("app exited early (single-instance forwarding?)".into());
            }
            if let Some(p) = port_of(roots) {
                if proc_probe::port_owned_by_group(p, self.leader) {
                    self.port = Some(p);
                    return Ok(p);
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        Err("no DevTools port owned by the app's group".into())
    }

    /// The child's environment: keys exactly the launcher's set; `PATH`,
    /// `LANG`, `DISPLAY` byte-equal; `HOME` and `XDG_*` under the profile.
    pub fn check_env(&self, profile: &Path) -> Result<(), String> {
        let got = proc_probe::environ(self.leader)?;
        let want_keys: Vec<&String> = self.env.keys().collect();
        let got_keys: Vec<&String> = got.keys().collect();
        if want_keys != got_keys {
            return Err(format!(
                "app environment keys differ: {} set, {} found",
                want_keys.len(),
                got_keys.len()
            ));
        }
        for (k, v) in &got {
            let ok = match k.as_str() {
                "PATH" | "LANG" | "DISPLAY" => self.env.get(k) == Some(v),
                _ => {
                    crate::open_under::canonical(Path::new(v)).is_ok_and(|c| c.starts_with(profile))
                }
            };
            if !ok {
                return Err(format!("app environment value for {k} is not confined"));
            }
        }
        Ok(())
    }

    /// Kill the group leader, wait, and sweep: the pids of the group still
    /// alive afterwards (empty when clean), and whether the port still listens.
    pub fn teardown(&mut self) -> (Vec<u32>, bool) {
        proc_probe::kill_group(self.leader, Duration::from_secs(10));
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        let end = Instant::now() + Duration::from_secs(15);
        let mut left = proc_probe::group_members(self.leader);
        while !left.is_empty() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(250));
            left = proc_probe::group_members(self.leader);
        }
        let listening = self
            .port
            .is_some_and(|p| !proc_probe::listening_inodes(p).is_empty());
        (left.into_iter().map(AppPid::get).collect(), listening)
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if self.child.is_some() {
            proc_probe::kill_group(self.leader, Duration::from_secs(5));
        }
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

fn port_of(roots: &Roots) -> Option<u16> {
    let bytes = roots.read_devtools_port().ok()?;
    String::from_utf8_lossy(&bytes)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}
