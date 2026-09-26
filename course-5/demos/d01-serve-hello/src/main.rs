//! D01 — serve hello (lesson 3.1).
//!
//! Ask apr what this build can dispatch to, start one resident server on the
//! pinned Qwen 3.5 4B, probe its health route, and stream one completion.
//! Load time is spawn → first healthy answer; time to first token (TTFT) is
//! request written → first streamed content chunk. Both go to the receipt.
//!
//! Provable contract: serve-hello-v1 — the server became healthy on the GPU,
//! streamed a non-empty completion, and TTFT is a measured receipt field
//! strictly between zero and the end-to-end time.

use demo_kit::harness::Harness;
use demo_kit::serve::{http_get, ServeGuard};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

const PROMPT: &str = "Say hello to a Rust developer in one short sentence.";

/// Stream one chat completion. Returns (ttft_ms, e2e_ms, text).
fn stream_completion(addr: SocketAddr) -> std::io::Result<(u128, u128, String)> {
    let body = json!({
        "model": "default",
        "messages": [{"role": "user", "content": PROMPT}],
        "max_tokens": 32, "temperature": 0, "stream": true
    })
    .to_string();
    let mut s = TcpStream::connect(addr)?;
    s.set_read_timeout(Some(Duration::from_secs(300)))?;
    let req = format!(
        "POST /v1/chat/completions/stream HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let t = Instant::now();
    s.write_all(req.as_bytes())?;
    let (mut ttft, mut text) = (None, String::new());
    for line in BufReader::new(s).lines() {
        let line = line?;
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data == "[DONE]" {
            break;
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(piece) = chunk["choices"][0]["delta"]["content"].as_str() {
            if !piece.is_empty() && ttft.is_none() {
                ttft = Some(t.elapsed().as_millis());
            }
            text.push_str(piece);
        }
    }
    let e2e = t.elapsed().as_millis();
    Ok((ttft.unwrap_or(0), e2e, text))
}

fn main() {
    let mut h = Harness::load(env!("CARGO_MANIFEST_DIR"));
    let mut measured: BTreeMap<String, Value> = BTreeMap::new();
    if h.not_run().is_empty() {
        let model = h.dir.join(&h.manifest.model.as_ref().expect("model").path);
        let model = model.to_string_lossy().into_owned();
        let (code, devices, _) = h.step("apr", &["serve", "run", "--list-devices"]);
        println!("{}", String::from_utf8_lossy(&devices).trim_end());
        measured.insert("list_devices_exit".into(), json!(code));

        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .unwrap_or(18181);
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");
        let port_s = port.to_string();
        println!("$ apr serve run <model> --gpu-layers all --port {port}");
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
                println!("healthy after {} ms", guard.load_ms);
                measured.insert("load_ms".into(), json!(guard.load_ms));
                if let Ok((200, body)) = http_get(addr, "/health") {
                    println!("GET /health -> {body}");
                    let health: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                    let mode = health["compute_mode"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string();
                    measured.insert("health_ok".into(), json!(health["status"] == "ok"));
                    measured.insert("on_gpu".into(), json!(mode != "cpu" && mode != "unknown"));
                    measured.insert("compute_mode".into(), json!(mode));
                }
                match stream_completion(addr) {
                    Ok((ttft, e2e, text)) => {
                        println!("completion: {}", text.trim());
                        println!("ttft {ttft} ms, end-to-end {e2e} ms");
                        measured.insert("ttft_ms".into(), json!(ttft));
                        measured.insert("e2e_ms".into(), json!(e2e));
                        measured.insert("text_nonempty".into(), json!(!text.trim().is_empty()));
                        measured.insert("ttft_within_e2e".into(), json!(ttft > 0 && ttft < e2e));
                    }
                    Err(e) => println!("stream failed: {e}"),
                }
                measured.insert("exit".into(), json!(0));
            }
            Err(e) => println!("server did not come up: {e}"),
        }
    }
    let verdict = h.finish(measured.clone());

    assert!(verdict.is_green(), "D01 is {verdict}");
    assert_eq!(
        measured["ttft_within_e2e"],
        json!(true),
        "serve-hello-v1: ttft"
    );
    println!("contract: serve-hello-v1 OK");
}
