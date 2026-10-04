#!/usr/bin/env bash
# verify-mutants.sh <d18 demo dir> <mutants.json>
#
# D19's mutant matrix, run against D18's contract with pv's shapes gate.
#   identity  the unpatched golden record passes: exit 0, verdict Pass, no findings.
#   kill      every kill row is rejected (exit 1) with EXACTLY .findings findings, and every finding
#             starts "<node><focus> violates shape `<target>` (<component>)". A mutant that also trips
#             a second constraint, or the right one on the wrong node, is NOT killed: it is impure.
#             Each finding must also name the row's .properties: the first "<ns><prop>" token after
#             the prefix (the constraint's path), or "-" where pv 0.70.1 names none (datatype).
#   survive   every survive row passes (exit 0, verdict Pass, no findings). The shapes cannot see it;
#             the row names the Rust assert that does. A survivor that starts failing is a change in
#             pv's semantics, and it is reported, never absorbed.
# Each judge run is materialised in a fresh directory outside any git work tree (PV-ONT-014).
# ph1 interim: ph2 ports this into `xtask verify --only d19-workflow-ontology`, with these arms as tests.
set -u
usage='usage: verify-mutants.sh <d18 demo dir> <mutants.json>'
d18=${1:?$usage}
table=${2:?$usage}
golden=$d18/fixtures/receipt.golden.json
for f in "$golden" "$table"; do
  [ -f "$f" ] || { echo "verify-mutants: $f: no such file"; exit 2; }
done
node=$(jq -er .node "$table") || { echo "verify-mutants: $table has no .node"; exit 2; }
# A row id that appears twice would count one defect twice; refuse the table outright.
jq -e '[.kill[].id, .survive[].id] | length == (unique | length)' "$table" > /dev/null \
  || { echo "verify-mutants: $table has a duplicate row id"; exit 2; }
ns=${node%/*}/
work=$(mktemp -d "${TMPDIR:-/tmp}/verify-mutants.XXXXXX") || exit 2
if git -C "$work" rev-parse --is-inside-work-tree > /dev/null 2>&1; then
  echo "verify-mutants: $work is inside a git work tree (PV-ONT-014); refusing"
  exit 2
fi

# RFC 6902, the subset the table uses: add (incl. append with "-"), remove, replace.
applier='
def ptr: split("/")[1:] | map(if test("^[0-9]+$") then tonumber else . end);
reduce $ops[] as $o (.;
  ($o.path | ptr) as $p
  | if $o.op == "remove" then delpaths([$p])
    elif $o.op == "add" and ($p[-1] == "-") then setpath($p[:-1]; getpath($p[:-1]) + [$o.value])
    elif $o.op == "add" or $o.op == "replace" then setpath($p; $o.value)
    else error("unsupported op \($o.op)") end)'

judge() { # judge <record> <label>: pv's exit status; findings at $work/<label>/messages.txt
  local run=$work/$2 e
  mkdir -p "$run/spec"
  cp "$d18"/spec/*.yaml "$run/spec/"
  cp "$1" "$run/receipt.json"
  pv lint "$run/spec" --gate shapes --format json > "$run/out.json" 2> "$run/err.txt"
  e=$?
  # No JSON is not "no findings": it is an unmeasured run, and it fails every arm below.
  if ! jq -e '.verdict' "$run/out.json" > /dev/null 2>&1; then echo "no-json" > "$run/verdict.txt"; return 3; fi
  jq -r '.verdict' "$run/out.json" > "$run/verdict.txt"
  jq -r '.findings[]?.message' "$run/out.json" > "$run/messages.txt"
  return "$e"
}

patched() { # patched <ops json> <out>: apply the patch to the golden record; refuse a no-op patch
  jq --argjson ops "$1" "$applier" "$golden" > "$2" || return 1
  ! jq -e --slurpfile g "$golden" '. == $g[0]' "$2" > /dev/null
}

failed=0
judge "$golden" identity; e=$?
nmsg=$(wc -l < "$work/identity/messages.txt" 2> /dev/null || echo missing)
if [ "$e" -eq 0 ] && [ "$(cat "$work/identity/verdict.txt")" = Pass ] && [ "$nmsg" = 0 ]; then
  echo "identity exit=0 findings=0 passes"
else
  echo "identity exit=$e findings=$nmsg IDENTITY-FAILED: the unpatched golden record must pass"
  failed=1
fi

n_kill=$(jq '.kill | length' "$table"); killed=0
for ((i = 0; i < n_kill; i++)); do
  row=$(jq -c ".kill[$i]" "$table")
  id=$(jq -r .id <<< "$row"); comp=$(jq -r .component <<< "$row"); tgt=$(jq -r .target <<< "$row")
  foc=$(jq -r .focus <<< "$row"); want=$(jq -r .findings <<< "$row")
  wantp=$(jq -r '.properties // [] | sort | join(" ")' <<< "$row")
  if ! patched "$(jq -c .patch <<< "$row")" "$work/$id.json"; then
    echo "$id PATCH-FAILED-OR-VACUOUS: the patch errors or leaves the golden record unchanged"; continue
  fi
  judge "$work/$id.json" "$id"; e=$?
  prefix="$node$foc violates shape \`$tgt\` ($comp)"
  got=$(wc -l < "$work/$id/messages.txt" 2> /dev/null || echo 0)
  off=$(awk -v p="$prefix" 'index($0, p) != 1' "$work/$id/messages.txt" 2> /dev/null | wc -l)
  gotp=$(awk -v p="$prefix" -v ns="$ns" '{ s = substr($0, length(p) + 1); i = index(s, ns); if (i == 0) { print "-"; next }
    s = substr(s, i + length(ns)); match(s, /^[A-Za-z0-9_:.-]+/); t = substr(s, 1, RLENGTH); sub(/:$/, "", t); print t }' \
    "$work/$id/messages.txt" 2> /dev/null | LC_ALL=C sort | paste -sd " " -)
  if [ "$e" -eq 1 ] && [ "$got" -eq "$want" ] && [ "$off" -eq 0 ] && [ -n "$wantp" ] && [ "$gotp" = "$wantp" ]; then
    verdict=killed; killed=$((killed + 1))
  elif [ "$e" -ne 1 ]; then verdict="NOT-KILLED(exit $e)"
  elif [ "$off" -ne 0 ]; then verdict="IMPURE($off finding(s) not \"$prefix\")"
  elif [ "$got" -ne "$want" ]; then verdict="WRONG-COUNT(want $want)"
  else verdict="WRONG-PROPERTY(want [$wantp] got [$gotp])"; fi
  printf '%s %-16s %-8s %-4s exit=%s findings=%s %s\n' "$id" "$comp" "$tgt" "${foc:-.}" "$e" "$got" "$verdict"
done

n_surv=$(jq '.survive | length' "$table"); survived=0
for ((i = 0; i < n_surv; i++)); do
  row=$(jq -c ".survive[$i]" "$table")
  id=$(jq -r .id <<< "$row")
  if ! patched "$(jq -c .patch <<< "$row")" "$work/$id.json"; then
    echo "$id PATCH-FAILED-OR-VACUOUS: the patch errors or leaves the golden record unchanged"; continue
  fi
  judge "$work/$id.json" "$id"; e=$?
  got=$(wc -l < "$work/$id/messages.txt" 2> /dev/null || echo missing)
  if [ "$e" -eq 0 ] && [ "$(cat "$work/$id/verdict.txt")" = Pass ] && [ "$got" = 0 ]; then
    survived=$((survived + 1)); echo "$id exit=0 findings=0 survived; caught by: $(jq -r .rust_assert <<< "$row")"
  else
    echo "$id exit=$e findings=$got SURVIVOR-KILLED: the shapes now see what this row says they cannot"
  fi
done

echo "killed+named: $killed/$n_kill"
echo "survived: $survived/$n_surv"
# An empty table is "did not look", not "nothing wrong".
if [ "$failed" -eq 0 ] && [ "$n_kill" -gt 0 ] && [ "$killed" -eq "$n_kill" ] && [ "$n_surv" -gt 0 ] && [ "$survived" -eq "$n_surv" ]; then
  rm -rf "${work:?}"
  exit 0
fi
echo "verify-mutants: FAILED; run directories kept in $work"
exit 1
