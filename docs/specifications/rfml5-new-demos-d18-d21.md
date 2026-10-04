# rfml5 new demos D18–D21: two agents on one server, the workflow as an ontology, and agent fan-out in the Antigravity app

**Ticket:** PMAT-020 (#20), epic #19. **Branch:** `PMAT-020-rfml5-new-demos`.
**Status:** spec, revised after quorum round 1 (§7). All four pv contracts validate, and the fixtures below pass or fail exactly as stated: `A_1` (§6) passes 17/17 checks, and its `--self-test` shows each of 8 sabotages refused by the check it targets. None of the four demo binaries exists yet.

## 0. Origin

The course owner asked for four new demos for rfml5, "Agentic Patterns: Parallel Subagent ML with Rust", in these words:

> Two demos using aprender .70.x where we focus on apr serve and show multiple agents (just two) and newest "pv" where we show full power of ontology to spec out the two agent workflow, then TWO scripting the antigravity electron app showing agent fan out.

> using paiml-implment quorum style to validate the ideas for these four demos then build each in parallel after quorum work. YOU will use pv SHACL ontology to spec out each demo and include.

The owner then corrected the tooling choice: "newest aprender has support with "pv" … for these demos focus on this". So every shape in this spec is judged by **pv's own SHACL-Core engine**. No external SHACL engine (Jena or any other) is used or installed. Moving other tooling off Jena is tracked separately and is out of scope here.

"Include" is read literally: each demo directory ships its pv contract (`spec/`), a golden run record that the shapes pass, and a planted run record that the shapes reject with named findings. The harness judges every live run with the same shapes before it can be Green (§4).

## 1. The four demos

Lesson numbers are proposals, marked **[U]** until the course outline places them. The existing lesson map runs to rfml5/5.2.

| Id | Proposed lesson | One-line claim |
|---|---|---|
| D18 `d18-two-agents-one-server` | rfml5/3.3 [U] | Two agents, a writer and a checker, share **one** resident `apr serve`. The checker decides, every output file has one writer, and the schedule does not change the bytes. |
| D19 `d19-workflow-ontology` | rfml5/3.4 [U] | D18's workflow, written as a pv ontology plus SHACL shapes. pv kills 16 named mutants of a good run, each for exactly the constraint it breaks, and stays silent on the good run itself. Five documented mutants survive: they are what the shapes cannot see, and each is owned by a Rust assert. |
| D20 `d20-agy-app-fanout` | rfml5/4.3 [U] | A Rust driver opens the Antigravity desktop app on a scratch profile and fans out three agents you can watch. Every CDP method it sends is from a fixed list of twenty, none of which runs JavaScript. It sends only Enter, Escape and Tab, and the record carries the evidence that the app itself was driven. |
| D21 `d21-agy-app-fanin` | rfml5/4.4 [U] | Fan-in with stop-the-line. One agent goes red, the driver stops the other two after the red and never before, refuses to merge, and the reduction is the same in all six arrival orders. |

### D18: two agents, one server

- **Agents.** Two roles, each an LLM call with its own system prompt and its own output file, sharing one `apr serve` 0.70.1 process.
  - The **writer** answers four fixed questions `q1..q4` into `out/answers.json`.
  - The **checker** reads each answer and records `accept` or `reject` into `out/verdicts.json`.
- **Schedules.** Two are run against the same server process:
  - *sequential*: the writer finishes all four, then the checker runs.
  - *pipelined*: the checker works on item *i* while the writer produces *i+1*.
- **What it claims** (equations in `spec/d18-run-v1.yaml`):
  - `one_load`: one server spawn, with the same pid and the same start time at the start and the end. The start time is field 22 of `/proc/<pid>/stat`, so a respawn that reused the pid number still differs.
  - `disjoint_writes`: the writer's files are exactly `out/answers.json`, and the checker's exactly `out/verdicts.json`. Both lists come from the program's sink registry, and each sink opens its file with `create_new`.
  - `causality`: per item, the writer ends before the checker starts, in both schedules.
  - `schedule_independence`: the two schedules produce byte-identical output digests.
  - `checker_decides`: each decision is `accept` or `reject`, parsed from `out/verdicts.json`, the one file only the checker wrote.
- **How the shapes see it.** Each item is a named node (`q1..q4`), and each claim relates two named properties of one node: the pid pair, the start-time pair, the digest pair, and the writer-end and checker-start of each schedule.
- **What it does not claim: speed.** Measured (M1): 0.70.1 queues concurrent requests, and pipelining gained about 5 %. A speed claim would be false on this server, so the demo does not make one. The point is roles and correctness on one resident model, not throughput.

### D19: the workflow as an ontology

- **Subject.** D19's subject is D18's contract (`d18-run-v1`) and D18's golden run record.
- **Run.** D19 does the following:
  1. validates the contract;
  2. judges the golden record (Pass, with the positive control fired);
  3. judges the planted record (exactly the expected five findings);
  4. runs the 21-row mutant table in `fixtures/mutants.json`. Each row is an RFC 6902 JSON Patch of the golden record:
     - 16 **kill** rows must each be rejected *purely*: exit 1, the exact finding count, and every finding naming the intended focus node, shape and SHACL component;
     - 5 **survive** rows must each pass. Each is a defect the shapes cannot see, and the row names the Rust assert that catches it;
  5. shows the A-box is tamper-evident through `pv extract spec --check`;
  6. shows the engine passes its own W3C conformance cases (19/19).
- **What it claims** (`spec/d19-run-v1.yaml`):
  - `every_mutant_killed`: all 16 kill rows are rejected purely.
  - `survivors_documented`: all 5 survive rows pass, so the list of what the shapes cannot see is current.
  - `components_covered`: the kills cover all seven components d18-run-v1 uses: `in`, `datatype`, `pattern`, `lessThanOrEquals`, `minCount`, `maxCount` and `closed`.
  - `golden_silent`: the golden record raises nothing.
  - `abox_tamper_evident`: an edited record is detected.
  - `engine_conformant`: the engine passes its W3C cases.
- **D19's own record is a summary, not a self-judgement.** Every field in it is copied from pv's JSON output, and `xtask verify` re-runs the whole table independently. Judging that record with D19's own shapes checks its form. The evidence is the re-run, which anyone can repeat.

### D20: fan-out you can watch

- **Launch.** A Rust CDP driver launches Antigravity 2.8.1 under a virtual display. It uses a scratch user-data and extensions directory, with `HOME` and the `XDG_*` directories also redirected to scratch. It then:
  1. creates three agents in the app's agent view. Each has its own fixture task (`fixture-alpha`, `fixture-beta`, `fixture-gamma`) and its own scratch workspace, `ws/agent-N/`;
  2. waits until each reports done;
  3. captures three screenshots: before the fan-out, fanned out, and all done. They must be distinct and non-blank.
- **What it claims** (`spec/d20-run-v1.yaml`). Each agent is a named property set (`agent_N_task`, `agent_N_state`, `agent_N_files`), so every claim about agent N constrains agent N's own properties:
  - `fan_out`: each agent ran its own fixture task and reached done.
  - `app_driven`: the record's `app_evidence` holds `Accessibility.getFullAXTree`, `Input.dispatchMouseEvent` and `Input.insertText`, computed from the driver's tally of sent frames. A CLI fallback sends none of the three.
  - `disjoint_workspaces`: every file of agent N is under `ws/agent-N/`, and `stray_writes = 0`.
  - `no_js`: every method sent is in ALLOW, every key sent is Enter, Escape or Tab, and `devtools_targets_opened = 0`.
  - `operator_untouched`: the operator's own app profile is never opened or modified.
  - `evidence_distinct`: the three screenshots have three distinct digests.

### D21: fan-in and stop-the-line

- **Run.** Same driver and same app, three agents.
  - Agent-3's task carries a planted check failure, so the stop path runs on every take.
  - Agents 1 and 2 hold a long step. When agent-3 is seen red, the driver reads which agents are still running from the accessibility tree at that moment (`running_at_red`). It stops each of them through the app's own stop control.
  - The reducer then refuses to merge, naming agent-3. It runs over all six arrival orders of the three results, and the digests must be equal.
- **What it claims** (`spec/d21-run-v1.yaml`). Every timing claim is a relation between two named properties (`red_seen_ms`, `stop_agent_N_sent_ms`, `stop_agent_N_stopped_ms`):
  - `stop_the_line`: red_seen ≤ stop_sent ≤ stopped for agents 1 and 2; `stopped_agents = running_at_red = {agent-1, agent-2}`; and `writes_after_stop = 0`.
  - `refuse_on_red`: `merged = false`, and `refusal_names = {agent-3}`, the lowest red agent id.
  - `order_free`: all six orders ran, and they produced one digest.
  - `app_driven`, `no_js` and `operator_untouched`: as in D20, with the same evidence, the same twenty methods, the same three keys and the same DevTools count. The stops send no JavaScript either.

## 2. Measured facts this spec rests on

Each fact was measured on the build named. Anything not in this list is **[U]** until measured.

- **M1: `apr serve` 0.70.1.**
  - Two concurrent clients on one resident server are queued: the pipelined schedule was about 5 % faster than sequential.
  - Outputs were byte-identical across the two schedules.
  - Model: Qwen3.5-4B-Q4_K_M, sha256 `00fe7986…a4`. The full digest is in D18's golden fixture and manifest.
  - The course machine's declared `apr` is **0.69.3**, not 0.70.1. This is escalation E1.
- **M2: pv 0.70.1, `pv lint <spec-dir> --gate shapes --format json`.**
  - The entity is declared as `entity: {type: json, ref: receipt.json}` together with a `vocabulary` (`prefix`, `root_class`, `nested: {key: Class}`).
    - Nested-in-nested works, `nested: {}` is accepted, and several keys may map to one class.
    - A nested object under a key the vocabulary does not map is malformed (exit 3).
  - `ref` resolves against the **parent** of the contract directory, so a run is laid out as `<run>/spec/*.yaml` beside `<run>/receipt.json`.
  - **Components that work:**
    - `in` (int, bool, string; a boolean renders in a finding as `"true"^^xsd:boolean`), `datatype`, `pattern` (including nested alternation groups);
    - `minCount`/`maxCount` (including `maxCount: 0`);
    - `lessThanOrEquals` on strings and integers; applied in both directions it gives equality;
    - `closed` with `ignoredProperties: [rdf:type]`.
  - **Refused by pv** (so never used here): `equals`, `disjoint`, `and/or/not/xone`, `sparql`, `minInclusive`, `hasValue` (exit 3), complex property paths. D20's and D21's app evidence therefore uses `in` plus `minCount: 3` where `hasValue` would have been the natural choice.
  - **Exit codes:** 0 Pass, 1 reject, 2 decline, 3 malformed.
  - **JSON fields used here:** `verdict`, `violations` (a count), `findings[].message`, `pc_shape`, `plant_violations`, `focus_nodes_n`, `shapes_n`, `armed_shapes`, `not_armed_shapes`, `unarmed_violations`, `w3c_cases_passed`, `w3c_cases_n`.
  - **Finding message shape:** ``<focus> violates shape `<id>` (<component>): …``. A finding carries this message only, with no structured component field, so every check in this spec matches the message prefix.
- **M3: literal collapse, and positional objects.**
  - Repeated equal scalar values become one RDF literal. So `maxCount: 1` on a repeated property proves that every value is equal, and `minCount: N` counts *distinct* values: three screenshots of which two are identical give "has 2 value(s)".
  - D21's `order_free` rests on both. Six equal digests pass `maxCount: 1` and one differing digest fails it; `minCount: 6` on `reduce_orders` needs six distinct orders.
  - Objects do not collapse. An array of objects becomes 0-indexed positional nodes (`ont:d20/d20-run-v1.cdp_methods.8`), and a named nested object becomes the node `<subject>.<key>` (`ont:d18/d18-run-v1.q3`).
- **M4: A-box and drift.**
  - `pv extract spec` writes `contracts.nt` (the A-box) and `shapes.ttl`.
  - `--check` exits 0 when they are fresh and 1 after a one-byte edit to the run record.
  - A run directory **inside a git work tree** trips PV-ONT-014 F-34, so every judge run is materialised outside git.
- **M5: Antigravity 2.8.1.**
  - It is Electron (Chrome/146.0.7680.72, CDP protocol 1.3).
  - It starts headless under `xvfb-run -a` with these flags: `--user-data-dir <scratch> --extensions-dir <scratch> --remote-debugging-port=<p> --no-sandbox --disable-gpu --new-window`.
  - Its agent hub is a page at `https://127.0.0.1:<dynamic port>/`.
  - On a fresh scratch profile, the bundled language server logged that auth succeeded. *How* it obtained auth is unmeasured; see E2.
  - Auto-update is on, so the build is pinned by content:
    - `resources/app.asar` sha256 `cb425e9a…af26`;
    - the launcher binary sha256 `b0d12777…72d6`.
    - Both full digests are in the D20 contract and fixtures.
- **M6: the allow-list hole.**
  - The first D20 shape used a domain-prefix pattern. It **admitted** `Page.addScriptToEvaluateOnNewDocument`, which injects JavaScript (exit 0, no finding).
  - Fixed by replacing it with an exact list of twenty methods. The same record now exits 1, with the finding naming that method.
  - D21 carries the identical list.
- **M7: D19's mutant matrix**, run by `xtask/proofs/verify-mutants.sh` against D18's contract.
  - The identity (the unpatched golden record) passes: exit 0, Pass, no findings.
  - **16/16 kill rows are killed purely.** Each exits 1 with the exact finding count, and every finding starts ``<node><focus> violates shape `<target>` (<component>)``. A row that also trips a second constraint, or trips the right one on the wrong node, is reported as impure and fails.
    - m04, m05 and m06 raise two findings each, by design and all of one component. Each patches both members of a pair (both start ticks, both of `q2`'s sequential times, both digests), so the pair's `lessThanOrEquals` still holds and only `datatype` or `pattern` fires.
    - Every component is killed on the run node, and every component except `pattern` (which the item shape does not use) is also killed on an item node.
  - **5/5 survive rows pass** with exit 0 and no findings. They are what the shapes cannot see, each caught in Rust:
    - s01, both digests are the sha256 of the empty string: the program recomputes sha256 over the output files after each schedule.
    - s02, both pids are 1: the pid is read from the child handle, and `/proc/<pid>/exe` must resolve to the pinned `apr`.
    - s03, one decision flipped: decisions are parsed from `out/verdicts.json`, which both digests cover.
    - s04, one item shifted 100 s later so that q2 follows q3: every timestamp comes from one monotonic clock at the sink, items increase within a schedule, and each is at most that schedule's elapsed time.
    - s05, `writer_files` lists one path twice: an equivalent mutant, since RDF values are a set. Each sink opens its file with `create_new`, so a second open of one path is an error.
  - Result: **killed and named 16/16, survived 5/5.**
- **M8: relations only between named properties of one node.** pv's `lessThanOrEquals` compares two properties of the same focus node, and pv has no arithmetic. So every timing or identity claim here is written as a relation between two *named* properties of one node: `q1..q4` in D18, `agent_N_*` in D20, `stop_agent_N_*` in D21. A repeated path carrying a second `lessThanOrEquals` is checked independently; this was measured on D21's `red_seen_ms`, which is bounded by both stops.

The twenty CDP methods (ALLOW), exact match, anchored:

```
Target.getTargets  Target.attachToTarget  Target.detachFromTarget  Target.setDiscoverTargets
Browser.getVersion
Page.enable  Page.captureScreenshot  Page.bringToFront
DOM.enable  DOM.getDocument  DOM.getBoxModel  DOM.scrollIntoViewIfNeeded  DOM.focus  DOM.describeNode
Accessibility.enable  Accessibility.getFullAXTree  Accessibility.queryAXTree
Input.dispatchMouseEvent  Input.dispatchKeyEvent  Input.insertText
```

No `Runtime.*`, no `Debugger.*`, no `Page.navigate`, no `Page.addScriptToEvaluateOnNewDocument`, and nothing else.

## 3. What is in each demo directory now

| Path | D18 | D19 | D20 | D21 |
|---|---|---|---|---|
| `spec/dNN-run-v1.yaml` (contract: entity, vocabulary, shapes, equations, invariants, falsifiers) | 2 shapes | 1 shape | 2 shapes | 2 shapes |
| `fixtures/receipt.golden.json` passes: `pv lint` exit 0, Pass, `pc_shape` fired, `not_armed_shapes` empty, `unarmed_violations` 0, W3C 19/19 | focus 5 | focus 1 | focus 9 | focus 10 |
| the golden run's `plant_violations` (pv's own positive control) | 21 | 17 | 21 | 24 |
| `fixtures/receipt.planted.json` is rejected with exit 1 | 5 findings | 5 findings | 8 findings | 9 findings |
| `fixtures/receipt.planted.expect`: the sorted finding messages, which must match byte for byte | 5 lines | 5 lines | 8 lines | 9 lines |
| `fixtures/mutants.json` (RFC 6902) | — | 21 rows: 16 kill, 5 survive | — | — |

**These fixtures test the contracts, not a run.** They are hand-built records and are not evidence that any demo ran. Only a live run judged by the harness is (§4).

Each planted record breaks several claims at once, and every finding maps to exactly one planted defect:

- **D18:**
  - two server spawns (`in`);
  - the server's start ticks changed, a respawn under the same pid (`lessThanOrEquals`);
  - the checker also wrote `out/answers.json` (`in` on `checker_files`);
  - the pipelined digest differs from the sequential one (`lessThanOrEquals`);
  - on q3, the pipelined checker started before the writer ended (`lessThanOrEquals` on the item node `.q3`).
- **D19:**
  - `extract --check` missed the edit (`in` on `extract_drift_exit`);
  - 18 of 19 W3C cases passed (`lessThanOrEquals`);
  - one kill row (m07) is missing from `killed`, and one survive row (s02) from `survived` (`minCount`, twice);
  - six planted findings are reported where D18's `.expect` has five (`in`).
- **D20:**
  - the operator profile was touched, and agent 2 never reached done (`in`);
  - agent 1 also wrote `ws/agent-2/result.md` (`pattern`);
  - F12 was sent (`in` on `keys_sent`);
  - the app evidence lacks `Input.insertText`, and two of the three screenshots are identical (`minCount: 3`, twice);
  - `Runtime.evaluate` and `Page.addScriptToEvaluateOnNewDocument` were sent, each named by its own `cdp_methods` node (`pattern`).
- **D21:**
  - agent-1's stop was sent before the red was seen (`lessThanOrEquals`);
  - agent-3, which had already finished, is among the stopped agents (`in`);
  - the run merged anyway, and the refusal names agent-2 (`in`, twice);
  - one order's digest differs (`maxCount`), and one order ran twice, so only five distinct orders ran (`minCount`);
  - F12 was sent (`in`), and the app evidence lacks `Input.insertText` (`minCount`);
  - `Runtime.callFunctionOn` was sent, named by its method node (`pattern`).

## 4. Harness work (shared, lands first)

The run record and the harness `Receipt` are two separate files.

- **The run record** is what the shapes judge. It is the pv entity, the demo writes it, and it contains only what the demo measured.
- **The `Receipt`** is the harness's own envelope: pins, verdict, durations. It references the run record by sha256.

The changes:

1. **Exact pins for the new tools.**
   - `DemoManifest` gains `pv` and `antigravity` (both default `"none"`) and `antigravity_asar_sha256`.
   - `verify_demo` applies `pin::parse_exact` to each tool a demo uses, as it already does for `apr` and `agy`; floors are refused.
   - D18 pins `apr = "=0.70.1"` and `pv = "=0.70.1"`. D19 pins `pv = "=0.70.1"`. D20 and D21 pin `antigravity = "=2.8.1"`, the asar sha and `pv = "=0.70.1"`.
2. **Preflight** checks the installed `pv --version`, Antigravity's version and the asar sha256 against the pins.
   - A mismatch is `NotRun(VersionMismatch {…})`.
   - A changed asar under the same version string is a new reason, `AppMismatch { pinned_sha, found_sha }`; both seats that answered Q1 agree (§7). Auto-update makes this the likely failure.
   - Missing `pv` is `NotRun(MissingTool)`.
3. **New `shapes` module in demo-kit:** `judge(spec_dir, record_json, run_root) -> ShapesOutcome`. It:
   - materialises `<run_root>/judge/{spec/*.yaml, receipt.json}`;
   - **refuses a `run_root` inside any git work tree** (it walks up looking for `.git`), because of M4;
   - runs `pv lint <run>/judge/spec --gate shapes --format json`.

   How the result maps to a verdict:

   | pv result | Verdict |
   |---|---|
   | exit 0, `verdict = Pass`, `pc_shape = fired`, `not_armed_shapes = []`, `unarmed_violations = 0` and `w3c_cases_passed = w3c_cases_n > 0` | Green |
   | exit 0 with any of those conditions failing | Red ("a pass that proves nothing") |
   | exit 1 | Red, carrying every `findings[].message` |
   | exit 2, exit 3, or pv not found | NotRun |
   | anything else | Red |

4. **`Receipt.shapes: Option<ShapesOutcome>`.** It holds the verdict, findings, `focus_nodes_n`, `shapes_n`, `plant_violations` and the W3C counts.
   - A demo whose manifest names `pv` cannot be Green with `shapes = None`, or with a shapes outcome that is not Green.
   - The existing `demo-refusal-not-green-v1` falsifier gains a shapes arm.
5. **`xtask verify` gains a shapes arm** for every demo directory that has `spec/`:
   - the golden record must be Green;
   - the planted record must be Red, with its sorted messages byte-equal to `receipt.planted.expect`;
   - D19 additionally runs the mutant matrix (§5.2).
   - D20 and D21 additionally check `app_evidence ⊆ {m.method : m ∈ cdp_methods}`. pv cannot relate one property's values to another node's, so this cross-check is Rust's (M8).
   - `xtask verify --only <id>` is added so each build phase has its own acceptance command.
   - The arm **ports** the two committed proof scripts, `xtask/proofs/spec-proofs.sh` and `xtask/proofs/verify-mutants.sh`, into Rust. Each of the scripts' eight `--self-test` sabotages becomes a test that must see its named check refuse. Once the port is green, ph2 deletes both scripts, so no gate is kept in two places.
6. **CI installs pv.** `aprender-contracts-cli` 0.70.1 is published on crates.io (MIT).
   - The `course-5-demos` job gains `cargo install aprender-contracts-cli --version 0.70.1 --locked`, cached the same way as `bashrs`.
   - The job's existing rule holds: *a missing tool fails, never skips.*
   - The judge's run root in CI is the runner's temp dir, which is outside the checkout.
7. **Run records go under `$RFML5_RECEIPTS/<id>/<run_id>/`**, so no host path is written into the repo. `$RFML5_MODELS` holds the model and `$RFML5_DECLARED_APR` names the `apr` binary.
8. **An `xtask` lint confines the WebSocket dependency:** `tungstenite` may appear only in `agy-cdp`'s manifest and sources. Every other crate reaches the app through `agy-cdp`'s closed method enum. This lands in ph2 with the other lints.

## 5. Per-demo build

The shapes check *structure and relations*. pv has no arithmetic and no access to the world outside the record. Each demo therefore also asserts in Rust what a shape cannot see, and the `Provable contract:` docstring names those assertions. Every bin follows the repository's existing lint: a `Provable contract:` docstring, a final `assert!`, and then `contract: … OK`.

### 5.1 D18 (`apr`, `pv`, GPU lock)

**Steps:**

1. Preflight: `apr` =0.70.1, model sha, GPU lock.
2. `ServeGuard` spawns one `apr serve`.
3. Run the sequential schedule, then the pipelined one, on the same process. Use temperature 0 and a fixed seed.
4. Recompute both digests from the bytes on disk.
5. Write the run record, judge it, and write the Receipt.

**Rust asserts beyond the shapes.** Each of the first five catches one of D19's survive rows (§5.2), so every defect the shapes cannot see has a named owner:

- **Role↔file binding by construction (s05).** Each role writes through its own sink, and the sink registry records every path a sink opened. `writer_files` and `checker_files` come from that registry, never from what an agent reports. Every open is `create_new`, so opening one path twice is an error, not a second entry.
- **Digests are recomputed (s01):** sha256 over `out/answers.json ‖ out/verdicts.json` after each schedule, and each must equal the recorded digest. A digest an agent reported is never used.
- **Server identity (s02).** `ServeGuard` reads the pid from the spawned child handle, at spawn and at the end, and `/proc/<pid>/exe` must resolve to the binary preflight pinned (in CI, the `fake-apr-serve` bin). Field 22 of `/proc/<pid>/stat`, the start ticks, is read at both moments, which rules out a respawn that reused the pid number.
- **Decisions come from the checker's file (s03).** Each item's `decision` is parsed from `out/verdicts.json`, the one file only the checker's sink wrote, and both digests cover that file.
- **One clock (s04).** Every timestamp comes from the orchestrator's monotonic clock at the sink, never from an agent. Within a schedule, items are increasing in i, and each is at most that schedule's elapsed time.
- **The pipelined schedule really overlaps, and the sequential one never does.** For at least one i, the checker's request on item i and the writer's request on item i+1 are both in flight at once: `checker_start(i) < writer_end(i+1)` and `writer_start(i+1) < checker_end(i)`, on the same clock. In the sequential schedule no two requests overlap. Without this assert, a "pipelined" run that was secretly sequential would make `schedule_independence` vacuous.

**CI:** the full flow runs against the existing `fake-apr-serve` bin, so `cargo test -p d18-two-agents-one-server` exercises roles, causality, digests and the shapes judge without a GPU. The live run is the recording take.

### 5.2 D19 (`pv` only)

**Steps:**

1. `pv validate` on D18's contract.
2. Judge the golden record (Green), then the planted record (Red, with output equal to `.expect`).
3. Apply each row of `fixtures/mutants.json` with an RFC 6902 subset of `add`, `remove` and `replace` over `serde_json` pointers, and judge it:
   - **each of the 16 kill rows must be killed purely:** exit 1, exactly the row's stated finding count, and every message beginning with exactly ``ont:d18/d18-run-v1<node> violates shape `<target>` (<component>)``, the row's own node, shape and component;
   - **each of the 5 survive rows must pass:** exit 0, Pass, no findings. A survivor that starts failing means the list of what the shapes cannot see is stale.
4. Run `pv extract spec`, then `--check` (0), then a one-byte record edit, then `--check` again (1).
5. Write D19's own record, judge it with D19's shapes, and write the Receipt.

**Rust asserts beyond the shapes:**

- **Every patch is non-vacuous:** the patched JSON is not equal to the golden JSON.
- **An identity arm** (an empty patch) passes, so a judge that rejects everything cannot pass.
- **The planted `.expect` is byte-equal.**
- **The positive control** fired on every judge call.
- **D19's record is cross-checked against its sources**, because its shapes can only see the record:
  - `planted_findings` equals the number of lines in D18's `.expect`;
  - `killed` equals the ids of the kill rows, and `survived` the ids of the survive rows;
  - `components_killed` equals the set of the kill rows' components.

  `xtask verify --only d19-workflow-ontology` re-runs the whole table independently of the demo, so D19's own record is a summary of runs anyone can repeat, never its own judge.

### 5.3 D20 (`antigravity`, `pv`): the new `agy-cdp` crate first

**`agy-cdp`** is a small CDP client written for this course:

- Discovery is HTTP `GET /json/version` and `/json/list` over `std::net::TcpStream`, the same style as demo-kit's `http_request`.
- The WebSocket is `tungstenite` with `default-features = false, features = ["handshake"]`, over plain `ws://127.0.0.1`.
- Screenshots are decoded with `base64`.
- Both seats that answered Q2 in quorum round 1 chose these two dependencies over a hand-rolled RFC 6455 client, which would also need SHA-1 (§7).

**The method surface is a closed Rust enum** of exactly the ALLOW twenty, each method with typed parameters:

- There is no `send(method: &str, …)`, and the crate contains no `Runtime` type. An `xtask` lint refuses the strings `Runtime.`, `Debugger.`, `addScriptToEvaluate` and `Page.navigate` in the crate's sources.
- Every frame passes through one write function. That function counts methods by name, and **that socket-level tally** becomes `cdp_methods` in the run record.

**Key events need care.** The allow-list includes `Input.dispatchKeyEvent` and `Input.insertText`. Typing alone could open the app's developer tools, with F12 or a modifier chord, and then type script into its console. That would be JavaScript through an allowed method. The hole was reasoned, not measured; ph4 measures it. The guards:

- **Keys are a closed enum** of `Enter`, `Escape` and `Tab`. It has no modifier field and no F-keys, so neither a chord nor F12 can be expressed. The record's `keys_sent` is tallied at the socket, and the shapes refuse any other key.
- **Text is inserted only after a focus check:** `Input.insertText` is sent only when the accessibility tree confirms the focused node is a text box inside the agent view.
- **DevTools targets are counted, not sampled.** The driver calls `Target.setDiscoverTargets` as soon as it attaches and counts every `Target.targetCreated` event whose type or URL is DevTools. That count is `devtools_targets_opened`, and the shapes require it to be 0.
- **`--disable-dev-tools` is not adopted** (seat 2's proposal in quorum round 1). Whether Antigravity 2.8.1 honours it is unmeasured, so it would be a guard nobody has seen work. ph4 measures F1 and F12 on a scratch profile instead.

**Control path:** accessibility tree → node → `DOM.getBoxModel` → `Input.dispatchMouseEvent` at the box centre → `Input.insertText` for prompts.

**Isolation, asserted in Rust:**

- The app runs in **its own process group** (`setsid` through `libc`). `Drop` kills only that group, and the driver never signals any pid outside it.
- **Scratch everything.** The app gets a scratch `--user-data-dir`, and `HOME` and every `XDG_*` directory point into scratch too. If the app needs a sign-in on that profile, the run is `NotRun` until the operator signs in. The demos never read, copy or receive a credential.
- **The driver attaches only to the port its own child opened.** The app is started with `--remote-debugging-port=0`, and the driver reads the chosen port from `DevToolsActivePort` in the scratch profile. The listening socket's inode must belong to a pid in the child's group, checked through `/proc/<pid>/fd` and `/proc/net/tcp`.
- **Single-instance forwarding is refused.** An Electron app may hand a second launch to an instance that is already running. If the child exits early, or no port appears in the scratch profile, the run is `NotRun`, and the driver never looks for any other port.
- **The operator's own state is untouched.** The operator's app, gemini, and XDG config, cache and state directories are listed by path, size and mtime before and after the run, and the two listings must be equal. A canary file is planted beside them and must be unchanged afterwards. No operator path is ever passed to the app. `operator_profile_touched` in the record comes from that comparison.
- **A leak sweep at teardown.** After the group is killed, no process from it may remain, and no socket may still be listening on its port.
- **Workspaces are isolated.** Each agent's workspace `ws/agent-N/` is a fresh scratch directory. `agent_N_files` comes from a filesystem diff of that workspace, cross-checked against what the agent view reports the agent edited. A change anywhere else in the scratch tree counts in `stray_writes`.

**Steps:**

1. Preflight.
2. Launch.
3. Find the hub target and wait for the agent view.
4. Create three agents, each with its own fixture task (`fixture-alpha`, `fixture-beta`, `fixture-gamma`) that writes one file in its own workspace.
5. Capture three screenshots: before the fan-out, fanned out, and all done. The program asserts each is non-blank, and the shapes require three distinct digests.
6. Tear down the process group, then run the leak sweep.
7. Write the record, judge it, and write the Receipt.

### 5.4 D21 (after D20; reuses `agy-cdp`)

**Steps:**

1. Launch as in D20, with the same isolation.
2. Start three agents. Agents 1 and 2 hold a long step; agent-3's fixture task carries the planted failure.
3. A pure Rust check on agent-3's workspace output marks it red at `red_seen_ms`. At that moment the driver reads `running_at_red` from the app's accessibility tree, not from its own bookkeeping.
4. For each agent in `running_at_red`, the driver clicks that agent's own stop control (accessibility node → `DOM.getBoxModel` → `Input.dispatchMouseEvent`) at `stop_agent_N_sent_ms`. It then reads the stopped state back from the tree at `stop_agent_N_stopped_ms`.
5. Each stopped agent's workspace is snapshotted at its stopped moment: path, size and sha256 of every file.
6. Run the reducer over the three results in all six orders.
7. At teardown, snapshot the same workspaces again. `writes_after_stop` is the number of entries that differ between the two snapshots.
8. Write the record, judge it, and write the Receipt.

**Rust asserts beyond the shapes:**

- **Stop latency is bounded:** `red_seen > 0`, and `stopped − red_seen ≤ L` for each stopped agent. pv cannot compute a difference, so this is a Rust assert, and a take that breaks it fails. L stays **[U]** until it is measured by seat 1's procedure (§7, Q3):
  - at least 30 stop samples on a scratch profile;
  - `L = max(p99, 2 × p95)`, rounded up;
  - the samples committed beside the demo.

  The long step must last at least 3 × (red + L + UI delay), so that agents 1 and 2 are reliably still running at red. L is re-measured whenever the pinned asar changes.
- **No writes after the stop, judged by content, not by time.** The two workspace snapshots compare path, size and sha256, so no mtime granularity or slack enters the check.
- **The reducer is a pure function** of the multiset of results: no clock, no I/O, `BTreeMap` order. Its unit tests:
  - all six permutations, on several different result sets, give one digest per set;
  - **sensitivity:** changing one result changes the digest. A constant reducer, run as a negative control, must fail this test. That proves the test can fail; without it, a reducer that ignores its input would pass as order-free.
  - with several red results, the refusal names the lowest red id in every arrival order.
- **`refusal_names` is the lowest red agent id**, and `merged` is false whenever any result is red.
- **`app_evidence ⊆ cdp_methods`**, as in D20 (§4, item 5).

## 6. Phase plan

The plan has seven phases. A **lane** is one independent reviewer in the quorum; lanes review, they never build or run anything. Phases 3, 4 and 5 run in parallel on disjoint paths, each with its own `CARGO_TARGET_DIR`. A worker that needs a dependency or file outside its scope stops and returns an open question; it never adds one.

| Phase | Scope | Acceptance command (re-run by the orchestrator) |
|---|---|---|
| ph1 spec grill (3-lane quorum, review only) | this spec and the four `spec/` + `fixtures/` dirs | `A_1`: the spec proofs below, both exit 0 |
| ph2 harness | `demo-kit/**`, `xtask/**`, `Cargo.toml`, `Cargo.lock`, `.github/workflows/ci.yml`, plus manifests and skeleton bins for d18–d21 and `agy-cdp`. **All** shared-file edits land here. | `cargo test -p demo-kit -p xtask && cargo build --workspace && cargo run -q -p xtask -- verify` |
| ph3 D18 | `d18-two-agents-one-server/**` | `cargo test -p d18-two-agents-one-server && cargo run -q -p xtask -- verify --only d18-two-agents-one-server`, then live run L3 Green |
| ph4 agy-cdp + D20 | `agy-cdp/**`, `d20-agy-app-fanout/**` | the entry gate below first; then `cargo test -p agy-cdp -p d20-agy-app-fanout && cargo run -q -p xtask -- verify --only d20-agy-app-fanout`, then live run L4 Green |
| ph5 D19 | `d19-workflow-ontology/**` | `cargo test -p d19-workflow-ontology && cargo run -q -p xtask -- verify --only d19-workflow-ontology && cargo run -q -p d19-workflow-ontology` (Green: pv is the only tool) |
| ph6 D21 | `d21-agy-app-fanin/**` | `cargo test -p d21-agy-app-fanin && cargo run -q -p xtask -- verify --only d21-agy-app-fanin`, then live run L6 Green |
| ph7 pre-PR | the whole diff | 3-lane diff quorum, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo run -q -p xtask -- verify` |

**The live runs.** Each writes its Receipt under `$RFML5_RECEIPTS/<id>/<run_id>/`, and only a Green verdict passes. `NotRun` is never Green: a missing tool, a pin mismatch or a missing sign-in leaves the phase open, not done.

- **L3 (D18):** `RFML5_MODELS=<dir> RFML5_DECLARED_APR=<apr 0.70.1> RFML5_RECEIPTS=<dir> cargo run -q --release -p d18-two-agents-one-server`
- **L4 (D20):** `RFML5_RECEIPTS=<dir> cargo run -q --release -p d20-agy-app-fanout`
- **L6 (D21):** `RFML5_RECEIPTS=<dir> cargo run -q --release -p d21-agy-app-fanin`

D20 and D21 stay `NotRun` until the operator has signed in on the demos' own profile (E2).

**Ordering rules:**

- **D18's contract is frozen at the ph1 commit.** ph5 judges it, so a change to `d18-run-v1.yaml` during ph3 stops ph3, re-runs `A_1`, and goes back through the quorum.
- **Q2 (§7) is settled before ph2 starts,** because ph2 writes `agy-cdp`'s manifest.

**ph4 entry gate (E3).** Before any D20 code is written, on a scratch profile:

1. Dump the hub's accessibility tree.
2. Show that five controls are reachable through the tree plus input events: new agent, the task box, send, an agent's state, and stop.
3. Measure whether F1 and F12 open DevTools on 2.8.1, counting `Target.targetCreated` events. The key is pressed through the window system, on the scratch instance's own display, because `agy-cdp` cannot express either key.
4. Commit the role/name map as a fixture.

If any step fails, ph4 and ph6 stop and escalate.

**`A_1`, the spec proofs,** is two committed commands, run from the repository root with pv 0.70.1 on `PATH`:

```bash
bash course-5/demos/xtask/proofs/spec-proofs.sh course-5/demos               # exactly 17 checks, 0 failing
bash course-5/demos/xtask/proofs/spec-proofs.sh --self-test course-5/demos   # 8/8 sabotages, each refused by its own check
```

The 17 checks:

| Checks | Count |
|---|---|
| `pv validate` on each contract | 4 |
| the golden record is Green | 4 |
| the planted record exits 1, with sorted findings byte-equal to `.expect` | 4 |
| D19's record agrees with D18's `.expect` and with `mutants.json` | 4 |
| the mutant table (`verify-mutants.sh`): 16/16 killed and named, 5/5 survived | 1 |

- Every judge run is materialised in a fresh directory outside any git work tree (M4).
- Any other number of checks fails: a gate over a different set of checks is not this gate.
- `--self-test` proves the gate can fail. It plants eight sabotages, one per fresh copy, and each must be refused by the check it targets.
- ph2 ports both scripts into `xtask verify` and deletes them (§4, item 5).

## 7. Escalations and open questions

**Quorum round 1** reviewed the previous revision of this spec. It had three seats:

| Seat | Model | Verdict |
|---|---|---|
| 1 | claude-sonnet-5-5-medium | D18 and D19: implement with changes. D20: do not implement as written. D21: do not implement until the D20 changes land and L is measured. |
| 2 | gemini-3.1-pro-high | do not implement as written |
| 3 | gpt-oss-120b-medium | PASS, without engaging any of Q1–Q5. Counted as low-information, not as agreement. |

Most of the findings are addressed in this revision:

- a writer list where two equal values collapse into one, so a role bound twice looked bound once;
- a duplicate mutant;
- D21's reduce check, which one arrival order could pass;
- D21's stops, which allowed one stopped agent where both must stop;
- the uppercase-digest mutant, which also broke `lessThanOrEquals`, so no mutant killed `pattern` alone;
- live runs with no command;
- D19 being too dense for one act (E4).

**Added after round 1, so not yet reviewed (round 2 must cover them):**

- **D20:**
  - `devtools_targets_opened`;
  - `app_evidence`, with the `app_driven` equation;
  - `evidence_distinct`.
- **D21, for parity with D20:**
  - `cdp_protocol`, `operator_profile_touched` and `devtools_targets_opened`;
  - `app_evidence` and `keys_sent`, with the `app_driven` equation, `D21-INV-006` and `FALSIFY-D21-006`.
- **The committed `A_1`:**
  - the two proof scripts;
  - the self-test with its eight sabotages.

**Escalations:**

- **E1: `apr` 0.70.1 is not the course machine's declared `apr` (0.69.3).** D18 pins `=0.70.1` and refuses anything else with `NotRun(VersionMismatch)`; the demo never falls back. There are two ways to fix it, and choosing is the owner's call:
  - seat 1: point `$RFML5_DECLARED_APR` at a pinned 0.70.1 build and record that build's sha256;
  - seat 2: upgrade the course machine's `apr`.
- **E2: Antigravity quota and auth.** Each D20/D21 take starts three agents and spends the app's agent quota, and quorum lanes draw on the same quota.
  - The demos run on their own profile, with `HOME` and `XDG_*` in scratch (§5.3), and never read a credential store.
  - If a take needs a sign-in, that sign-in is the operator's, and the run is `NotRun` until it is done.
  - Whether one sign-in persists across takes, on a dedicated demo profile that is reused, is unmeasured; ph4 measures it.
  - Per seat 1, each take caps its agents and tasks at the three fixtures and carries a take budget. Per-take cost is [U] until D20's first run.
  - **Rejected:** seat 2's "inject a dedicated service account token via env". Credentials are the operator's, and the demos never read or receive one.
- **E3: the agent view's accessibility roles and names are unmeasured.**
  - ph4 starts with its entry gate (§6): dump the tree, reach the five controls, measure F1 and F12, and commit the role/name map.
  - The same question covers file attribution: which agent edited which file must come from the agent view and agree with the filesystem diff.
  - If creating, stopping or attributing cannot be done through the accessibility tree plus input events, ph4 and ph6 **stop and escalate**.
  - Seat 2 proposes a fallback that walks the DOM (`DOM.getDocument`, `DOM.describeNode`, all on ALLOW). Seat 1 says escalate. A DOM-only driver would not send `Accessibility.getFullAXTree`, so it fails `app_driven` as the contracts stand, and adopting it is a contract change that goes back through the quorum.
  - Agents created from the CLI are never a fallback: D20 and D21 would no longer be fan-out *in the app*.
- **E4: D19 is too dense for one act.** Both seats that answered say so. A proposal for the outline owner, who decides:
  - split the demo into `--act shapes` and `--act mutants`;
  - run about five mutants live, including at least one survivor;
  - show the rest of the table on screen.

**Questions, with the round 1 answers:**

- **Q1: should `AppMismatch` be its own `NotRunReason`?** Both answering seats say yes, as `{pinned_sha, found_sha}`. Adopted in §4.
- **Q2: `tungstenite` + `base64`, or a hand-rolled WebSocket client?** Both answering seats chose `tungstenite` + `base64`. Adopted, and settled before ph2 (§6).
- **Q3: what is the stop-latency bound L, and how long must the step be?**
  - Seat 1: at least 30 samples; `L = max(p99, 2 × p95)`, rounded up; commit the samples; the assert fails the take; a long step of at least 3 × (red + L + UI delay); re-measure on an asar change.
  - Seat 2: `L = p99 + 500 ms`.
  - §5.4 adopts seat 1's procedure, the stricter of the two, and round 2 may overturn it. L itself stays [U] until measured.
- **Q4: should D19 also judge the latest live D18 record when one exists?** **The seats disagree; this is open, and the owner's call.**
  - Seat 1: yes, as an optional live arm, `NotRun` when no live record exists, and never Green because one is absent.
  - Seat 2: no, because it couples ph5 to ph3 and to the GPU.
  - Until decided, D19 judges the golden fixture only and needs no GPU.
- **Q5: is the DevTools key hole real on 2.8.1, and are the guards sufficient?** Both answering seats say to treat it as real.
  - Seat 1: a key enum with no F-keys, plus F12 measured. Adopted in §5.3, with DevTools targets counted from events.
  - Seat 2: `--disable-dev-tools`. Not adopted: whether 2.8.1 honours the flag is unmeasured.

## 8. Out of scope

- Moving other tooling from Jena to pv (tracked separately).
- Recording, narration and the course outline itself.
- Publishing to any course platform.
- Changing the CI runner image for this repository.
