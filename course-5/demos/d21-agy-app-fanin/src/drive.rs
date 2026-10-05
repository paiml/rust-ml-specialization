//! Driving the app's agent view through agy-cdp's closed method set: find a
//! control in the tree, click its box centre, type only into a focused box,
//! and stop an agent by clicking that agent's own stop control.

use std::time::{Duration, Instant};

use agy_cdp::ax::{self, Node, Page};
use agy_cdp::methods::{Key, Method, MousePhase};
use agy_cdp::open_under::Roots;
use agy_cdp::transport::Conn;

use crate::tree::{self, RNode, RowState};

/// Radio choices on the app's permission prompt, most specific first. "In
/// this conversation" scopes the grant to the one agent that asked.
const GRANTS: [&str; 2] = ["always allow in this conversation", "allow this time"];

/// How many agents a take drives.
pub const AGENTS: usize = 3;

/// One stop: when the click went out and when the tree first read the agent
/// stopped, both on the drive clock, plus the control that was clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stop {
    pub sent_ms: u64,
    pub control: i64,
}

pub struct Drive<'a> {
    pub conn: Conn,
    pub page: Page,
    roots: &'a Roots,
    t0: Instant,
    /// Each agent's sidebar link, by DOM backing.
    pub agents: Vec<Option<i64>>,
    /// The task each agent's own view showed, read back from the tree.
    pub tasks: Vec<Option<String>>,
    /// Whether the tree ever read each agent running.
    pub seen_running: Vec<bool>,
    /// Write a dump of every snapshot into `trees/`.
    pub dump: bool,
    tick: usize,
    turn: usize,
}

fn find<'n>(nodes: &'n [Node], role: &str, needle: &str) -> Option<&'n Node> {
    ax::find(nodes, role, needle)
        .into_iter()
        .find(|n| n.backend.is_some())
}

fn node(role: &str, backend: i64) -> Node {
    Node {
        role: role.into(),
        name: String::new(),
        backend: Some(backend),
        focused: false,
        ignored: false,
    }
}

impl<'a> Drive<'a> {
    pub fn new(conn: Conn, page: Page, roots: &'a Roots, slots: usize) -> Self {
        Drive {
            conn,
            page,
            roots,
            t0: Instant::now(),
            agents: vec![None; slots],
            tasks: vec![None; slots],
            seen_running: vec![false; slots],
            dump: true,
            tick: 0,
            turn: 0,
        }
    }

    /// Restart the drive clock.
    pub fn start_clock(&mut self) {
        self.t0 = Instant::now();
    }

    pub fn ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    /// Milliseconds on the drive clock, with a fraction.
    pub fn ms_f(&self) -> f64 {
        self.t0.elapsed().as_secs_f64() * 1000.0
    }

    /// One tree snapshot, reduced; dumped to `trees/` when `dump` is set.
    pub fn snapshot(&mut self) -> Result<Vec<RNode>, String> {
        let raw = self
            .conn
            .send(Method::AccessibilityGetFullAxTree, Some(&self.page.session))?;
        let t = self.ms();
        let nodes: Vec<RNode> = raw["nodes"]
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
            .collect();
        if self.dump {
            self.roots.write_run(
                &format!("trees/t-{:05}-{t}.tsv", self.tick),
                tree::dump(&nodes).as_bytes(),
            )?;
        }
        self.tick += 1;
        for (i, s) in self.states(&nodes).iter().enumerate() {
            if *s == Some(RowState::Running) {
                self.seen_running[i] = true;
            }
        }
        Ok(nodes)
    }

    pub fn states(&self, nodes: &[RNode]) -> Vec<Option<RowState>> {
        self.agents
            .iter()
            .map(|a| a.and_then(|b| tree::row_state(nodes, b)))
            .collect()
    }

    pub fn wait_for(&mut self, role: &str, needle: &str, secs: u64) -> Result<Vec<Node>, String> {
        let end = Instant::now() + Duration::from_secs(secs);
        loop {
            let nodes = ax::tree(&mut self.conn, &self.page)?;
            if find(&nodes, role, needle).is_some() {
                return Ok(nodes);
            }
            if Instant::now() > end {
                return Err(format!("{role} {needle:?} not in the tree within {secs} s"));
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    pub fn click(&mut self, role: &str, needle: &str) -> Result<bool, String> {
        let nodes = ax::tree(&mut self.conn, &self.page)?;
        match find(&nodes, role, needle) {
            Some(n) => ax::click(&mut self.conn, &self.page, n).map(|_| true),
            None => Ok(false),
        }
    }

    /// Dismiss the cold-start notice when the tree shows it; true if clicked.
    pub fn dismiss_notice(&mut self) -> Result<bool, String> {
        let hit = self.click("button", "dismiss")?;
        if hit {
            std::thread::sleep(Duration::from_secs(2));
        }
        Ok(hit)
    }

    /// Type `text` into the task box once the tree shows it focused, then Enter.
    fn send_task(&mut self, text: &str) -> Result<(), String> {
        let nodes = self.wait_for("combobox", "message input", 20)?;
        let b = find(&nodes, "combobox", "message input")
            .ok_or("no task box")?
            .clone();
        ax::click(&mut self.conn, &self.page, &b)?;
        if let Some(id) = b.backend {
            self.conn.send(
                Method::DomFocus {
                    backend_node_id: id,
                },
                Some(&self.page.session),
            )?;
        }
        let mut last = String::new();
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(500));
            match ax::insert_text_checked(&mut self.conn, &self.page, text) {
                Ok(()) => return ax::press(&mut self.conn, &self.page, Key::Enter),
                Err(m) => last = m,
            }
        }
        Err(last)
    }

    /// Create agent `slot` (0-based) with `prompt`: new conversation, its
    /// task, then wait for its sidebar row and read `task` back from its view.
    pub fn new_agent(&mut self, slot: usize, task: &str, prompt: &str) -> Result<(), String> {
        let before: Vec<i64> = tree::conversation_links(&self.snapshot()?)
            .iter()
            .filter_map(|l| l.backend)
            .collect();
        if !self.click("button", "new conversation")? {
            return Err("no New Conversation button".into());
        }
        std::thread::sleep(Duration::from_secs(2));
        if self.click("button", "not now")? {
            std::thread::sleep(Duration::from_secs(1));
        }
        self.send_task(prompt)?;
        self.find_new_row(slot, task, &before)
    }

    fn find_new_row(&mut self, slot: usize, task: &str, before: &[i64]) -> Result<(), String> {
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end {
            let nodes = self.snapshot()?;
            let new = tree::conversation_links(&nodes)
                .iter()
                .filter_map(|l| l.backend)
                .find(|b| !before.contains(b) && !self.agents.contains(&Some(*b)));
            if let Some(b) = new {
                self.agents[slot] = Some(b);
                if tree::shows_text(&nodes, task) {
                    self.tasks[slot] = Some(task.to_string());
                }
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        Err(format!(
            "agent {}: no new conversation row within 20 s",
            slot + 1
        ))
    }

    /// Answer a visible permission prompt with the narrowest grant, if any.
    pub fn grant(&mut self, nodes: &[RNode]) -> Result<bool, String> {
        let radio = GRANTS.iter().find_map(|g| {
            nodes
                .iter()
                .find(|n| n.role == "radio" && n.name.to_lowercase().contains(g))
        });
        let Some(r) = radio.and_then(|r| r.backend) else {
            return Ok(false);
        };
        ax::click(&mut self.conn, &self.page, &node("radio", r))?;
        std::thread::sleep(Duration::from_millis(300));
        self.click("button", "submit")
    }

    /// Open agent `slot`'s own conversation view.
    pub fn open(&mut self, slot: usize) -> Result<(), String> {
        match self.agents[slot] {
            Some(b) => ax::click(&mut self.conn, &self.page, &node("link", b)),
            None => Ok(()),
        }
    }

    /// Bring the next agent in `wanted` into view, reading its task back the
    /// first time it is shown.
    pub fn rotate(&mut self, wanted: &[bool]) -> Result<(), String> {
        let n = self.agents.len();
        for k in 0..n {
            let i = (self.turn + k) % n;
            if wanted[i] {
                self.turn = i + 1;
                self.open(i)?;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Record a task read back from the open view.
    pub fn read_back(&mut self, nodes: &[RNode], tasks: &[&str]) {
        for (i, t) in tasks.iter().enumerate() {
            if self.tasks[i].is_none() && tree::shows_text(nodes, t) {
                self.tasks[i] = Some(t.to_string());
            }
        }
    }

    /// Click agent `slot`'s own stop control, named in `nodes`: the
    /// `Stop execution` button in that agent's sidebar row, through
    /// `DOM.getBoxModel` and `Input.dispatchMouseEvent` at the box centre.
    pub fn click_stop(&mut self, nodes: &[RNode], slot: usize) -> Result<Stop, String> {
        let link = self.agents[slot].ok_or("agent has no row")?;
        let control = tree::row_button(nodes, link, tree::RUNNING_BUTTON)
            .ok_or_else(|| format!("agent {}: no stop control in its row", slot + 1))?;
        let (x, y) = ax::centre(&mut self.conn, &self.page, control)?;
        let sent_ms = self.ms();
        for phase in [MousePhase::Moved, MousePhase::Pressed, MousePhase::Released] {
            self.conn.send(
                Method::InputDispatchMouseEvent { phase, x, y },
                Some(&self.page.session),
            )?;
        }
        Ok(Stop { sent_ms, control })
    }

    /// Is agent `slot`'s row present and not running?
    pub fn stopped(&self, nodes: &[RNode], slot: usize) -> bool {
        let state = self.agents[slot].and_then(|b| tree::row_state(nodes, b));
        matches!(state, Some(s) if s != RowState::Running)
    }

    /// Archive agent `slot`'s conversation once it is idle (housekeeping
    /// between samples); false when the row has no archive control.
    pub fn archive(&mut self, slot: usize) -> Result<bool, String> {
        let nodes = self.snapshot()?;
        let Some(link) = self.agents[slot] else {
            return Ok(false);
        };
        let Some(b) = tree::row_button(&nodes, link, tree::IDLE_BUTTON) else {
            return Ok(false);
        };
        ax::click(&mut self.conn, &self.page, &node("button", b))?;
        Ok(true)
    }
}
