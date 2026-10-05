//! A small JSON value and pretty writer for the run record and the
//! stop-latency file. The crate keeps to the dependencies its manifest
//! already names, so both files are built from this one type. Output matches
//! the two-space pretty layout of the committed fixtures.

#[derive(Debug, Clone, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn str(s: impl Into<String>) -> J {
        J::Str(s.into())
    }

    pub fn int(n: u64) -> J {
        J::Int(i64::try_from(n).unwrap_or(i64::MAX))
    }

    pub fn opt(n: Option<u64>) -> J {
        n.map_or(J::Null, J::int)
    }

    pub fn strs<S: AsRef<str>>(it: impl IntoIterator<Item = S>) -> J {
        J::Arr(it.into_iter().map(|s| J::str(s.as_ref())).collect())
    }

    /// The value at `key` of an object.
    pub fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The keys of an object, in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        match self {
            J::Obj(kv) => kv.iter().map(|(k, _)| k.as_str()).collect(),
            _ => Vec::new(),
        }
    }

    /// Pretty text with a trailing newline.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            J::Null => out.push_str("null"),
            J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            J::Int(n) => out.push_str(&n.to_string()),
            J::Str(s) => out.push_str(&quote(s)),
            J::Arr(a) => write_seq(out, depth, '[', ']', a.iter().map(|v| (None, v))),
            J::Obj(kv) => write_seq(
                out,
                depth,
                '{',
                '}',
                kv.iter().map(|(k, v)| (Some(k.as_str()), v)),
            ),
        }
    }
}

fn write_seq<'a>(
    out: &mut String,
    depth: usize,
    open: char,
    close: char,
    items: impl ExactSizeIterator<Item = (Option<&'a str>, &'a J)>,
) {
    if items.len() == 0 {
        out.push(open);
        out.push(close);
        return;
    }
    out.push(open);
    let pad = "  ".repeat(depth + 1);
    for (i, (k, v)) in items.enumerate() {
        out.push_str(if i == 0 { "\n" } else { ",\n" });
        out.push_str(&pad);
        if let Some(k) = k {
            out.push_str(&quote(k));
            out.push_str(": ");
        }
        v.write(out, depth + 1);
    }
    out.push('\n');
    out.push_str(&"  ".repeat(depth));
    out.push(close);
}

/// A JSON string literal.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The top-level keys of a pretty-printed object (two-space indent), read
/// from text, as the committed fixtures are laid out.
pub fn top_level_keys(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("  \""))
        .filter_map(|l| l.split_once("\":").map(|(k, _)| k.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_matches_the_fixture_layout() {
        let v = J::Obj(vec![
            ("a".into(), J::int(1)),
            ("b".into(), J::strs(["x", "y"])),
            ("c".into(), J::Arr(vec![])),
            (
                "d".into(),
                J::Arr(vec![J::Obj(vec![("m".into(), J::Bool(false))])]),
            ),
            ("e".into(), J::Null),
        ]);
        let want = "{\n  \"a\": 1,\n  \"b\": [\n    \"x\",\n    \"y\"\n  ],\n  \"c\": [],\n  \"d\": [\n    {\n      \"m\": false\n    }\n  ],\n  \"e\": null\n}\n";
        assert_eq!(v.pretty(), want);
        assert_eq!(top_level_keys(want), ["a", "b", "c", "d", "e"]);
        assert_eq!(v.keys(), ["a", "b", "c", "d", "e"]);
        assert_eq!(v.get("a"), Some(&J::Int(1)));
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(quote("a\"b\\c\nd\u{1}"), "\"a\\\"b\\\\c\\nd\\u0001\"");
    }
}
