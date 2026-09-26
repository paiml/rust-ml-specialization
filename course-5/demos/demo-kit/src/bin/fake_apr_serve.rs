//! Test double for `apr serve run`, used only by demo-kit's orphan falsifier.
//! Serves `GET /health` → 200 on `--port`, optionally spawns a grandchild into
//! its own process group (`--spawn-child`) to prove group-wide cleanup, and
//! `--exit-now` exits immediately to prove an early exit is reported.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--exit-now") {
        std::process::exit(3);
    }
    if args.iter().any(|a| a == "--idle") {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    }
    let port = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .expect("--port N");
    // Grandchild: the same binary, idling, in our process group. It is never
    // waited on by design: the falsifier proves the GUARD reaps the group.
    #[allow(clippy::zombie_processes)]
    let _grandchild = args.iter().any(|a| a == "--spawn-child").then(|| {
        Command::new(&args[0])
            .arg("--idle")
            .spawn()
            .expect("spawn grandchild")
    });
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).expect("bind");
    for stream in listener.incoming() {
        let Ok(mut s) = stream else { continue };
        let mut buf = [0u8; 1024];
        let _ = s.read(&mut buf);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    }
}
