//! Confinement lints (spec §4.8, §5.3).
//!
//! 1. **WebSocket confinement:** the WebSocket crate may be named only in
//!    `agy-cdp`'s manifest and sources. Every other member reaches the app
//!    through `agy-cdp`'s closed method enum.
//! 2. **The closure lint:** every workspace crate in d20's and d21's normal
//!    dependency closure (from `cargo metadata`; today `agy-cdp` and
//!    `demo-kit`) obeys the credential, environment, socket, process, `rustix`
//!    and `libc` rules, and none of them has a build script. `agy-cdp`, d20 and
//!    d21 additionally obey the file-confinement rule: no `std::fs`/`File`
//!    outside `open_under`, `operator_listing` and `proc_probe`.
//!
//! What is not linted, and why:
//! - a module declared under `#[cfg(feature = "process")] mod x;` (and every
//!   file beneath it): the confined crates build demo-kit with
//!   `default-features = false`, so that code is not in their closure;
//! - a `#[cfg(test)]` block: test code is never linked into a bin;
//! - `//` comments;
//! - another crate's `src/bin/`: a dependency's bins are not linked into
//!   its dependents.
//!
//! The lint is lexical. It reviews our own driver for a mistake; it is not a
//! sandbox against hostile code.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};

/// The WebSocket crate's name, assembled so this file does not itself name it.
pub const WS_CRATE: &str = concat!("tung", "stenite");

/// The crates whose own paths must come only from `open_under`.
const FILE_CONFINED: &[&str] = &["agy-cdp", "d20-agy-app-fanout", "d21-agy-app-fanin"];

/// The roots whose closure is linted.
const CLOSURE_ROOTS: &[&str] = &["d20-agy-app-fanout", "d21-agy-app-fanin"];

/// `env!`/`option_env!` names Cargo itself sets for a build. Matched exactly or,
/// for the two `*` families, by that family's own prefix; never as `CARGO_`.
const CARGO_SET: &[&str] = &["CARGO_MANIFEST_DIR", "CARGO_CRATE_NAME"];
const CARGO_SET_FAMILIES: &[&str] = &["CARGO_PKG_", "CARGO_BIN_EXE_"];

/// `(needle, label, the agy-cdp modules allowed to contain it)`. An empty list
/// allows it nowhere.
const RULES: &[(&str, &str, &[&str])] = &[
    ("TcpStream", "socket", &["src/transport.rs"]),
    ("TcpListener", "socket", &[]),
    ("UdpSocket", "socket", &[]),
    (WS_CRATE, "socket", &["src/transport.rs"]),
    ("Command", "process", &["src/launch.rs"]),
    ("rustix::net", "rustix", &[]),
    (
        "rustix",
        "rustix",
        &[
            "src/open_under.rs",
            "src/operator_listing.rs",
            "src/proc_probe.rs",
        ],
    ),
    ("libc", "libc", &[]),
    ("home_dir", "credential", &["src/operator_listing.rs"]),
    ("\"HOME\"", "credential", &["src/operator_listing.rs"]),
    ("XDG_", "credential", &["src/operator_listing.rs"]),
    ("/.gemini", "credential", &["src/operator_listing.rs"]),
    ("\".gemini", "credential", &["src/operator_listing.rs"]),
    ("/.antigravity", "credential", &["src/operator_listing.rs"]),
    ("\".antigravity", "credential", &["src/operator_listing.rs"]),
    ("Antigravity/", "credential", &["src/operator_listing.rs"]),
    ("/proc/self/environ", "credential", &[]),
    ("\"~", "credential", &[]),
    (concat!("\"/ho", "me/"), "credential", &[]),
    ("\"/root", "credential", &[]),
    ("RFML5_AGY_PROFILE", "environment", &["src/launch.rs"]),
    ("env::vars", "environment", &[]),
    ("Runtime.", "cdp", &[]),
    ("Debugger.", "cdp", &[]),
    ("addScriptToEvaluate", "cdp", &[]),
    ("Page.navigate", "cdp", &[]),
];

/// File-confinement rule, for [`FILE_CONFINED`] crates only.
const FILE_RULES: &[&str] = &["std::fs", "fs::", "File::", "OpenOptions"];
const FILE_ALLOWED: &[&str] = &[
    "src/open_under.rs",
    "src/operator_listing.rs",
    "src/proc_probe.rs",
];

/// One crate under the lint.
pub struct Ctx<'a> {
    pub name: &'a str,
    pub dir: &'a Path,
    /// Lint `src/bin/` too (true for the closure roots and `agy-cdp`).
    pub with_bins: bool,
    /// Is this path a git-tracked file? (`include*!` targets must be.)
    pub tracked: &'a dyn Fn(&Path) -> bool,
}

/// Source with `//` comments removed (outside string literals).
pub fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let mut in_str = false;
        let mut prev = '\0';
        let mut cut = line.len();
        for (i, c) in line.char_indices() {
            if c == '"' && prev != '\\' {
                in_str = !in_str;
            } else if c == '/' && prev == '/' && !in_str {
                cut = i - 1;
                break;
            }
            prev = c;
        }
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

/// `(opening, closing)` brace counts on one line.
fn braces(line: &str) -> (i64, i64) {
    let n = |b: u8| line.bytes().filter(|&c| c == b).count() as i64;
    (n(b'{'), n(b'}'))
}

fn is_test_attr(line: &str) -> bool {
    let t = line.trim();
    t == "#[cfg(test)]" || t.starts_with("#[cfg(all(test")
}

/// Source with every `#[cfg(test)]` / `#[cfg(all(test, …))]` item removed
/// (brace-balanced, or up to the `;` of a `mod x;`). Expects comment-free text.
pub fn strip_tests(src: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    let (mut depth, mut opened) = (0i64, false);
    for line in src.lines() {
        if !skipping && is_test_attr(line) {
            (skipping, depth, opened) = (true, 0, false);
            continue;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let (opens, closes) = braces(line);
        depth += opens - closes;
        opened |= opens > 0;
        if (opened && depth <= 0) || (!opened && line.trim_end().ends_with(';')) {
            skipping = false;
        }
    }
    out
}

/// The code the lint reads: comments and test items removed.
pub fn code_only(src: &str) -> String {
    strip_tests(&strip_comments(src))
}

/// Modules declared `#[cfg(feature = "process")] mod NAME;` in `src`.
fn process_gated_mods(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut gated = false;
    for line in src.lines().map(str::trim) {
        if line == "#[cfg(feature = \"process\")]" {
            gated = true;
            continue;
        }
        if gated && !line.starts_with("#[") {
            let decl = line
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub ");
            if let Some(name) = decl.strip_prefix("mod ").and_then(|r| r.strip_suffix(';')) {
                out.push(name.trim().to_string());
            }
            gated = false;
        }
    }
    out
}

/// The directory a module file's children live in.
fn child_dir(file: &Path) -> PathBuf {
    let parent = file.parent().unwrap_or(Path::new(""));
    match file.file_name().and_then(|n| n.to_str()) {
        Some("lib.rs" | "main.rs" | "mod.rs") => parent.to_path_buf(),
        _ => parent.join(file.file_stem().unwrap_or_default()),
    }
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// The `.rs` files of a crate the lint reads, with process-gated modules removed.
pub fn lint_files(ctx: &Ctx) -> Vec<PathBuf> {
    let src = ctx.dir.join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let mut gated_dirs: Vec<PathBuf> = Vec::new();
    let mut gated_files: BTreeSet<PathBuf> = BTreeSet::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        for m in process_gated_mods(&strip_comments(&text)) {
            let d = child_dir(f);
            gated_files.insert(d.join(format!("{m}.rs")));
            gated_dirs.push(d.join(&m));
        }
    }
    let bins = src.join("bin");
    files.retain(|f| {
        !gated_files.contains(f)
            && !gated_dirs.iter().any(|d| f.starts_with(d))
            && (ctx.with_bins || !f.starts_with(&bins))
    });
    files
}

fn rel(ctx: &Ctx, f: &Path) -> String {
    f.strip_prefix(ctx.dir)
        .unwrap_or(f)
        .to_string_lossy()
        .into_owned()
}

fn allowed(ctx: &Ctx, rel: &str, modules: &[&str]) -> bool {
    ctx.name == "agy-cdp" && modules.contains(&rel)
}

fn token_rules(ctx: &Ctx, rel: &str, code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen_rustix_net = false;
    for (needle, label, modules) in RULES {
        if !code.contains(needle) || allowed(ctx, rel, modules) {
            continue;
        }
        if *needle == "rustix" && seen_rustix_net {
            continue;
        }
        seen_rustix_net |= *needle == "rustix::net";
        out.push(format!("{label}: `{needle}` is not allowed here"));
    }
    if FILE_CONFINED.contains(&ctx.name) && !(ctx.name == "agy-cdp" && FILE_ALLOWED.contains(&rel))
    {
        if let Some(n) = FILE_RULES.iter().find(|n| code.contains(*n)) {
            out.push(format!(
                "file: `{n}` outside open_under/operator_listing/proc_probe"
            ));
        }
    }
    if ctx.name == "agy-cdp" && rel == "src/operator_listing.rs" {
        out.extend(listing_rules(code));
    }
    out
}

/// `operator_listing` may only `read_dir` and `symlink_metadata`.
fn listing_rules(code: &str) -> Vec<String> {
    let reads = code
        .match_indices("fs::read")
        .any(|(i, _)| !code[i..].starts_with("fs::read_dir"));
    let mut out = Vec::new();
    for (bad, hit) in [
        ("File::open", code.contains("File::open")),
        ("fs::read*", reads),
        ("fs::copy", code.contains("fs::copy")),
    ] {
        if hit {
            out.push(format!(
                "credential: operator_listing may only read_dir/symlink_metadata, found `{bad}`"
            ));
        }
    }
    out
}

/// The string literal opening at `s` (which must start with `"`), if any.
fn literal_at(s: &str) -> Option<&str> {
    let rest = s.trim_start().strip_prefix('"')?;
    rest.find('"').map(|e| &rest[..e])
}

fn cargo_set(name: &str) -> bool {
    CARGO_SET.contains(&name) || CARGO_SET_FAMILIES.iter().any(|p| name.starts_with(p))
}

/// `env::var`/`var_os` of a name outside `RFML5_*`, and `env!`/`option_env!`
/// of a name outside `RFML5_*` and [`CARGO_SET`].
fn env_rules(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for call in ["env::var(", "env::var_os("] {
        for (i, _) in code.match_indices(call) {
            match literal_at(&code[i + call.len()..]) {
                Some(n) if n.starts_with("RFML5_") => {}
                Some(n) => out.push(format!("environment: `{call}\"{n}\")` outside RFML5_*")),
                None => out.push(format!("environment: `{call}…)` with a non-literal name")),
            }
        }
    }
    for (i, _) in code.match_indices("env!(") {
        match literal_at(&code[i + 5..]) {
            Some(n) if n.starts_with("RFML5_") || cargo_set(n) => {}
            Some(n) => out.push(format!(
                "environment: `env!(\"{n}\")` is not RFML5_* or Cargo-set"
            )),
            None => out.push("environment: `env!(…)` with a non-literal name".into()),
        }
    }
    out
}

/// Resolve `rel` lexically against `base`; `None` if it climbs above `root`.
fn lexical(root: &Path, base: &Path, rel: &str) -> Option<PathBuf> {
    let mut parts: Vec<PathBuf> = base
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|c| PathBuf::from(c.as_os_str()))
        .collect();
    for c in Path::new(rel).components() {
        match c {
            Component::Normal(n) => parts.push(PathBuf::from(n)),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts.iter().fold(root.to_path_buf(), |p, c| p.join(c)))
}

/// `include!`/`include_str!`/`include_bytes!`: a literal relative path that
/// stays inside the crate and names a regular, git-tracked file.
fn include_rules(ctx: &Ctx, file: &Path, code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let base = file.parent().unwrap_or(ctx.dir);
    for mac in ["include!(", "include_str!(", "include_bytes!("] {
        for (i, _) in code.match_indices(mac) {
            let Some(arg) = literal_at(&code[i + mac.len()..]) else {
                out.push(format!("include: `{mac}…)` with a non-literal path"));
                continue;
            };
            let target = lexical(ctx.dir, base, arg);
            let regular = target
                .as_ref()
                .and_then(|t| std::fs::symlink_metadata(t).ok())
                .is_some_and(|m| m.file_type().is_file());
            match target {
                None => out.push(format!("include: `{arg}` leaves the crate")),
                Some(_) if !regular => out.push(format!("include: `{arg}` is not a regular file")),
                Some(t) if !(ctx.tracked)(&t) => {
                    out.push(format!("include: `{arg}` is not git-tracked"))
                }
                Some(_) => {}
            }
        }
    }
    out
}

/// Every closure finding for one crate, as `<crate>/<file>: <finding>`.
pub fn lint_crate(ctx: &Ctx) -> Vec<String> {
    let mut out = Vec::new();
    if ctx.dir.join("build.rs").exists() {
        out.push(format!("{}: build.rs in the confined closure", ctx.name));
    }
    for f in lint_files(ctx) {
        let text = match std::fs::read_to_string(&f) {
            Ok(t) => t,
            Err(e) => {
                out.push(format!("{}: {e}", f.display()));
                continue;
            }
        };
        let code = code_only(&text);
        let r = rel(ctx, &f);
        let mut found = token_rules(ctx, &r, &code);
        found.extend(env_rules(&code));
        found.extend(include_rules(ctx, &f, &code));
        out.extend(found.into_iter().map(|x| format!("{}/{r}: {x}", ctx.name)));
    }
    out
}

/// WebSocket confinement over every member except `agy-cdp`: its manifest and
/// its sources (comments and tests included, because a test that opens a
/// WebSocket still opens one).
pub fn lint_ws(name: &str, dir: &Path) -> Vec<String> {
    if name == "agy-cdp" {
        return Vec::new();
    }
    let mut files = vec![dir.join("Cargo.toml")];
    rust_files(&dir.join("src"), &mut files);
    rust_files(&dir.join("tests"), &mut files);
    files
        .into_iter()
        .filter(|f| {
            std::fs::read_to_string(f)
                .map(|t| strip_comments(&t).contains(WS_CRATE))
                .unwrap_or(false)
        })
        .map(|f| {
            let r = f.strip_prefix(dir).unwrap_or(&f).display().to_string();
            format!("{name}/{r}: socket: the WebSocket crate is agy-cdp's alone")
        })
        .collect()
}

/// `name -> (dir, normal path-dependency names, has a build script)` for every
/// workspace member, from `cargo metadata --no-deps`.
pub type Members = BTreeMap<String, (PathBuf, Vec<String>, bool)>;

pub fn members(root: &Path) -> Result<Members, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = std::process::Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    let mut m = Members::new();
    for p in v["packages"].as_array().into_iter().flatten() {
        let name = p["name"].as_str().unwrap_or_default().to_string();
        let dir = Path::new(p["manifest_path"].as_str().unwrap_or_default())
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let deps = p["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| d["kind"].is_null() && d["path"].is_string())
            .filter_map(|d| d["name"].as_str().map(str::to_string))
            .collect();
        let build = p["targets"].as_array().into_iter().flatten().any(|t| {
            t["kind"]
                .as_array()
                .is_some_and(|k| k.iter().any(|k| k == "custom-build"))
        });
        m.insert(name, (dir, deps, build));
    }
    Ok(m)
}

/// The closure of [`CLOSURE_ROOTS`] over normal path dependencies.
pub fn closure(m: &Members) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut q: VecDeque<String> = CLOSURE_ROOTS.iter().map(|s| s.to_string()).collect();
    while let Some(n) = q.pop_front() {
        let Some((_, deps, _)) = m.get(&n) else {
            continue;
        };
        if seen.insert(n) {
            q.extend(deps.iter().cloned());
        }
    }
    seen
}

fn git_tracked(root: &Path, p: &Path) -> bool {
    std::process::Command::new("git")
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(p)
        .current_dir(root)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Both lints over the workspace at `root`.
pub fn lint_workspace(root: &Path) -> Vec<String> {
    let m = match members(root) {
        Ok(m) => m,
        Err(e) => return vec![e],
    };
    let mut out: Vec<String> = m.iter().flat_map(|(n, (d, _, _))| lint_ws(n, d)).collect();
    let set = closure(&m);
    for root_name in CLOSURE_ROOTS {
        if !set.contains(*root_name) {
            out.push(format!(
                "{root_name}: not a workspace member; the closure lint has no root"
            ));
        }
    }
    let tracked = |p: &Path| git_tracked(root, p);
    for name in &set {
        let (dir, _, build) = &m[name];
        if *build {
            out.push(format!("{name}: build script in the confined closure"));
        }
        let ctx = Ctx {
            name,
            dir,
            with_bins: FILE_CONFINED.contains(&name.as_str()),
            tracked: &tracked,
        };
        out.extend(lint_crate(&ctx));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xtask-confine-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        for (p, body) in files {
            let f = d.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, body).unwrap();
        }
        d
    }

    fn lint(name: &str, dir: &Path) -> Vec<String> {
        let yes = |_: &Path| true;
        lint_crate(&Ctx {
            name,
            dir,
            with_bins: true,
            tracked: &yes,
        })
    }

    #[test]
    fn clean_skeleton_passes() {
        let d = fixture(
            "clean",
            &[(
                "src/main.rs",
                "//! uses TcpStream in prose only\nfn main() { let _ = std::process::ExitCode::SUCCESS; }\n",
            )],
        );
        assert_eq!(lint("d20-agy-app-fanout", &d), Vec::<String>::new());
    }

    #[test]
    fn socket_process_and_env_are_refused_outside_their_modules() {
        let body = "use std::net::TcpStream;\nfn f() { std::process::Command::new(\"x\"); let _ = std::env::var(\"PATH\"); let _ = env!(\"CARGO_REGISTRY_TOKEN\"); }\n";
        let d = fixture("bad", &[("src/lib.rs", body)]);
        let f = lint("demo-kit", &d);
        for want in [
            "socket",
            "process",
            "`env::var(\"PATH\")`",
            "CARGO_REGISTRY_TOKEN",
        ] {
            assert!(f.iter().any(|l| l.contains(want)), "{want}: {f:?}");
        }
        // the same code is admitted in agy-cdp's two named modules, except env
        let d = fixture(
            "launch",
            &[("src/launch.rs", "fn f() { std::process::Command::new(\"x\"); let _ = std::env::var(\"RFML5_AGY_PROFILE\"); }\n")],
        );
        assert_eq!(lint("agy-cdp", &d), Vec::<String>::new());
    }

    #[test]
    fn profile_name_and_credentials_are_refused_outside_their_modules() {
        let d = fixture(
            "cred",
            &[(
                "src/lib.rs",
                concat!(
                    "const P: &str = \"RFML5_AGY_PROFILE\"; const H: &str = \"HOME\";",
                    " const G: &str = \"/srv/x/.gemini\"; const A: &str = \"/ho",
                    "me/x\";\n"
                ),
            )],
        );
        let f = lint("agy-cdp", &d);
        assert_eq!(f.len(), 4, "{f:?}");
        assert!(f.iter().any(|l| l.contains("RFML5_AGY_PROFILE")), "{f:?}");
        assert!(f.iter().any(|l| l.contains("\"HOME\"")), "{f:?}");
        assert!(f.iter().any(|l| l.contains(".gemini")), "{f:?}");
        assert!(f.iter().any(|l| l.contains("me/`")), "{f:?}");
    }

    #[test]
    fn process_gated_modules_and_tests_are_not_linted() {
        let d = fixture(
            "gated",
            &[
                ("src/lib.rs", "#[cfg(feature = \"process\")]\npub mod serve;\npub mod shapes;\n"),
                ("src/serve.rs", "use std::net::TcpStream;\n"),
                ("src/shapes.rs", "#[cfg(feature = \"process\")]\nmod run;\n#[cfg(test)]\nmod tests {\n    fn f() { std::process::Command::new(\"x\"); }\n}\npub fn ok() {}\n"),
                ("src/shapes/run.rs", "fn f() { std::process::Command::new(\"pv\"); }\n"),
            ],
        );
        assert_eq!(lint("demo-kit", &d), Vec::<String>::new());
        // an ungated module with the same text is refused
        let d = fixture(
            "ungated",
            &[
                ("src/lib.rs", "pub mod serve;\n"),
                ("src/serve.rs", "use std::net::TcpStream;\n"),
            ],
        );
        assert!(!lint("demo-kit", &d).is_empty());
    }

    #[test]
    fn file_rule_binds_only_confined_crates() {
        let body = "fn f() { let _ = std::fs::read(\"x\"); }\n";
        let d = fixture("fs", &[("src/main.rs", body)]);
        assert!(lint("d21-agy-app-fanin", &d)
            .iter()
            .any(|l| l.contains("file:")));
        assert!(lint("demo-kit", &d).is_empty());
        let d = fixture(
            "listing",
            &[(
                "src/operator_listing.rs",
                "fn f() { let _ = std::fs::read_dir(\"x\"); let _ = std::fs::read(\"y\"); }\n",
            )],
        );
        let f = lint("agy-cdp", &d);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("fs::read*"), "{f:?}");
    }

    #[test]
    fn include_must_stay_in_the_crate_and_be_a_tracked_regular_file() {
        let d = fixture(
            "inc",
            &[
                ("fixtures/m.json", "{}"),
                ("src/lib.rs", "const A: &str = include_str!(\"../fixtures/m.json\");\nconst B: &str = include_str!(\"../../x.json\");\nconst C: &str = include_str!(\"../fixtures/none.json\");\n"),
            ],
        );
        let f = lint("agy-cdp", &d);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f.iter().any(|l| l.contains("leaves the crate")), "{f:?}");
        assert!(f.iter().any(|l| l.contains("not a regular file")), "{f:?}");
        let no = |_: &Path| false;
        let f = lint_crate(&Ctx {
            name: "agy-cdp",
            dir: &d,
            with_bins: true,
            tracked: &no,
        });
        assert!(f.iter().any(|l| l.contains("not git-tracked")), "{f:?}");
    }

    #[test]
    fn build_script_is_refused() {
        let d = fixture(
            "build",
            &[("build.rs", "fn main() {}\n"), ("src/lib.rs", "")],
        );
        assert!(lint("demo-kit", &d).iter().any(|l| l.contains("build.rs")));
    }

    #[test]
    fn websocket_crate_is_agy_cdps_alone() {
        let manifest = format!("[dependencies]\n{WS_CRATE} = \"0.30\"\n");
        let d = fixture("ws", &[("Cargo.toml", &manifest), ("src/lib.rs", "")]);
        assert_eq!(lint_ws("d18-two-agents-one-server", &d).len(), 1);
        assert!(lint_ws("agy-cdp", &d).is_empty());
    }

    #[test]
    fn this_workspace_is_clean() {
        let f = lint_workspace(&crate::demos_root());
        assert_eq!(f, Vec::<String>::new());
    }
}
