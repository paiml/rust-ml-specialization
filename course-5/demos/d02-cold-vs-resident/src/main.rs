//! D02 — cold runs, measured (lesson 1.1).
//!
//! Five cold `apr run --json` calls, each a fresh process that loads the
//! model, then five requests to one resident `apr serve run` that loaded it
//! once. Every number printed here is written to the receipt; the narration
//! may cite only those fields.
//!
//! Provable contract: cold-vs-resident-v1 — exactly RUNS cold and RUNS
//! resident completions ran on the GPU, each returned text, and every
//! latency on screen is a receipt field.

use demo_kit::harness::Harness;
use demo_kit::serve::{http_request, ServeGuard};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::net::{SocketAddr, TcpListener};
use std::time::{Duration, Instant};

const RUNS: usize = 5;
const PROMPT: &str = "In one sentence, what does a reducer do in a fan-out pipeline?";
const MAX_TOKENS: &str = "32";

fn median(v: &mut [u128]) -> u128 {
    v.sort_unstable();
    v[v.len() / 2]
}

fn main() {
    let mut h = Harness::load(env!("CARGO_MANIFEST_DIR"));
    let mut measured: BTreeMap<String, Value> = BTreeMap::new();
    if h.not_run().is_empty() {
        let model = h.dir.join(&h.manifest.model.as_ref().expect("model").path);
        let model = model.to_string_lossy().into_owned();

        println!("-- cold: {RUNS} fresh processes --");
        let (mut cold, mut cold_gpu, mut cold_text) = (Vec::new(), 0usize, 0usize);
        for i in 1..=RUNS {
            let args = [
                "run",
                &model,
                "--prompt",
                PROMPT,
                "-n",
                MAX_TOKENS,
                "--temperature",
                "0",
                "--seed",
                "42",
                "--json",
            ];
            let (code, stdout, ms) = h.step("apr", &args);
            let r: Value = serde_json::from_slice(&stdout).unwrap_or(Value::Null);
            let gpu = r["used_gpu"].as_bool() == Some(true);
            let text = r["text"].as_str().is_some_and(|t| !t.trim().is_empty());
            println!("  cold {i}: exit {code}, {ms} ms wall, gpu {gpu}");
            if code == 0 {
                cold.push(ms);
            }
            cold_gpu += usize::from(gpu);
            cold_text += usize::from(text);
        }

        println!("-- resident: one server, {RUNS} requests --");
        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .unwrap_or(18180);
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");
        let port_s = port.to_string();
        let mut resident = Vec::new();
        let mut resident_text = 0usize;
        match ServeGuard::spawn(
            "apr",
            &[
                "serve",
                "run",
                &model,
                "--gpu-layers",
                "all",
                "--port",
                &port_s,
            ],
            addr,
            "/health",
            Duration::from_secs(600),
        ) {
            Ok(guard) => {
                println!("  server healthy after {} ms (load)", guard.load_ms);
                measured.insert("load_ms".into(), json!(guard.load_ms));
                let body = json!({
                    "model": "default",
                    "messages": [{"role": "user", "content": PROMPT}],
                    "max_tokens": 32, "temperature": 0
                })
                .to_string();
                for i in 1..=RUNS {
                    let t = Instant::now();
                    let r = http_request(addr, "POST", "/v1/chat/completions", Some(&body));
                    let ms = t.elapsed().as_millis();
                    let ok = r.as_ref().is_ok_and(|(c, _)| *c == 200);
                    let text = r
                        .as_ref()
                        .ok()
                        .and_then(|(_, b)| serde_json::from_str::<Value>(b).ok())
                        .is_some_and(|v| {
                            v["choices"][0]["message"]["content"]
                                .as_str()
                                .is_some_and(|t| !t.trim().is_empty())
                        });
                    println!(
                        "  resident {i}: {} , {ms} ms",
                        if ok { "200" } else { "error" }
                    );
                    if ok {
                        resident.push(ms);
                    }
                    resident_text += usize::from(text);
                }
            }
            Err(e) => println!("  server did not come up: {e}"),
        }

        measured.insert("cold_runs".into(), json!(cold.len()));
        measured.insert("cold_on_gpu".into(), json!(cold_gpu));
        measured.insert("resident_runs".into(), json!(resident.len()));
        measured.insert("texts_returned".into(), json!(cold_text + resident_text));
        measured.insert("cold_ms".into(), json!(cold.clone()));
        measured.insert("resident_ms".into(), json!(resident.clone()));
        if !cold.is_empty() {
            measured.insert("cold_median_ms".into(), json!(median(&mut cold)));
        }
        if !resident.is_empty() {
            measured.insert("resident_median_ms".into(), json!(median(&mut resident)));
        }
        measured.insert("exit".into(), json!(0));
    }
    let verdict = h.finish(measured.clone());

    assert!(verdict.is_green(), "D02 is {verdict}");
    assert_eq!(
        measured["cold_runs"],
        json!(RUNS),
        "cold-vs-resident-v1: cold runs"
    );
    assert_eq!(
        measured["resident_runs"],
        json!(RUNS),
        "cold-vs-resident-v1: resident runs"
    );
    println!("contract: cold-vs-resident-v1 OK");
}
