//! Test double for `apr serve run`, used only by demo-kit's orphan falsifier.
//! The body is [`demo_kit::fake_serve::run`].
//!
//! Provable contract: fake-serve-v1 — the double serves until killed; it
//! returns only on a usage or bind error, and that return fails the process.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let r = demo_kit::fake_serve::run(&args);
    assert!(r.is_ok(), "fake-apr-serve: {r:?}");
    println!("contract: fake-serve-v1 OK");
}
