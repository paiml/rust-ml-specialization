//! A resident server whose whole process group dies with its guard.
//!
//! `ServeGuard::spawn` starts the server in a new process group and waits for
//! `GET <health>` to answer 200, measuring load time. Dropping the guard sends
//! SIGTERM to the group, waits, then SIGKILLs anything left, and reaps the
//! child — so a demo that panics mid-run leaves no orphan `apr` behind
//! (demo-pin-v1, orphan arm). Health is probed over a raw `TcpStream`, because
//! demo logic never shells out to curl (H-6).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct ServeGuard {
    child: Option<Child>,
    pub addr: SocketAddr,
    /// Spawn → first 200 on the health route, in milliseconds.
    pub load_ms: u128,
}

impl ServeGuard {
    pub fn spawn(
        program: &str,
        args: &[&str],
        addr: SocketAddr,
        health_path: &str,
        timeout: Duration,
    ) -> Result<Self, String> {
        let start = Instant::now();
        let child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|e| format!("spawn {program}: {e}"))?;
        let mut guard = ServeGuard {
            child: Some(child),
            addr,
            load_ms: 0,
        };
        loop {
            if let Some(child) = guard.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    return Err(format!("{program} exited before healthy: {status}"));
                }
            }
            if http_get(addr, health_path).is_ok_and(|(code, _)| code == 200) {
                guard.load_ms = start.elapsed().as_millis();
                return Ok(guard);
            }
            if start.elapsed() > timeout {
                return Err(format!("{program} not healthy within {timeout:?}"));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }
}

impl Drop for ServeGuard {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let pgid = child.id() as libc::pid_t;
        // SAFETY: kill(2) with a negative pid signals our own child's group.
        unsafe { libc::kill(-pgid, libc::SIGTERM) };
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // SAFETY: as above; harmless if the group is already gone.
        unsafe { libc::kill(-pgid, libc::SIGKILL) };
        let _ = child.wait();
    }
}

/// Minimal HTTP/1.1 GET: returns (status code, body).
pub fn http_get(addr: SocketAddr, path: &str) -> std::io::Result<(u16, String)> {
    http_request(addr, "GET", path, None)
}

/// Minimal HTTP/1.1 request with an optional JSON body.
pub fn http_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    json_body: Option<&str>,
) -> std::io::Result<(u16, String)> {
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(500))?;
    s.set_read_timeout(Some(Duration::from_secs(120)))?;
    let body = json_body.unwrap_or("");
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    if json_body.is_some() {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes())?;
    let mut raw = String::new();
    s.read_to_string(&mut raw)?;
    let code = raw
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| std::io::Error::other("bad status line"))?;
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    Ok((code, body))
}

/// Is a process with this pid still present (and not a zombie we own)?
pub fn pid_alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks for existence/permission.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}
