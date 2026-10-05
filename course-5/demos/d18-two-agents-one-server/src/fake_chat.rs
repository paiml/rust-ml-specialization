//! The body of `d18-fake-apr-serve`: an `apr serve run` stand-in for CI, with
//! no GPU and no model. It takes `apr serve run`'s arguments, uses only
//! `--port`, and serves:
//!
//! - `GET /health` → 200 `{"status":"ok"}`;
//! - `POST /v1/chat/completions` → 200 with one choice, after a fixed delay so
//!   two requests in flight at once overlap on a millisecond clock.
//!
//! Connections are served on their own threads, so the writer and the checker
//! can both be in flight, as they are against the real server. Replies are
//! deterministic: the writer's answer is derived from the question, and the
//! checker accepts or rejects by a fixed rule over the bytes it was shown.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// How long the fake writer and checker take to answer.
pub const WRITER_DELAY: Duration = Duration::from_millis(40);
pub const CHECKER_DELAY: Duration = Duration::from_millis(30);

/// Run the fake with `args` (`args[0]` is the program). Returns only on a
/// usage or bind error.
pub fn run(args: &[String]) -> Result<(), String> {
    let port = args
        .iter()
        .position(|a| a == "--port" || a == "-p")
        .and_then(|i| args.get(i + 1))
        .ok_or("usage: serve run <FILE> --port N")?;
    let listener =
        TcpListener::bind(format!("127.0.0.1:{port}")).map_err(|e| format!("bind: {e}"))?;
    for stream in listener.incoming() {
        let Ok(s) = stream else { continue };
        std::thread::spawn(move || serve(s));
    }
    Ok(())
}

/// The fake checker's rule: reject what mentions a prime (q3; 91 = 7 x 13,
/// and the fake writer never says so), accept the rest. Deterministic, and it
/// yields both decisions over the four items.
pub fn fake_decision(user: &str) -> &'static str {
    if user.contains("prime") {
        "reject"
    } else {
        "accept"
    }
}

fn reply(body: &str) -> Result<String, String> {
    let req: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let system = req["messages"][0]["content"].as_str().unwrap_or_default();
    let user = req["messages"][1]["content"].as_str().unwrap_or_default();
    let content = if system.starts_with("You are the checker") {
        std::thread::sleep(CHECKER_DELAY);
        fake_decision(user).to_string()
    } else {
        std::thread::sleep(WRITER_DELAY);
        format!("A fixed answer to: {user}")
    };
    Ok(json!({
        "object": "chat.completion",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": content}}]
    })
    .to_string())
}

fn read_request(s: &TcpStream) -> std::io::Result<(String, String)> {
    let mut r = BufReader::new(s);
    let mut start = String::new();
    r.read_line(&mut start)?;
    let mut len = 0usize;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h)? == 0 || h == "\r\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok((start, String::from_utf8_lossy(&body).into_owned()))
}

fn serve(mut s: TcpStream) {
    let Ok((start, body)) = read_request(&s) else {
        return;
    };
    let (code, text) = if start.starts_with("GET /health") {
        (200, r#"{"status":"ok"}"#.to_string())
    } else if start.starts_with("POST /v1/chat/completions") {
        match reply(&body) {
            Ok(t) => (200, t),
            Err(e) => (400, json!({"error": e}).to_string()),
        }
    } else {
        (404, r#"{"error":"not found"}"#.to_string())
    };
    let _ = write!(
        s,
        "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_checker_yields_both_decisions_over_the_four_items() {
        let decisions: Vec<&str> = crate::QUESTIONS
            .iter()
            .map(|(_, q)| {
                let answer = format!("A fixed answer to: {q}");
                fake_decision(&crate::agents::checker_user(q, &answer))
            })
            .collect();
        assert!(decisions.contains(&"accept"), "{decisions:?}");
        assert!(decisions.contains(&"reject"), "{decisions:?}");
    }
}
