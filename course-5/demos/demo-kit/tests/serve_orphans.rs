//! demo-pin-v1 orphan arm: a ServeGuard leaves no process behind.

use demo_kit::serve::{http_get, pid_alive, ServeGuard};
use std::net::TcpListener;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn fake_bin() -> String {
    env!("CARGO_BIN_EXE_fake-apr-serve").to_string()
}

/// demo-pin-v1 (orphan arm): after the guard drops, the server and every
/// process in its group are gone — even when the guard drops by panic.
#[test]
fn demo_pin_v1_zero_orphans_after_drop() {
    let port = free_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let port_s = port.to_string();
    let pid = {
        let g = ServeGuard::spawn(
            &fake_bin(),
            &["--port", &port_s, "--spawn-child"],
            addr,
            "/health",
            Duration::from_secs(10),
        )
        .unwrap();
        let pid = g.pid().unwrap();
        assert!(pid_alive(pid));
        assert_eq!(http_get(addr, "/health").unwrap().0, 200);
        pid
    };
    assert!(!pid_alive(pid), "server survived its guard");
    // the grandchild the fake spawned into the same group must be gone too
    std::thread::sleep(Duration::from_millis(200));
    assert!(TcpStream::connect(addr).is_err(), "port still served");
}

#[test]
fn zero_orphans_after_panic() {
    let port = free_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let port_s = port.to_string();
    let pid = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    let p2 = pid.clone();
    let r = std::thread::spawn(move || {
        let g = ServeGuard::spawn(
            &fake_bin(),
            &["--port", &port_s],
            addr,
            "/health",
            Duration::from_secs(10),
        )
        .unwrap();
        *p2.lock().unwrap() = g.pid().unwrap();
        panic!("demo failed mid-run");
    })
    .join();
    assert!(r.is_err());
    assert!(!pid_alive(*pid.lock().unwrap()), "panic left an orphan");
}

#[test]
fn early_exit_is_an_error_not_a_hang() {
    let addr: SocketAddr = format!("127.0.0.1:{}", free_port()).parse().unwrap();
    let r = ServeGuard::spawn(
        &fake_bin(),
        &["--exit-now"],
        addr,
        "/health",
        Duration::from_secs(10),
    );
    assert!(r.is_err());
}

/// Processes whose process-group id is `pgid`, read from /proc/<pid>/stat.
fn group_members(pgid: u32) -> Vec<u32> {
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return vec![];
    };
    rd.filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .ok()
                .and_then(|s| {
                    // fields after the ")" of comm: state ppid pgrp ...
                    let rest = s.rsplit_once(')')?.1;
                    rest.split_whitespace().nth(2)?.parse::<u32>().ok()
                })
                == Some(pgid)
        })
        .collect()
}

/// The grandchild spawned into the server's group dies with the guard too.
#[test]
fn demo_pin_v1_whole_group_reaped() {
    let port = free_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let port_s = port.to_string();
    let pgid = {
        let g = ServeGuard::spawn(
            &fake_bin(),
            &["--port", &port_s, "--spawn-child"],
            addr,
            "/health",
            Duration::from_secs(10),
        )
        .unwrap();
        let pgid = g.pid().unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            group_members(pgid).len() >= 2,
            "grandchild should be in the group"
        );
        pgid
    };
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        group_members(pgid),
        Vec::<u32>::new(),
        "orphans left in group"
    );
}
