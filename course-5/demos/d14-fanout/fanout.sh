#!/usr/bin/env bash
# D14 -- fan-out: one resident apr server vs four, same 48 prompts, same bytes out.
#
# Provable contract: fanout-disjoint-v1 + reduce-deterministic-v1
#   every prompt id is answered exactly once across the lanes, and the merged
#   result has the same sha256 as the serial run.
set -euo pipefail
cd "$(dirname "$0")" || exit 1
PIN=0.69.3 N=4 MAX_TOKENS=48
SHA=${RFML5_MODEL_SHA:-00fe7986ff5f6b463e62455821146049db6f9313603938a70800d1fb69ef11a4}  # override: CI stubs only
MODEL=${RFML5_MODELS:-"/mnt/nvme-raid0/lqw/models"}/Qwen3.5-4B-Q4_K_M.gguf
OUT=${RFML5_RECEIPTS:-"$HOME/rfml5-receipts"}
W=$(mktemp -d "${TMPDIR:-/tmp}/d14.XXXXXX") pids=()
# every clock reading is appended to one log; durations and the receipt are derived from it
stamp() { printf "utc " >> "$W/ticks.log"; date -u +%FT%TZ >> "$W/ticks.log"; }
tick() { printf "%s " "$1" >> "$W/ticks.log"; date +%s%N >> "$W/ticks.log"; }
at() { awk -v k="$1" '$1 == k { print $2 }' "$W/ticks.log"; }
span_ms() { echo $(( ($(at "$2") - $(at "$1")) / 1000000 )); }
load1() { cut -d' ' -f1 /proc/loadavg; }
die() { echo "preflight: $*"; exit 1; }
trap 'kill "${pids[@]}" 2>/dev/null; wait 2>/dev/null; rm -rf "${W:?}"' EXIT INT TERM

# 1 preflight
[[ $(sha256sum "$MODEL" | cut -d' ' -f1) == "$SHA" ]] || die "model sha256 differs"
[[ $(apr --version) == "apr $PIN"* ]] || die "apr is not $PIN"
L1_START=$(load1); (( $(bc <<< "$L1_START < 32") )) || die "load1 $L1_START >= 32"
FREE=$(nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits)
(( FREE >= N * 5000 )) || die "GPU free ${FREE} MiB < $N servers x 5000"
[[ $OUT/ != "$(git rev-parse --show-toplevel)"/* ]] || die "receipts belong outside this repo"

ask() { jq -nc --argjson r "$2" '{model:"default",messages:[{role:"user",content:$r.prompt}],max_tokens:'$MAX_TOKENS',temperature:0}' |
  curl -sf "localhost:$1/v1/chat/completions" -H 'content-type: application/json' -d @- |
  jq -c --argjson r "$2" '{id:$r.id,text:.choices[0].message.content}'; }
lane() { while read -r row; do ask "$1" "$row"; done < "$2"; }
serve() { apr serve run "$MODEL" --gpu-layers all --port "$1" >/dev/null 2>&1 & pids+=($!)
  curl -sf --retry 300 --retry-delay 1 --retry-connrefused "localhost:$1/health" >/dev/null
  ask "$1" "$(head -1 prompts.jsonl)" >/dev/null; }   # warm-up, not timed
export -f ask lane; export MAX_TOKENS

# 2 serial: one server, one prompt at a time
serve 8080
tick t1_start; lane 8080 prompts.jsonl > "$W/serial.jsonl"; tick t1_end; T1=$(span_ms t1_start t1_end)
echo "serial   1 server : $T1 ms"
kill "${pids[@]}"; wait; pids=()

# 3 fan-out: four resident servers, one lane per port
for p in 8081 8082 8083 8084; do serve "$p"; done
nvidia-smi --query-compute-apps=pid,used_memory --format=csv
split -n "r/$N" -d prompts.jsonl "$W/part"
tick t4_start
for i in 0 1 2 3; do echo "$((8081 + i)) $W/part0$i"; done |
  xargs -P "$N" -L1 bash -c 'lane $0 $1 > $1.out'
tick t4_end; T4=$(span_ms t4_start t4_end)
echo "fan-out  $N servers: $T4 ms"

# 4 reduce: sort by id, hash the bytes
tick merge_start
jq -sc 'sort_by(.id)' "$W"/part0?.out > "$W/merged.json"
tick merge_end; MERGE_MS=$(span_ms merge_start merge_end)
jq -sc 'sort_by(.id)' "$W/serial.jsonl" > "$W/serial.json"
S_SHA=$(sha256sum < "$W/serial.json" | cut -d' ' -f1)
M_SHA=$(sha256sum < "$W/merged.json" | cut -d' ' -f1)
[[ "$(jq -c '[.[].id]' "$W/merged.json")" == "$(jq -sc '[.[].id] | sort' prompts.jsonl)" ]] ||
  { echo "contract: FAIL fanout-disjoint-v1 (an id is missing or repeated)"; exit 1; }
[[ $S_SHA == "$M_SHA" ]] ||
  { echo "contract: FAIL reduce-deterministic-v1 (serial $S_SHA != fan-out $M_SHA)"; exit 1; }

# 5 Amdahl: S = 1 / (f + (1 - f) / N), solved for the serial fraction f
SPEEDUP=$(bc <<< "scale=3; $T1 / $T4")
F=$(bc <<< "scale=3; (1 / $SPEEDUP - 1 / $N) / (1 - 1 / $N)")
echo "speedup $SPEEDUP x on $N lanes, serial fraction $F, merge $MERGE_MS ms"
echo "contract: fanout-disjoint-v1 + reduce-deterministic-v1 OK"

# receipt: one JSON line appended per run; the receipt file is derived from that line
stamp
RUN_ID=$(at t1_start)-$$ LOG=$OUT/d14-fanout/runs.jsonl DIR=$OUT/d14-fanout/$(hostname)/$PIN
mkdir -p "$DIR"
jq -nc --arg run_id "$RUN_ID" --arg utc "$(at utc)" --argjson pid $$ \
  --arg l0 "$L1_START" --arg l1 "$(load1)" --arg sha "$M_SHA" --arg model "$SHA" \
  --argjson t1 "$T1" --argjson t4 "$T4" --argjson su "$SPEEDUP" --argjson f "$F" --argjson m "$MERGE_MS" \
  '{id:"d14-fanout",run_id:$run_id,utc:$utc,pid:$pid,apr_version:"'$PIN'",model_sha256:$model,
    measured:{t1_ms:$t1,t4_ms:$t4,speedup:$su,serial_fraction:$f,merge_ms:$m,load1_start:($l0|tonumber),
              load1_end:($l1|tonumber),sha256:$sha,prompts:48,lanes:4},
    verdict:{verdict:"Green"}}' >> "$LOG"
tail -1 "$LOG" | jq . > "$DIR/$RUN_ID.json"
echo "receipt: $DIR/$RUN_ID.json"
