#!/usr/bin/env bash
# D17 -- one model three times is one reviewer (rfml5 4.2.1). Bash + agy + jq only.
# Provable contract: quorum-one-fail-blocks-v1 + notrun-never-green-v1
# The contract covers the reduction rules, not which model catches the bug:
# what each model says is recorded in the receipt, never asserted.
# Clocks only ever go to append-only logs (tee -a); the receipt is derived from them.
set -uo pipefail
cd "$(dirname "$0")" || exit 1
PIN=1.2.14 TIMEOUT=${LANE_TIMEOUT_S:-120} GRACE=${LANE_GRACE_S:-10}
DIFF=../fixtures/diffs/planted-01.diff SCHEMA=../d04-json-verdict/schemas/verdict.json
DIFF_SHA=2de769e98ab5588e3bba95f5edf656c99f81c0f0554e6f62d14f1f5a9aa0ff73
SCHEMA_SHA=c26af7566343260f383163047f7590f060bfe447ff9c90835f9996ed0fefe35f
SAME=gemini-3.8-flash-high
MIXED=(gemini-3.8-flash-high claude-sonnet-4-6 gpt-oss-120b-medium)
die() { echo "preflight: FAIL $*"; exit 1; }

# 1 preflight
have=$(agy --version 2>/dev/null) || die "agy not runnable"
[[ $have == "$PIN" ]] || die "agy $have != pin $PIN"
[[ $(sha256sum < "$DIFF" | cut -d' ' -f1) == "$DIFF_SHA" ]] || die "fixture sha256 drift"
[[ $(sha256sum < "$SCHEMA" | cut -d' ' -f1) == "$SCHEMA_SHA" ]] || die "schema sha256 drift"
load1=$(cut -d' ' -f1 /proc/loadavg)
awk -v l="$load1" -v n="$(nproc)" 'BEGIN{exit !(l < n)}' || die "load1 $load1 >= $(nproc) cores"
echo "preflight: OK agy=$have load1=$load1"

PROMPT="$(cat prompt.txt)
$(cat "$DIFF")"   # same prompt text + schema for every lane
OUT=$(mktemp -d "${TMPDIR:-/tmp}/d17.XXXXXX") || die "mktemp failed"; trap 'rm -rf "${OUT:?}"' EXIT
LOG=$OUT/receipt.log   # append-only: one JSON line per lane, one for the run
date -u +%FT%TZ | tee -a "$OUT/utc" > /dev/null

lane() { # lane <act> <idx> <model>: raw json -> $OUT/<act><idx>.json; start/end ns appended to .t
  date +%s%N | tee -a "$OUT/$1$2.t" > /dev/null
  timeout $((TIMEOUT + GRACE)) agy -p "$PROMPT" --model "$3" --output-format json --json-schema "$SCHEMA" \
    --print-timeout "${TIMEOUT}s" > "$OUT/$1$2.json" 2>/dev/null
  echo "$?" >> "$OUT/$1$2.rc"
  date +%s%N | tee -a "$OUT/$1$2.t" > /dev/null
}
# validated lane -> verdict; anything else (non-zero exit, timeout, missing, off-schema) is NotRun
VALID=$(cat valid.jq)
judge() { # judge <act> <idx> <model>: appends the lane's JSON line to the receipt log
  local rc v=NotRun n=0 ms
  read -r rc < "$OUT/$1$2.rc" 2>/dev/null || rc=missing
  if [[ $rc == 0 ]] && jq -e "$VALID" "$OUT/$1$2.json" > /dev/null 2>&1; then
    v=$(jq -r "$VALID | .verdict" "$OUT/$1$2.json"); n=$(jq "$VALID | .findings | length" "$OUT/$1$2.json")
  fi
  ms=$(jq -s '((.[1] - .[0]) / 1000000) | floor' "$OUT/$1$2.t" 2>/dev/null || echo 0)
  jq -nc --arg act "$1" --arg m "$3" --arg v "$v" --argjson n "$n" --argjson ms "$ms" --arg rc "$rc" \
    '{act:$act, model:$m, verdict:$v, findings:$n, wall_ms:$ms, exit:$rc}' >> "$LOG"
}
lanes() { jq -sc --arg a "$1" '[.[] | select(.act == $a)]' "$LOG"; }
run_act() { # run_act <act> <models...> ; prints verdicts side by side
  local act=$1; shift; local i=0
  for m in "$@"; do lane "$act" $i "$m" & i=$((i + 1)); done; wait
  i=0; for m in "$@"; do judge "$act" $i "$m"; i=$((i + 1)); done
  lanes "$act" | jq -r '.[] | "  \(.model | .[0:26] | . + " " * (27 - length))\(.verdict)  findings=\(.findings)  \(.wall_ms)ms"'
}
# 4 reduce: one FAIL blocks; NotRun never counts as PASS
REDUCE='if any(.[]; . == "FAIL") then "FAIL" elif any(.[]; . == "NotRun") then "NotRun" else "PASS" end'
reduce() { lanes "$1" | jq -r "map(.verdict) | $REDUCE"; }

echo "Act 1 homogeneous: $SAME x3"; run_act a "$SAME" "$SAME" "$SAME"
echo "Act 2 heterogeneous:"; run_act b "${MIXED[@]}"
q1=$(reduce a); q2=$(reduce b)
echo "quorum: homogeneous=$q1 heterogeneous=$q2"

# contract: reduction rules on synthetic verdicts, independent of any model
red() { echo "$1" | jq -r "$REDUCE"; }
[[ $(red '["PASS","PASS","FAIL"]') == FAIL ]] || { echo "contract: FAIL quorum-one-fail-blocks-v1"; exit 1; }
[[ $(red '["PASS","NotRun","FAIL"]') == FAIL ]] || { echo "contract: FAIL quorum-one-fail-blocks-v1"; exit 1; }
[[ $(red '["PASS","PASS","NotRun"]') != PASS ]] || { echo "contract: FAIL notrun-never-green-v1"; exit 1; }
[[ $(red '["NotRun","NotRun","NotRun"]') != PASS ]] || { echo "contract: FAIL notrun-never-green-v1"; exit 1; }
[[ $(red '["PASS","PASS","PASS"]') == PASS ]] || { echo "contract: FAIL reduction of all-PASS"; exit 1; }
echo "contract: quorum-one-fail-blocks-v1 + notrun-never-green-v1 OK"

# 5 receipt, derived from the append-only log
read -r utc < "$OUT/utc"
jq -s --arg run "d17-${utc//[-:]/}-$$" --argjson pid $$ --arg utc "$utc" --argjson load1 "$load1" \
  --arg agy "$have" --arg q1 "$q1" --arg q2 "$q2" '
  (map(select(.act == "a") | del(.act))) as $a | (map(select(.act == "b") | del(.act))) as $b |
  {run_id:$run, pid:$pid, utc:$utc, load1:$load1, agy_version:$agy, planted_diff:"planted-01.diff",
   homogeneous:{models:($a|map(.model)), lanes:$a, quorum:$q1},
   heterogeneous:{models:($b|map(.model)), lanes:$b, quorum:$q2},
   planted_defect_caught:{homogeneous:($q1=="FAIL"), heterogeneous:($q2=="FAIL")}}' "$LOG"
