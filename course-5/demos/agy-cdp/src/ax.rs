//! The control path: accessibility tree -> node -> `DOM.getBoxModel` ->
//! `Input.dispatchMouseEvent` at the box centre; `Input.insertText` only after
//! the tree confirms a focused text box.

use base64::Engine;
use serde_json::Value;

use crate::methods::{Key, KeyPhase, Method, MousePhase};
use crate::transport::{is_devtools, Conn};

/// One accessibility node, as the driver uses it.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub role: String,
    pub name: String,
    pub backend: Option<i64>,
    pub focused: bool,
    pub ignored: bool,
}

fn prop_true(n: &Value, name: &str) -> bool {
    n["properties"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|p| p["name"] == name && p["value"]["value"] == true)
}

fn parse_node(n: &Value) -> Node {
    Node {
        role: n["role"]["value"].as_str().unwrap_or_default().to_string(),
        name: n["name"]["value"].as_str().unwrap_or_default().to_string(),
        backend: n["backendDOMNodeId"].as_i64(),
        focused: prop_true(n, "focused"),
        ignored: n["ignored"] == true,
    }
}

/// An attached app page.
pub struct Page {
    pub target_id: String,
    pub session: String,
}

/// App page targets: `type == "page"` and not a DevTools URL.
pub fn app_pages(targets: &Value) -> Vec<Value> {
    targets["targetInfos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["type"] == "page" && !is_devtools(t))
        .cloned()
        .collect()
}

/// Attach to the app page whose URL starts with `url_prefix` (or the first
/// app page when no prefix is given). The attach is built only from a
/// `Target.getTargets` entry that passed [`app_pages`].
pub fn attach(conn: &mut Conn, url_prefix: Option<&str>) -> Result<Option<Page>, String> {
    let t = conn.send(Method::TargetGetTargets, None)?;
    let pages = app_pages(&t);
    let pick = pages
        .iter()
        .find(|p| url_prefix.is_none_or(|u| p["url"].as_str().unwrap_or_default().starts_with(u)));
    let Some(p) = pick else { return Ok(None) };
    let target_id = p["targetId"].as_str().unwrap_or_default().to_string();
    let r = conn.send(
        Method::TargetAttachToTarget {
            target_id: target_id.clone(),
        },
        None,
    )?;
    let session = r["sessionId"].as_str().ok_or("no sessionId")?.to_string();
    for m in [
        Method::PageEnable,
        Method::DomEnable,
        Method::AccessibilityEnable,
    ] {
        conn.send(m, Some(&session))?;
    }
    Ok(Some(Page { target_id, session }))
}

/// The full accessibility tree, parsed.
pub fn tree(conn: &mut Conn, page: &Page) -> Result<Vec<Node>, String> {
    let r = conn.send(Method::AccessibilityGetFullAxTree, Some(&page.session))?;
    Ok(r["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(parse_node)
        .filter(|n| !n.ignored)
        .collect())
}

/// Nodes with `role` whose name contains `needle` (case-insensitive).
pub fn find<'a>(nodes: &'a [Node], role: &str, needle: &str) -> Vec<&'a Node> {
    let needle = needle.to_lowercase();
    nodes
        .iter()
        .filter(|n| n.role == role && n.name.to_lowercase().contains(&needle))
        .collect()
}

/// Centre of a node's content box.
pub fn centre(conn: &mut Conn, page: &Page, backend: i64) -> Result<(f64, f64), String> {
    let r = conn.send(
        Method::DomGetBoxModel {
            backend_node_id: backend,
        },
        Some(&page.session),
    )?;
    let q: Vec<f64> = r["model"]["content"]
        .as_array()
        .ok_or("no content quad")?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    if q.len() != 8 {
        return Err("content quad is not 8 numbers".into());
    }
    Ok((
        (q[0] + q[2] + q[4] + q[6]) / 4.0,
        (q[1] + q[3] + q[5] + q[7]) / 4.0,
    ))
}

/// Click a node at its box centre.
pub fn click(conn: &mut Conn, page: &Page, node: &Node) -> Result<(), String> {
    let b = node.backend.ok_or("node has no DOM backing")?;
    let (x, y) = centre(conn, page, b)?;
    for phase in [MousePhase::Moved, MousePhase::Pressed, MousePhase::Released] {
        conn.send(
            Method::InputDispatchMouseEvent { phase, x, y },
            Some(&page.session),
        )?;
    }
    Ok(())
}

/// Press one of the three keys.
pub fn press(conn: &mut Conn, page: &Page, key: Key) -> Result<(), String> {
    for phase in [KeyPhase::Down, KeyPhase::Up] {
        conn.send(
            Method::InputDispatchKeyEvent { phase, key },
            Some(&page.session),
        )?;
    }
    Ok(())
}

/// Roles that count as a text box for the focus check.
pub const TEXT_ROLES: [&str; 3] = ["textbox", "searchbox", "combobox"];

/// Insert `text` only when the tree shows a focused text box.
pub fn insert_text_checked(conn: &mut Conn, page: &Page, text: &str) -> Result<(), String> {
    let nodes = tree(conn, page)?;
    let focused_box = nodes
        .iter()
        .any(|n| n.focused && TEXT_ROLES.contains(&n.role.as_str()));
    if !focused_box {
        return Err("focus check: no focused text box; insertText not sent".into());
    }
    conn.send(
        Method::InputInsertText {
            text: text.to_string(),
        },
        Some(&page.session),
    )
    .map(|_| ())
}

/// A PNG screenshot of the page.
pub fn screenshot(conn: &mut Conn, page: &Page) -> Result<Vec<u8>, String> {
    let r = conn.send(Method::PageCaptureScreenshot, Some(&page.session))?;
    let b64 = r["data"].as_str().ok_or("no screenshot data")?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("screenshot: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn app_pages_exclude_devtools_and_non_pages() {
        let t = json!({"targetInfos": [
            {"type": "page", "url": "https://127.0.0.1:1/", "targetId": "a"},
            {"type": "page", "url": "devtools://devtools/x", "targetId": "b"},
            {"type": "service_worker", "url": "https://x/", "targetId": "c"},
        ]});
        let p = app_pages(&t);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0]["targetId"], "a");
    }

    #[test]
    fn parse_node_reads_role_name_focus() {
        let n = json!({"role": {"value": "textbox"}, "name": {"value": "Ask"},
            "backendDOMNodeId": 7, "properties": [{"name": "focused", "value": {"value": true}}]});
        let p = parse_node(&n);
        assert_eq!(
            (p.role.as_str(), p.name.as_str(), p.backend, p.focused),
            ("textbox", "Ask", Some(7), true)
        );
    }
}
