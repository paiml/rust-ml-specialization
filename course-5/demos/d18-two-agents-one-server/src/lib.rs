//! D18: two agents, a writer and a checker, share one resident `apr serve`.
//!
//! The bin (`src/main.rs`) is the recording; this library is everything it
//! runs, so `cargo test` drives the same code against `d18-fake-apr-serve`.
//!
//! - [`sink`]: each role writes through its own sink; the registry is the only
//!   source of `writer_files` / `checker_files`, and every open is `create_new`.
//! - [`ident`]: server identity from the child pid and `/proc` (field 22).
//! - [`agents`]: the two roles' prompts and the one HTTP client.
//! - [`schedule`]: the sequential and pipelined schedules, on one clock.
//! - [`checks`]: the Rust asserts beyond the shapes (spec §5.1).
//! - [`record`]: the run record the shapes judge.
//! - [`flow`]: the whole run, step by step, with its on-screen cues.
//! - [`fake_chat`]: the body of `d18-fake-apr-serve`.

pub mod agents;
pub mod checks;
pub mod fake_chat;
pub mod flow;
pub mod ident;
pub mod record;
pub mod schedule;
pub mod sink;

/// The writer's one file (spec §1, `disjoint_writes`).
pub const WRITER_FILE: &str = "out/answers.json";
/// The checker's one file; decisions are parsed from it and nothing else.
pub const CHECKER_FILE: &str = "out/verdicts.json";

/// The model both agents share (M1), pinned by content in the contract.
pub const MODEL_FILE: &str = "Qwen3.5-4B-Q4_K_M.gguf";
pub const MODEL_SHA256: &str = "00fe7986ff5f6b463e62455821146049db6f9313603938a70800d1fb69ef11a4";

/// The four fixed questions, `q1..q4`, in item order.
pub const QUESTIONS: [(&str, &str); 4] = [
    ("q1", "What does Rust's borrow checker prevent?"),
    ("q2", "What does a sha256 digest of a file let you check?"),
    ("q3", "Is the number 91 prime?"),
    (
        "q4",
        "What does temperature 0 mean for a language model's sampling?",
    ),
];

/// Item ids in order.
pub fn item_ids() -> [&'static str; 4] {
    QUESTIONS.map(|(id, _)| id)
}
