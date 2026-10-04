# rfml5 new demos D18–D21: two agents on one server, the workflow as an ontology, and agent fan-out in the Antigravity app

**Ticket:** PMAT-020 (#20), epic #19. **Branch:** `PMAT-020-rfml5-new-demos`.
**Status:** spec. All four pv contracts validate, and the fixtures below pass or fail exactly as stated. None of the four demo binaries exists yet.

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
| D19 `d19-workflow-ontology` | rfml5/3.4 [U] | D18's workflow, written as a pv ontology plus SHACL shapes. pv rejects 14 named mutants of a good run and stays silent on the good run itself. |
| D20 `d20-agy-app-fanout` | rfml5/4.3 [U] | A Rust driver opens the Antigravity desktop app on a scratch profile and fans out three agents you can watch. Every CDP method it sends is from a fixed list of twenty, none of which runs JavaScript. |
| D21 `d21-agy-app-fanin` | rfml5/4.4 [U] | Fan-in with stop-the-line. One agent goes red, the driver stops the other two after the red and never before, refuses to merge, and the reduction is the same in all six arrival orders. |

### D18: two agents, one server

- **Agents.** Two roles, each an LLM call with its own system prompt and its own output file, sharing one `apr serve` 0.70.1 process.
  - The **writer** answers four fixed questions `q1..q4` into `out/answers.json`.
  - The **checker** reads each answer and records `accept` or `reject` into `out/verdicts.json`.
- **Schedules.** Two are run against the same server process:
  - *sequential*: the writer finishes all four, then the checker runs.
  - *pipelined*: the checker works on item *i* while the writer produces *i+1*.
- **What it claims** (equations in `spec/d18-run-v1.yaml`):
  - `one_load`: one server spawn, with the same pid at the start and the end.
  - `disjoint_writes`: each output file has exactly one writer role.
  - `causality`: per item, the writer ends before the checker starts.
  - `schedule_independence`: the two schedules produce byte-identical output digests.
  - `checker_decides`: only the checker decides.
- **What it does not claim: speed.** Measured (M1): 0.70.1 queues concurrent requests, and pipelining gained about 5 %. A speed claim would be false on this server, so the demo does not make one. The point is roles and correctness on one resident model, not throughput.

### D19: the workflow as an ontology

- **Subject.** D19's subject is D18's contract (`d18-run-v1`) and D18's golden run record.
- **Run.** D19 does the following:
  1. validates the contract;
  2. judges the golden record (Pass, with the positive control fired);
  3. judges the planted record (exactly the expected six findings);
  4. applies 14 mutants, written as RFC 6902 JSON Patch in `fixtures/mutants.json`, and requires each to be rejected and *named* (the right focus node, shape and SHACL component);
  5. shows the A-box is tamper-evident through `pv extract spec --check`;
  6. shows the engine passes its own W3C conformance cases (19/19).
- **What it claims** (`spec/d19-run-v1.yaml`):
  - `every_mutant_killed`: all 14 mutants are rejected.
  - `golden_silent`: the golden record raises nothing.
  - `abox_tamper_evident`: an edited record is detected.
  - `engine_conformant`: the engine passes its W3C cases.
- **The record is self-judged.** D19's own run record is judged by D19's own shapes, so the demo is checked by the same tool it teaches.

### D20: fan-out you can watch

- **Launch.** A Rust CDP driver launches Antigravity 2.8.1 on a scratch user-data and extensions directory under a virtual display. It then:
  1. creates three agents in the app's agent view, each on its own scratch workspace;
  2. waits until each reports done;
  3. captures one screenshot per agent.
- **What it claims** (`spec/d20-run-v1.yaml`):
  - `fan_out`: three agents, each reaching done.
  - `disjoint_workspaces`: each file has one agent.
  - `no_js`: every method sent is in ALLOW, and `js_methods_sent = 0`.
  - `operator_untouched`: the operator's own app profile is never opened or modified.

### D21: fan-in and stop-the-line

- **Run.** Same driver and same app, three agents.
  - Agent-3's task carries a planted check failure, so the stop path runs on every take.
  - Agents 1 and 2 hold a long step. When agent-3 is seen red, the driver stops both through the app's own stop control.
  - The reducer then refuses to merge, naming agent-3. It runs over all six arrival orders of the three results, and the digests must be equal.
- **What it claims** (`spec/d21-run-v1.yaml`):
  - `stop_the_line`: red_seen ≤ stop_sent ≤ stopped for every agent still running.
  - `refuse_on_red`: `merged = false`, and `refusal_names = agent-3`.
  - `order_free`: one digest across the six orders.
  - `no_js`: the same twenty methods; the stops send no JavaScript either.

## 2. Measured facts this spec rests on

Each fact was measured on the build named. Anything not in this list is **[U]** until measured.

- **M1: `apr serve` 0.70.1.**
  - Two concurrent clients on one resident server are queued: the pipelined schedule was about 5 % faster than sequential.
  - Outputs were byte-identical across the two schedules.
  - Model: Qwen3.5-4B-Q4_K_M, sha256 `00fe7986…a4`. The full digest is in D18's golden fixture and manifest.
  - The course machine's declared `apr` is **0.69.3**, not 0.70.1. This is escalation E1.
- **M2: pv 0.70.1, `pv lint <spec-dir> --gate shapes --format json`.**
  - The entity is declared as `entity: {type: json, ref: receipt.json}` together with a `vocabulary` (`prefix`, `root_class`, `nested: {key: Class}`); nested-in-nested works.
  - `ref` resolves against the **parent** of the contract directory, so a run is laid out as `<run>/spec/*.yaml` beside `<run>/receipt.json`.
  - **Components that work:**
    - `in` (int, bool, string), `datatype`, `pattern` (including nested alternation groups);
    - `minCount`/`maxCount` (including `maxCount: 0`);
    - `lessThanOrEquals` on strings and integers; applied in both directions it gives equality;
    - `closed` with `ignoredProperties: [rdf:type]`.
  - **Refused by pv** (so never used here): `equals`, `disjoint`, `and/or/not/xone`, `sparql`, `minInclusive`, complex property paths.
  - **Exit codes:** 0 Pass, 1 reject, 2 decline, 3 malformed.
  - **JSON fields used here:** `verdict`, `violations` (a count), `findings[].message`, `pc_shape`, `plant_violations`, `focus_nodes_n`, `shapes_n`, `armed_shapes`, `not_armed_shapes`, `unarmed_violations`, `w3c_cases_passed`, `w3c_cases_n`.
  - **Finding message shape:** ``<focus> violates shape `<id>` (<component>): …``
- **M3: literal collapse.** Repeated equal scalar values become one RDF literal. So `maxCount: 1` on a repeated property proves that every value is equal. D21's `order_free` rests on this: six equal digests pass, and one differing digest fails `maxCount`.
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
- **M7: D19's mutant matrix.**
  - The identity patch passes: focus 7, plant 17, shapes 3, W3C 19/19.
  - All 14 mutants exit 1, and each is named by the expected focus, shape and component.
  - m03 and m06 raise two findings each, both measured:
    - m03 sets `server_spawns` to the string `"1"`. That fails `datatype`, and it also fails `in [1]`, because a string is not the integer 1.
    - m06 sets `digest_sequential` to 64 `F`s. That fails `pattern`, and it also breaks the two-way `lessThanOrEquals` with `digest_pipelined`.
  - Result: **killed and named, 14/14.**

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
| `spec/dNN-run-v1.yaml` (contract: entity, vocabulary, shapes, equations, invariants, falsifiers) | 3 shapes | 2 shapes | 5 shapes | 3 shapes |
| `fixtures/receipt.golden.json` passes: `pv lint` exit 0, Pass, `pc_shape` fired, `not_armed_shapes` empty, `unarmed_violations` 0, W3C 19/19 | focus 7 | focus 15 | focus 16 | focus 10 |
| `fixtures/receipt.planted.json` is rejected with exit 1 | 6 findings | 5 findings | 6 findings | 6 findings |
| `fixtures/receipt.planted.expect`: the sorted finding messages, which must match byte for byte | 6 lines | 5 lines | 6 lines | 6 lines |
| `fixtures/mutants.json` (RFC 6902) | — | 14 mutants | — | — |

The planted records each break several claims at once, so one run shows several distinct findings:

- **D18:** the checker is not the decider, two server spawns, a pid that changed, causality reversed on one item, a file with two writers, and a digest mismatch.
- **D20:**
  - `js_methods_sent` is non-zero;
  - the operator profile was touched;
  - one agent never reached done;
  - one file has two agents;
  - `Runtime.evaluate` and `Page.addScriptToEvaluateOnNewDocument` were sent.
- **D21:**
  - the run merged anyway, with the wrong refusal name;
  - the reduce digests differ (`maxCount`);
  - a stop was sent before the red;
  - a malformed agent id;
  - `Runtime.callFunctionOn` was sent.

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
   - A changed asar under the same version string is a new reason, `AppMismatch { pinned, found }`, which the grill should confirm. Auto-update makes this the likely failure.
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
   - `xtask verify --only <id>` is added so each build phase has its own acceptance command.
6. **CI installs pv.** `aprender-contracts-cli` 0.70.1 is published on crates.io (MIT).
   - The `course-5-demos` job gains `cargo install aprender-contracts-cli --version 0.70.1 --locked`, cached the same way as `bashrs`.
   - The job's existing rule holds: *a missing tool fails, never skips.*
   - The judge's run root in CI is the runner's temp dir, which is outside the checkout.
7. **Run records go under `$RFML5_RECEIPTS/<id>/<run_id>/`**, so no host path is written into the repo. `$RFML5_MODELS` holds the model and `$RFML5_DECLARED_APR` names the `apr` binary.

## 5. Per-demo build

The shapes check *structure and relations*. pv has no arithmetic and no access to the world outside the record. Each demo therefore also asserts in Rust what a shape cannot see, and the `Provable contract:` docstring names those assertions. Every bin follows the repository's existing lint: a `Provable contract:` docstring, a final `assert!`, and then `contract: … OK`.

### 5.1 D18 (`apr`, `pv`, GPU lock)

**Steps:**

1. Preflight: `apr` =0.70.1, model sha, GPU lock.
2. `ServeGuard` spawns one `apr serve`.
3. Run the sequential schedule, then the pipelined one, on the same process. Use temperature 0 and a fixed seed.
4. Recompute both digests from the bytes on disk.
5. Write the run record, judge it, and write the Receipt.

**Rust asserts beyond the shapes:**

- **Role↔file binding by construction.** Each role owns the only writer handle to its file; the other role gets a read-only handle. The record's `files[].writers` comes from those handles, not from what an agent reports.
- **Digests are recomputed** over the actual output bytes after the run. A digest the agents reported is never used.
- **Server pid and start time** come from `ServeGuard` at spawn and at the end. The `/proc/<pid>/stat` start time must also be equal, which rules out a respawn that reused the same pid number.
- **Causality timestamps** come from the orchestrator's monotonic clock, not from the agents.

**CI:** the full flow runs against the existing `fake-apr-serve` bin, so `cargo test -p d18-two-agents-one-server` exercises roles, causality, digests and the shapes judge without a GPU. The live run is the recording take.

### 5.2 D19 (`pv` only)

**Steps:**

1. `pv validate` on D18's contract.
2. Judge the golden record (Green), then the planted record (Red, with output equal to `.expect`).
3. Apply each mutant in `fixtures/mutants.json` with an RFC 6902 subset of `add`, `remove` and `replace` over `serde_json` pointers. Judge each mutant.
4. Run `pv extract spec`, then `--check` (0), then a one-byte record edit, then `--check` again (1).
5. Write D19's own record, judge it with D19's shapes, and write the Receipt.

**Rust asserts beyond the shapes:**

- **Every patch is non-vacuous:** the patched JSON is not equal to the golden JSON.
- **An identity arm** (an empty patch) passes, so a judge that rejects everything cannot pass.
- **Every mutant exits 1** with a message that begins with exactly ``ont:d18/d18-run-v1<focus> violates shape `<target>` (<component>)``.
- **The planted `.expect` is byte-equal.**
- **The positive control** fired on every judge call.

### 5.3 D20 (`antigravity`, `pv`): the new `agy-cdp` crate first

**`agy-cdp`** is a small CDP client written for this course:

- Discovery is HTTP `GET /json/version` and `/json/list` over `std::net::TcpStream`, the same style as demo-kit's `http_request`.
- The WebSocket is `tungstenite` with `default-features = false, features = ["handshake"]`, over plain `ws://127.0.0.1`.
- Screenshots are decoded with `base64`.
- These two dependencies are the grill's decision. The alternative is a hand-rolled RFC 6455 client, which also needs SHA-1.

**The method surface is a closed Rust enum** of exactly the ALLOW twenty, each method with typed parameters:

- There is no `send(method: &str, …)`, and the crate contains no `Runtime` type. An `xtask` lint refuses the strings `Runtime.`, `Debugger.`, `addScriptToEvaluate` and `Page.navigate` in the crate's sources.
- Every frame passes through one write function. That function counts methods by name, and **that socket-level tally** becomes `cdp_methods` in the run record.

**Key events need care.** The allow-list includes `Input.dispatchKeyEvent` and `Input.insertText`. Typing alone could open the app's developer tools with a modifier chord and then type script into its console, which would be JavaScript through an allowed method. This was reasoned, not measured. The guards:

- the key-event type cannot express Ctrl, Alt or Meta modifiers;
- text is inserted only into a node whose accessibility role is a text box inside the agent view;
- `Target.getTargets` is sampled during the run, and any `devtools://` target makes the run Red.

**Control path:** accessibility tree → node → `DOM.getBoxModel` → `Input.dispatchMouseEvent` at the box centre → `Input.insertText` for prompts.

**Isolation, asserted in Rust:**

- The app runs in **its own process group** (`setsid` through `libc`). `Drop` kills only that group, and the driver never signals any pid outside it.
- The driver **attaches only to the port its own child opened.** The listening socket's inode must belong to a pid in the child's group, checked through `/proc/<pid>/fd` and `/proc/net/tcp`.
- **The operator's own app profile is untouched.** Its default data and extensions directories are listed by path, size and mtime before and after the run, and the two lists must be equal. No operator path is ever passed to the app. `operator_profile_touched` in the record comes from that comparison.
- **Workspaces are isolated:** each agent's workspace is a fresh scratch directory, and the record's `files[].agents` comes from where each file actually is.

**Steps:**

1. Preflight.
2. Launch.
3. Find the hub target and wait for the agent view.
4. Create three agents, each with a short task that writes one file.
5. Wait until all three are done, then capture three screenshots.
6. Tear down the process group.
7. Write the record, judge it, and write the Receipt.

### 5.4 D21 (after D20; reuses `agy-cdp`)

**Steps:**

1. Launch as in D20.
2. Start three agents. Agents 1 and 2 hold a long step; agent-3's task carries the planted failure.
3. A pure Rust check on agent-3's workspace output marks it red at `red_seen`.
4. The driver clicks each running agent's stop control at `stop_sent`, then observes the stopped state at `stopped`.
5. Run the reducer over the three results in all six orders.
6. Write the record, judge it, and write the Receipt.

**Rust asserts beyond the shapes:**

- **Stop latency:** `stopped − red_seen ≤ L` for each stopped agent. L is **[U]** until measured. pv cannot compute a difference, so this is a Rust assert.
- **No writes after the stop:** no file in agents 1 and 2's workspaces has an mtime after `stopped` plus the filesystem's timestamp granularity.
- **The reducer is a pure function** of the multiset of results: no clock, no I/O, `BTreeMap` order. It is unit-tested on all six permutations, besides being shape-checked through M3.
- **`refusal_names` is the lowest red agent id**, and `merged` is false whenever any result is red.

## 6. Phase plan

The plan has seven phases. A **lane** is one independent reviewer in the quorum; lanes review, they never build or run anything. Phases 3, 4 and 5 run in parallel on disjoint paths, each with its own `CARGO_TARGET_DIR`. A worker that needs a dependency or file outside its scope stops and returns an open question; it never adds one.

| Phase | Scope | Acceptance command (re-run by the orchestrator) |
|---|---|---|
| ph1 spec grill (3-lane quorum, review only) | this spec and the four `spec/` + `fixtures/` dirs | `A_1`: the spec proofs below, all exit 0 |
| ph2 harness | `demo-kit/**`, `xtask/**`, `Cargo.toml`, `Cargo.lock`, `.github/workflows/ci.yml`, plus manifests and skeleton bins for d18–d21 and `agy-cdp`. **All** shared-file edits land here. | `cargo test -p demo-kit -p xtask && cargo build --workspace && cargo run -q -p xtask -- shapes` |
| ph3 D18 | `d18-two-agents-one-server/**` | `cargo test -p d18-two-agents-one-server && cargo run -q -p xtask -- verify --only d18-two-agents-one-server`, then the live run Green |
| ph4 agy-cdp + D20 | `agy-cdp/**`, `d20-agy-app-fanout/**` | `cargo test -p agy-cdp -p d20-agy-app-fanout && cargo run -q -p xtask -- verify --only d20-agy-app-fanout`, then the live run Green |
| ph5 D19 | `d19-workflow-ontology/**` | `cargo test -p d19-workflow-ontology && cargo run -q -p xtask -- verify --only d19-workflow-ontology && cargo run -q -p d19-workflow-ontology` (Green: pv is the only tool) |
| ph6 D21 | `d21-agy-app-fanin/**` | as ph4, for `d21-agy-app-fanin` |
| ph7 pre-PR | the whole diff | 3-lane diff quorum, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo run -q -p xtask -- verify` |

`A_1`, the spec proofs, is run from `course-5/demos`. `$RUN` is a fresh temp dir outside any git tree.

1. `pv validate dNN/spec/dNN-run-v1.yaml` for each of the four contracts.
2. For each demo:
   - copy `spec/*.yaml` to `$RUN/spec/`;
   - copy the golden record to `$RUN/receipt.json`;
   - run `pv lint $RUN/spec --gate shapes --format json`. It must give exit 0, Pass, `pc_shape` fired, nothing unarmed, and W3C 19/19.
3. The same with the planted record. It must give exit 1, with the sorted `findings[].message` byte-equal to `receipt.planted.expect`.
4. D19's matrix: the identity arm passes, and all 14 patches are non-vacuous, exit 1, and carry the expected message prefix.

Until ph2 lands the `xtask shapes` arm, these steps run as a scratch script, recorded with the phase receipt.

## 7. Escalations and open questions (for the grill)

- **E1: `apr` 0.70.1 is not the course machine's declared `apr` (0.69.3).** D18 pins `=0.70.1` and refuses anything else with `NotRun(VersionMismatch)`. Either the course machine's `apr` is upgraded before recording, or the declared path points at a 0.70.1 build through `$RFML5_DECLARED_APR`. Which one is the owner's call. The demo never falls back.
- **E2: Antigravity quota and auth.** Each D20/D21 take starts three agents and spends the app's agent quota. Quorum lanes draw on the same quota.
  - The fresh-profile auth mechanism is unmeasured, and the demos never read a credential store.
  - If a take needs a sign-in, that sign-in is the operator's, and the run is `NotRun` until it is done.
  - Per-take cost is [U] until D20's first run.
- **E3: the agent view's accessibility roles and names are unmeasured.**
  - ph4 starts by dumping the hub's accessibility tree on a scratch profile and committing the role/name map as a fixture.
  - If creating or stopping an agent cannot be reached through the accessibility tree plus input events, ph4 and ph6 **stop and escalate**. Falling back to agents created from the CLI would change what D20 and D21 claim: they would no longer be fan-out *in the app*.
- **Q1:** Does `AppMismatch` deserve its own `NotRunReason`, or is `VersionMismatch` with the sha as `found` enough?
- **Q2:** `tungstenite` + `base64`, or a hand-rolled WebSocket client?
- **Q3:** For D21, what is the stop-latency bound L, and how long a step makes "agents 1 and 2 still running at red" reliable on every take?
- **Q4:** Should D19 also judge the latest *live* D18 record when one exists, not only the golden fixture? Today it judges the fixture only, so D19 needs no GPU.
- **Q5:** Is the reasoned devtools-chord hole (§5.3) real on 2.8.1, and are the three guards sufficient?

## 8. Out of scope

- Moving other tooling from Jena to pv (tracked separately).
- Recording, narration and the course outline itself.
- Publishing to any course platform.
- Changing the CI runner image for this repository.
