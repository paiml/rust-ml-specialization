//! `demo.toml` (MEGA-001 §5.2): one file drives the test, the receipt and the
//! recording card, so the three cannot drift apart.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DemoManifest {
    pub id: String,
    pub title: String,
    pub lesson: String,
    /// Exact apr pin, e.g. `"=0.69.3"`, or `"none"` for a demo that runs no apr.
    pub apr: String,
    /// Exact agy pin, or `"none"`.
    #[serde(default = "none")]
    pub agy: String,
    pub model: Option<Model>,
    pub host_class: String,
    #[serde(default)]
    pub needs: Vec<String>,
    pub fixtures: Option<Fixtures>,
    pub schema_copy_sha256: Option<String>,
    #[serde(default)]
    pub step: Vec<Step>,
    #[serde(default)]
    pub assert: BTreeMap<String, toml::Value>,
    pub record: Record,
}

fn none() -> String {
    "none".into()
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub name: String,
    pub sha256: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Fixtures {
    pub dir: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub cmd: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub target_duration_s: u32,
    pub resolution: String,
    pub terminal: Option<Terminal>,
    #[serde(default)]
    pub narration_may_cite: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    pub font_pt: toml::Value,
    pub cols: u32,
    pub theme: String,
}

impl DemoManifest {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("demo.toml: {e}"))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text)
    }

    /// Does this demo run apr at all?
    pub fn uses_apr(&self) -> bool {
        self.apr != "none"
    }

    pub fn uses_agy(&self) -> bool {
        self.agy != "none"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const SAMPLE: &str = r#"
id = "d13-reducer"
title = "A deterministic reducer"
lesson = "rfml5/2.2"
apr = "none"
host_class = "any"
[[step]]
cmd = "cargo run -p d13-reducer"
[assert]
exit = 0
[record]
target_duration_s = 300
resolution = "1920x1080"
narration_may_cite = ["workers"]
"#;

    #[test]
    fn parses_sample() {
        let m = DemoManifest::parse(SAMPLE).unwrap();
        assert_eq!(m.id, "d13-reducer");
        assert!(!m.uses_apr());
        assert!(!m.uses_agy());
        assert_eq!(m.record.target_duration_s, 300);
    }

    #[test]
    fn unknown_field_is_refused() {
        let bad = SAMPLE.replace("host_class", "hostclass");
        assert!(DemoManifest::parse(&bad).is_err());
    }
}
