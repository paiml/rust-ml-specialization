//! D18's test double for `apr serve run`, so the two-agent harness can be
//! falsified without a GPU. The body is
//! [`d18_two_agents_one_server::fake_chat::run`]: health plus a chat route,
//! one thread per connection, deterministic replies.
//!
//! Provable contract: fake-serve-v1 — the double serves until killed; it
//! returns only on a usage or bind error, and that return fails the process.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let r = d18_two_agents_one_server::fake_chat::run(&args);
    assert!(r.is_ok(), "d18-fake-apr-serve: {r:?}");
    println!("contract: fake-serve-v1 OK");
}
