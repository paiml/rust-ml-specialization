//! The fan-in reducer: a pure function of the multiset of results. No clock,
//! no I/O; the results are put in one canonical order before anything is
//! read, so the arrival order cannot reach the answer. A red result refuses
//! the merge and names the lowest red agent id.
//!
//! `permutations()` is the one helper that builds every arrival order, used
//! both by the live run (`reduce_orders`) and by the unit tests below.

/// One agent's result as the reducer sees it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentResult {
    /// 1-based agent id.
    pub agent: u8,
    /// The pure Rust acceptance check's verdict on the agent's output.
    pub red: bool,
    /// `(path, size, sha256)` of every file in the agent's workspace.
    pub files: Vec<(String, u64, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reduction {
    pub merged: bool,
    pub refusal_names: Vec<String>,
    pub digest: String,
}

pub type Reducer = fn(&[AgentResult]) -> Reduction;

/// The reducer.
pub fn reduce(results: &[AgentResult]) -> Reduction {
    let mut canon: Vec<AgentResult> = results.to_vec();
    for r in &mut canon {
        r.files.sort();
    }
    canon.sort();
    let lowest_red = canon.iter().filter(|r| r.red).map(|r| r.agent).min();
    let merged = lowest_red.is_none();
    let refusal_names: Vec<String> = lowest_red
        .map(|a| format!("agent-{a}"))
        .into_iter()
        .collect();
    let mut text = String::new();
    for r in &canon {
        let verdict = if r.red { "red" } else { "green" };
        text.push_str(&format!("agent-{}\t{verdict}\n", r.agent));
        for (p, s, h) in &r.files {
            text.push_str(&format!("\t{p}\t{s}\t{h}\n"));
        }
    }
    text.push_str(&format!(
        "merged\t{merged}\nrefusal\t{}\n",
        refusal_names.join(",")
    ));
    Reduction {
        merged,
        refusal_names,
        digest: demo_kit::sha::sha256_bytes(text.as_bytes()),
    }
}

/// Every ordering of `items`, in lexicographic order of their positions:
/// for `[1, 2, 3]` that is 1-2-3, 1-3-2, 2-1-3, 2-3-1, 3-1-2, 3-2-1.
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

/// An arrival order's label, the agent ids joined by `-`.
pub fn order_label(order: &[AgentResult]) -> String {
    order
        .iter()
        .map(|r| r.agent.to_string())
        .collect::<Vec<_>>()
        .join("-")
}

/// Run `reducer` over every arrival order of `results`, built by
/// [`permutations`]: `(label, reduction)` per order.
pub fn reduce_orders(results: &[AgentResult], reducer: Reducer) -> Vec<(String, Reduction)> {
    permutations(results)
        .into_iter()
        .map(|order| (order_label(&order), reducer(&order)))
        .collect()
}

/// Does `reducer` give one reduction over every arrival order of `results`?
pub fn order_free(results: &[AgentResult], reducer: Reducer) -> bool {
    let all = reduce_orders(results, reducer);
    all.windows(2).all(|w| w[0].1 == w[1].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(agent: u8, red: bool, body: &str) -> AgentResult {
        AgentResult {
            agent,
            red,
            files: vec![(
                format!("ws/agent-{agent}/result.md"),
                body.len() as u64,
                demo_kit::sha::sha256_bytes(body.as_bytes()),
            )],
        }
    }

    fn sets() -> Vec<Vec<AgentResult>> {
        vec![
            vec![
                r(1, false, ""),
                r(2, false, ""),
                r(3, true, "fixture-gamma FAIL"),
            ],
            vec![r(1, false, "a"), r(2, false, "b"), r(3, false, "c")],
            vec![r(1, true, "a"), r(2, false, "b"), r(3, true, "c")],
            vec![
                r(1, false, "a"),
                AgentResult {
                    agent: 2,
                    red: false,
                    files: vec![],
                },
                r(3, true, "x"),
            ],
        ]
    }

    /// The negative control: ignores its input.
    fn constant(_: &[AgentResult]) -> Reduction {
        Reduction {
            merged: false,
            refusal_names: vec!["agent-3".into()],
            digest: "0".repeat(64),
        }
    }

    /// A second control: depends on who arrived first.
    fn first_arrival(results: &[AgentResult]) -> Reduction {
        let mut x = reduce(results);
        x.digest = demo_kit::sha::sha256_bytes(order_label(results).as_bytes());
        x
    }

    /// Changing any one result changes the digest.
    fn sensitive(reducer: Reducer) -> bool {
        sets().iter().all(|set| {
            let base = reducer(set).digest;
            (0..set.len()).all(|i| {
                let mut flip = set.clone();
                flip[i].red = !flip[i].red;
                let mut edit = set.clone();
                edit[i].files.push(("ws/extra".into(), 1, "e".into()));
                reducer(&flip).digest != base && reducer(&edit).digest != base
            })
        })
    }

    #[test]
    fn permutations_are_the_six_orders_in_label_order() {
        let p = permutations(&[1, 2, 3]);
        assert_eq!(
            p,
            vec![
                vec![1, 2, 3],
                vec![1, 3, 2],
                vec![2, 1, 3],
                vec![2, 3, 1],
                vec![3, 1, 2],
                vec![3, 2, 1]
            ]
        );
        let labels: Vec<String> = reduce_orders(&sets()[0], reduce)
            .into_iter()
            .map(|(l, _)| l)
            .collect();
        assert_eq!(
            labels,
            ["1-2-3", "1-3-2", "2-1-3", "2-3-1", "3-1-2", "3-2-1"]
        );
        assert_eq!(permutations(&[7]).len(), 1);
        assert_eq!(permutations(&[1, 2, 3, 4]).len(), 24);
    }

    #[test]
    fn every_order_gives_one_digest_on_every_set() {
        for set in sets() {
            assert!(order_free(&set, reduce), "{set:?}");
        }
        assert!(!order_free(&sets()[0], first_arrival), "control must fail");
    }

    #[test]
    fn changing_one_result_changes_the_digest_and_the_constant_control_fails() {
        assert!(sensitive(reduce));
        assert!(
            !sensitive(constant),
            "a constant reducer passed as sensitive"
        );
        assert!(
            order_free(&sets()[0], constant),
            "constant is trivially order-free"
        );
    }

    #[test]
    fn several_reds_name_the_lowest_in_every_order() {
        let reds = [
            vec![r(1, true, "a"), r(2, false, "b"), r(3, true, "c")],
            vec![r(1, false, "a"), r(2, true, "b"), r(3, true, "c")],
            vec![r(1, true, "a"), r(2, true, "b"), r(3, true, "c")],
        ];
        let want = ["agent-1", "agent-2", "agent-1"];
        for (set, w) in reds.iter().zip(want) {
            for (label, red) in reduce_orders(set, reduce) {
                assert!(!red.merged, "{label}");
                assert_eq!(red.refusal_names, [w], "{label}");
            }
        }
        let green = reduce(&sets()[1]);
        assert!(green.merged);
        assert!(green.refusal_names.is_empty());
    }
}
