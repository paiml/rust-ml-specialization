//! Practice lab: three exercises ported from the course demos. Each runs
//! offline, uses only the standard library, and checks its Provable contracts
//! with `assert!` before printing `contract: <name> OK`.

pub mod fanin;
pub mod judge;
pub mod one_server;

/// FNV-1a, 64-bit. A stdlib stand-in for the demos' SHA-256: the exercises
/// need a stable digest, not a cryptographic one.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}
