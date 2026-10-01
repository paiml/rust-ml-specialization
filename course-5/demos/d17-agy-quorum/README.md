# D17 -- one model three times is one reviewer (rfml5 4.2.1)

`bash quorum.sh` (needs `agy` =1.2.14, `jq`). Same planted diff, same schema, two acts:
the same model x3, then three model families. Lanes are read-only (no
`--dangerously-skip-permissions`; the prompt forbids tools). `--sandbox` is not used: with
`-p` it auto-denies the command permission and the lane returns no output.

Reduction: any FAIL => FAIL; else any NotRun => NotRun; else PASS. A lane that exits
non-zero, times out, or returns output off the schema is NotRun. NotRun is never green.
Contract `quorum-one-fail-blocks-v1 + notrun-never-green-v1` is checked on synthetic
verdicts, so it holds whatever the models say. What they say goes in the receipt (stdout JSON).
