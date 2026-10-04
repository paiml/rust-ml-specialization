//! D18's test double for `apr serve run`, so the two-agent harness can be
//! falsified without a GPU. The body is [`demo_kit::fake_serve::run`].
//!
//! Provable contract: fake-serve-v1 — the double serves until killed; it
//! returns only on a usage or bind error, and that return fails the process.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let r = demo_kit::fake_serve::run(&args);
    assert!(r.is_ok(), "d18-fake-apr-serve: {r:?}");
    println!("contract: fake-serve-v1 OK");
}
