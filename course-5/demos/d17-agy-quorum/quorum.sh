#!/usr/bin/env bash
# D17 -- one model three times is one reviewer (rfml5 4.2.1). Bash + agy + jq only.
# Provable contract: quorum-one-fail-blocks-v1 + notrun-never-green-v1
# The contract covers the reduction rules, not which model catches the bug:
# what each model says is recorded in the receipt, never asserted.
set -uo pipefail
cd "$(dirname "$0")"
PIN=1.2.14 TIMEOUT=120s
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

PROMPT="You are a code reviewer. Do not run commands or tools; answer only from the diff. Verdict FAIL if the diff has a defect, with one finding per defect. Diff:
$(cat "$DIFF")"
OUT=$(mktemp -d); trap 'rm -rf "$OUT"' EXIT

lane() { # lane <act> <idx> <model>: raw json -> $OUT/<act><idx>.json, exit code + wall ms alongside
  local t0=$(date +%s%N)
  timeout 130 agy -p "$PROMPT" --model "$3" --output-format json --json-schema "$SCHEMA" \
    --print-timeout "$TIMEOUT" > "$OUT/$1$2.json" 2>/dev/null
  echo "$? $(( ($(date +%s%N) - t0) / 1000000 ))" > "$OUT/$1$2.rc"
}
# validated lane -> {model,verdict,findings,wall_ms}; anything else is NotRun
VALID='(.structured_output // empty) | select(
  (keys | sort) == ["findings","verdict"] and (.verdict | IN("PASS","FAIL")) and
  (.findings | type == "array" and all(.[]; (keys | sort) == ["class","line","why"] and
    (.line | type == "number" and floor == . and . >= 1) and (.class | type == "string" and length > 0) and
    (.why | type == "string" and length > 0))))'
judge() { # judge <act> <idx> <model>
  read -r rc ms < "$OUT/$1$2.rc"
  local v=NotRun n=0
  if [[ $rc == 0 ]] && jq -e "$VALID" "$OUT/$1$2.json" > /dev/null 2>&1; then
    v=$(jq -r "$VALID | .verdict" "$OUT/$1$2.json"); n=$(jq "$VALID | .findings | length" "$OUT/$1$2.json")
  fi
  jq -n --arg m "$3" --arg v "$v" --argjson n "$n" --argjson ms "$ms" --argjson rc "$rc" \
    '{model:$m, verdict:$v, findings:$n, wall_ms:$ms, exit:$rc}' > "$OUT/$1$2.lane"
}
run_act() { # run_act <act> <models...> ; prints verdicts side by side
  local act=$1; shift; local i=0
  for m in "$@"; do lane "$act" $i "$m" & i=$((i + 1)); done; wait
  i=0; for m in "$@"; do judge "$act" $i "$m"; i=$((i + 1)); done
  jq -s . "$OUT/$act"?.lane > "$OUT/$act.lanes"
  jq -r '.[] | "  \(.model | .[0:26] | . + " " * (27 - length))\(.verdict)  findings=\(.findings)  \(.wall_ms)ms"' "$OUT/$act.lanes"
}
# 4 reduce: one FAIL blocks; NotRun never counts as PASS
REDUCE='if any(.[]; . == "FAIL") then "FAIL" elif any(.[]; . == "NotRun") then "NotRun" else "PASS" end'
reduce() { jq -r "map(.verdict) | $REDUCE" "$1"; }

echo "Act 1 homogeneous: $SAME x3"; run_act a "$SAME" "$SAME" "$SAME"
echo "Act 2 heterogeneous:"; run_act b "${MIXED[@]}"
q1=$(reduce "$OUT/a.lanes"); q2=$(reduce "$OUT/b.lanes")
echo "quorum: homogeneous=$q1 heterogeneous=$q2"

# contract: reduction rules on synthetic verdicts, independent of any model
red() { echo "$1" | jq -r "$REDUCE"; }
[[ $(red '["PASS","PASS","FAIL"]') == FAIL ]] || { echo "contract: FAIL quorum-one-fail-blocks-v1"; exit 1; }
[[ $(red '["PASS","NotRun","FAIL"]') == FAIL ]] || { echo "contract: FAIL quorum-one-fail-blocks-v1"; exit 1; }
[[ $(red '["PASS","PASS","NotRun"]') != PASS ]] || { echo "contract: FAIL notrun-never-green-v1"; exit 1; }
[[ $(red '["NotRun","NotRun","NotRun"]') != PASS ]] || { echo "contract: FAIL notrun-never-green-v1"; exit 1; }
[[ $(red '["PASS","PASS","PASS"]') == PASS ]] || { echo "contract: FAIL reduction of all-PASS"; exit 1; }
echo "contract: quorum-one-fail-blocks-v1 + notrun-never-green-v1 OK"

# 5 receipt
jq -n --arg run "d17-$(date -u +%Y%m%dT%H%M%SZ)-$$" --argjson pid $$ --arg utc "$(date -u +%FT%TZ)" \
  --argjson load1 "$load1" --arg agy "$have" --arg q1 "$q1" --arg q2 "$q2" \
  --slurpfile a "$OUT/a.lanes" --slurpfile b "$OUT/b.lanes" \
  '{run_id:$run, pid:$pid, utc:$utc, load1:$load1, agy_version:$agy, planted_diff:"planted-01.diff",
    homogeneous:{models:($a[0]|map(.model)), lanes:$a[0], quorum:$q1},
    heterogeneous:{models:($b[0]|map(.model)), lanes:$b[0], quorum:$q2},
    planted_defect_caught:{homogeneous:($q1=="FAIL"), heterogeneous:($q2=="FAIL")}}'
