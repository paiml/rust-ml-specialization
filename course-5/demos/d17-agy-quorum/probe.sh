#!/usr/bin/env bash
# D17 diff-selection probe (bash + agy + jq). Runs every planted corpus diff through the three
# quorum models, K rounds each, same prompt.txt + schema as quorum.sh, and appends one JSON line
# per run to probe-log.ndjson (append-only, committed; a rerun skips runs already logged). Selection rule: see README.
set -uo pipefail
cd "$(dirname "$0")" || exit 1
K=${1:-5} JOBS=${JOBS:-10} LOG=probe-log.ndjson
SCHEMA=../d04-json-verdict/schemas/verdict.json
MODELS=(gemini-3.8-flash-high claude-sonnet-4-6 gpt-oss-120b-medium)
VALID=$(cat valid.jq)
one() { # one <diff> <model> <round>
  local d=../fixtures/diffs/$1 raw v=NotRun rc
  raw=$(timeout 130 agy -p "$(cat prompt.txt)
$(cat "$d")" --model "$2" --output-format json --json-schema "$SCHEMA" --print-timeout 120s 2>/dev/null)
  rc=$?
  if [[ $rc == 0 ]] && v=$(jq -er "$VALID | .verdict" <<< "$raw" 2>/dev/null); then :; else v=NotRun; fi
  jq -nc --arg d "$1" --arg m "$2" --argjson r "$3" --arg v "$v" --argjson rc "$rc" \
    '{diff:$d, model:$m, round:$r, verdict:$v, exit:$rc}' | tee -a "$LOG" > /dev/null
}
for f in ../fixtures/diffs/planted-*.diff; do
  for ((r = 1; r <= K; r++)); do
    for m in "${MODELS[@]}"; do
      jq -e --arg d "$(basename "$f")" --arg m "$m" --argjson r "$r" -s \
        'any(.[]; .diff == $d and .model == $m and .round == $r)' "$LOG" > /dev/null 2>&1 && continue   # resume
      while (($(jobs -rp | wc -l) >= JOBS)); do wait -n; done
      one "$(basename "$f")" "$m" "$r" &
    done
  done
done
wait
