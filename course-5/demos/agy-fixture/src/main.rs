//! agy-fixture: archive demo-profile conversations between takes.
//!
//! ```text
//! agy-fixture ids     --home H                       ids, one per line
//! agy-fixture archive --home H --log L --backlog     every active id
//! agy-fixture archive --home H --log L --new-since F --expect N
//!                     ids not listed in F (an earlier `ids`); exactly N
//! agy-fixture restore --home H --log L               undo every row in L
//! ```
//!
//! `archive` appends to L the counts before, one `archived<TAB>id<TAB>prior`
//! row per id, and the counts after; `restore` reads those rows back. Every
//! command refuses a home that is not under the demo profile, and the writing
//! ones refuse while any process has a file open under it. Exit 0 done, 1
//! refused or failed, 2 usage.
//!
//! Provable contract: archive-is-a-pure-move — after an archive, archived
//! rises and active falls by exactly the number of ids archived, and the set
//! of conversation ids is unchanged (nothing deleted).

use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use agy_fixture as fx;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn append(log: &str, text: &str) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut f| f.write_all(text.as_bytes()))
        .map_err(|e| format!("{log}: {e}"))
}

fn now_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let home = PathBuf::from(arg(args, "--home").ok_or("usage: --home H")?);
    match cmd {
        "ids" => {
            let home = fx::guard(&home)?;
            for id in fx::ids(&home)? {
                println!("{id}");
            }
            Ok(())
        }
        "archive" => {
            let log = arg(args, "--log").ok_or("usage: --log L")?;
            let (label, want) = if args.iter().any(|a| a == "--backlog") {
                ("backlog", fx::active_ids(&home)?)
            } else {
                let since = arg(args, "--new-since").ok_or("usage: --backlog or --new-since F")?;
                let expect: usize = arg(args, "--expect")
                    .ok_or("usage: --expect N")?
                    .parse()
                    .map_err(|_| "usage: --expect N")?;
                let old: BTreeSet<String> = std::fs::read_to_string(&since)
                    .map_err(|e| format!("{since}: {e}"))?
                    .lines()
                    .map(str::to_string)
                    .collect();
                let new: BTreeSet<String> = fx::ids(&fx::guard(&home)?)?
                    .difference(&old)
                    .cloned()
                    .collect();
                if new.len() != expect {
                    return Err(format!(
                        "refused: {} new conversation(s), expected {expect}",
                        new.len()
                    ));
                }
                ("run", new)
            };
            let ids_before = fx::ids(&fx::guard(&home)?)?;
            let r = fx::archive(&home, &want, now_s())?;
            let summary = format!(
                "{label}: before active={} archived={}; archived {}; already {}; after active={} archived={}\n",
                r.before.0,
                r.before.1,
                r.archived.len(),
                r.already.len(),
                r.after.0,
                r.after.1
            );
            append(&log, &format!("# {summary}{}", fx::log_rows(&r.archived)))?;
            for a in &r.archived {
                println!("archived {}", a.id);
            }
            print!("{summary}");
            let n = r.archived.len();
            assert_eq!(r.after.1, r.before.1 + n, "archived must rise by {n}");
            assert_eq!(r.after.0 + n, r.before.0, "active must fall by {n}");
            assert_eq!(fx::ids(&fx::guard(&home)?)?, ids_before, "no id may vanish");
            println!("contract: archive-is-a-pure-move OK");
            Ok(())
        }
        "restore" => {
            let log = arg(args, "--log").ok_or("usage: --log L")?;
            let text = std::fs::read_to_string(&log).map_err(|e| format!("{log}: {e}"))?;
            let rows = fx::parse_log(&text)?;
            let n = fx::restore(&home, &rows)?;
            println!("restored {n}");
            Ok(())
        }
        _ => Err("usage: agy-fixture ids|archive|restore --home H …".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.starts_with("usage") => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
