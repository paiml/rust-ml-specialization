//! Driving the app's agent view through agy-cdp's closed method set: find a
//! control in the tree, click its box centre, type only into a focused box.

use std::time::{Duration, Instant};

use agy_cdp::ax::{self, Node, Page};
use agy_cdp::methods::{Key, Method};
use agy_cdp::open_under::Roots;
use agy_cdp::transport::Conn;

use crate::record::TASKS;
use crate::timeline::Timeline;
use crate::tree::{self, RNode, RowState};

/// Radio choices on the app's permission prompt, most specific first. "In
/// this conversation" scopes the grant to the one agent that asked.
const GRANTS: [&str; 2] = ["always allow in this conversation", "allow this time"];

pub struct Drive<'a> {
    pub conn: Conn,
    pub page: Page,
    roots: &'a Roots,
    t0: Instant,
    /// Each agent's sidebar link, by DOM backing.
    pub agents: [Option<i64>; 3],
    pub tasks: [Option<String>; 3],
    pub tl: Timeline,
    tick: usize,
    turn: usize,
}

fn find<'n>(nodes: &'n [Node], role: &str, needle: &str) -> Option<&'n Node> {
    ax::find(nodes, role, needle)
        .into_iter()
        .find(|n| n.backend.is_some())
}

fn link(backend: i64) -> Node {
    Node {
        role: "link".into(),
        name: String::new(),
        backend: Some(backend),
        focused: false,
        ignored: false,
    }
}

impl<'a> Drive<'a> {
    pub fn new(conn: Conn, page: Page, roots: &'a Roots) -> Self {
        Drive {
            conn,
            page,
            roots,
            t0: Instant::now(),
            agents: [None; 3],
            tasks: [None, None, None],
            tl: Timeline::default(),
            tick: 0,
            turn: 0,
        }
    }

    /// Restart the fan-out clock.
    pub fn start_clock(&mut self) {
        self.t0 = Instant::now();
    }

    pub fn ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    /// One tree snapshot: dumped to the run directory and folded into the
    /// timeline as one observation.
    pub fn snapshot(&mut self) -> Result<Vec<RNode>, String> {
        let raw = self
            .conn
            .send(Method::AccessibilityGetFullAxTree, Some(&self.page.session))?;
        let t = self.ms();
        let nodes = tree::reduce(&raw);
        self.roots.write_run(
            &format!("trees/t-{:04}-{t}.json", self.tick),
            tree::dump(&nodes).to_string().as_bytes(),
        )?;
        self.tick += 1;
        let states = self
            .agents
            .map(|a| a.and_then(|b| tree::row_state(&nodes, b)));
        self.tl.observe(states, t);
        Ok(nodes)
    }

    pub fn states(&self, nodes: &[RNode]) -> [Option<RowState>; 3] {
        self.agents
            .map(|a| a.and_then(|b| tree::row_state(nodes, b)))
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

    /// Create agent `n` (1-based): new conversation, its task, then wait for
    /// its sidebar row and read its task back from its own view.
    pub fn new_agent(&mut self, n: usize) -> Result<(), String> {
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
        let task = TASKS[n - 1];
        let dir = self.roots.run_path(&format!("ws/agent-{n}"))?;
        let text = format!(
            "{task}: create one file named result.md in the folder {} whose only content is the line {task}. Do not read, create or edit any other file and do not run any command.",
            dir.display()
        );
        self.send_task(&text)?;
        self.find_new_row(n, &before)
    }

    fn find_new_row(&mut self, n: usize, before: &[i64]) -> Result<(), String> {
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end {
            let nodes = self.snapshot()?;
            let new = tree::conversation_links(&nodes)
                .iter()
                .filter_map(|l| l.backend)
                .find(|b| !before.contains(b) && !self.agents.contains(&Some(*b)));
            if let Some(b) = new {
                self.agents[n - 1] = Some(b);
                if tree::shows_text(&nodes, TASKS[n - 1]) {
                    self.tasks[n - 1] = Some(TASKS[n - 1].to_string());
                }
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        Err(format!("agent {n}: no new conversation row within 20 s"))
    }

    /// Snapshots until one holds all three agents running.
    pub fn wait_all_running(&mut self, secs: u64) -> Result<(), String> {
        let end = Instant::now() + Duration::from_secs(secs);
        while self.tl.all_running_ms.is_none() {
            if Instant::now() > end {
                return Err(format!("no snapshot with three running agents in {secs} s"));
            }
            self.snapshot()?;
            std::thread::sleep(Duration::from_millis(300));
        }
        Ok(())
    }

    /// Answer a visible permission prompt with the narrowest grant, if any.
    fn grant(&mut self, nodes: &[RNode]) -> Result<bool, String> {
        let radio = GRANTS.iter().find_map(|g| {
            nodes
                .iter()
                .find(|n| n.role == "radio" && n.name.to_lowercase().contains(g))
        });
        let Some(r) = radio.and_then(|r| r.backend) else {
            return Ok(false);
        };
        let mut node = link(r);
        node.role = "radio".into();
        ax::click(&mut self.conn, &self.page, &node)?;
        std::thread::sleep(Duration::from_millis(300));
        self.click("button", "submit")
    }

    /// Bring the next running agent's conversation into view.
    fn rotate(&mut self, states: [Option<RowState>; 3]) -> Result<(), String> {
        for k in 0..3 {
            let i = (self.turn + k) % 3;
            if states[i] == Some(RowState::Running) {
                self.turn = i + 1;
                if let Some(b) = self.agents[i] {
                    ax::click(&mut self.conn, &self.page, &link(b))?;
                    if self.tasks[i].is_none() {
                        std::thread::sleep(Duration::from_millis(800));
                        let nodes = self.snapshot()?;
                        if tree::shows_text(&nodes, TASKS[i]) {
                            self.tasks[i] = Some(TASKS[i].to_string());
                        }
                    }
                }
                return Ok(());
            }
        }
        Ok(())
    }

    /// Until every agent is done: answer each agent's permission prompt in
    /// its own view, visiting the running agents in turn.
    pub fn run_to_done(&mut self, secs: u64) -> Result<(), String> {
        let end = Instant::now() + Duration::from_secs(secs);
        while !self.tl.all_done() {
            if Instant::now() > end {
                return Err(format!("agents not all done in {secs} s"));
            }
            let nodes = self.snapshot()?;
            if !self.grant(&nodes)? {
                self.rotate(self.states(&nodes))?;
            }
            std::thread::sleep(Duration::from_millis(1200));
        }
        Ok(())
    }
}
