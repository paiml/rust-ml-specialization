---
epic: 19
status: active
---

# rfml5 new demos D18–D21: two agents on one server, the workflow as an ontology, and agent fan-out in the Antigravity app

**Ticket:** PMAT-020 (#20), epic #19. **Branch:** `PMAT-020-rfml5-new-demos`.
**Status:** spec, revised after quorum rounds 1 to 4p (§7). All four pv contracts validate, and the fixtures below pass or fail exactly as stated: `A_1` (§6) passes 17/17 checks, and its `--self-test` refuses all 28 sabotages exactly as each row states — the exact set of failing checks, and the reason wherever the check alone does not say why. None of the four demo binaries exists yet.

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
| D19 `d19-workflow-ontology` | rfml5/3.4 [U] | D18's workflow, written as a pv ontology plus SHACL shapes. pv kills 18 named mutants of a good run, each for exactly the constraint it breaks, and stays silent on the good run itself. Six documented mutants survive: they are what the shapes cannot see, and each is owned by a Rust assert. |
| D20 `d20-agy-app-fanout` | rfml5/4.3 [U] | A Rust driver opens the Antigravity desktop app on its own dedicated demo profile and fans out three agents that run at the same time, watchable in the recording. Every CDP method it sends is from a fixed list of twenty, none of which runs JavaScript. It sends only Enter, Escape and Tab, and the record carries the evidence that the app itself was driven. |
| D21 `d21-agy-app-fanin` | rfml5/4.4 [U] | Fan-in with stop-the-line. One agent goes red, the driver stops the other two after the red and never before, refuses to merge, and the reduction is the same in all six arrival orders. |

### D18: two agents, one server

- **Agents.** Two roles, each an LLM call with its own system prompt and its own output file, sharing one `apr serve` process from the 0.70.x series (E1).
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
  4. runs the 23-row mutant table in `fixtures/mutants.json`. Each row is an RFC 6902 JSON Patch of the golden record:
     - 18 **kill** rows must each be rejected *purely*: exit 1, the exact finding count, every finding naming the intended focus node, shape and SHACL component, and the findings naming exactly the row's `properties`;
     - 6 **survive** rows must each pass. Each is a defect the shapes cannot see, and the row names the Rust assert that catches it;
  5. shows the A-box is tamper-evident through `pv extract spec --check`;
  6. shows the engine passes its own W3C conformance cases (19/19).
- **What it claims** (`spec/d19-run-v1.yaml`):
  - `every_mutant_killed`: all 18 kill rows are rejected purely.
  - `survivors_documented`: all 6 survive rows pass, so the list of what the shapes cannot see is current.
  - `components_covered`: the kills cover all seven components d18-run-v1 uses: `in`, `datatype`, `pattern`, `lessThanOrEquals`, `minCount`, `maxCount` and `closed`.
  - `golden_silent`: the golden record raises nothing.
  - `abox_tamper_evident`: an edited record is detected.
  - `engine_conformant`: the engine passes its W3C cases.
- **D19's own record is a summary, not a self-judgement.** Every field in it is copied from pv's JSON output, and `xtask verify` re-runs the whole table independently. Judging that record with D19's own shapes checks its form. The evidence is the re-run, which anyone can repeat.

### D20: fan-out, watchable in the recording

- **Launch.** A Rust CDP driver launches Antigravity 2.8.1 under a virtual display, so there is nothing to watch live: the recording is what a viewer watches. It uses the course's one dedicated demo profile (§5.3): its own user-data and extensions directories, with `HOME` and every `XDG_*` directory redirected into that profile. It then:
  1. creates three agents in the app's agent view. Each has its own fixture task (`fixture-alpha`, `fixture-beta`, `fixture-gamma`) and its own fresh workspace, `ws/agent-N/`;
  2. waits until each reports done;
  3. captures three screenshots: before the fan-out, fanned out, and all done. They must be distinct and non-blank.
- **What it claims** (`spec/d20-run-v1.yaml`). Each agent is a named property set (`agent_N_task`, `agent_N_state`, `agent_N_files`), so every claim about agent N constrains agent N's own properties:
  - `fan_out`: each agent ran its own fixture task and reached done.
  - `app_driven`: the record's `app_evidence` holds all five of `Accessibility.getFullAXTree`, `DOM.getBoxModel`, `Input.dispatchMouseEvent`, `Input.insertText` and `Target.setDiscoverTargets`, computed from the driver's tally of sent frames, and every listed method has a count of at least 1. A CLI fallback sends none of the five.
  - `disjoint_workspaces`: every file of agent N is under `ws/agent-N/`, and `stray_writes = 0`.
  - `no_js`: every method sent is in ALLOW, every key sent is Enter, Escape or Tab, and `devtools_targets_opened = 0`.
  - `concurrent`: the three agents really ran at the same time. One accessibility-tree snapshot shows all three running (`all_running_seen_ms`), and for each agent N, `agent_N_started_ms ≤ all_running_seen_ms ≤ agent_N_done_ms`, on one clock. A driver that ran the agents one after another has no such snapshot.
  - `operator_untouched`: the run's profile is not the operator's own, and a listing of the operator's profile, config and XDG directories is identical before and after the run.
  - `evidence_distinct`: the three screenshots have three distinct digests.

### D21: fan-in and stop-the-line

- **Run.** Same driver and same app, three agents.
  - Agent-3's task carries a planted check failure, so the stop path runs on every take.
  - Agents 1 and 2 hold a long step. When agent-3 is seen red, the driver reads which agents are still running from the accessibility tree at that moment (`running_at_red`). It stops each of them through the app's own stop control.
  - The reducer then refuses to merge, naming agent-3. It runs over all six arrival orders of the three results, and the digests must be equal.
  - The driver also captures screenshots, so the record lists `Page.captureScreenshot`. They are for the recording only: D21 makes no claim that they are distinct.
- **What it claims** (`spec/d21-run-v1.yaml`). Every timing claim is a relation between two named properties (`red_seen_ms`, `stop_agent_N_sent_ms`, `stop_agent_N_stopped_ms`):
  - `stop_the_line`: red_seen ≤ stop_sent ≤ stopped for agents 1 and 2; `stopped_agents = running_at_red = {agent-1, agent-2}`; and `writes_after_stop = 0`.
  - `refuse_on_red`: `merged = false`, and `refusal_names = {agent-3}`, the lowest red agent id.
  - `order_free`: all six orders ran, and they produced one digest.
  - `app_driven`, `no_js` and `operator_untouched`: as in D20, with the same evidence, the same twenty methods, the same three keys and the same DevTools count. The stops send no JavaScript either. `operator_untouched` is as strong as D20's: the same before/after listing equality, and the record also carries `stray_writes = 0`.

## 2. Measured facts this spec rests on

Each fact was measured on the build named. Anything not in this list is **[U]** until measured.

- **M1: `apr serve` 0.70.1.**
  - Two concurrent clients on one resident server are queued: the pipelined schedule was about 5 % faster than sequential.
  - Outputs were byte-identical across the two schedules.
  - Model: Qwen3.5-4B-Q4_K_M, sha256 `00fe7986…a4`. The full digest is in D18's golden fixture and manifest.
  - The course machine's declared `apr` was **0.69.3** when this was measured. E1 is now resolved: the course machine must run a 0.70.x `apr` (§7).
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
  - **Refused by pv** (so never used here): `equals`, `disjoint`, `and/or/not/xone`, `sparql`, `minInclusive`, `hasValue` (exit 3), complex property paths. D20's and D21's app evidence therefore uses `in` plus `minCount: 5` where `hasValue` would have been the natural choice.
  - **Exit codes:** 0 Pass, 1 reject, 2 decline, 3 malformed.
  - **JSON fields used here:** `verdict`, `violations` (a count), `findings[].message`, `pc_shape`, `plant_violations`, `focus_nodes_n`, `shapes_n`, `armed_shapes`, `not_armed_shapes`, `unarmed_violations`, `w3c_cases_passed`, `w3c_cases_n`.
  - **Finding message shape:** ``<focus> violates shape `<id>` (<component>): …``. A finding carries this message only, with no structured component field, so every check in this spec matches the message prefix.
  - **Measured in round 2**, each by a fixture or a mutant row that would fail if it stopped holding:
    - a `datatype` finding names the offending value but **no property** (see m04 and m05 in M7);
    - `lessThanOrEquals` between an integer and a string **fires** (it is not silently skipped), and between two strings it compares **lexically**, so every timing pair here is `xsd:integer` on both sides;
    - a JSON key spelled `rdf:type` is **not** covered by `ignoredProperties: [rdf:type]`: `closed` rejects it (mutant m17);
    - `pattern` works on integers. Its finding prints the value with **no datatype**, e.g. ``ont:d20/…cdp_methods.8 … count: "0" does not match /^[1-9][0-9]*$/``;
    - `LC_ALL=C sort` orders positional nodes as text, so `cdp_methods.10` sorts before `cdp_methods.8`. Every `.expect` is sorted with `LC_ALL=C`, and the gate sorts the same way.
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
  - It starts headless under `xvfb-run -a` with these flags: `--user-data-dir <profile>/user-data --extensions-dir <profile>/extensions --remote-debugging-port=<p> --no-sandbox --disable-gpu --new-window`.
  - Its agent hub is a page at `https://127.0.0.1:<dynamic port>/`.
  - On a fresh test profile, the bundled language server logged that auth succeeded. *How* it obtained auth is unmeasured. The demos do not depend on it: the operator signs in once on the dedicated demo profile (§5.3, E2), and that sign-in survived a cold restart of the app.
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
  - **18/18 kill rows are killed purely.** Each exits 1 with the exact finding count, the findings name exactly the row's `properties`, and every finding starts ``<node><focus> violates shape `<target>` (<component>)``. A row that also trips a second constraint, or trips the right one on the wrong node, is reported as impure and fails. And the sorted findings must equal the row's `messages` byte for byte: the right shape on a different value, or a changed message format, is reported as `WRONG-MESSAGE` and fails.
    - m04 and m06 raise two findings each and m05 four, by design and all of one component. Each patches every member of its pairs (both start ticks; all four of `q2`'s sequential times, writer and checker, start and end; both digests) to strings, so every `lessThanOrEquals` among them still holds as a string comparison and only `datatype` or `pattern` fires.
    - A `datatype` finding names no property, so m04's and m05's `properties` are `-`, one per finding: those rows are held to focus node, shape, component and count.
    - Every component is killed on the run node, and every component except `pattern` (which the item shape does not use) is also killed on an item node.
  - **6/6 survive rows pass** with exit 0 and no findings. They are what the shapes cannot see, each caught in Rust. A row whose `rust_assert` names nothing (empty, `none`, `null`, `-`, `n/a`, `tbd`, `todo`) is reported as `NO-RUST-ASSERT` and fails:
    - s01, both digests are the sha256 of the empty string: the program recomputes sha256 over the output files after each schedule.
    - s02, both pids are 41388, a plausible pid the shapes cannot tell apart from the real one (pid 0 or 1 would be refused by the pid pattern, so that mutant would be a kill, not a survivor): the pid is read from the child handle, and `/proc/<pid>/exe` must resolve to the pinned `apr`.
    - s03, one decision flipped: decisions are parsed from `out/verdicts.json`, which both digests cover.
    - s04, one item shifted 100 s later so that q2 follows q3: every timestamp comes from one monotonic clock at the sink, items increase within a schedule, and each is at most that schedule's elapsed time.
    - s05, `writer_files` lists one path twice: an equivalent mutant, since RDF values are a set. Each sink opens its file with `create_new`, so a second open of one path is an error.
    - s06, a pipelined schedule that never overlapped: each writer starts only after the previous item's checker ended, so it is a sequential run under the pipelined label, and every per-item chain is still ordered. It is caught by `pipelined_overlaps`, stated once in §5.1.
    - m17 is the kill row behind the round-2 finding that a JSON key spelled `rdf:type` might pass `ignoredProperties: [rdf:type]`: it does not. `closed` rejects it on the run node, naming `rdf:type`.
    - m18 is the kill row behind FALSIFY-D18-005: a writer that also wrote `out/verdicts.json` is refused by `sh:in` on `writer_files`, naming that value.
  - Result: **killed and named 18/18, survived 6/6.**
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
| `fixtures/receipt.golden.json` passes: `pv lint` exit 0, Pass, `pc_shape` fired, `not_armed_shapes` empty, `unarmed_violations` 0, W3C 19/19 | focus 5 | focus 1 | focus 10 | focus 10 |
| the golden run's `plant_violations` (pv's own positive control); `focus_nodes_n`, `shapes_n` and this count are asserted by `A_1` | 25 | 17 | 28 | 25 |
| `fixtures/receipt.planted.json` is rejected with exit 1 | 5 findings | 5 findings | 10 findings | 14 findings |
| `fixtures/receipt.planted.expect`: the sorted finding messages (`LC_ALL=C`), which must match byte for byte | 5 lines | 5 lines | 10 lines | 14 lines |
| `fixtures/mutants.json` (RFC 6902) | — | 24 rows: 18 kill, 6 survive | — | — |

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
  - the app evidence lacks `Input.insertText` (`minCount: 5`), and two of the three screenshots are identical (`minCount: 3`);
  - `Page.captureScreenshot` is listed with a count of 0 (`pattern` on that method node's `count`);
  - `Runtime.evaluate` and `Page.addScriptToEvaluateOnNewDocument` were sent, each named by its own `cdp_methods` node (`pattern`).
  - agent 3 started after the snapshot that claims all three were running (`lessThanOrEquals` on `agent_3_started_ms`).
- **D21:**
  - agent-1's and agent-2's stops were both sent before the red was seen, and each agent's stopped state was read before its stop was sent (`lessThanOrEquals`, four times, each naming the earlier property);
  - agent-3, which had already finished, is among the stopped agents (`in`);
  - the run merged anyway, and the refusal names agent-2 (`in`, twice);
  - one order's digest differs (`maxCount`), and one order ran twice, so only five distinct orders ran (`minCount`);
  - F12 was sent (`in`), the app evidence lacks `Input.insertText` (`minCount: 5`), and `Page.captureScreenshot` is listed with a count of 0 (`pattern`);
  - `Runtime.callFunctionOn` was sent, named by its method node (`pattern`).
  - the driver wrote one file outside every agent workspace (`in` on `stray_writes`).

## 4. Harness work (shared, lands first)

The run record and the harness `Receipt` are two separate files.

- **The run record** is what the shapes judge. It is the pv entity, the demo writes it, and it contains only what the demo measured.
- **The `Receipt`** is the harness's own envelope: pins, verdict, durations. It references the run record by sha256.

The changes:

1. **Exact pins for the new tools.**
   - `DemoManifest` gains `pv` and `antigravity` (both default `"none"`) and `antigravity_asar_sha256`.
   - `verify_demo` applies `pin::parse_exact` to each tool a demo uses, as it already does for `apr` and `agy`; floors are refused.
   - D18 pins `apr` to the **series** 0.70.x (E1) and `pv = "=0.70.1"` exactly.
   - Measured: `pin::parse_exact` accepts only `"=X.Y.Z"` today. This phase adds one series form, `apr = "0.70.*"` → `SeriesPin { major: 0, minor: 70 }`. It accepts any `0.70.<n>` and refuses 0.69.x, 0.71.x, 0.701.x and any suffix such as `-dirty`. A series is not a floor: the upper bound is the minor version. The Receipt records `apr`'s full version string and the sha256 of the binary that ran, so a patch change is visible even though it is admitted. pv stays exact, because the planted `.expect` bytes depend on pv's message format.
   - D19 pins `pv = "=0.70.1"`. D20 and D21 pin `antigravity = "=2.8.1"`, the asar sha and `pv = "=0.70.1"`. `agy-cdp`'s `e3-probe` (`E_4`) also pins `xdotool = "=3.20160805.1"`, checked against `xdotool version`. Every bin that starts a display (`e3-probe`, `measure-stop-latency`, D20, D21) also requires `Xvfb`, which preflight finds on `PATH` and whose `Xvfb -version` string it records. Xvfb is recorded but not pinned: it only supplies a blank framebuffer, and no contract claim depends on its version. A missing `Xvfb` is `NotRun`. Until ph2 deletes the two proof scripts, they need `jq`; `A_1` runs `jq --version` first and records the string, and a missing `jq` fails `A_1` outright. jq is not pinned, for the same reason as Xvfb: the scripts use only filters stable since jq 1.5, and the self-test re-measures every one of them.
2. **Preflight** checks the installed `pv --version`, Antigravity's version and the asar sha256 against the pins.
   - A mismatch is `NotRun(VersionMismatch {…})`.
   - A changed asar under the same version string is a new reason, `AppMismatch { pinned_sha, found_sha }`; both seats that answered Q1 agree (§7). Auto-update makes this the likely failure.
   - Missing `pv` is `NotRun(MissingTool)`.
3. **New `shapes` module in demo-kit:** `judge(spec_dir, record_json, run_root) -> ShapesOutcome`. It:
   - materialises `<run_root>/judge-<n>/{spec/*.yaml, receipt.json}` in a fresh directory per call: it is created with `create_dir`, which fails if the name exists, and retried with the next `n`, so parallel phases and repeated calls never share one;
   - **refuses a `run_root` inside any git work tree** (it walks up looking for `.git`), because of M4;
   - runs `pv lint <run_root>/judge-<n>/spec --gate shapes --format json`.

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
   - D20 and D21 additionally check `app_evidence = EVIDENCE ∩ {m.method : m ∈ cdp_methods}`. With the shapes' `minCount: 5` this means each of the five evidence methods was sent at least once. pv cannot relate one property's values to another node's, so this cross-check is Rust's (M8).
   - `xtask verify --only <id>` is added so each build phase has its own acceptance command.
   - The arm **ports** the two committed proof scripts, `xtask/proofs/spec-proofs.sh` and `xtask/proofs/verify-mutants.sh`, into Rust. Each of the scripts' twenty-eight `--self-test` sabotages becomes a test that must see exactly its named set of checks refuse, and, for the four mutant-table rows, its named reason. Once the port is green, ph2 deletes both scripts, so no gate is kept in two places.
6. **CI installs pv.** `aprender-contracts-cli` 0.70.1 is published on crates.io (MIT).
   - The `course-5-demos` job gains `cargo install aprender-contracts-cli --version 0.70.1 --locked`, cached the same way as `bashrs`.
   - The job's existing rule holds: *a missing tool fails, never skips.*
   - The judge's run root in CI is the runner's temp dir, which is outside the checkout.
7. **Run records go under `$RFML5_RECEIPTS/<id>/<run_id>/`**, so no host path is written into the repo. `$RFML5_MODELS` holds the model and `$RFML5_DECLARED_APR` names the `apr` binary. For D20 and D21, `$RFML5_AGY_BIN` names the Antigravity launcher and `$RFML5_AGY_PROFILE` the dedicated demo profile (§5.3). Preflight refuses a run with either unset as `NotRun`; neither has a default, so no host path is ever compiled in or guessed.
8. **An `xtask` lint confines the WebSocket dependency:** `tungstenite` may appear only in `agy-cdp`'s manifest and sources. Every other crate reaches the app through `agy-cdp`'s closed method enum. This lands in ph2 with the other lints.

## 5. Per-demo build

The shapes check *structure and relations*. pv has no arithmetic and no access to the world outside the record. Each demo therefore also asserts in Rust what a shape cannot see, and the `Provable contract:` docstring names those assertions. Every bin follows the repository's existing lint: a `Provable contract:` docstring, a final `assert!`, and then `contract: … OK`.

### 5.1 D18 (`apr`, `pv`, GPU lock)

**Steps:**

1. Preflight: `apr` in the 0.70 series (`SeriesPin`, with its version string and binary sha256 recorded), model sha, GPU lock.
2. `ServeGuard` spawns one `apr serve`.
3. Run the sequential schedule, then the pipelined one, on the same process. Use temperature 0 and a fixed seed.
4. Recompute both digests from the bytes on disk.
5. Write the run record, judge it, and write the Receipt.

**Rust asserts beyond the shapes.** Each of the first five catches one of D19's survive rows (§5.2), so every defect the shapes cannot see has a named owner:

- **Role↔file binding by construction (s05).** Each role writes through its own sink, and the sink registry records every path a sink opened. `writer_files` and `checker_files` come from that registry, never from what an agent reports. Every open is `create_new`, so opening one path twice is an error, not a second entry.
- **Digests are recomputed (s01):** sha256 over `out/answers.json ‖ out/verdicts.json` after each schedule, and each must equal the recorded digest. A digest an agent reported is never used.
- **Server identity (s02).** `ServeGuard` reads the pid from the spawned child handle, at spawn and at the end, and `/proc/<pid>/exe` must resolve to the binary preflight pinned (in CI, D18's `d18-fake-apr-serve` bin). Field 22 of `/proc/<pid>/stat`, the start ticks, is read at both moments, which rules out a respawn that reused the pid number.
- **Decisions come from the checker's file (s03).** Each item's `decision` is parsed from `out/verdicts.json`, the one file only the checker's sink wrote, and both digests cover that file.
- **One clock (s04).** Every timestamp comes from the orchestrator's monotonic clock at the sink, never from an agent. Within a schedule, items are increasing in i, and each is at most that schedule's elapsed time.
- **`pipelined_overlaps`: the pipelined schedule really overlaps, and the sequential one never does.** This is the one statement of it; §2 M7 (s06) refers here. For at least one i ∈ {1, 2, 3}, the checker's request on q(i) and the writer's request on q(i+1) are both in flight at once: `checker_start(q i) < writer_end(q i+1)` and `writer_start(q i+1) < checker_end(q i)`, on the same clock. In the sequential schedule no two requests overlap. Without this assert, a "pipelined" run that was secretly sequential would make `schedule_independence` vacuous.

**CI:** the full flow runs against the existing fake server. Cargo sets `CARGO_BIN_EXE_<name>` only for bins of the package under test, so ph2 moves the fake's body into a `demo_kit::fake_serve::run()` library function, and each demo that tests against it, D18 first, carries its own two-line `src/bin/fake_apr_serve.rs` calling it, declared as `[[bin]] name = "d18-fake-apr-serve"`. The name is explicit because Cargo would otherwise take the file stem, and distinct from demo-kit's `fake-apr-serve` because two bins of one name in a workspace collide in the shared output directory. `env!("CARGO_BIN_EXE_d18-fake-apr-serve")` then resolves inside that demo's own package and target directory, so `cargo test -p d18-two-agents-one-server` exercises roles, causality, digests and the shapes judge without a GPU. The live run is the recording take.

### 5.2 D19 (`pv` only)

**Steps:**

1. `pv validate` on D18's contract.
2. Judge the golden record (Green), then the planted record (Red, with output equal to `.expect`).
3. Apply each row of `fixtures/mutants.json` with an RFC 6902 subset of `add`, `remove` and `replace` over `serde_json` pointers, and judge it:
   - **each of the 18 kill rows must be killed purely:** exit 1, exactly the row's stated finding count, the findings naming exactly the row's `properties`, and every message beginning with exactly ``ont:d18/d18-run-v1<node> violates shape `<target>` (<component>)``, the row's own node, shape and component;
   - **each of the 6 survive rows must pass:** exit 0, Pass, no findings. A survivor that starts failing means the list of what the shapes cannot see is stale.
4. Copy D18's `spec/` and the record into D19's own run directory, outside any git work tree, and run `pv extract spec` there, then `--check` (0), then a one-byte record edit, then `--check` again (1). D18's tree is never written, so ph5 and ph3 share no output path.
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
- Confined file access uses `rustix` (`openat` with `O_NOFOLLOW`), the third dependency, added in quorum round 4f for `open_under` (below).
- Both seats that answered Q2 in quorum round 1 chose these two dependencies over a hand-rolled RFC 6455 client, which would also need SHA-1 (§7).

**The method surface is a closed Rust enum** of exactly the ALLOW twenty, each method with typed parameters:

- There is no `send(method: &str, …)`, and the crate contains no `Runtime` type. An `xtask` lint refuses the strings `Runtime.`, `Debugger.`, `addScriptToEvaluate` and `Page.navigate` in the crate's sources.
- Every frame passes through one write function. That function counts methods by name, and **that socket-level tally** becomes `cdp_methods` in the run record.

**Key events need care.** The allow-list includes `Input.dispatchKeyEvent` and `Input.insertText`. Typing alone could open the app's developer tools, with F12 or a modifier chord, and then type script into its console. That would be JavaScript through an allowed method. The hole was reasoned, not measured; ph4 measures it. The guards:

- **Keys are a closed enum** of `Enter`, `Escape` and `Tab`. It has no modifier field and no F-keys, so neither a chord nor F12 can be expressed. The record's `keys_sent` is tallied at the socket, and the shapes refuse any other key.
- **Text is inserted only after a focus check:** `Input.insertText` is sent only when the accessibility tree confirms the focused node is a text box inside the agent view.
- **DevTools targets are counted, not sampled.** The driver calls `Target.setDiscoverTargets` as soon as it attaches and counts every `Target.targetCreated` event whose type or URL is DevTools. That count is `devtools_targets_opened`, and the shapes require it to be 0. The count is derived, not written by hand. The transport logs every event it receives, and a Rust assert recomputes `devtools_targets_opened` from that log, counting `Target.targetCreated` events whose type or URL is DevTools plus DevTools entries in the teardown snapshot, and requires it to equal the recorded value. Closing the connection cannot drop an event uncounted. Before closing, the driver sends `Target.getTargets` and reads every event until that reply arrives. It then counts DevTools targets in the reply as well, so a target whose `Target.targetCreated` was still in flight is either counted or listed in the snapshot. A snapshot that lists a DevTools target fails the run. The snapshot is asserted, not assumed. The tally's last frame on the browser connection must be that `Target.getTargets`, and its reply must have been received and parsed before the socket closed. A connection that closes or drops before the reply arrives makes the run Red, not Green with a count of 0.
- **`--disable-dev-tools` is not adopted** (seat 2's proposal in quorum round 1). Whether Antigravity 2.8.1 honours it is unmeasured, so it would be a guard nobody has seen work. ph4 measures F1 and F12 on a scratch profile instead.

**Control path:** accessibility tree → node → `DOM.getBoxModel` → `Input.dispatchMouseEvent` at the box centre → `Input.insertText` for prompts.

**Isolation, asserted in Rust:**

- The app runs in **its own process group** (`setsid` through `libc`). `Drop` kills only that group, and the driver never signals any pid outside it.
- **One dedicated, reused demo profile.** D20 and D21 run on ONE profile directory that belongs to the course, outside the repo and outside the operator's own app, config and XDG directories. It holds `user-data/`, `extensions/`, `home/` and `xdg/`: the app gets `--user-data-dir` and `--extensions-dir` inside it, `--password-store=basic`, and `HOME` and every `XDG_*` directory point inside it too. A fresh scratch profile per take would need a fresh sign-in per take, so the profile is reused. The operator signs in on it once, by hand; a run whose profile is not signed in is `NotRun`. The demos never read, copy, list or receive a credential, and nothing from the profile is ever committed. Each agent's workspace is still fresh per take (below).
- **The app gets a cleared environment.** The driver launches it with `env_clear()` and sets only `PATH`, `LANG`, `DISPLAY`, `HOME` and the `XDG_*` variables, each pointing inside the demo profile. Nothing the operator's shell exports, such as an API key, reaches the app. The driver reads the child's `/proc/<pid>/environ` after launch and asserts that its keys are exactly that set, and it checks the values as well. `HOME` and every `XDG_*` value must canonicalize to a path under the canonical demo profile. `PATH`, `LANG` and `DISPLAY` must be byte-equal to the values the launcher set. A value that names the operator's home, however it was built, therefore fails the run.
- **The driver owns its display.** It starts its own `Xvfb` with `-displayfd` and reads the display number from that pipe; `xvfb-run -a` was only the §2 measurement harness. The same `DISPLAY` value is passed explicitly to the app and to `xdotool` in `E_4`, so neither can reach the operator's display.
- **The driver attaches only to the port its own child opened.** The app is started with `--remote-debugging-port=0`, and the driver reads the chosen port from `DevToolsActivePort` in the demo profile. The listening socket's inode must belong to a pid in the child's group, checked through `/proc/<pid>/fd` and `/proc/net/tcp`.
- **Single-instance forwarding is refused.** An Electron app may hand a second launch to an instance that is already running. If the child exits early, or no port appears in the demo profile, the run is `NotRun`, and the driver never looks for any other port.
- **The operator's own state is untouched.** The operator's app and gemini directories and the directories named by `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` and `XDG_STATE_HOME` (each with its XDG default when unset) are listed by path, inode, size, mtime and ctime before and after the run, and the two listings must be equal. ctime is in the listing because no unprivileged process can set it: a write followed by restoring the old mtime and size still moves ctime, since the restore is itself a metadata change. Content is never hashed there, because hashing is reading, and those directories hold credentials. A canary file is planted beside them and must be unchanged afterwards. No operator path is ever passed to the app. `operator_profile_touched` in the record comes from that comparison.
- **The demo profile's `extensions/` is fixed.** A manifest of it (path, size, sha256; extensions are code, not credentials) is taken before and after each run and must be equal, or the run fails. Only the operator changes the installed extensions, by hand. `no_js` covers the CDP channel; this manifest covers the extension channel, the other way script could enter the app.
- **Stated limit: a read leaves no trace.** Reading a file changes no ctime, and atime is not reliable under `relatime`, so no listing can prove the operator's credentials were not read. That property is held by construction instead: the app's `HOME` and `XDG_*` point inside the demo profile, every path the driver opens comes from `RFML5_AGY_PROFILE` or the run directory, and an `xtask` lint refuses any `agy-cdp`, d20 or d21 source that calls `std::env::home_dir`, reads `HOME` or an `XDG_*` variable, or names the operator's app or gemini directories. The one exception is `agy-cdp`'s `operator_listing` module, which must resolve those directories to list them; the same lint refuses any `File::open`, `fs::read*` or `fs::copy` in it, so it can only `read_dir` and `symlink_metadata`. The lint also refuses `std::env::vars`/`vars_os`, any `env::var` of a name outside `RFML5_*`, and any string literal naming `/proc/self/environ`, an absolute home path or `~`. It covers compile time too: `env!` and `option_env!` are refused for any name outside `RFML5_*` and the names Cargo itself sets for a build (`CARGO_MANIFEST_DIR`, `CARGO_PKG_*`, `CARGO_CRATE_NAME`, `CARGO_BIN_EXE_*`), matched against that list and never as a `CARGO_` prefix, so a user-exported name such as `CARGO_REGISTRY_TOKEN` is refused, and `include!`, `include_str!` and `include_bytes!` are refused unless their argument is a literal relative path with no `..` component that names a regular, git-tracked file (not a symlink), so a build cannot embed a file from outside its own crate. It reviews our own driver for a mistake; it is not a sandbox against hostile code.
- **Every path the driver touches is confined.** `RFML5_AGY_PROFILE` is canonicalized once at preflight. It must not equal, contain, or sit inside any operator directory in the listing, or the run is `NotRun`. Every later path goes through one `open_under(root, rel)` helper. `rel` must be relative and made only of normal components (no `..`, no root, no prefix). The helper holds a directory file descriptor on each canonical root, opened once at preflight, and walks `rel` one component at a time with `openat(dirfd, component, O_NOFOLLOW)` (through `rustix`, `agy-cdp`'s third dependency), using `O_DIRECTORY` for the intermediate components. New files are created at the leaf with `O_CREAT | O_EXCL`. No path is canonicalized and then re-opened, so there is no window between check and use: a component swapped for a symlink at any moment is refused, and a file that does not exist yet can still be created. The lint refuses `std::fs` and `File` calls, and any use of `rustix` or `libc`, anywhere in `agy-cdp`, d20 or d21 outside that helper (`rustix::fs::openat` is reachable only there; `rustix::net` and raw `libc` calls nowhere) and `operator_listing` and `agy-cdp::proc_probe`, so a `..` in a joined path cannot escape. `proc_probe` is the only reader of `/proc`, and it reads exactly three things: `/proc/<pid>/environ` (keys and values, for the cleared-environment assert; the values are compared against the launcher's and never written to the record), `/proc/<pid>/fd` (socket inodes) and `/proc/net/tcp`. The `pid` is an `AppPid` that only `agy-cdp::launch` can construct, from the app `Child` it spawned or a pid in that child's process group. `Xvfb` and `xdotool` children get no `AppPid`, so `proc_probe` cannot name them or any other process's `/proc` entry.
- **The driver writes nothing into the demo profile.** Its only profile accesses are reads of a closed set: `user-data/DevToolsActivePort` and the files under `extensions/` for the manifest. Under the profile root, `open_under` takes a `ProfilePath` enum with exactly those two variants, not a free path, so `home/`, `xdg/` and the rest of `user-data/`, where the sign-in lives, cannot be named. It opens the profile read-only and opens for writing only under the run directory. The extensions manifest is the run-time check on the code channel.
- **One CDP channel, one launcher.** The lint refuses `std::net::TcpStream`, `tungstenite` and `std::process::Command` anywhere in `agy-cdp`, d20 or d21 outside two modules. `agy-cdp::transport` is the only one that opens a socket: its discovery GET goes only to the port read from `DevToolsActivePort`, and every WebSocket frame it sends passes the method allow-list and the tally. `agy-cdp::launch` is the only one that spawns a process, and only three programs: the pinned app binary, `Xvfb` and `xdotool`, each with an argument vector built from fixed literals plus the display number and profile paths. All three are spawned with `env_clear()`: the app gets the set above, `Xvfb` gets only `PATH`, and `xdotool` gets only `PATH` and `DISPLAY`, so no helper inherits what the operator's shell exports. `xdotool` takes one typed argument, `XdotoolKey`, whose only variants are `F1` and `F12`, and its argument vector is exactly `key <F1|F12>`. No `type` command, key chord or free text can be expressed. Only `e3-probe` (`E_4`) spawns it; the D20 and D21 bins never do, and the lint refuses the constructor anywhere else. A second socket that could send `Runtime.evaluate`, or a shell that could write anywhere, therefore cannot be written without failing the lint.
- **The lint covers the whole closure, not three crates.** Its credential, environment, socket, process, `rustix` and `libc` rules apply to every workspace crate in d20's and d21's dependency closure, taken from `cargo metadata` (today `agy-cdp` and `demo-kit`), and the lint refuses any `build.rs` among them (today there are none), so nothing in the closure runs code at build time. demo-kit's one non-`RFML5_*` read, `CARGO_MANIFEST_DIR`, is admitted by name, because Cargo sets it and the operator's shell does not. Within the closure, only `agy-cdp::launch` may read `RFML5_AGY_PROFILE`, and the lint refuses that name anywhere else. demo-kit, and every crate outside the file-confinement rule, therefore cannot form a path into the demo profile, and the profile's root descriptor is held only inside `open_under`. The file-confinement rule stays on `agy-cdp`, d20 and d21, whose paths come only from `open_under`.
- **The driver attaches only to app pages.** `Target.attachToTarget` is built only from a `Target.getTargets` entry whose `type` is `page` and whose URL is not `devtools://`, and a Rust assert checks every attach in the tally against that entry. Together with the committed-prompt rule below, no allowed method can reach a DevTools console.
- **The driver types only committed prompts.** Every `Input.insertText` payload must be byte-equal to one of the take's committed fixture prompts. The tally records the sha256 of each inserted text, and a Rust assert checks every hash is in the fixture set. **Stated limit:** what an agent reads while it carries out a fixture task is the app's behaviour, not the driver's. The agents' reach is the app's own, and no listing can show a read.
- **Stated limit: writes are judged in three places, not everywhere.** `stray_writes` covers the take's run tree, the operator listing covers the operator's state, and the extensions manifest covers the demo profile's code. The rest of the demo profile is the app's own state and changes on every run by design. The rest of the filesystem is not diffed. The app's `HOME` and `XDG_*` point inside the profile, so its ordinary writes land there.
- **A leak sweep at teardown.** After the group is killed, no process from it may remain, and no socket may still be listening on its port.
- **Workspaces are isolated.** Each agent's workspace `ws/agent-N/` is a fresh scratch directory. `agent_N_files` comes from a filesystem diff of that workspace, cross-checked against what the agent view reports the agent edited. A change anywhere else in the take's run tree counts in `stray_writes`; D21 carries the same count, also required to be 0.

**Steps:**

1. Preflight.
2. Launch.
3. Find the hub target and wait for the agent view.
4. Create three agents, each with its own fixture task (`fixture-alpha`, `fixture-beta`, `fixture-gamma`) that writes one file in its own workspace.
5. Capture three screenshots: before the fan-out, fanned out, and all done. The program asserts each is non-blank, and the shapes require three distinct digests.
6. Tear down the process group, then run the leak sweep.
7. Write the record, judge it, and write the Receipt.

**Rust asserts beyond the shapes.** The shapes see only the record. Each line below is a claim the record cannot carry by itself, so the program asserts it before writing the record:

- **`app_evidence = EVIDENCE ∩ {m.method}` against the socket tally**, not against the record. The shapes bound `app_evidence` to the five and require all five, but cannot relate it to the `cdp_methods` nodes.
- **`Target.setDiscoverTargets` is the first frame sent on the browser connection**, so no `Target.targetCreated` can precede the subscription and `devtools_targets_opened` counts every DevTools target. Its parameters are not free: `agy-cdp` builds it only through a typed constructor that hard-codes `discover: true` and takes no argument, it is sent exactly once per connection, and a Rust assert checks that the tally holds one `Target.setDiscoverTargets` frame whose recorded params are `{"discover":true}`. A later frame that turns discovery off therefore cannot be written. This holds whatever the count: the constructor is the only way to build the frame, and the transport refuses a second one on the same connection. The shapes' count pattern checks only that the frame was sent; the params and the once-per-connection rule are owned by those Rust asserts.
- **Every `count` in `cdp_methods` is the tally's own number**, written from the one write function. The shapes refuse a count of 0, but only the tally can say a count is true.
- **Each screenshot is non-blank** (more than one distinct pixel value). The shapes see digests, not pixels.
- **`Input.insertText` follows a focus check**, as above.
- **`agent_N_files` equals the filesystem diff of `ws/agent-N/`**, and the agent view's own report of what it edited agrees with it.
- **The isolation list above**: the driver's own port only, the operator listing and canary unchanged, and the leak sweep empty.
- **`concurrent` comes from one snapshot.** `all_running_seen_ms` is the time of ONE `Accessibility.getFullAXTree` result in which all three agents show the running state, and it is greater than 0. The shapes bound it between each agent's start and done times; only the program can say all three states came from the same snapshot, so it asserts that before writing the field. Three snapshots that each show one agent running do not count.

**Advisory (round 3, security): what `no_js` cannot see.** `no_js` proves that no method that *runs* JavaScript was sent: every method is in ALLOW and the key set is three keys. It cannot prove that an allowed method never *causes* the page to run its own JavaScript: a click (`Input.dispatchMouseEvent`) on a control runs that control's handler, and `Input.insertText` into an input fires its listeners. That is the app being driven as a user drives it, which is the point of the demo, not JavaScript the driver authored. The claim is worded accordingly: the driver injects no JavaScript.

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
  - at least 30 stop samples on the dedicated demo profile (§5.3);
  - `L = max(p99, 2 × p95)`, rounded up;
  - the samples committed beside the demo.

  The long step must last at least 3 × (red + L + UI delay), so that agents 1 and 2 are reliably still running at red. L is re-measured whenever the pinned asar changes.

  **Measuring L is ph6's entry gate** (`E_6`, §6). ph6 records no take until the samples are committed and L is computed from them. Taking the samples drives the app, so it needs the demo profile signed in (E2, now done). Until the samples are committed, L, and with it the live D21 take, is `NotRun`, never estimated.
- **`stopped_agents = running_at_red`.** The shapes bound each set to {agent-1, agent-2} and require two values in each, but cannot equate two value sets, so the program asserts the equality.
- **Each stop is that agent's own control, and the stop is read back.** Each `stop_agent_N_sent_ms` is the moment of a click on agent N's own stop control: its accessibility node, then `DOM.getBoxModel`, then `Input.dispatchMouseEvent` at the box centre. `stop_agent_N_stopped_ms` is the moment the tree first reports agent N stopped. The shapes order the three moments (FALSIFY-D21-001 and -007); only the program knows which node was clicked.
- **No writes after the stop, judged by content, not by time.** The two workspace snapshots compare path, size and sha256, so no mtime granularity or slack enters the check.
- **The reducer is a pure function** of the multiset of results: no clock, no I/O, `BTreeMap` order. Its unit tests:
  - all six permutations, on several different result sets, give one digest per set;
  - **sensitivity:** changing one result changes the digest. A constant reducer, run as a negative control, must fail this test. That proves the test can fail; without it, a reducer that ignores its input would pass as order-free.
  - with several red results, the refusal names the lowest red id in every arrival order.
- **`refusal_names` is the lowest red agent id**, and `merged` is false whenever any result is red.
- **`app_evidence = EVIDENCE ∩ {m.method}` against the tally, and `Target.setDiscoverTargets` first on the browser connection**, as in D20 (§5.3).

## 6. Phase plan

The plan has seven phases. A **lane** is one independent reviewer in the quorum; lanes review, they never build or run anything. Phases 3, 4 and 5 run in parallel on disjoint paths, each with its own `CARGO_TARGET_DIR`, which the worker exports before it runs its cell; the cells are written without it, because a `VAR=… a && b` prefix binds only the first command. Each `A_i` is its phase's exit gate and is run at that boundary. ph1's names the two proof scripts that ph2 deletes; after ph2 its successor is `cargo run -q -p xtask -- verify`, which ph2 and ph7 run. `E_4` and `E_6` need the app and the signed-in profile, so they run only on the recording host, inside ph4 and ph6, and the fixtures they write are committed in that phase's own commit. CI runs ph7's commands, none of which runs the app, and ph7 ends with `git diff --exit-code`, so a gate that writes into the tree fails it. The scope column is a phase's write scope, plus any path it reads that another phase owns, marked "read-only". Every phase from ph2 on may also read the workspace it builds in: `Cargo.toml`, `Cargo.lock`, `xtask/**`, `demo-kit/**`, and every demo's `spec/` and `fixtures/`, which `xtask verify` judges. It never edits them outside its own scope. A worker that needs to edit a file outside its scope, or to add a dependency, stops and returns an open question; it never does either.

| Phase | Scope | Acceptance command (re-run by the orchestrator) |
|---|---|---|
| ph1 spec grill (five-role quorum: security, crux, architecture, adversarial, quality; review only) | this spec and the four `spec/` + `fixtures/` dirs | `jq --version && bash course-5/demos/xtask/proofs/spec-proofs.sh course-5/demos && bash course-5/demos/xtask/proofs/spec-proofs.sh --self-test course-5/demos && pmat spec review --record docs/audits/spec-rfml5-new-demos-d18-d21-review.json` |
| ph2 harness | `demo-kit/**`, `xtask/**`, `Cargo.toml`, `Cargo.lock`, `.github/workflows/ci.yml`, plus manifests and skeleton bins for d18–d21 and `agy-cdp`. **All** shared-file edits land here. | `cargo test -p demo-kit -p xtask && cargo build --workspace && cargo run -q -p xtask -- verify` |
| ph3 D18 | `d18-two-agents-one-server/**` | `cargo test -p d18-two-agents-one-server && cargo run -q -p xtask -- verify --only d18-two-agents-one-server && cargo run -q --release -p d18-two-agents-one-server --bin d18-two-agents-one-server` |
| ph4 agy-cdp + D20 | `agy-cdp/**`, `d20-agy-app-fanout/**` | `cargo run -q --release -p agy-cdp --bin e3-probe && cargo test -p agy-cdp -p d20-agy-app-fanout && cargo run -q -p xtask -- verify --only d20-agy-app-fanout && cargo run -q --release -p d20-agy-app-fanout` |
| ph5 D19 | `d19-workflow-ontology/**`; read-only: `d18-two-agents-one-server/spec/**` and `d18-two-agents-one-server/fixtures/**`, which D19 judges and never edits | `cargo test -p d19-workflow-ontology && cargo run -q -p xtask -- verify --only d19-workflow-ontology && cargo run -q --release -p d19-workflow-ontology` |
| ph6 D21 | `d21-agy-app-fanin/**`; read-only: `agy-cdp/**` and `d20-agy-app-fanout/fixtures/**` (the role/name map), which ph4 owns and ph6 never edits | `cargo run -q --release -p d21-agy-app-fanin --bin measure-stop-latency && cargo test -p d21-agy-app-fanin && cargo run -q -p xtask -- verify --only d21-agy-app-fanin && cargo run -q --release -p d21-agy-app-fanin --bin d21-agy-app-fanin` |
| ph7 pre-PR | the whole diff | `jq -e '(.lanes \| length == 5) and all(.lanes[]; .verdict == "PASS") and ([.lanes[].role] \| sort == ["adversarial","architecture","crux","quality","security"])' ../../docs/audits/diff-rfml5-new-demos-d18-d21-review.json && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo run -q -p xtask -- verify && git diff --exit-code` |

**Working directory.** ph1 (`A_1`) runs from the repository root. Every command in ph2 to ph7 runs from `course-5/demos`, the only Cargo workspace (there is no root `Cargo.toml`), and ph7 therefore reads its review record as `../../docs/audits/…`.

**The live runs.** Each writes its Receipt under `$RFML5_RECEIPTS/<id>/<run_id>/`, and only a Green verdict passes. `NotRun` is never Green: a missing tool, a pin mismatch or a missing sign-in leaves the phase open, not done.

**The live runs and entry gates read their inputs from the environment,** which the operator exports once, and the acceptance cells above run them as written (the last command of ph3–ph6 is that phase's live run, the first command of ph4 and ph6 its entry gate). Every demo bin and gate exits 0 only on Green, 1 on Red and 2 on `NotRun`, so a `NotRun` stops the `&&` chain. The inputs: `RFML5_RECEIPTS` (all), `RFML5_MODELS` and `RFML5_DECLARED_APR` (L3), `RFML5_AGY_BIN` and `RFML5_AGY_PROFILE` (L4, L6, `E_4`, `E_6`). Each command below is therefore runnable as written; an unset variable is `NotRun`, never a default.

- **L3 (D18):** `cargo run -q --release -p d18-two-agents-one-server --bin d18-two-agents-one-server`
- **L4 (D20):** `cargo run -q --release -p d20-agy-app-fanout`
- **L5 (D19):** `cargo run -q --release -p d19-workflow-ontology`
- **L6 (D21):** `cargo run -q --release -p d21-agy-app-fanin --bin d21-agy-app-fanin`
- **E_4 (ph4 entry gate, E3):** `cargo run -q --release -p agy-cdp --bin e3-probe`. Exit 0 only when all six controls are found and the F1/F12 DevTools counts are recorded; it writes the role/name map to `d20-agy-app-fanout/fixtures/role-name-map.json` for commit.
- **E_6 (ph6 entry gate, L):** `cargo run -q --release -p d21-agy-app-fanin --bin measure-stop-latency`. Exit 0 only with at least 30 samples; it writes them and the computed L to `d21-agy-app-fanin/fixtures/stop-latency.json` for commit.

L4, L6 and the measurement of L (§5.4) needed the operator to sign in once on the demos' own profile (E2). That is done, and the sign-in survived a cold restart; a take whose profile is found signed out is `NotRun`. Nothing here estimates a value it could not measure.

**Ordering rules:**

- **D18's contract and fixtures are frozen at the ph1 commit:** `d18-run-v1.yaml`, `receipt.golden.json`, `receipt.planted.json`, `receipt.planted.expect`, and D19's `mutants.json`. ph5 reads all of them, so a change to any during ph3 stops ph3 and goes back through the quorum. The re-check is `A_1` before ph2 lands; after ph2, which ports and deletes the two scripts, it is `cargo run -q -p xtask -- verify` plus `cargo test -p xtask`, whose tests are the ported sabotages.
- **Q2 (§7) is settled before ph2 starts,** because ph2 writes `agy-cdp`'s manifest.
- **ph6 starts after ph4 is Green.** D21 drives the app through `agy-cdp`, which ph4 builds, and uses ph4's committed role/name map and demo profile. ph6 therefore never runs in parallel with ph4.

**ph4 entry gate (E3).** Before any D20 or D21 recording, on the dedicated demo profile (§5.3), on a virtual display of its own:

1. Dump the hub's accessibility tree.
2. Show that five controls are reachable through the tree plus input events: new agent, the task box, send, an agent's state, and stop. A cold start also shows a "Changes to third-party model access" notice with a Dismiss button; the driver dismisses it through the tree before recording, so Dismiss is in the map too.
3. Measure whether F1 and F12 open DevTools on 2.8.1, counting `Target.targetCreated` events. The key is pressed with `xdotool` 3.20160805.1 (pinned; the run records `xdotool version`) on the demo instance's own virtual `DISPLAY`, never the operator's, because `agy-cdp` cannot express either key.
4. Commit the role/name map as a fixture.

Steps 1–3 are `E_4` above; its exit status is the gate. If any step fails, ph4 and ph6 stop and escalate.

**`A_1`, the spec proofs,** is two committed commands, run from the repository root with pv 0.70.1 on `PATH`:

```bash
bash course-5/demos/xtask/proofs/spec-proofs.sh course-5/demos               # exactly 17 checks, 0 failing
bash course-5/demos/xtask/proofs/spec-proofs.sh --self-test course-5/demos   # 28/28 sabotages, each refused exactly as its row states
```

The 17 checks:

| Checks | Count |
|---|---|
| `spec/` holds exactly one contract; `pv validate` accepts it and refuses the same contract with a key declared twice (the negative control: `pv validate` is lenient, and a validate that cannot fail proves nothing by passing) | 4 |
| the golden record is Green: exit 0, Pass, zero findings and violations, the positive control fired, nothing unarmed, W3C 19/19, and `focus_nodes_n`, `shapes_n`, `plant_violations` equal to §3 | 4 |
| the planted record exits 1, with sorted findings byte-equal to `.expect` | 4 |
| D19's record agrees with D18's `.expect` and with `mutants.json` | 4 |
| the mutant table (`verify-mutants.sh`): 18/18 killed and named, each finding byte-equal to the row's `messages`; 6/6 survived, each naming its Rust assert | 1 |

- Every judge run is materialised in a fresh directory outside any git work tree (M4).
- Any other number of checks fails: a gate over a different set of checks is not this gate.
- `--self-test` proves the gate can fail. It plants twenty-eight sabotages, each in a fresh copy of the demos tree. A row reads `name|exit|want[|reason]`:
  - for exit 1, `want` is the exact set of failing checks, sorted with `LC_ALL=C` and joined with `;`. A sabotage that also trips a check it does not name fails the self-test;
  - for exit 2, `want` is the exact last line. The one such row runs an unpinned pv (0.70.2), which is refused as not measured;
  - `reason`, on every row where the check alone does not say why, is a fixed string the output must contain, so the gate must fail for the stated reason, not merely fail;
  - a row missing its exit, its `want`, or a declared reason is `BAD-ROW`, and fails the self-test.
- The self-test was falsified in turn (round 2, at sixteen rows). Five defects were planted in a scratch copy: two rows with the named set narrowed, two rows with `want` or the reason emptied, and the empty-output guard reverted (`jq -nr 'input | …'` back to `jq -r`). It reported 11/16 with exit 1, and the five failing rows were exactly the five planted defects. The last one shows the guard is load-bearing: without it, an empty pv output is not caught as such.
- The twelve rows added in round 3 sabotage what the earlier ones could not reach. Five use a pv shim that runs the real pv and rewrites its JSON on a passing run: `pc_shape` not fired, a shape not armed, an unarmed violation, W3C 18/19, and a focus count one off; each fails all four golden checks for its own reason. Three misreport pv's exit: a reject that exits 2, a reject that exits 0, and a pass that carries a finding. One makes `pv validate` always exit 0, caught only by the negative control. The rest plant a second contract in D20's `spec/`, a survive row whose `rust_assert` is `None`, and a kill row whose expected message is off by one digit.
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
- live runs with no command (completed in round 2, which found D19's L5 missing).

D19's density (E4) is not addressed here: it is a proposal waiting on the outline owner.

**Added after round 1, and reviewed by round 2:**

- **D20:**
  - `devtools_targets_opened`;
  - `app_evidence`, with the `app_driven` equation;
  - `evidence_distinct`.
- **D21, for parity with D20:**
  - `cdp_protocol`, `operator_profile_touched` and `devtools_targets_opened`;
  - `app_evidence` and `keys_sent`, with the `app_driven` equation, `D21-INV-006` and `FALSIFY-D21-006`.
- **The committed `A_1`:**
  - the two proof scripts;
  - the self-test, then with eight sabotages, then sixteen, now twenty-eight.

**Quorum round 2** reviewed the revision above, with the same three models:

| Seat | Model | Verdict |
|---|---|---|
| 1 | claude-sonnet-5-5-medium | FAIL, with fifteen findings |
| 2 | gemini-3.1-pro-high | FAIL: the round-1 "live runs with no command" fix lacked D19's live run |
| 3 | gpt-oss-120b-medium | First run BLIND: the provider answered 503 "No capacity available", so the seat returned no verdict and none is counted. The retry answered PASS, with no structured output. It said D18 and D19 are ready and that D20 and D21 need Rust asserts beyond the shapes, and it recommended the E4 split. The retry also wrote an analysis file into its own review clone although it ran read-only; the file was kept as evidence and is not part of this change. |

What round 2 found, and what changed:

- **The gate could pass on nothing.** An empty pv output with exit 0 left the golden check Green. The check now reads pv's output through a guard that treats empty output as a failure, and a sabotage plants exactly that.
- **The self-test proved too little.** A sabotage passed if its named check failed, whatever else failed with it. Each row now names the exact set of failing checks, and the four mutant-table rows also name the reason.
- **Missing sabotages:** empty pv output, a D20 golden without `Input.insertText`, a D21 golden that sent F12, a deleted contract, and an empty `.expect`. All are now rows.
- **Kill purity ignored the property.** A mutant killed by a different property of the same component counted as killed. Every kill row now names its `properties`, and `verify-mutants.sh` reports `WRONG-PROPERTY` when the findings name others.
- **D18's overlap could not be re-checked from the record,** which carried each item's writer end and checker start only. It now carries all four times per item (`pipe_writer_start_ms`, `pipe_writer_end_ms`, `pipe_checker_start_ms`, `pipe_checker_end_ms`), and survivor s06, a pipelined run that never overlapped, names the Rust assert that refuses it.
- **D21's timing relations had no killing mutant.** The planted D21 record now carries both stops sent before red, and each stopped state read before its stop was sent.
- **`app_evidence` and the CDP tally were unrelated to pv.** `Target.setDiscoverTargets` joins the evidence set, every method count must match `^[1-9][0-9]*$`, and §5.3 lists the Rust asserts that tie `app_evidence` to the driver's own tally.
- **§5.3 and §5.4 had no list of Rust asserts.** Both now do, covering non-blank screenshots, the filesystem diff, each stop being that agent's own control read back from the tree, and `stopped_agents = running_at_red`.
- **No phase owned L.** Measuring L is now ph6's entry gate, and stays `NotRun` until the operator signs in.
- **Hidden ordering.** ph6 now runs after ph4, and the ph4 key press names its tool and display.
- **E4 was listed as addressed while still open.** It is now listed as open.
- **D19 had no live run.** L5 is added (seat 2).

Two round-2 findings are recorded rather than changed:

- s01 and s03 survive, although the model, seed and temperature are pinned, so `in` could in principle pin their digests and decisions. Both depend on the inference build, and the owner's ruling (E1) leaves the `apr` patch version free. A shape pinning them would refuse a correct run on the next 0.70.x, so they stay survivors, each with its reason in `mutants.json`.
- `closed` with `ignoredProperties: [rdf:type]` has no row showing the ignore is not a wider escape hatch. Mutant m17 adds a JSON key literally named `rdf:type` and is refused by `closed` (M2), which is the case the ignore could have let through.

**Quorum round 3** reviewed the revision above with five roles, each a separate lane:

| Role | Model | Verdict |
|---|---|---|
| security | gpt-oss-120b-medium | FAIL, nine findings |
| crux | gemini-3.1-pro-high | FAIL, two findings |
| architecture | gemini-3.1-pro-high | FAIL, four findings |
| adversarial | claude-sonnet-5-5-medium | FAIL, four findings and one advisory |
| quality | claude-sonnet-5-5-medium | FAIL, four findings and three advisories |

Every finding was re-read against the files it cites. What changed:

- **Crux: "fans out three agents" had no concurrency claim.** A driver that ran the agents one after another passed every check. D20 now has `concurrent` (§1): one accessibility snapshot shows all three running, and each agent's start ≤ that snapshot ≤ its done, as `lessThanOrEquals` relations; §5.3 asserts the single snapshot in Rust. The planted record has agent 3 starting after the snapshot.
- **Crux: "you can watch" contradicted the virtual display.** It is now "watchable in the recording", in the spec and the contract.
- **Architecture: a fresh scratch profile cannot hold a sign-in.** D20 and D21 now run on one dedicated, reused demo profile outside the repo (§5.3). The operator signs in once; the demos never read, copy or receive a credential.
- **Architecture: the ph3 fallback re-ran a script that ph2 deletes.** After ph2, the re-check is `xtask verify` plus `cargo test -p xtask` (§6).
- **Architecture: ph5 needs files outside its scope, and D18's golden record was not frozen.** ph5's scope now names D18's `spec/` and `fixtures/` read-only, and the freeze covers D18's contract, golden, planted, `.expect` and D19's mutant table.
- **Adversarial: a survivor's `rust_assert` was never checked.** `verify-mutants.sh` now fails a row whose `rust_assert` names nothing (`NO-RUST-ASSERT`).
- **Adversarial: the golden sub-criteria and pv's exit reading had no sabotage.** Twelve rows were added (§6), including pv shims that misreport the verdict or the exit, and an always-0 `validate`. That last row showed `pv validate` is lenient: it accepts a contract with no name, no equations or a bad version. The validate check therefore gained a negative control, the same contract with a key declared twice, which a working `pv validate` refuses.
- **Adversarial: kills did not compare message text, and counted findings by lines.** Every kill row now carries its exact sorted `messages`, compared byte for byte, and the count is read from pv's JSON.
- **Adversarial advisory: a second yaml in a spec dir.** The validate check now requires exactly one contract, and a sabotage plants a second.
- **Quality: s02's description was wrong.** Its pids are 41388; a pid-1 mutant is refused by `pattern` (measured: two `pattern` findings), so it would be a kill.
- **Quality: the overlap assert was stated three ways.** It is now stated once, as `pipelined_overlaps` in §5.1, with i ∈ {1, 2, 3}; §2 and `mutants.json` refer to it.
- **Quality: the golden sequential schedule interleaved.** It is now truly sequential: all four writer requests end before the first checker request starts.
- **Quality: D21's `operator_untouched` was weaker than D20's.** It now has the listing equality and `stray_writes = 0`, with a falsifier; the planted record has one stray write.
- **Quality advisories.** The §3 counts are now asserted by `A_1`; the phase table says five roles; D21's screenshots are stated as recording-only.

Recorded, not changed:

- **Security, seven findings: the planted fixtures contain the defects.** `Runtime.evaluate`, `Page.addScriptToEvaluateOnNewDocument`, `Runtime.callFunctionOn`, F12, a touched operator profile and a cross-agent write all appear in `receipt.planted.json`. They are false positives: a planted record exists to carry exactly those defects, and `A_1` requires pv to reject each one with the finding in `.expect`.
- **Security: "JavaScript injection possible via `Page.addScriptToEvaluateOnNewDocument`" (§2 M6) and "DevTools via F12" (§5.3).** False positives as findings: M6 records a hole that was measured and closed, and the F12 risk is the one E3 measures before any recording, with F12 outside the three-key set. "Runtime methods present despite the enum claim" cites the line stating that the crate has no `Runtime` type and a lint refusing the strings, which is the opposite claim.
- **Security advisory: `no_js` cannot see JavaScript the page runs itself in response to an allowed input.** True, and now stated in §5.3: the claim is that the driver injects none.

**Quorum round 4** reviewed the round-3 revision with the same five roles. No lane ran on the author's model family.

| Role | Model | Verdict |
|---|---|---|
| security | gpt-oss-120b-medium | FAIL, ten findings |
| security (second lane) | gemini-3.1-pro-high | FAIL, three findings and one advisory |
| crux | gemini-3.1-pro-high | PASS |
| architecture | gemini-3.1-pro-high | FAIL, two findings (and thirteen confirmations of round 3) |
| adversarial | gemini-3.1-pro-high | PASS (a gpt-oss-120b-medium lane returned no review: the model had no capacity, 503) |
| quality | gemini-3.1-pro-high | PASS |

All three PASS lanes and the architecture lane confirmed every round-3 change listed above, and `A_1` and its self-test were re-run by lanes (17/17, 28/28). What changed:

- **Architecture: ph6 needs files outside its scope.** ph6's scope now names `agy-cdp/**` and D20's `fixtures/**` read-only (§6).
- **Architecture: the entry gates and live runs were not runnable as written.** The E3 gate is now `E_4` and the L measurement `E_6`, each a command whose exit status is the gate; L4, L6, `E_4` and `E_6` take the launcher and the demo profile from `RFML5_AGY_BIN` and `RFML5_AGY_PROFILE`, which have no default (§4 item 7, §6).
- **Security: a write to the operator's state, followed by restoring mtime and size, evaded the listing.** The listing now carries inode and ctime, which no unprivileged process can restore (§5.3).
- **Security: script could enter through an extension in the demo profile.** A manifest of `extensions/` must be equal before and after each run (§5.3).
- **Security: a read of the operator's credentials is undetectable.** True; no metadata records a read. It is now a stated limit in §5.3, held by construction and by an `xtask` lint on the driver's sources rather than by measurement.

Recorded, not changed:

- **Security (gpt-oss), nine findings: the planted fixtures contain the defects,** as in round 3. Each names a defect the planted record carries on purpose so that the shapes can be seen to refuse it.
- **Security (gpt-oss): "D21's concurrency claim is unbacked".** D21's contract has no concurrency claim; `concurrent` is D20's (§1), and D21's own timing claims are the ordered stop moments and the latency bound L.

**Round 4b** re-ran the two failing roles on the round-4 revision, both on gemini-3.1-pro-high. A gpt-oss-120b-medium security lane returned no evidence of reading the tree and was not counted. Both lanes confirmed the five round-4 changes and returned FAIL on new findings. What changed:

- **Security: the app inherited the operator's environment,** so an exported API key reached it. The app now starts from a cleared environment, and its keys are asserted from `/proc/<pid>/environ` (§5.3).
- **Security: the lint did not cover `/proc/self/environ` or other home paths.** It now refuses enumerating the environment, any non-`RFML5_*` variable, and literals naming `/proc/self/environ`, an absolute home path or `~`. It is stated as a review of our own driver, not a sandbox (§5.3).
- **Security: `stray_writes` covers only the run tree.** True, and now a stated limit that names the three places writes are judged (§5.3).
- **Architecture: `<dir>`-style placeholders and prose in the acceptance column were not runnable.** Inputs now come from exported `RFML5_*` variables, and each acceptance cell is a chain of commands and named runs joined by `&&` (§6).
- **Architecture: `xdotool` could not learn `xvfb-run -a`'s display, and its pin was not in §4.** The driver owns its `Xvfb` through `-displayfd` and passes `DISPLAY` explicitly; `xdotool` is pinned in §4 item 1.

**Round 4c** re-ran the same two roles on gemini-3.1-pro-high. Both confirmed the round-4b changes and returned FAIL on new findings. What changed:

- **Architecture: the acceptance cells still held named runs and commas.** Each cell for ph2–ph6 is now one literal `&&` chain, with the entry gate first and the live run last. ph7 chains its four commands after the quorum (§6).
- **Architecture: ph5's `pv extract` wrote into D18's spec directory while ph3 runs.** D19 extracts from a copy in its own run directory, outside any git work tree (§5.2).
- **Architecture: Xvfb was an unchecked dependency.** Preflight requires it and records its version; it is recorded, not pinned, because no claim depends on it (§4 item 1).
- **Architecture advisory: a `NotRun` that exits 0 would pass an `&&` chain.** Every bin and gate exits 0 only on Green, 1 on Red and 2 on `NotRun` (§6).
- **Security: path traversal from `RFML5_AGY_PROFILE`.** The profile is canonicalized and must be disjoint from the operator's directories, and every path goes through one confining `open_under` helper that the lint makes the only file access (§5.3).
- **Security: a payload written into the demo profile's `user-data/`.** The driver opens the profile read-only (§5.3).
- **Security: a credential read through a typed prompt.** The driver types only committed fixture prompts, checked by hash. What the app's own agents read while doing a fixture task is a stated limit (§5.3).

**Round 4d** re-ran architecture and security on gemini-3.1-pro-high with the Round 4c record quoted. Architecture confirmed all seven round-4c changes. Both lanes returned FAIL on new findings, and each was fixed:

- **Architecture: the ph1 and ph7 cells were still prose.** ph1 now runs `jq --version`, both spec-proof commands and `pmat spec review --record`. ph7 checks the committed diff-review record with `jq -e` before its four commands (§6).
- **Architecture: `open_under` could not create a file.** Canonicalizing a path that does not exist yet fails. The helper now canonicalizes the parent, opens the leaf with `O_NOFOLLOW` and creates new files with `create_new` (§5.3).
- **Architecture: `jq` was an unchecked dependency of the proof scripts.** `A_1` records `jq --version` first, and a missing jq fails it (§4 item 1).
- **Architecture: `measure-stop-latency` starts a display but was not in the Xvfb list.** Added (§4 item 1).
- **Security: `open_under` could name any file in the profile, including the sign-in.** Under the profile root it takes a two-variant `ProfilePath` enum, `DevToolsActivePort` and `extensions/`, so `home/`, `xdg/` and the rest of `user-data/` cannot be named (§5.3).
- **Security: a second socket or a spawned process could bypass the CDP allow-list or write outside the judged places.** The lint confines `TcpStream`, `tungstenite` and `Command` to `agy-cdp::transport` and `agy-cdp::launch`, and the launcher spawns only the app, `Xvfb` and `xdotool`, with fixed argument vectors (§5.3).

**Round 4e** re-ran architecture and security on gemini-3.1-pro-high, quoting the Round 4d record. Both returned FAIL on new findings, and each was fixed:

- **Architecture: ph1's `--self-test` call omitted the demos directory.** Measured, it exits 1 with a usage error; the cell now passes `course-5/demos`, and the corrected command was re-run: 28/28 (§6).
- **Security: the `/proc` reads the isolation checks need contradicted the file lint.** `agy-cdp::proc_probe` is the one `/proc` reader, limited to a child's `environ` keys, its `fd` list and `/proc/net/tcp`, for a `ChildPid` only the launcher can mint (§5.3).
- **Security: `Target.setDiscoverTargets` with `discover: false` would satisfy the method count.** A typed constructor hard-codes `discover: true`, the frame is sent once per connection, and a Rust assert checks its recorded params (§5.3).

**Round 4f** re-ran architecture and security on gemini-3.1-pro-high, quoting the Round 4e record. Architecture confirmed all three round-4e changes. Both lanes returned FAIL on new findings:

- **Architecture: the parallel phases' cells shared one `target/`.** The worker exports its own `CARGO_TARGET_DIR` before running its cell, and §6 says so (§6).
- **Architecture: D18's tests could not reach `fake-apr-serve`, a bin of another package.** The fake's body becomes a `demo_kit` library function, and each demo that tests against it carries its own two-line bin (§5.1).
- **Architecture: ph1's command names scripts that ph2 deletes.** Each `A_i` is run at its own boundary, and §6 names `xtask verify` as ph1's successor after ph2.
- **Architecture: `E_4` and `E_6` write fixtures, which would dirty a CI tree.** They run only on the recording host and are committed in their phase. CI runs ph7, which now ends with `git diff --exit-code` (§6).
- **Security: a race between canonicalizing and opening in `open_under`.** The helper walks components with `openat` and `O_NOFOLLOW` from a held root descriptor and re-opens no path (§5.3).
- **Security: closing the socket could drop a `Target.targetCreated` uncounted.** Before closing, the driver drains events up to a `Target.getTargets` reply and counts DevTools targets in that snapshot too (§5.3).
- **Security: a host path in D14's `fanout.sh`.** Triaged out of scope: it predates this ticket on `main`, is in a demo outside D18–D21 and outside this spec, and is reported for a separate fix.

**Round 4g** re-ran architecture and security on gemini-3.1-pro-high, quoting the Round 4f record and scoping the review to this spec and the D18–D21 directories. Architecture confirmed the round-4f changes. Both lanes returned FAIL on new findings, and each was fixed:

- **Architecture: Cargo names a bin after its file stem**, so D18's `fake_apr_serve.rs` would set `CARGO_BIN_EXE_fake_apr_serve`. D18 declares `[[bin]] name = "fake-apr-serve"`, as demo-kit does (§5.1).
- **Security: ph7's `jq` gate passed with a failing lane beside a passing one.** It now requires exactly five lanes, all PASS, one per role. Measured: a record with an extra FAIL lane exits 1 and five PASS lanes exit 0 (§6).
- **Security: `rustix` and `libc` could open a socket or read a file around the lint.** Both are refused outside `open_under` (§5.3).
- **Security: the operator listing missed `XDG_DATA_HOME`.** Added (§5.3).

**Round 4h** re-ran architecture and security on gemini-3.1-pro-high, quoting the Round 4g record. Both lanes returned FAIL. How each finding was handled:

- **Architecture: the listing named "XDG data" in prose but not the variable.** It now names `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` and `XDG_STATE_HOME`, with their defaults (§5.3).
- **Architecture: ph3's and ph6's `cargo run -p` was ambiguous**, because each package has a second bin. Those two live runs name their `--bin` (§6, L3, L6). D19 and D20 have one bin each and need none.
- **Security: `Target.setDiscoverTargets` could be sent a second time with `discover: false`, so DevTools could open and close unseen.** The constructor cannot build `false`, and the transport refuses a second frame. §5.3 now says the shapes' count shows only that the frame was sent, and the Rust asserts own the params.
- **Security: an allowed method could reach a DevTools console.** `Target.attachToTarget` is built only from a `page` target that is not `devtools://`, and is asserted against the tally (§5.3).
- **Security: the lint skipped demo-kit and build scripts.** Its credential, environment, socket, process, `rustix` and `libc` rules now cover the workspace dependency closure and its `build.rs` files (§5.3).
- **Security: `~/.ssh` and `~/.aws` are not listed.** Triaged as beyond a stated limit: §5.3 says writes are judged in three places, and the rest of the filesystem is not diffed. The driver itself cannot spawn a shell or open a file outside `open_under`.

**Round 4i** re-ran both roles on gemini-3.1-pro-high, quoting the Round 4h record. Each lane was told that a FAIL must be a command that cannot run, a contradiction, or a guarantee its mechanism does not deliver. Architecture returned PASS: the round-4h changes landed and every acceptance command runs as written. Security returned FAIL on two guarantees whose mechanism fell short, and both were fixed:

- **The teardown `Target.getTargets` snapshot was never asserted.** It must be the last frame on the browser connection, with its reply parsed before close; a dropped connection is Red (§5.3).
- **demo-kit was outside the file-confinement rule and could read `RFML5_AGY_PROFILE`.** Only `agy-cdp::launch` may read that variable, so no other crate can form a profile path (§5.3).

**Round 4j** re-ran both roles on gemini-3.1-pro-high, quoting the Round 4i record. Every FAIL finding was tagged cannot-run, contradiction or undelivered-guarantee. Both lanes returned FAIL, and all five tagged defects were fixed:

- **cannot-run: demo-kit and D18 both declared a bin named `fake-apr-serve`,** which collide in the workspace output directory. D18's is `d18-fake-apr-serve` (§5.1).
- **undelivered-guarantee: the parallel phases' `judge` runs shared `<run_root>/judge/`.** Each call now creates a fresh `judge-<n>/` that fails if the name exists (§4 item 3).
- **contradiction: the Round 4h record said "every live run" names its `--bin`.** It now says ph3's and ph6's do, and that D19 and D20 have one bin each (§7).
- **undelivered-guarantee: the cleared-environment assert checked keys, not values.** `HOME` and every `XDG_*` value must canonicalize under the demo profile, and the rest must equal what the launcher set (§5.3).
- **undelivered-guarantee: `devtools_targets_opened` was written, not derived.** A Rust assert recomputes it from the transport's event log and the teardown snapshot (§5.3).

**Round 4k** re-ran both roles on gemini-3.1-pro-high, quoting the Round 4j record. Architecture returned PASS with no findings. Security returned FAIL on one contradiction, which was fixed:

- **contradiction: `proc_probe` was still described as reading environ "keys only"** after round 4j added the value check. It now reads keys and values, compares the values against the launcher's, and never writes them to the record (§5.3).

**Round 4l** re-ran both roles on gemini-3.1-pro-high, quoting the Round 4k record. Both returned FAIL. Each finding was checked against the files and fixed:

- **cannot-run (architecture): ph7 mixed a repo-root `jq` path with `cargo` commands that need the `course-5/demos` workspace.** There is no root `Cargo.toml`. The phase table now names the working directory: ph1 from the repository root, ph2 to ph7 from `course-5/demos`. ph7 reads its record as `../../docs/audits/…`.
- **undelivered-guarantee (security): `Xvfb` and `xdotool` inherited the operator's environment, and `proc_probe` could read it through their `ChildPid`.** All three children are now spawned with `env_clear()`. `Xvfb` gets `PATH`, and `xdotool` gets `PATH` and `DISPLAY`. `proc_probe` now takes an `AppPid`, which is minted only for the app's process group (§5.3).
- **undelivered-guarantee (security): `xdotool` literals could drive a native file dialog outside the CDP tally.** `xdotool` now takes only the typed `XdotoolKey::{F1, F12}`, with the argument vector `key <F1|F12>`. Only `e3-probe` spawns it, and the D20 and D21 bins never do (§5.3).

**Round 4m** re-ran both roles on gemini-3.1-pro-high, quoting the Round 4l record. Both returned FAIL. Each finding was checked against the files and fixed:

- **cannot-run (architecture): the ph2 to ph6 scopes did not cover what their own commands read**, namely the workspace manifest, `xtask/` and every demo's `spec/` and `fixtures/`. The scope column is now defined as the write scope plus read-only paths owned by another phase. Every phase from ph2 on may read the workspace it builds in, and never edits outside its own scope (§6).
- **contradiction (security): the D20 and D21 `operator_untouched` domains disagreed with §5.3.** D21 said "path, size and mtime", and both left out XDG data. Both contracts now say "XDG config, data, cache and state directories listed by path, inode, size, mtime and ctime".
- **undelivered-guarantee (security): compile-time macros got past the credential lint.** The lint now refuses `env!` and `option_env!` for any name outside `RFML5_*` and Cargo's own `CARGO_*`. It refuses `include!`, `include_str!` and `include_bytes!` unless the path is a literal relative path with no `..` component (§5.3).

**Round 4n** re-ran both roles on gemini-3.1-pro-high, quoting the Round 4m record. Architecture returned PASS with no defects. Security returned FAIL with three findings, all closed:

- **contradiction: D20's `operator_untouched` domain left out the canary file** that §5.3 plants. It now names the canary, matching D21.
- **undelivered-guarantee: `env!("CARGO_REGISTRY_TOKEN")` read as admitted.** As written, the rule admits only the names Cargo sets, so the lane misread it. Even so, the rule now lists those names and says it never matches on a `CARGO_` prefix (§5.3).
- **undelivered-guarantee: a closure `build.rs` could plant a symlink for `include_str!` to embed.** This needs a deliberately hostile author, which is outside the lint's stated limit. Even so, the lint now refuses any `build.rs` in the closure, and an included path must name a regular, git-tracked file (§5.3).

**Round 4o** re-ran architecture and security on gemini-3.1-pro-high, quoting the Round 4n record. Both returned PASS with no defects; the spec was not changed.

**Round 4p** re-ran crux, adversarial and quality on the same text, because their round-4 PASS predated rounds 4b to 4n. Crux returned PASS. Adversarial and quality each returned FAIL with one finding. Neither breaks a stated guarantee, but each names a real gap, and both were closed:

- **adversarial: the self-test row `d21-spec-deleted` named no reason,** so deleting the one-contract guard left the row green. The row now requires the reason `spec/ must hold exactly one contract`, and deleting the guard was measured to fail it (27/28).
- **quality: FALSIFY-D18-005 was exercised by no fixture.** Mutant m18 now adds `out/verdicts.json` to `writer_files`, and `sh:in` kills it purely. The mutant table, D19's contract (`killed` minCount 18), both D19 receipts and the counts in this spec moved from 17 kill rows to 18.

**Escalations:**

- **E1: resolved by the owner.** The ruling, verbatim: "apr version is 0.70.*".
  - D18's contract pins `apr_version` with `pattern: '^0\.70\.[0-9]+$'`. Measured on pv 0.70.1: the golden record and 0.70.12 pass, while 0.69.3, 0.701.0 and 0.70.1-dirty are each rejected.
  - The harness pins `apr` as `SeriesPin` 0.70 (§4 item 1) and records the binary's sha256 in the Receipt.
  - The course machine must therefore run a 0.70.x `apr`. A 0.69.3 install is `NotRun(VersionMismatch)`, and the demo never falls back.
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
