# D14 — fan-out (rfml5 1.2.1)

Bash and `apr` only. `fanout.sh` is the whole demo and is meant to be read on screen.

1. **Preflight** — model sha256, `apr --version` equals the pin, load1 < 32, GPU memory for 4 servers. Exit 1 with a reason otherwise.
2. **Serial** — one resident `apr serve run` on 8080, 48 prompts one at a time (temperature 0, 48 max tokens). Prints T1.
3. **Fan-out** — four resident servers on 8081–8084, `split -n r/4`, `xargs -P 4`, one lane per port. Prints T4 and the four `nvidia-smi` processes.
4. **Reduce** — `jq -s 'sort_by(.id)'`; every id exactly once, and the merged sha256 equals the serial sha256.
5. **Amdahl** — speedup `T1/T4`, serial fraction back-solved from `S = 1/(f + (1-f)/N)`, merge time in ms.
6. A trap kills every server on EXIT/INT; nothing is left running.

Provable contract: `fanout-disjoint-v1` + `reduce-deterministic-v1`. A violation prints `contract: FAIL ...` and exits 1; the sha is never weakened.

Receipt: `$RFML5_RECEIPTS/d14-fanout/<host>/<apr>/<run-id>.json` (default `~/rfml5-receipts`; refused inside this repo), in the shape D16 reads.

Run: `bash course-5/demos/d14-fanout/fanout.sh`. Weights are found under `$RFML5_MODELS` (default `/mnt/nvme-raid0/lqw/models`).

## What it measures

On lambda (one RTX 4090) four servers share one GPU, so decode time-slices rather than overlaps: measured speedup is about 1.0–1.1x, serial fraction about 0.85–0.95. The GPU is the serial part; a second GPU or host is what would move it. The numbers are in the receipt, not in this file.

## Gates

- Timings go to an append-only tick log; durations and the receipt derive from it, and each run appends one JSON line to `<receipts>/d14-fanout/runs.jsonl`.
- `xtask verify` runs `bashrs lint` (any error fails) and `shellcheck -S warning` (any finding fails) on every `demos/*/*.sh`.
- `xtask/tests/fanout_falsifier.rs` runs `fanout.sh` against bash stubs in `tests/stubs/` (apr, curl, nvidia-smi; no GPU): faithful => `contract: ... OK`; one port answering differently => `contract: FAIL`; a dropped or duplicated id => `contract: FAIL`.
