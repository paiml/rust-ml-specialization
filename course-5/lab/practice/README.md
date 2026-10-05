# Practice lab: two agents, a judged workflow, and fan-in

Three exercises, ported from the course demos so they run anywhere with only
the Rust standard library. Nothing here needs a GPU, a model, a network or an
account. Each program checks its **Provable contracts** with `assert!` and only
then prints `contract: <name> OK`.

```bash
cargo test --offline
cargo run --offline --bin d18_one_server   # two agents, one model server
cargo run --offline --bin d19_mini_judge   # a workflow judged by named claims
cargo run --offline --bin d21_fan_in       # fan-in and stop-the-line
```

| Exercise | Contract | What it proves |
|---|---|---|
| `d18_one_server` | `one-server-v1` | The shared server loads once; each agent writes only its own sink; the checker reads only after the draft exists; any schedule gives the same bytes; the verdict is the checker's. |
| `d19_mini_judge` | `workflow-judge-v1` | The golden record passes; each mutant is refused for exactly its named claim; harmless mutations pass. |
| `d21_fan_in` | `fan-in-v1` | Every arrival order reduces to one digest; a red result refuses the merge and names the lowest red agent; no stop lands before the red. |

## Try it: make each contract fire

A contract you have never seen fail is a contract you have not tested.

- **D18.** In `src/one_server.rs`, change `ensure_loaded` so it loads every
  time (`self.loads += 1;` with no `if`). `one_load` fires.
- **D19.** In `src/judge.rs`, delete the `writes-disjoint` check from `judge`.
  The kill row "checker overwrites the draft" is now refused for no claim, and
  the run fails naming that row.
- **D21.** In `src/fanin.rs`, stop agents on the red's own tick
  (`Some(tick)` instead of `Some(tick + 1)`). `stops_follow_red` fires and
  prints the timeline.

Put each change back before moving on.
