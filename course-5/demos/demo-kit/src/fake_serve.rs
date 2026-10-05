//! The body of the `apr serve run` test double (demo-kit's `fake-apr-serve`
//! and d18's `d18-fake-apr-serve`). Serves `GET /health` → 200 on `--port`,
//! optionally spawns a grandchild into its own process group (`--spawn-child`)
//! to prove group-wide cleanup, and `--exit-now` exits immediately (code 3) to
//! prove an early exit is reported. `--idle` sleeps forever.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

/// Run the double with `args` (`args[0]` is the program). Returns only on a
/// usage or bind error; the listener otherwise serves until the process dies.
pub fn run(args: &[String]) -> Result<(), String> {
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
        .ok_or("usage: --port N")?;
    // Grandchild: the same binary, idling, in our process group. It is never
    // waited on by design: the falsifier proves the GUARD reaps the group.
    #[allow(clippy::zombie_processes)]
    let _grandchild = if args.iter().any(|a| a == "--spawn-child") {
        Some(
            Command::new(&args[0])
                .arg("--idle")
                .spawn()
                .map_err(|e| format!("spawn grandchild: {e}"))?,
        )
    } else {
        None
    };
    let listener =
        TcpListener::bind(format!("127.0.0.1:{port}")).map_err(|e| format!("bind: {e}"))?;
    for stream in listener.incoming() {
        let Ok(mut s) = stream else { continue };
        let mut buf = [0u8; 1024];
        let _ = s.read(&mut buf);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    }
    Ok(())
}
