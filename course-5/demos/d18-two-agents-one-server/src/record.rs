//! The run record the shapes judge (spec §4): what the demo measured, and
//! nothing else. Its keys are exactly the closed shapes' properties in
//! `spec/d18-run-v1.yaml`; each item is a named node `q1..q4`.

use crate::checks::Times;
use crate::ident::Ident;
use serde_json::{json, Map, Value};

/// Everything one run contributes to its record.
pub struct Inputs<'a> {
    pub apr_version: &'a str,
    pub model_sha256: &'a str,
    pub server_spawns: u32,
    pub start: &'a Ident,
    pub end: &'a Ident,
    pub writer_files: &'a [String],
    pub checker_files: &'a [String],
    pub digest_sequential: &'a str,
    pub digest_pipelined: &'a str,
    pub seq: &'a [Times; 4],
    pub pipe: &'a [Times; 4],
    pub decisions: &'a [String; 4],
}

const FIELDS: [&str; 4] = ["writer_start", "writer_end", "checker_start", "checker_end"];

fn get(t: &Times, field: &str) -> u64 {
    match field {
        "writer_start" => t.writer_start,
        "writer_end" => t.writer_end,
        "checker_start" => t.checker_start,
        _ => t.checker_end,
    }
}

fn set(t: &mut Times, field: &str, v: u64) {
    match field {
        "writer_start" => t.writer_start = v,
        "writer_end" => t.writer_end = v,
        "checker_start" => t.checker_start = v,
        _ => t.checker_end = v,
    }
}

fn item(seq: &Times, pipe: &Times, decision: &str) -> Value {
    let mut m = Map::new();
    for (prefix, t) in [("seq", seq), ("pipe", pipe)] {
        for f in FIELDS {
            m.insert(format!("{prefix}_{f}_ms"), json!(get(t, f)));
        }
    }
    m.insert("decision".into(), json!(decision));
    Value::Object(m)
}

pub fn build(i: &Inputs) -> Value {
    let mut r = json!({
        "demo": "d18-two-agents-one-server",
        "apr_version": i.apr_version,
        "model_sha256": i.model_sha256,
        "server_spawns": i.server_spawns,
        "server_pid_start": i.start.pid,
        "server_pid_end": i.end.pid,
        "server_start_ticks_start": i.start.start_ticks,
        "server_start_ticks_end": i.end.start_ticks,
        "writer_files": i.writer_files,
        "checker_files": i.checker_files,
        "digest_sequential": i.digest_sequential,
        "digest_pipelined": i.digest_pipelined,
    });
    for (n, id) in crate::item_ids().iter().enumerate() {
        r[*id] = item(&i.seq[n], &i.pipe[n], &i.decisions[n]);
    }
    r
}

/// The (sequential, pipelined) times of a record, `None` if any is missing.
pub fn times_from(record: &Value) -> Option<([Times; 4], [Times; 4])> {
    let mut seq = [Times::default(); 4];
    let mut pipe = [Times::default(); 4];
    for (n, id) in crate::item_ids().iter().enumerate() {
        for (prefix, t) in [("seq", &mut seq[n]), ("pipe", &mut pipe[n])] {
            for f in FIELDS {
                set(t, f, record[*id][format!("{prefix}_{f}_ms")].as_u64()?);
            }
        }
    }
    Some((seq, pipe))
}
