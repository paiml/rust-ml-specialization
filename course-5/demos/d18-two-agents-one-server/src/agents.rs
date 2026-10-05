//! The two roles and the one client they share.
//!
//! Both roles are LLM calls with their own system prompt, against the same
//! resident server, at temperature 0 with a fixed seed. Output files are one
//! JSON object per line, written in item order, so the bytes do not depend on
//! the schedule.

use serde_json::{json, Value};
use std::net::SocketAddr;

/// Fixed sampling seed; with temperature 0 decoding is greedy anyway.
pub const SEED: u64 = 1818;
pub const WRITER_MAX_TOKENS: u32 = 64;
pub const CHECKER_MAX_TOKENS: u32 = 4;

pub const WRITER_SYSTEM: &str =
    "You are the writer. Answer the question in one short factual sentence.";
pub const CHECKER_SYSTEM: &str = "You are the checker. Read the question and the writer's answer. Reply with exactly one word: accept if the answer is correct, reject otherwise.";

/// A chat completion: one system prompt, one user message, one reply.
pub trait Llm: Sync {
    fn chat(&self, system: &str, user: &str, max_tokens: u32) -> Result<String, String>;
}

/// The OpenAI-compatible route of one `apr serve` process.
#[derive(Debug, Clone, Copy)]
pub struct Http {
    pub addr: SocketAddr,
}

impl Llm for Http {
    fn chat(&self, system: &str, user: &str, max_tokens: u32) -> Result<String, String> {
        let body = json!({
            "model": "default",
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ],
            "max_tokens": max_tokens,
            "temperature": 0,
            "seed": SEED
        });
        let (code, text) = demo_kit::serve::http_request(
            self.addr,
            "POST",
            "/v1/chat/completions",
            Some(&body.to_string()),
        )
        .map_err(|e| format!("chat: {e}"))?;
        if code != 200 {
            return Err(format!("chat: HTTP {code}"));
        }
        content_of(&text)
    }
}

/// `choices[0].message.content` of a chat-completion response body.
pub fn content_of(body: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("chat body: {e}"))?;
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "chat body: no choices[0].message.content".into())
}

/// The checker's user message for one item.
pub fn checker_user(question: &str, answer: &str) -> String {
    format!("Question: {question}\nAnswer: {answer}")
}

/// `accept` or `reject`, from the checker's reply: its first word, lowercased,
/// without punctuation. Anything else is an error, never a default.
pub fn parse_decision(reply: &str) -> Result<&'static str, String> {
    let word: String = reply
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_alphabetic)
        .collect::<String>()
        .to_ascii_lowercase();
    match word.as_str() {
        "accept" => Ok("accept"),
        "reject" => Ok("reject"),
        _ => Err(format!("checker replied {reply:?}, not accept or reject")),
    }
}

/// One output line: `{"id":…,"<key>":…}`.
pub fn line(id: &str, key: &str, value: &str) -> String {
    let mut m = serde_json::Map::new();
    m.insert("id".into(), json!(id));
    m.insert(key.into(), json!(value));
    Value::Object(m).to_string()
}

/// The `key` field of the line whose `id` is `id`, in a one-object-per-line file.
pub fn field_for(text: &str, id: &str, key: &str) -> Option<String> {
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["id"] == id)
        .and_then(|v| v[key].as_str().map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_parse_or_refuse() {
        assert_eq!(parse_decision("accept"), Ok("accept"));
        assert_eq!(parse_decision(" Reject."), Ok("reject"));
        assert!(parse_decision("").is_err());
        assert!(parse_decision("maybe").is_err());
        assert!(parse_decision("accepted").is_err());
    }

    #[test]
    fn lines_round_trip() {
        let text = [
            line("q1", "answer", "a \"b\"\nc"),
            line("q2", "answer", "d"),
        ]
        .join("\n");
        assert_eq!(
            field_for(&text, "q1", "answer").as_deref(),
            Some("a \"b\"\nc")
        );
        assert_eq!(field_for(&text, "q2", "answer").as_deref(), Some("d"));
        assert_eq!(field_for(&text, "q3", "answer"), None);
    }

    #[test]
    fn content_is_read_from_the_first_choice() {
        let body = r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"reject"}}]}"#;
        assert_eq!(content_of(body).as_deref(), Ok("reject"));
        assert!(content_of("ok").is_err());
        assert!(content_of("{}").is_err());
    }
}
