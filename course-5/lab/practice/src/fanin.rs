//! Fan-in and stop-the-line (ported from D21).
//!
//! Agents finish in any order. The reducer must give one digest for every
//! arrival order, refuse the merge when any agent is red, and name the
//! lowest red agent. The live demo's stop clicks become a simulated timeline.

use crate::fnv1a;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentResult {
    pub agent: u8,
    pub red: bool,
    /// (path, size, content digest)
    pub files: Vec<(String, u64, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reduction {
    pub merged: bool,
    pub refusal: Option<String>,
    pub digest: u64,
}

pub type Reducer = fn(&[AgentResult]) -> Reduction;

pub fn result(agent: u8, red: bool, body: &str) -> AgentResult {
    AgentResult {
        agent,
        red,
        files: vec![(
            format!("ws/agent-{agent}/result.md"),
            body.len() as u64,
            fnv1a(body.as_bytes()),
        )],
    }
}

/// Canonicalise (sort), then decide and digest. Order of arrival is erased
/// before anything is computed from it.
pub fn reduce(results: &[AgentResult]) -> Reduction {
    let mut canon = results.to_vec();
    for r in &mut canon {
        r.files.sort();
    }
    canon.sort();
    let refusal = canon
        .iter()
        .filter(|r| r.red)
        .map(|r| r.agent)
        .min()
        .map(|a| format!("agent-{a}"));
    let merged = refusal.is_none();
    let mut text = String::new();
    for r in &canon {
        text.push_str(&format!(
            "agent-{}\t{}\n",
            r.agent,
            if r.red { "red" } else { "green" }
        ));
        for (p, s, h) in &r.files {
            text.push_str(&format!("\t{p}\t{s}\t{h:016x}\n"));
        }
    }
    text.push_str(&format!(
        "merged\t{merged}\nrefusal\t{}\n",
        refusal.clone().unwrap_or_default()
    ));
    Reduction {
        merged,
        refusal,
        digest: fnv1a(text.as_bytes()),
    }
}

pub fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut out = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let head = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, head.clone());
            out.push(tail);
        }
    }
    out
}

/// True when every arrival order reduces to the same result.
pub fn order_free(results: &[AgentResult], reducer: Reducer) -> bool {
    let all: Vec<Reduction> = permutations(results).iter().map(|o| reducer(o)).collect();
    all.windows(2).all(|w| w[0] == w[1])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Finished { agent: u8, red: bool },
    Stopped { agent: u8 },
}

/// Agents finish at the given ticks. When a red result lands, every agent
/// still running is stopped on the NEXT tick (a stop takes time to arrive).
pub fn timeline(finishes: &[(u32, u8, bool)]) -> Vec<(u32, Event)> {
    let mut sorted = finishes.to_vec();
    sorted.sort();
    let mut events = Vec::new();
    let mut stopped_at: Option<u32> = None;
    for &(tick, agent, red) in &sorted {
        if let Some(s) = stopped_at {
            if tick > s {
                events.push((s, Event::Stopped { agent }));
                continue;
            }
        }
        events.push((tick, Event::Finished { agent, red }));
        if red && stopped_at.is_none() {
            stopped_at = Some(tick + 1);
        }
    }
    events.sort_by_key(|&(t, _)| t);
    events
}

/// The stop-the-line contract: no stop without a red, and every stop comes
/// strictly after the first red.
pub fn stops_follow_red(events: &[(u32, Event)]) -> bool {
    let first_red = events
        .iter()
        .find(|(_, e)| matches!(e, Event::Finished { red: true, .. }))
        .map(|&(t, _)| t);
    events.iter().all(|&(t, e)| match e {
        Event::Stopped { .. } => first_red.is_some_and(|r| t > r),
        Event::Finished { .. } => true,
    })
}
