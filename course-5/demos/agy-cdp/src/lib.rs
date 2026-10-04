//! agy-cdp: the course's closed Chrome DevTools Protocol client for the
//! Antigravity app. It is the only workspace crate that may name
//! `tungstenite`; `xtask verify` refuses it anywhere else, and refuses
//! sockets, processes and file access outside this crate's two named modules.
//!
//! Skeleton: the transport, launcher and method enum land in ph4. What ships
//! now is the embedded role/name map, committed as the unmeasured sentinel
//! until `xtask promote-fixture e3-probe` replaces it.

/// The role/name map measured by `e3-probe`, embedded at build time so no
/// confined bin reads a fixture at run time.
const ROLE_NAME_MAP: &str = include_str!("../fixtures/role-name-map.json");

/// A committed fixture: the bootstrap sentinel, or a measured map.
#[derive(Debug, Clone, PartialEq)]
pub enum RoleNameMap {
    /// `{"status":"unmeasured"}`. D20 and D21 refuse it: `NotRun(FixtureUnmeasured)`.
    Unmeasured,
    Measured(serde_json::Value),
}

/// Parse a role/name map; the sentinel parses as [`RoleNameMap::Unmeasured`].
pub fn parse_role_name_map(text: &str) -> Result<RoleNameMap, String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("role-name-map.json: {e}"))?;
    if v == serde_json::json!({"status": "unmeasured"}) {
        return Ok(RoleNameMap::Unmeasured);
    }
    match v {
        serde_json::Value::Object(_) => Ok(RoleNameMap::Measured(v)),
        _ => Err("role-name-map.json: not an object".into()),
    }
}

/// The embedded map.
pub fn role_name_map() -> Result<RoleNameMap, String> {
    parse_role_name_map(ROLE_NAME_MAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_is_unmeasured_and_anything_else_is_not() {
        assert_eq!(
            parse_role_name_map(r#"{"status":"unmeasured"}"#),
            Ok(RoleNameMap::Unmeasured)
        );
        assert!(matches!(
            parse_role_name_map(r#"{"status":"unmeasured","x":1}"#),
            Ok(RoleNameMap::Measured(_))
        ));
        assert!(parse_role_name_map("[]").is_err());
        assert!(parse_role_name_map("").is_err());
        assert!(role_name_map().is_ok());
    }
}
