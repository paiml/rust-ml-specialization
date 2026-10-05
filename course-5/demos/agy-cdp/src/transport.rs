//! The one CDP channel. The only module in the crate that opens a socket.
//!
//! Discovery is a plain HTTP GET to the port read from `DevToolsActivePort`;
//! the WebSocket is opened to the browser endpoint that discovery names, on
//! that same port. Every frame passes through [`Conn::send`], which tallies the
//! method by name at the socket; that tally is `cdp_methods` in the record.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

use crate::methods::{Key, KeyPhase, Method};

/// What the socket saw: the source of `cdp_methods`, `keys_sent`, the inserted
/// texts and the DevTools count.
#[derive(Debug, Default, Clone)]
pub struct Tally {
    pub methods: BTreeMap<String, u64>,
    /// Every frame's method, in send order.
    pub frames: Vec<String>,
    /// The params of every `Target.setDiscoverTargets` frame.
    pub discover_params: Vec<Value>,
    pub keys: BTreeSet<String>,
    pub inserted: Vec<String>,
    /// Target ids passed to `Target.attachToTarget`.
    pub attached: Vec<String>,
    /// Every event received, in order.
    pub events: Vec<Value>,
    /// The teardown `Target.getTargets` reply, once parsed.
    pub snapshot: Option<Value>,
}

/// Is this target info a DevTools target?
pub fn is_devtools(info: &Value) -> bool {
    let ty = info["type"].as_str().unwrap_or_default();
    let url = info["url"].as_str().unwrap_or_default();
    ty.contains("devtools") || url.starts_with("devtools://")
}

impl Tally {
    /// `Target.targetCreated` DevTools events in the log.
    pub fn devtools_created(&self) -> u64 {
        self.events
            .iter()
            .filter(|e| e["method"] == "Target.targetCreated")
            .filter(|e| is_devtools(&e["params"]["targetInfo"]))
            .count() as u64
    }

    /// DevTools targets listed in the teardown snapshot.
    pub fn devtools_in_snapshot(&self) -> u64 {
        self.snapshot
            .as_ref()
            .and_then(|s| s["targetInfos"].as_array())
            .map(|a| a.iter().filter(|t| is_devtools(t)).count() as u64)
            .unwrap_or(0)
    }
}

/// An HTTP GET to `127.0.0.1:<port><path>`, body as JSON.
pub fn discover(port: u16, path: &str) -> Result<Value, String> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("discover: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let body = read_http_body(&mut s)?;
    serde_json::from_str(&body).map_err(|e| format!("discover {path}: {e}"))
}

/// One browser-level CDP connection (sessions are flattened onto it).
pub struct Conn {
    ws: WebSocket<TcpStream>,
    next_id: u64,
    pub tally: Tally,
    closed: bool,
}

impl Conn {
    /// Connect to the browser endpoint on `port` and send
    /// `Target.setDiscoverTargets` as the first frame.
    pub fn open(port: u16) -> Result<Conn, String> {
        let v = discover(port, "/json/version")?;
        let url = v["webSocketDebuggerUrl"]
            .as_str()
            .ok_or("no webSocketDebuggerUrl")?
            .to_string();
        let want = format!("ws://127.0.0.1:{port}/devtools/browser/");
        if !url.starts_with(&want) {
            return Err(format!("browser endpoint is not on our port: {url}"));
        }
        let stream =
            TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("connect: {e}"))?;
        let (ws, _) = tungstenite::client::client(url.as_str(), stream)
            .map_err(|e| format!("handshake: {e}"))?;
        let mut c = Conn {
            ws,
            next_id: 1,
            tally: Tally::default(),
            closed: false,
        };
        c.send(Method::TargetSetDiscoverTargets, None)?;
        Ok(c)
    }

    fn record(&mut self, m: &Method) -> Result<(), String> {
        let name = m.name();
        if matches!(m, Method::TargetSetDiscoverTargets) && !self.tally.discover_params.is_empty() {
            return Err("Target.setDiscoverTargets twice on one connection".into());
        }
        if matches!(m, Method::TargetSetDiscoverTargets) {
            self.tally.discover_params.push(m.params());
        }
        if let Method::InputDispatchKeyEvent {
            phase: KeyPhase::Down,
            key,
        } = m
        {
            self.tally.keys.insert(Key::name(*key).to_string());
        }
        if let Method::InputInsertText { text } = m {
            self.tally.inserted.push(text.clone());
        }
        if let Method::TargetAttachToTarget { target_id } = m {
            self.tally.attached.push(target_id.clone());
        }
        *self.tally.methods.entry(name.to_string()).or_default() += 1;
        self.tally.frames.push(name.to_string());
        Ok(())
    }

    /// The one write function. Sends `m` (to `session` when given) and returns
    /// the reply's `result`, logging every event read on the way.
    pub fn send(&mut self, m: Method, session: Option<&str>) -> Result<Value, String> {
        if self.closed {
            return Err("connection closed".into());
        }
        self.record(&m)?;
        let id = self.next_id;
        self.next_id += 1;
        let mut frame = json!({"id": id, "method": m.name(), "params": m.params()});
        if let Some(s) = session {
            frame["sessionId"] = json!(s);
        }
        self.ws
            .send(Message::Text(frame.to_string().into()))
            .map_err(|e| format!("send {}: {e}", m.name()))?;
        self.wait_reply(id, m.name())
    }

    fn wait_reply(&mut self, id: u64, name: &str) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            let msg = self.ws.read().map_err(|e| format!("read ({name}): {e}"))?;
            let Ok(text) = msg.to_text() else { continue };
            let Ok(v) = serde_json::from_str::<Value>(text) else {
                continue;
            };
            if v["id"].as_u64() == Some(id) {
                if !v["error"].is_null() {
                    return Err(format!("{name}: {}", v["error"]));
                }
                return Ok(v["result"].clone());
            }
            if v.get("method").is_some() {
                self.tally.events.push(v);
            }
        }
        Err(format!("{name}: no reply in 60 s"))
    }

    /// Teardown: `Target.getTargets` is the last frame; its reply is read and
    /// parsed before the socket closes, and becomes the snapshot.
    pub fn close_with_snapshot(mut self) -> Result<Tally, String> {
        let snap = self.send(Method::TargetGetTargets, None)?;
        self.tally.snapshot = Some(snap);
        self.closed = true;
        let _ = self.ws.close(None);
        Ok(self.tally)
    }
}

/// Read one HTTP response's body: headers to the blank line, then exactly
/// `Content-Length` bytes (the DevTools server keeps the connection open).
fn read_http_body(s: &mut TcpStream) -> Result<String, String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let text = String::from_utf8_lossy(&buf).into_owned();
        if let Some((head, body)) = text.split_once("\r\n\r\n") {
            let len = content_length(head);
            if len.is_none_or(|n| body.len() >= n) {
                return Ok(body.to_string());
            }
        }
        let n = s
            .read(&mut chunk)
            .map_err(|e| format!("discover read: {e}"))?;
        if n == 0 {
            let text = String::from_utf8_lossy(&buf).into_owned();
            return Ok(text
                .split_once("\r\n\r\n")
                .map(|x| x.1.to_string())
                .unwrap_or_default());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn content_length(head: &str) -> Option<usize> {
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case("content-length")
            .then(|| v.trim().parse().ok())?
    })
}
