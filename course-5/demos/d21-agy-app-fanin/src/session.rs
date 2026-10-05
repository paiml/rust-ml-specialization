//! Preflight, run directory, launch and attach: D20's isolation, shared by
//! both D21 bins. Every path comes from `RFML5_*` or the run directory.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agy_cdp::ax::{self, Page};
use agy_cdp::launch;
use agy_cdp::open_under;
use agy_cdp::operator_listing;
use agy_cdp::transport::Conn;
use agy_cdp::RoleNameMap;

pub const APP_VERSION: &str = "2.8.1";
pub const APP_ASAR_SHA256: &str =
    "cb425e9ac098e9bc7958accc4bedb17a718c7506ae13548a0eb7cef55cdaaf26";

pub struct Env {
    pub profile: PathBuf,
    pub bin: PathBuf,
    pub run: PathBuf,
}

/// demo-kit's run id: `<secs>-<nanos:09>-<pid>`.
fn run_id() -> Result<String, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "{}-{:09}-{}",
        now.as_secs(),
        now.subsec_nanos(),
        std::process::id()
    ))
}

/// Preflight as D20 does it, then create `$RFML5_RECEIPTS/<parts…>/<run_id>/`.
pub fn env(parts: &[&str]) -> Result<Env, String> {
    if agy_cdp::role_name_map()? == RoleNameMap::Unmeasured {
        return Err("FixtureUnmeasured: agy-cdp role-name-map.json".into());
    }
    let profile = launch::profile_from_env()?;
    if operator_listing::overlaps(&profile)? {
        return Err("the demo profile overlaps an operator directory".into());
    }
    let bin = launch::app_bin_from_env()?;
    let app = demo_kit::preflight::app_from_launcher(&bin)?;
    if app.version != APP_VERSION || app.asar_sha256 != APP_ASAR_SHA256 {
        return Err(format!("app is not the pinned {APP_VERSION} build"));
    }
    let receipts = std::env::var_os("RFML5_RECEIPTS").ok_or("RFML5_RECEIPTS")?;
    let id = run_id()?;
    let mut all: Vec<&str> = parts.to_vec();
    all.push(&id);
    let run = open_under::create_run_dir(Path::new(&receipts), &all)?;
    Ok(Env { profile, bin, run })
}

/// Attach to the app page, waiting up to 90 s for it to appear.
pub fn attach_page(conn: &mut Conn) -> Result<Page, String> {
    let end = Instant::now() + Duration::from_secs(90);
    while Instant::now() < end {
        if let Some(p) = ax::attach(conn, Some("https://127.0.0.1"))? {
            return Ok(p);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Err("no app page target".into())
}
