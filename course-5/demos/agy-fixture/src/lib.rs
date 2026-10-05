//! agy-fixture: keep the demo profile's conversation list short between takes.
//!
//! Every take adds three conversations to the demo profile, and the app shows
//! only so many rows: once enough have gathered, a new run's rows can fall out
//! of view. This crate archives conversations by id, the same way the app's
//! own "Archive conversation" button does: it sets `archived:true` and an
//! archival timestamp in that conversation's annotation file. Nothing is ever
//! deleted, and every archive is logged with the annotation's prior text, so
//! [`restore`] can put it back exactly.
//!
//! What it may touch, and nothing else, inside the profile's home:
//! - the NAMES under `.gemini/antigravity/conversations/` (never contents);
//! - `.gemini/antigravity/annotations/<id>.pbtxt`, where `<id>` is a UUID.
//!
//! It refuses ([`guard`]) any home that is not under a directory named
//! `agy-demo-profile` inside one named `rfml5-newdemos`, after symlinks are
//! resolved, and it refuses ([`holders`]) to write while any process has a file
//! open under that home: the app must be down, or it could write over the edit.
//!
//! It is not in d20's or d21's dependency closure; the demos never write the
//! profile.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

/// The two directory names the demo profile must sit under, in order.
pub const PROFILE_PARENTS: [&str; 2] = ["rfml5-newdemos", "agy-demo-profile"];

const APP_DIR: [&str; 2] = [".gemini", "antigravity"];

/// Resolve `home` and refuse it unless it is strictly below
/// `…/rfml5-newdemos/agy-demo-profile/`. Returns the resolved path.
pub fn guard(home: &Path) -> Result<PathBuf, String> {
    let real = std::fs::canonicalize(home)
        .map_err(|e| format!("refused: home {} does not resolve: {e}", home.display()))?;
    let names: Vec<&std::ffi::OsStr> = real
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s),
            _ => None,
        })
        .collect();
    let under = names.windows(2).enumerate().any(|(i, w)| {
        w[0] == PROFILE_PARENTS[0] && w[1] == PROFILE_PARENTS[1] && i + 2 < names.len()
    });
    if under {
        Ok(real)
    } else {
        Err(format!(
            "refused: {} is not under {}/{}",
            real.display(),
            PROFILE_PARENTS[0],
            PROFILE_PARENTS[1]
        ))
    }
}

/// A conversation id: a lowercase UUID, `8-4-4-4-12` hex. Anything else is
/// refused, so no id can name a path.
pub fn is_id(s: &str) -> bool {
    let lens = [8, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == lens.len()
        && parts.iter().zip(lens).all(|(p, n)| {
            p.len() == n
                && p.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

fn app_dir(home: &Path) -> PathBuf {
    APP_DIR.iter().fold(home.to_path_buf(), |p, d| p.join(d))
}

fn annotation(home: &Path, id: &str) -> Result<PathBuf, String> {
    if !is_id(id) {
        return Err(format!("refused: `{id}` is not a conversation id"));
    }
    Ok(app_dir(home)
        .join("annotations")
        .join(format!("{id}.pbtxt")))
}

/// The conversation ids in the profile, from the names under
/// `conversations/` (`<id>.db`). Names only; no file is opened.
pub fn ids(home: &Path) -> Result<BTreeSet<String>, String> {
    let dir = app_dir(home).join("conversations");
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = BTreeSet::new();
    for e in rd {
        let name = e.map_err(|e| e.to_string())?.file_name();
        let name = name.to_string_lossy();
        if let Some(id) = name.strip_suffix(".db") {
            if is_id(id) {
                out.insert(id.to_string());
            }
        }
    }
    Ok(out)
}

/// Whether an annotation's text marks it archived.
pub fn is_archived(text: &str) -> bool {
    text.split_whitespace().any(|t| t == "archived:true")
}

/// The annotation with `archived:true` and an archival timestamp at `secs`.
/// Any earlier `archived:` and `archival_status_timestamp:` fields are
/// replaced; every other field is kept as it was. `None` when already archived.
pub fn archived_text(text: &str, secs: u64) -> Option<String> {
    if is_archived(text) {
        return None;
    }
    let mut kept = String::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let (field, tail) = split_field(rest);
        if !field.starts_with("archived:") && !field.starts_with("archival_status_timestamp:") {
            if !kept.is_empty() {
                kept.push(' ');
            }
            kept.push_str(field);
        }
        rest = tail.trim_start();
    }
    let head = format!("archived:true archival_status_timestamp:{{seconds:{secs} nanos:0}}");
    Some(if kept.is_empty() {
        head
    } else {
        format!("{head} {kept}")
    })
}

/// One top-level text-proto field (`name:value` or `name:{…}`) and the rest.
fn split_field(s: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return (&s[..=i], &s[i + 1..]);
                }
            }
            c if c.is_whitespace() && depth == 0 => return (&s[..i], &s[i..]),
            _ => {}
        }
    }
    (s, "")
}

/// `(active, archived)` over every conversation id in the profile. An id with
/// no annotation file counts as active, as the app shows it.
pub fn counts(home: &Path) -> Result<(usize, usize), String> {
    let mut active = 0;
    let mut archived = 0;
    for id in ids(home)? {
        match read_annotation(home, &id)? {
            Some(t) if is_archived(&t) => archived += 1,
            _ => active += 1,
        }
    }
    Ok((active, archived))
}

fn read_annotation(home: &Path, id: &str) -> Result<Option<String>, String> {
    let p = annotation(home, id)?;
    match std::fs::read_to_string(&p) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

fn write_annotation(home: &Path, id: &str, text: &str) -> Result<(), String> {
    let p = annotation(home, id)?;
    let tmp = p.with_file_name(format!(".{id}.pbtxt.tmp"));
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &p).map_err(|e| format!("{}: {e}", p.display()))
}

/// Pids (other than this one) with a file, a cwd or a root open under `home`.
/// Read from `/proc/<pid>/{fd/*,cwd,root}` link targets only; a process this
/// user cannot inspect is skipped.
pub fn holders(home: &Path) -> Vec<u32> {
    let me = std::process::id();
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        if pid == me {
            continue;
        }
        let base = e.path();
        let mut links: Vec<PathBuf> = vec![base.join("cwd"), base.join("root")];
        if let Ok(fds) = std::fs::read_dir(base.join("fd")) {
            links.extend(fds.flatten().map(|f| f.path()));
        }
        if links
            .iter()
            .filter_map(|l| std::fs::read_link(l).ok())
            .any(|t| t.starts_with(home))
        {
            out.push(pid);
        }
    }
    out
}

/// One archived id and the annotation text it had before (empty when it had
/// no file), enough for [`restore`].
#[derive(Debug, Clone, PartialEq)]
pub struct Archived {
    pub id: String,
    pub prior: String,
}

/// What one archive call did.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// `(active, archived)` before.
    pub before: (usize, usize),
    pub archived: Vec<Archived>,
    /// Ids asked for that were already archived.
    pub already: Vec<String>,
    /// `(active, archived)` after.
    pub after: (usize, usize),
}

fn require_quiet(home: &Path) -> Result<(), String> {
    let h = holders(home);
    if h.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "refused: {} process(es) have files open under the profile; stop the app first",
            h.len()
        ))
    }
}

/// Archive `want` (each a conversation id present in the profile) at `secs`.
/// Refused before any write if the home fails [`guard`], a process holds
/// files under it, or any id is malformed or absent.
pub fn archive(home: &Path, want: &BTreeSet<String>, secs: u64) -> Result<Report, String> {
    let home = guard(home)?;
    require_quiet(&home)?;
    let present = ids(&home)?;
    if let Some(bad) = want.iter().find(|id| !is_id(id) || !present.contains(*id)) {
        return Err(format!(
            "refused: `{bad}` is not a conversation in this profile"
        ));
    }
    let before = counts(&home)?;
    let mut archived = Vec::new();
    let mut already = Vec::new();
    for id in want {
        let prior = read_annotation(&home, id)?.unwrap_or_default();
        match archived_text(&prior, secs) {
            Some(text) => {
                write_annotation(&home, id, &text)?;
                archived.push(Archived {
                    id: id.clone(),
                    prior,
                });
            }
            None => already.push(id.clone()),
        }
    }
    let after = counts(&home)?;
    Ok(Report {
        before,
        archived,
        already,
        after,
    })
}

/// Every active id: the backlog.
pub fn active_ids(home: &Path) -> Result<BTreeSet<String>, String> {
    let home = guard(home)?;
    let mut out = BTreeSet::new();
    for id in ids(&home)? {
        if !read_annotation(&home, &id)?.is_some_and(|t| is_archived(&t)) {
            out.insert(id);
        }
    }
    Ok(out)
}

/// Put each annotation back to its logged prior text. Same refusals as
/// [`archive`]. An empty prior text writes an empty annotation (the app reads
/// it as no fields set), never a deletion.
pub fn restore(home: &Path, rows: &[Archived]) -> Result<usize, String> {
    let home = guard(home)?;
    require_quiet(&home)?;
    for r in rows {
        annotation(&home, &r.id)?;
    }
    for r in rows {
        write_annotation(&home, &r.id, &r.prior)?;
    }
    Ok(rows.len())
}

/// Log lines: one `archived<TAB>id<TAB>prior` per id (prior with tabs and
/// newlines escaped), then nothing else; [`parse_log`] reads them back.
pub fn log_rows(rows: &[Archived]) -> String {
    rows.iter()
        .map(|r| {
            let prior = r
                .prior
                .replace('\\', "\\\\")
                .replace('\t', "\\t")
                .replace('\n', "\\n");
            format!("archived\t{}\t{prior}\n", r.id)
        })
        .collect()
}

/// The `archived` rows of a log; every other line is ignored.
pub fn parse_log(text: &str) -> Result<Vec<Archived>, String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut cols = line.splitn(3, '\t');
        if cols.next() != Some("archived") {
            continue;
        }
        let id = cols.next().unwrap_or_default();
        if !is_id(id) {
            return Err(format!("log: `{id}` is not a conversation id"));
        }
        let mut prior = String::new();
        let mut esc = false;
        for c in cols.next().unwrap_or_default().chars() {
            match (esc, c) {
                (true, 't') => prior.push('\t'),
                (true, 'n') => prior.push('\n'),
                (true, c) => prior.push(c),
                (false, '\\') => {
                    esc = true;
                    continue;
                }
                (false, c) => prior.push(c),
            }
            esc = false;
        }
        out.push(Archived {
            id: id.to_string(),
            prior,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "031cf040-7ca0-46e6-ae1c-72ea4eceaa38";
    const B: &str = "065b78b4-ba67-441c-973d-d6ad7137ed52";
    const C: &str = "08965e17-e056-477d-a9b5-b32d1dbbbe79";

    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(tag: &str) -> Scratch {
        let d = std::env::temp_dir().join(format!("agy-fixture-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Scratch(d)
    }

    /// A demo-shaped home with three conversations: A active with a view
    /// time, B archived, C with no annotation file. A credential-shaped file
    /// sits beside them, and no test path reads or writes it.
    fn demo_home(root: &Path) -> PathBuf {
        let home = root
            .join(PROFILE_PARENTS[0])
            .join(PROFILE_PARENTS[1])
            .join("home");
        let app = app_dir(&home);
        std::fs::create_dir_all(app.join("conversations")).unwrap();
        std::fs::create_dir_all(app.join("annotations")).unwrap();
        for id in [A, B, C] {
            std::fs::write(app.join("conversations").join(format!("{id}.db")), "").unwrap();
        }
        std::fs::write(
            app.join("annotations").join(format!("{A}.pbtxt")),
            "last_user_view_time:{seconds:5 nanos:6}",
        )
        .unwrap();
        std::fs::write(
            app.join("annotations").join(format!("{B}.pbtxt")),
            "archived:true archival_status_timestamp:{seconds:1 nanos:2}",
        )
        .unwrap();
        std::fs::write(home.join(".gemini").join("oauth_creds.json"), "SECRET").unwrap();
        home
    }

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_guard_admits_only_a_home_under_the_demo_profile() {
        let s = scratch("guard");
        let home = demo_home(&s.0);
        assert!(guard(&home).is_ok());
        // The profile directory itself is not a home below it.
        assert!(guard(home.parent().unwrap()).is_err());
        // A home elsewhere, and one under only one of the two names.
        let other = s.0.join("home");
        std::fs::create_dir_all(&other).unwrap();
        assert!(guard(&other).is_err());
        let half = s.0.join(PROFILE_PARENTS[1]).join("home");
        std::fs::create_dir_all(&half).unwrap();
        assert!(guard(&half).is_err());
        let swapped =
            s.0.join(PROFILE_PARENTS[1])
                .join(PROFILE_PARENTS[0])
                .join("h");
        std::fs::create_dir_all(&swapped).unwrap();
        assert!(guard(&swapped).is_err());
        // A path that does not exist.
        assert!(guard(&s.0.join("nope")).is_err());
    }

    #[test]
    fn the_guard_refuses_a_symlink_named_like_the_profile_but_pointing_elsewhere() {
        let s = scratch("link");
        let elsewhere = s.0.join("operator-home");
        std::fs::create_dir_all(elsewhere.join("home")).unwrap();
        let parent = s.0.join(PROFILE_PARENTS[0]);
        std::fs::create_dir_all(&parent).unwrap();
        std::os::unix::fs::symlink(&elsewhere, parent.join(PROFILE_PARENTS[1])).unwrap();
        let named = parent.join(PROFILE_PARENTS[1]).join("home");
        let e = guard(&named).unwrap_err();
        assert!(e.starts_with("refused:"), "{e}");
    }

    #[test]
    fn archive_refuses_a_home_outside_the_profile_and_writes_nothing() {
        let s = scratch("refuse");
        let home = s.0.join("elsewhere").join("home");
        let app = app_dir(&home);
        std::fs::create_dir_all(app.join("conversations")).unwrap();
        std::fs::create_dir_all(app.join("annotations")).unwrap();
        std::fs::write(app.join("conversations").join(format!("{A}.db")), "").unwrap();
        let e = archive(&home, &set(&[A]), 9).unwrap_err();
        assert!(e.starts_with("refused:"), "{e}");
        assert!(!app.join("annotations").join(format!("{A}.pbtxt")).exists());
        assert!(active_ids(&home).is_err());
        assert!(restore(&home, &[]).is_err());
    }

    #[test]
    fn ids_must_be_uuids_so_no_id_names_a_path() {
        assert!(is_id(A));
        for bad in [
            "",
            "../x",
            "031CF040-7CA0-46E6-AE1C-72EA4ECEAA38",
            "oauth_creds",
            "a-b-c-d-e",
        ] {
            assert!(!is_id(bad), "{bad}");
            assert!(annotation(Path::new("/x"), bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn archived_text_keeps_other_fields_and_replaces_an_old_flag() {
        assert_eq!(
            archived_text("last_user_view_time:{seconds:5 nanos:6}", 9).unwrap(),
            "archived:true archival_status_timestamp:{seconds:9 nanos:0} last_user_view_time:{seconds:5 nanos:6}"
        );
        assert_eq!(
            archived_text("", 9).unwrap(),
            "archived:true archival_status_timestamp:{seconds:9 nanos:0}"
        );
        assert_eq!(
            archived_text(
                "archived:false archival_status_timestamp:{seconds:1 nanos:2} x:3",
                9
            )
            .unwrap(),
            "archived:true archival_status_timestamp:{seconds:9 nanos:0} x:3"
        );
        assert_eq!(archived_text("archived:true", 9), None);
    }

    #[test]
    fn archive_flags_only_the_asked_ids_counts_and_restores() {
        let s = scratch("arch");
        let home = demo_home(&s.0);
        assert_eq!(counts(&home).unwrap(), (2, 1));
        assert_eq!(active_ids(&home).unwrap(), set(&[A, C]));
        let r = archive(&home, &set(&[A, B]), 9).unwrap();
        assert_eq!(r.before, (2, 1));
        assert_eq!(r.after, (1, 2));
        assert_eq!(r.already, vec![B.to_string()]);
        assert_eq!(
            r.archived,
            vec![Archived {
                id: A.into(),
                prior: "last_user_view_time:{seconds:5 nanos:6}".into()
            }]
        );
        // C was not asked for and is untouched; nothing was deleted.
        assert_eq!(active_ids(&home).unwrap(), set(&[C]));
        assert_eq!(ids(&home).unwrap(), set(&[A, B, C]));
        // The log round-trips and restore puts A back byte for byte.
        let rows = parse_log(&log_rows(&r.archived)).unwrap();
        assert_eq!(rows, r.archived);
        assert_eq!(restore(&home, &rows).unwrap(), 1);
        assert_eq!(
            read_annotation(&home, A).unwrap().unwrap(),
            "last_user_view_time:{seconds:5 nanos:6}"
        );
        assert_eq!(counts(&home).unwrap(), (2, 1));
        // The credential-shaped file is never written.
        assert_eq!(
            std::fs::read_to_string(home.join(".gemini").join("oauth_creds.json")).unwrap(),
            "SECRET"
        );
    }

    #[test]
    fn archive_refuses_an_absent_or_malformed_id_before_any_write() {
        let s = scratch("absent");
        let home = demo_home(&s.0);
        let absent = "11111111-2222-3333-4444-555555555555";
        assert!(archive(&home, &set(&[A, absent]), 9).is_err());
        assert!(archive(&home, &set(&[A, "../oauth_creds"]), 9).is_err());
        assert_eq!(
            counts(&home).unwrap(),
            (2, 1),
            "A must not have been archived"
        );
    }

    #[test]
    fn archive_refuses_while_another_process_holds_a_file_under_the_home() {
        let s = scratch("held");
        let home = demo_home(&s.0);
        let real = guard(&home).unwrap();
        let held = real.join(".gemini").join("antigravity").join("held.log");
        std::fs::write(&held, "").unwrap();
        let mut child = std::process::Command::new("sh")
            .arg("-c")
            .arg("exec 3<\"$1\"; read _ ")
            .arg("sh")
            .arg(&held)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        // Wait (bounded) until the child has the descriptor open.
        let t0 = std::time::Instant::now();
        while !holders(&real).contains(&child.id()) {
            assert!(t0.elapsed().as_secs() < 10, "child never opened the file");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let e = archive(&home, &set(&[A]), 9).unwrap_err();
        drop(child.stdin.take());
        let _ = child.wait();
        assert!(e.contains("stop the app"), "{e}");
        assert_eq!(counts(&home).unwrap(), (2, 1));
        // With the holder gone the same call goes through.
        assert_eq!(archive(&home, &set(&[A]), 9).unwrap().after, (1, 2));
    }

    #[test]
    fn a_log_with_a_bad_id_is_refused_and_escapes_round_trip() {
        assert!(parse_log("archived\t../x\tp\n").is_err());
        let rows = vec![Archived {
            id: A.into(),
            prior: "a\tb\nc\\d".into(),
        }];
        assert_eq!(
            parse_log(&format!("# note\n{}", log_rows(&rows))).unwrap(),
            rows
        );
    }
}
