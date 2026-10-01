# D17 -- one model three times is one reviewer (rfml5 4.2.1)

`bash quorum.sh` (needs `agy` =1.2.14, `jq`). Same planted diff, same schema, two acts:
the same model x3, then three model families. Lanes are read-only (no
`--dangerously-skip-permissions`; the prompt forbids tools). `--sandbox` is not used: with
`-p` it auto-denies the command permission and the lane returns no output.

Reduction: any FAIL => FAIL; else any NotRun => NotRun; else PASS. A lane that exits
non-zero, times out, or returns output off the schema is NotRun. NotRun is never green.
Contract `quorum-one-fail-blocks-v1 + notrun-never-green-v1` is checked on synthetic
verdicts, so it holds whatever the models say. What they say goes in the receipt (stdout JSON).

## Diff selection (rule fixed before probing)

`probe.sh 5` runs all 10 planted corpus diffs through the three quorum models, 5 rounds each,
with `prompt.txt` + the verdict schema for every lane, and appends one line per run to
`probe-log.ndjson` (150 runs, committed, append-only). Rule: pick the planted diff with the
lowest gemini-3.8-flash-high catch rate (verdict FAIL / 5); ties go to the lowest filename.
Only corpus diffs qualify; no diff or prompt is authored for any model.
Stop condition: if no diff shows a homogeneous-vs-heterogeneous gap, escalate instead of picking.

| diff | gemini | sonnet-4-6 | gpt-oss | NotRun g/s/o | rounds any-lane FAIL |
|---|---|---|---|---|---|
| planted-01..08 | 5/5 | 5/5 | 1-4/5 | 0/0/0-4 | 5/5 |
| planted-09 | 4/5 | 5/5 | 4/5 | 1/0/1 | 5/5 |
| planted-10 | 3/5 | 5/5 | 2/5 | 2/0/3 | 5/5 |

The rule selects **planted-10** (not adopted yet: see below). Every gemini non-catch in the
probe was a NotRun (one empty/off-schema, one 124 timeout), never a PASS: no model returned PASS
on any planted diff in 150 runs. The gap is reliability (a homogeneous panel can go NotRun
together), not a shared blind spot. Awaiting a ruling before `quorum.sh` switches diffs.
