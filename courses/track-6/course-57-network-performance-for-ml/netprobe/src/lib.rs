//! `netprobe` — a minimal network performance probe (course-ideas build track).
//!
//! This crate is built **contract-first**: every public function satisfies the
//! provable contract in `../contracts/netprobe-v1.yaml` (validated with `pv`).
//! The invariants in the doc comments are the contract's invariants verbatim;
//! the `falsify_np_*` tests are the contract's falsification tests.
//!
//! Latency is measured as **TCP-connect round-trip time** (no raw sockets / root
//! required), which is the same technique `tcping`-style tools use.

use std::io::{self, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// Result of a throughput measurement over a TCP stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Throughput {
    /// Bytes transferred (equals the requested count on success).
    pub bytes: u64,
    /// Wall-clock time the transfer took (always > 0).
    pub elapsed: Duration,
    /// Throughput in bits/second (`bytes * 8 / elapsed`).
    pub bits_per_sec: f64,
}

/// Summary of round-trip latency probes. Times are in milliseconds.
#[derive(Debug, Clone, PartialEq)]
pub struct LatencyStats {
    /// Probes attempted.
    pub sent: u32,
    /// Probes that completed (`received <= sent`).
    pub received: u32,
    /// Fastest round trip (ms).
    pub min_ms: f64,
    /// Mean round trip over `received` samples (ms).
    pub avg_ms: f64,
    /// Slowest round trip (ms).
    pub max_ms: f64,
    /// Percentage of probes lost, in `[0.0, 100.0]`.
    pub loss_pct: f64,
}

/// Errors a probe can return. Probes never panic on network failure.
#[derive(Debug)]
pub enum ProbeError {
    /// An I/O failure (connection reset, short write, refused, timeout, ...).
    Io(io::Error),
    /// Every latency probe failed, so no statistics can be computed.
    AllProbesLost,
    /// A precondition was violated (e.g. zero bytes or zero count).
    InvalidArg(&'static str),
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbeError::Io(e) => write!(f, "io error: {e}"),
            ProbeError::AllProbesLost => write!(f, "all latency probes were lost"),
            ProbeError::InvalidArg(m) => write!(f, "invalid argument: {m}"),
        }
    }
}

impl std::error::Error for ProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProbeError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ProbeError {
    fn from(e: io::Error) -> Self {
        ProbeError::Io(e)
    }
}

/// Convert a byte/second rate to gigabits/second (Module 0: bits vs bytes).
///
/// INVARIANT: `result == bps * 8.0 / 1_000_000_000.0`
/// INVARIANT: `bps >= 0.0` implies `result >= 0.0`
/// INVARIANT: total over every finite non-negative `bps` — no panic, no NaN.
#[must_use]
pub fn bytes_per_sec_to_gbit(bps: f64) -> f64 {
    bps * 8.0 / 1_000_000_000.0
}

/// Send exactly `bytes` over an established TCP stream and time the transfer.
///
/// INVARIANT: `Ok(t)` implies `t.elapsed > Duration::ZERO`.
/// INVARIANT: `Ok(t)` implies `t.bytes == bytes` — a short write is `Err`, never
///            a truncated success (`write_all` fails on a short write).
/// INVARIANT: `Ok(t)` implies `t.bits_per_sec == bytes * 8 / t.elapsed.as_secs_f64()`.
/// INVARIANT: `Err(e)` preserves the underlying `io::ErrorKind` — never a panic.
///
/// # Errors
/// Returns [`ProbeError::InvalidArg`] if `bytes == 0`, or [`ProbeError::Io`] if the
/// transfer fails.
pub fn measure_throughput(stream: &mut TcpStream, bytes: u64) -> Result<Throughput, ProbeError> {
    if bytes == 0 {
        return Err(ProbeError::InvalidArg("bytes must be > 0"));
    }
    let buf = [0u8; 64 * 1024];
    let mut remaining = bytes;
    let start = Instant::now();
    while remaining > 0 {
        let n = remaining.min(buf.len() as u64) as usize;
        stream.write_all(&buf[..n])?;
        remaining -= n as u64;
    }
    stream.flush()?;
    // Clamp to >= 1ns so the "elapsed > 0" invariant holds even on an
    // impossibly fast loopback, and derive bits_per_sec from the SAME value so
    // the bits_per_sec invariant stays exactly consistent with elapsed.
    let elapsed = start.elapsed().max(Duration::from_nanos(1));
    let bits_per_sec = (bytes as f64) * 8.0 / elapsed.as_secs_f64();
    Ok(Throughput {
        bytes,
        elapsed,
        bits_per_sec,
    })
}

/// Probe round-trip latency to `addr` `count` times and summarize (TCP-connect RTT).
///
/// INVARIANT: `Ok(s)` implies `s.received <= count`.
/// INVARIANT: `Ok(s)` implies `s.min_ms <= s.avg_ms && s.avg_ms <= s.max_ms`.
/// INVARIANT: `Ok(s)` implies `0.0 <= s.loss_pct <= 100.0`.
/// INVARIANT: `Ok(s)` implies `s.loss_pct == (count - received) / count * 100`.
/// INVARIANT: a lost probe increments loss and continues — a timeout never panics.
/// INVARIANT: `received == 0` implies `Err` — no division by zero on total loss.
///
/// # Errors
/// Returns [`ProbeError::InvalidArg`] if `count == 0`, or [`ProbeError::AllProbesLost`]
/// if every probe fails.
pub fn measure_latency(addr: SocketAddr, count: u32) -> Result<LatencyStats, ProbeError> {
    if count == 0 {
        return Err(ProbeError::InvalidArg("count must be > 0"));
    }
    let timeout = Duration::from_secs(2);
    let mut rtts: Vec<f64> = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let start = Instant::now();
        // A failed connect (refused/timeout) is a lost probe, not an error.
        if TcpStream::connect_timeout(&addr, timeout).is_ok() {
            rtts.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let received = u32::try_from(rtts.len()).unwrap_or(u32::MAX);
    if received == 0 {
        return Err(ProbeError::AllProbesLost);
    }
    let min_ms = rtts.iter().copied().fold(f64::INFINITY, f64::min);
    let max_ms = rtts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let avg_ms = rtts.iter().sum::<f64>() / f64::from(received);
    let loss_pct = f64::from(count - received) / f64::from(count) * 100.0;
    Ok(LatencyStats {
        sent: count,
        received,
        min_ms,
        avg_ms,
        max_ms,
        loss_pct,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    /// A localhost TCP server that drains everything it receives. Returns its
    /// address and a join handle. Accepts `accepts` connections then stops.
    fn drain_server(accepts: usize) -> (SocketAddr, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            for _ in 0..accepts {
                if let Ok((mut sock, _)) = listener.accept() {
                    let mut sink = [0u8; 64 * 1024];
                    while let Ok(n) = sock.read(&mut sink) {
                        if n == 0 {
                            break;
                        }
                    }
                }
            }
        });
        (addr, handle)
    }

    /// A closed port: bind then immediately drop the listener so connects refuse.
    fn closed_addr() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        addr
    }

    // FALSIFY-NP-003: 1_250_000_000 bytes/s -> 10.0 Gbit/s.
    #[test]
    fn falsify_np_003_bits_bytes_conversion() {
        assert!((bytes_per_sec_to_gbit(1_250_000_000.0) - 10.0).abs() < 1e-9);
        assert_eq!(bytes_per_sec_to_gbit(0.0), 0.0);
        // 620 MB/s ~= 4.96 Gbit/s (the Module 0 lesson).
        assert!((bytes_per_sec_to_gbit(620_000_000.0) - 4.96).abs() < 1e-9);
    }

    // FALSIFY-NP-001: transferring N bytes reports t.bytes == N (no truncation).
    #[test]
    fn falsify_np_001_throughput_transfers_all_bytes() {
        let (addr, handle) = drain_server(1);
        let mut stream = TcpStream::connect(addr).unwrap();
        let n: u64 = 4 * 1024 * 1024;
        let t = measure_throughput(&mut stream, n).unwrap();
        assert_eq!(t.bytes, n, "must report exactly the requested bytes");
        assert!(t.elapsed > Duration::ZERO, "elapsed must be > 0");
        assert!(t.bits_per_sec > 0.0 && t.bits_per_sec.is_finite());
        // bits_per_sec is consistent with bytes and elapsed.
        let expected = (n as f64) * 8.0 / t.elapsed.as_secs_f64();
        assert!((t.bits_per_sec - expected).abs() / expected < 1e-9);
        drop(stream);
        handle.join().unwrap();
    }

    #[test]
    fn throughput_rejects_zero_bytes() {
        let (addr, handle) = drain_server(1);
        let mut stream = TcpStream::connect(addr).unwrap();
        assert!(matches!(
            measure_throughput(&mut stream, 0),
            Err(ProbeError::InvalidArg(_))
        ));
        drop(stream);
        // server still waiting to accept; connect once so its thread can finish
        let _ = TcpStream::connect(addr);
        handle.join().unwrap();
    }

    #[test]
    fn latency_happy_path_orders_min_avg_max() {
        let (addr, handle) = drain_server(3);
        let s = measure_latency(addr, 3).unwrap();
        assert_eq!(s.sent, 3);
        assert!(s.received <= 3);
        assert!(s.min_ms <= s.avg_ms && s.avg_ms <= s.max_ms);
        assert!((0.0..=100.0).contains(&s.loss_pct));
        handle.join().unwrap();
    }

    // FALSIFY-NP-002: with every probe lost, returns Err — never divides by zero.
    #[test]
    fn falsify_np_002_all_probes_lost_is_err_not_panic() {
        let addr = closed_addr();
        match measure_latency(addr, 2) {
            Err(ProbeError::AllProbesLost) => {}
            other => panic!("expected AllProbesLost, got {other:?}"),
        }
    }

    #[test]
    fn latency_rejects_zero_count() {
        let addr = closed_addr();
        assert!(matches!(
            measure_latency(addr, 0),
            Err(ProbeError::InvalidArg(_))
        ));
    }
}
