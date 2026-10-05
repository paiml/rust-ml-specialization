//! The accessibility tree reduced to what D21 reads: id, parent, role, name,
//! DOM backing. Pure functions only, so every rule here has a unit test.
//!
//! Measured on the app (2.8.1), as D20 found it: each conversation in the
//! sidebar is a `link` whose parent is the row; the row holds a
//! `Stop execution` button while the agent runs and an `Archive conversation`
//! button when it is idle. That `Stop execution` button is the agent's own
//! stop control.

use std::collections::{BTreeSet, HashMap};

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

/// Roles that carry no meaning for the tab-separated dump.
fn layout_only(role: &str) -> bool {
    matches!(role, "" | "none" | "InlineTextBox")
}

/// A compact dump for the run directory: one `id\tparent\trole\tname` line
/// per meaningful node.
pub fn dump(nodes: &[RNode]) -> String {
    nodes
        .iter()
        .filter(|n| !layout_only(&n.role))
        .map(|n| {
            format!(
                "{}\t{}\t{}\t{}\n",
                n.id,
                n.parent.as_deref().unwrap_or("-"),
                n.role,
                n.name.replace(['\t', '\n'], " ")
            )
        })
        .collect()
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

/// The buttons under `row`; empty unless the row holds exactly one
/// `More options` (a container of many rows is not a row).
fn row_button_nodes<'a>(nodes: &'a [RNode], row: &str) -> Vec<&'a RNode> {
    let p = parents(nodes);
    let buttons: Vec<&RNode> = nodes
        .iter()
        .filter(|n| n.role == "button" && under(&p, &n.id, row))
        .collect();
    if buttons.iter().filter(|n| n.name == ROW_MARKER).count() != 1 {
        return Vec::new();
    }
    buttons
}

fn row_buttons<'a>(nodes: &'a [RNode], row: &str) -> BTreeSet<&'a str> {
    row_button_nodes(nodes, row)
        .into_iter()
        .map(|n| n.name.as_str())
        .collect()
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

fn row_of(nodes: &[RNode], backend: i64) -> Option<&str> {
    nodes
        .iter()
        .find(|n| n.role == "link" && n.backend == Some(backend))?
        .parent
        .as_deref()
}

/// The state of the row whose link has DOM backing `backend`.
pub fn row_state(nodes: &[RNode], backend: i64) -> Option<RowState> {
    let b = row_buttons(nodes, row_of(nodes, backend)?);
    if b.contains(RUNNING_BUTTON) {
        Some(RowState::Running)
    } else if b.contains(IDLE_BUTTON) {
        Some(RowState::Idle)
    } else {
        None
    }
}

/// The DOM backing of the button named `name` in the row of link `backend`:
/// that agent's own control, never another row's.
pub fn row_button(nodes: &[RNode], backend: i64, name: &str) -> Option<i64> {
    row_button_nodes(nodes, row_of(nodes, backend)?)
        .into_iter()
        .find(|n| n.name == name)
        .and_then(|n| n.backend)
}

/// Does any `StaticText` in the open view contain `needle`?
pub fn shows_text(nodes: &[RNode], needle: &str) -> bool {
    nodes
        .iter()
        .any(|n| n.role == "StaticText" && n.name.contains(needle))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    fn node(id: &str, parent: &str, role: &str, name: &str, backend: Option<i64>) -> RNode {
        RNode {
            id: id.into(),
            parent: (!parent.is_empty()).then(|| parent.to_string()),
            role: role.into(),
            name: name.into(),
            backend,
        }
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
        nodes
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
    fn the_stop_control_is_that_rows_own_button() {
        let n = sidebar(&[(1, RUNNING_BUTTON), (2, RUNNING_BUTTON), (3, IDLE_BUTTON)]);
        assert_eq!(row_button(&n, 1, RUNNING_BUTTON), Some(2001));
        assert_eq!(row_button(&n, 2, RUNNING_BUTTON), Some(2002));
        assert_eq!(row_button(&n, 3, RUNNING_BUTTON), None);
        assert_eq!(row_button(&n, 3, IDLE_BUTTON), Some(2003));
        assert_eq!(row_button(&n, 9, RUNNING_BUTTON), None);
    }

    #[test]
    fn a_container_of_many_rows_is_not_a_row() {
        let mut n = sidebar(&[(1, RUNNING_BUTTON), (2, IDLE_BUTTON)]);
        n.push(node("nc", "root", "link", "New Conversation", Some(77)));
        assert_eq!(conversation_links(&n).len(), 2);
        assert_eq!(row_state(&n, 77), None);
        assert_eq!(row_button(&n, 77, RUNNING_BUTTON), None);
    }

    #[test]
    fn dump_skips_layout_only_nodes() {
        let mut n = sidebar(&[(1, RUNNING_BUTTON)]);
        n.push(node("x", "root", "InlineTextBox", "t", None));
        n.push(node("y", "root", "StaticText", "a\tb", None));
        let d = dump(&n);
        assert_eq!(d.lines().count(), n.len() - 1);
        assert!(d.contains("y\troot\tStaticText\ta b\n"));
        assert!(shows_text(&n, "a\tb"));
    }
}
