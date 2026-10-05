//! The raw accessibility tree, reduced to what D20 reads: id, parent, role,
//! name. Pure functions only, so every rule here has a unit test.
//!
//! Measured on the app (2.8.1): each conversation in the sidebar is a `link`
//! whose parent is the row; the row holds a `Stop execution` button while the
//! agent runs and an `Archive conversation` button when it is idle.

use std::collections::{BTreeSet, HashMap};

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct RNode {
    pub id: String,
    pub parent: Option<String>,
    pub role: String,
    pub name: String,
    pub backend: Option<i64>,
}

/// The state a conversation row reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Running,
    Idle,
}

pub const RUNNING_BUTTON: &str = "Stop execution";
pub const IDLE_BUTTON: &str = "Archive conversation";
pub const ROW_MARKER: &str = "More options";

/// Text the app shows while an agent waits on the user (measured 06:47Z on
/// 2.8.1: the pane read both). A waiting agent is blocked, not running.
pub const APPROVAL_MARKS: [&str; 2] = ["allow write access", "waiting for user input"];

/// The non-ignored nodes of an `Accessibility.getFullAXTree` result.
pub fn reduce(raw: &Value) -> Vec<RNode> {
    raw["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| n["ignored"] != true)
        .map(|n| RNode {
            id: n["nodeId"].as_str().unwrap_or_default().to_string(),
            parent: n["parentId"].as_str().map(str::to_string),
            role: n["role"]["value"].as_str().unwrap_or_default().to_string(),
            name: n["name"]["value"].as_str().unwrap_or_default().to_string(),
            backend: n["backendDOMNodeId"].as_i64(),
        })
        .collect()
}

/// A compact dump for the run directory: `[id, parent, role, name]` rows,
/// skipping layout-only roles.
pub fn dump(nodes: &[RNode]) -> Value {
    Value::Array(
        nodes
            .iter()
            .filter(|n| !matches!(n.role.as_str(), "" | "none" | "InlineTextBox"))
            .map(|n| json!([n.id, n.parent, n.role, n.name]))
            .collect(),
    )
}

/// Is `root` an ancestor of `id` (within eight levels)?
fn under(parents: &HashMap<&str, &str>, id: &str, root: &str) -> bool {
    let mut cur = id;
    for _ in 0..8 {
        match parents.get(cur) {
            Some(p) if *p == root => return true,
            Some(p) => cur = p,
            None => return false,
        }
    }
    false
}

fn parents(nodes: &[RNode]) -> HashMap<&str, &str> {
    nodes
        .iter()
        .filter_map(|n| Some((n.id.as_str(), n.parent.as_deref()?)))
        .collect()
}

/// Button names under `row`; empty unless the row holds exactly one
/// `More options` (a container of many rows is not a row).
fn row_buttons<'a>(nodes: &'a [RNode], row: &str) -> BTreeSet<&'a str> {
    let p = parents(nodes);
    let names: Vec<&str> = nodes
        .iter()
        .filter(|n| n.role == "button" && under(&p, &n.id, row))
        .map(|n| n.name.as_str())
        .collect();
    if names.iter().filter(|n| **n == ROW_MARKER).count() != 1 {
        return BTreeSet::new();
    }
    names.into_iter().collect()
}

/// Conversation links in the sidebar: a `link` whose row holds `More options`.
pub fn conversation_links(nodes: &[RNode]) -> Vec<&RNode> {
    nodes
        .iter()
        .filter(|n| n.role == "link" && n.backend.is_some())
        .filter(|n| {
            n.parent
                .as_deref()
                .is_some_and(|row| row_buttons(nodes, row).contains(ROW_MARKER))
        })
        .collect()
}

/// The state of the row whose link has DOM backing `backend`.
pub fn row_state(nodes: &[RNode], backend: i64) -> Option<RowState> {
    let link = nodes
        .iter()
        .find(|n| n.role == "link" && n.backend == Some(backend))?;
    let b = row_buttons(nodes, link.parent.as_deref()?);
    if b.contains(RUNNING_BUTTON) {
        Some(RowState::Running)
    } else if b.contains(IDLE_BUTTON) {
        Some(RowState::Idle)
    } else {
        None
    }
}

/// The rows in `now` that are not in `before`, oldest first: the app assigns
/// DOM backings in creation order.
pub fn new_rows(now: &[i64], before: &[i64]) -> Vec<i64> {
    let mut new: Vec<i64> = now
        .iter()
        .copied()
        .filter(|b| !before.contains(b))
        .collect();
    new.sort_unstable();
    new.dedup();
    new
}

/// Does any node in the snapshot, sidebar row or open pane, show an approval
/// prompt or its radio choices?
pub fn shows_approval(nodes: &[RNode]) -> bool {
    nodes.iter().any(|n| {
        let name = n.name.to_lowercase();
        APPROVAL_MARKS.iter().any(|m| name.contains(m))
            || (n.role == "radio" && name.contains("allow"))
    })
}

/// Does any `StaticText` in the open view contain `needle`?
pub fn shows_text(nodes: &[RNode], needle: &str) -> bool {
    nodes
        .iter()
        .any(|n| n.role == "StaticText" && n.name.contains(needle))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn node(id: &str, parent: &str, role: &str, name: &str, backend: Option<i64>) -> Value {
        let mut v = json!({"nodeId": id, "role": {"value": role}, "name": {"value": name}});
        if !parent.is_empty() {
            v["parentId"] = json!(parent);
        }
        if let Some(b) = backend {
            v["backendDOMNodeId"] = json!(b);
        }
        v
    }

    /// The measured shape: link and button group are siblings under one row.
    pub fn sidebar(states: &[(i64, &str)]) -> Vec<RNode> {
        let mut nodes = vec![node("root", "", "RootWebArea", "hub", None)];
        for (b, state) in states {
            let row = format!("row{b}");
            let grp = format!("grp{b}");
            nodes.push(node(&row, "root", "generic", "", None));
            nodes.push(node(&format!("l{b}"), &row, "link", "Some Title", Some(*b)));
            nodes.push(node(&grp, &row, "generic", "", None));
            nodes.push(node(
                &format!("m{b}"),
                &grp,
                "button",
                ROW_MARKER,
                Some(b + 1000),
            ));
            nodes.push(node(
                &format!("s{b}"),
                &grp,
                "button",
                state,
                Some(b + 2000),
            ));
        }
        reduce(&json!({ "nodes": nodes }))
    }

    #[test]
    fn reduce_drops_ignored_and_keeps_parent() {
        let raw = json!({"nodes": [
            {"nodeId": "1", "role": {"value": "RootWebArea"}, "name": {"value": "x"}},
            {"nodeId": "2", "parentId": "1", "ignored": true, "role": {"value": "generic"}},
            {"nodeId": "3", "parentId": "1", "role": {"value": "link"}, "name": {"value": "A"}, "backendDOMNodeId": 9},
        ]});
        let r = reduce(&raw);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].parent.as_deref(), Some("1"));
        assert_eq!(r[1].backend, Some(9));
        assert_eq!(dump(&r).as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn row_state_reads_each_row_by_its_own_buttons() {
        let n = sidebar(&[(1, RUNNING_BUTTON), (2, IDLE_BUTTON), (3, RUNNING_BUTTON)]);
        assert_eq!(conversation_links(&n).len(), 3);
        assert_eq!(row_state(&n, 1), Some(RowState::Running));
        assert_eq!(row_state(&n, 2), Some(RowState::Idle));
        assert_eq!(row_state(&n, 3), Some(RowState::Running));
        assert_eq!(row_state(&n, 4), None);
    }

    #[test]
    fn a_container_of_many_rows_is_not_a_row() {
        let mut n = sidebar(&[(1, RUNNING_BUTTON), (2, IDLE_BUTTON)]);
        n.push(RNode {
            id: "nc".into(),
            parent: Some("root".into()),
            role: "link".into(),
            name: "New Conversation".into(),
            backend: Some(77),
        });
        assert_eq!(conversation_links(&n).len(), 2);
        assert_eq!(row_state(&n, 77), None);
    }

    /// Falsifier for the 06:47Z false Green: three rows read running while the
    /// pane asked for write access. The planted prompt must be seen.
    #[test]
    fn a_planted_write_access_prompt_is_seen() {
        let mut n = sidebar(&[
            (1, RUNNING_BUTTON),
            (2, RUNNING_BUTTON),
            (3, RUNNING_BUTTON),
        ]);
        assert!(!shows_approval(&n));
        n.push(RNode {
            id: "p".into(),
            parent: Some("root".into()),
            role: "StaticText".into(),
            name: "Allow write access to this path?".into(),
            backend: None,
        });
        assert!(shows_approval(&n));
        let mut w = sidebar(&[(1, RUNNING_BUTTON)]);
        w.push(RNode {
            id: "w".into(),
            parent: Some("root".into()),
            role: "StaticText".into(),
            name: "Waiting for user input.".into(),
            backend: None,
        });
        assert!(shows_approval(&w));
    }

    #[test]
    fn new_rows_are_the_unseen_ones_oldest_first() {
        assert_eq!(new_rows(&[90, 12, 77, 40], &[12, 40]), vec![77, 90]);
        assert!(new_rows(&[12], &[12]).is_empty());
    }

    #[test]
    fn a_link_without_a_row_marker_is_not_a_conversation() {
        let raw = json!({"nodes": [
            node("r", "", "RootWebArea", "", None),
            node("l", "r", "link", "New Conversation", Some(5)),
        ]});
        assert!(conversation_links(&reduce(&raw)).is_empty());
    }
}
