#!/usr/bin/env bash
# spec-proofs.sh [--self-test] <course-5/demos dir>
#
# The pv contract proofs for the D18..D21 run records: exactly 17 checks.
#   validate  x4  pv validate accepts each demo's contract.
#   golden    x4  the golden record is Green: exit 0, verdict Pass, the positive control fired, no
#                 shape left unarmed, no unarmed violation, every W3C SHACL-Core case passed.
#   planted   x4  the planted record exits 1, and its sorted findings are byte-equal to
#                 fixtures/receipt.planted.expect.
#   d19       x4  D19's golden record agrees with what it summarises: planted_findings is the line
#                 count of D18's .expect; killed, survived and components_killed are exactly the ids
#                 and components of fixtures/mutants.json.
#   mutants   x1  verify-mutants.sh: 16/16 killed and named, 5/5 survived.
# Any other count fails: a gate over a different set of checks is not this gate.
#
# --self-test copies the four demos' specs and fixtures to a scratch tree and requires the copy to
# pass. It then plants one sabotage per fresh copy. Each must make this script exit 1 AND fail the
# check that sabotage targets; a sabotage that changes nothing, passes, or is refused by some other
# check fails the self-test. A gate that cannot fail is not a gate.
#
# The fixtures test the contracts, not a run: they are hand-built records, and they are not
# evidence that any demo ran. Every judge run is materialised outside any git work tree
# (PV-ONT-014). pv must be 0.70.1.
# ph1 interim: ph2 ports this into `xtask verify --only`, with the sabotages as tests, and deletes it.
set -u
usage='usage: spec-proofs.sh [--self-test] <course-5/demos dir>'
self_test=0
if [ "${1:-}" = --self-test ]; then self_test=1; shift; fi
demos=${1:?$usage}
here=$(cd "$(dirname "$0")" && pwd)
DEMOS=(d18-two-agents-one-server d19-workflow-ontology d20-agy-app-fanout d21-agy-app-fanin)
PV_PIN=0.70.1
EXPECTED_CHECKS=17

pv_version=$(pv --version 2> /dev/null | awk 'NR == 1 { print $2 }')
if [ "$pv_version" != "$PV_PIN" ]; then
  echo "spec-proofs: pv ${pv_version:-absent}, pinned $PV_PIN; refusing (not measured)"
  exit 2
fi

outside_git() { # outside_git <dir>: a run root inside a git work tree is refused (PV-ONT-014)
  if git -C "$1" rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    echo "spec-proofs: $1 is inside a git work tree (PV-ONT-014); refusing"
    return 1
  fi
}

if [ "$self_test" -eq 1 ]; then
  st=$(mktemp -d "${TMPDIR:-/tmp}/spec-proofs-selftest.XXXXXX") || exit 2
  outside_git "$st" || exit 2
  base=$st/base
  for d in "${DEMOS[@]}"; do
    mkdir -p "$base/$d"
    cp -r "$demos/$d/spec" "$demos/$d/fixtures" "$base/$d/" || { echo "spec-proofs: cannot copy $d"; exit 2; }
  done
  mkdir -p "$st/tmp"
  prove() { TMPDIR=$st/tmp bash "$here/spec-proofs.sh" "$1" > "$st/last.txt" 2>&1; }
  failed_check() { # failed_check <name>: the last run printed "PROOF <name> ... FAIL"
    awk -v p="PROOF $1 " 'index($0, p) == 1 && / FAIL( |$)/ { f = 1 } END { exit !f }' "$st/last.txt"
  }
  sabotage() { # sabotage <name> <copy>: plant one defect in <copy>
    local c=$2 t=$2/d19-workflow-ontology/fixtures/mutants.json f
    case $1 in
      swap-d18-golden-planted)
        f=$c/d18-two-agents-one-server/fixtures
        mv "$f/receipt.golden.json" "$f/swap.json" && mv "$f/receipt.planted.json" "$f/receipt.golden.json" \
          && mv "$f/swap.json" "$f/receipt.planted.json" ;;
      d20-expect-extra-line)
        echo 'an extra line' >> "$c/d20-agy-app-fanout/fixtures/receipt.planted.expect" ;;
      empty-demos-dir)
        for f in "${DEMOS[@]}"; do rm -rf "${c:?}/${f:?}"; done ;;
      m07-wrong-component)
        jq '(.kill[] | select(.id == "m07") | .component) = "in"' "$t" > "$t.new" && mv "$t.new" "$t" ;;
      m03-vacuous-patch)
        jq '(.kill[] | select(.id == "m03") | .patch) = [{op: "replace", path: "/q1/decision", value: "accept"}]' \
          "$t" > "$t.new" && mv "$t.new" "$t" ;;
      m01-moved-to-survive)
        jq '.survive += [.kill[] | select(.id == "m01") | {id, why: "sabotage", rust_assert: "none", patch}]
            | .kill |= map(select(.id != "m01"))' "$t" > "$t.new" && mv "$t.new" "$t" ;;
      no-survivors)
        jq '.survive = []' "$t" > "$t.new" && mv "$t.new" "$t" ;;
      d18-checker-files-unpinned)
        f=$c/d18-two-agents-one-server/spec/d18-run-v1.yaml
        sed 's|{path: d18:checker_files, minCount: 1, in: \[out/verdicts.json\]}|{path: d18:checker_files, minCount: 1}|' \
          "$f" > "$f.new" && mv "$f.new" "$f" ;;
      *) return 1 ;;
    esac
  }
  # name, then the check that sabotage must fail
  SABOTAGES=(
    "swap-d18-golden-planted|d18-two-agents-one-server golden Green"
    "d20-expect-extra-line|d20-agy-app-fanout planted == .expect"
    "empty-demos-dir|d18-two-agents-one-server present"
    "m07-wrong-component|d19 mutants 16/16 killed+named, 5/5 survived"
    "m03-vacuous-patch|d19 mutants 16/16 killed+named, 5/5 survived"
    "m01-moved-to-survive|d19 killed == mutants.json kill ids"
    "no-survivors|d19 survived == mutants.json survive ids"
    "d18-checker-files-unpinned|d18-two-agents-one-server planted == .expect"
  )
  failures=0; refused=0
  if prove "$base"; then
    echo "SELF-TEST baseline passes ok"
  else
    echo "SELF-TEST baseline BASELINE-FAILED: $(tail -n 1 "$st/last.txt")"; failures=$((failures + 1))
  fi
  for row in "${SABOTAGES[@]}"; do
    s=${row%%|*}; want=${row#*|}; c=$st/$s
    cp -r "$base" "$c"
    if ! sabotage "$s" "$c"; then echo "SELF-TEST $s SABOTAGE-FAILED"; failures=$((failures + 1)); continue; fi
    if diff -rq "$base" "$c" > /dev/null 2>&1; then
      echo "SELF-TEST $s SABOTAGE-NOT-APPLIED: the copy is unchanged"; failures=$((failures + 1)); continue
    fi
    prove "$c"; e=$?
    if [ "$e" -eq 0 ]; then
      echo "SELF-TEST $s PASSED-WRONGLY"; failures=$((failures + 1))
    elif [ "$e" -ne 1 ] || ! failed_check "$want"; then
      echo "SELF-TEST $s REFUSED-FOR-ANOTHER-REASON (exit $e; want a FAIL on \"$want\")"; failures=$((failures + 1))
    else
      echo "SELF-TEST $s refused ok by \"$want\""; refused=$((refused + 1))
    fi
  done
  echo "self-test: $refused/${#SABOTAGES[@]} sabotages refused by their check, $failures failing"
  if [ "$failures" -eq 0 ] && [ "$refused" -eq "${#SABOTAGES[@]}" ]; then rm -rf "${st:?}"; exit 0; fi
  echo "spec-proofs: SELF-TEST FAILED; copies kept in $st"
  exit 1
fi

root=$(mktemp -d "${TMPDIR:-/tmp}/spec-proofs.XXXXXX") || exit 2
outside_git "$root" || exit 2
checks=0; failed=0
ok() { checks=$((checks + 1)); printf 'PROOF %-52s ok%s\n' "$1" "${2:+ ($2)}"; }
bad() { checks=$((checks + 1)); failed=$((failed + 1)); printf 'PROOF %-52s FAIL %s\n' "$1" "$2"; }

judge() { # judge <demo dir> <record> <run dir>: pv's exit status; its JSON at <run dir>/out.json
  mkdir -p "$3/spec"
  if ! cp "$1"/spec/*.yaml "$3/spec/" || ! cp "$2" "$3/receipt.json"; then return 3; fi
  pv lint "$3/spec" --gate shapes --format json > "$3/out.json" 2> "$3/err.txt"
}

for d in "${DEMOS[@]}"; do
  dir=$demos/$d
  if [ ! -d "$dir/spec" ] || [ ! -d "$dir/fixtures" ]; then bad "$d present" "no spec or fixtures dir"; continue; fi
  for f in "$dir"/spec/*.yaml; do
    if pv validate "$f" > "$root/validate-$d.txt" 2>&1; then ok "$d validate"; else bad "$d validate" "pv validate exit $?"; fi
  done

  judge "$dir" "$dir/fixtures/receipt.golden.json" "$root/$d-golden"; e=$?
  # An absent field is not a pass: every criterion must be present and hold.
  why=$(jq -r 'if .verdict != "Pass" then "verdict \(.verdict)"
               elif .pc_shape != "fired" then "pc_shape \(.pc_shape)"
               elif (.not_armed_shapes | type) != "array" or (.not_armed_shapes | length) != 0 then "not armed \(.not_armed_shapes)"
               elif .unarmed_violations != 0 then "unarmed \(.unarmed_violations)"
               elif (.w3c_cases_n // 0) == 0 or .w3c_cases_passed != .w3c_cases_n then "w3c \(.w3c_cases_passed)/\(.w3c_cases_n)"
               else "" end' "$root/$d-golden/out.json" 2> /dev/null) || why="no json"
  if [ "$e" -eq 0 ] && [ -z "$why" ]; then ok "$d golden Green"; else bad "$d golden Green" "exit $e $why"; fi

  judge "$dir" "$dir/fixtures/receipt.planted.json" "$root/$d-planted"; e=$?
  jq -r '.findings[]?.message' "$root/$d-planted/out.json" 2> /dev/null | LC_ALL=C sort > "$root/$d-planted/messages.txt"
  if [ "$e" -ne 1 ]; then bad "$d planted == .expect" "exit $e, want 1"
  elif [ ! -s "$root/$d-planted/messages.txt" ]; then bad "$d planted == .expect" "exit 1 with no findings"
  elif cmp -s "$root/$d-planted/messages.txt" "$dir/fixtures/receipt.planted.expect"; then
    ok "$d planted == .expect" "$(wc -l < "$dir/fixtures/receipt.planted.expect") findings"
  else bad "$d planted == .expect" "diff $root/$d-planted/messages.txt $dir/fixtures/receipt.planted.expect"; fi
done

# D19's golden record summarises runs anyone can repeat; it must agree with what it summarises.
d19g=$demos/d19-workflow-ontology/fixtures/receipt.golden.json
table=$demos/d19-workflow-ontology/fixtures/mutants.json
d18x=$demos/d18-two-agents-one-server/fixtures/receipt.planted.expect
cross() { # cross <name> <jq test over $g, the D19 golden record, and $t, the mutant table>
  if jq -en --slurpfile G "$d19g" --slurpfile T "$table" '($G | first) as $g | ($T | first) as $t | '"$2" > /dev/null 2>&1; then
    ok "$1"
  else bad "$1" "disagrees, or a file is missing"; fi
}
n18=$(wc -l < "$d18x" 2> /dev/null || echo 0)
cross "d19 planted_findings == d18 .expect lines" "$n18 > 0 and \$g.planted_findings == $n18"
cross "d19 killed == mutants.json kill ids" '($t.kill | length) > 0 and ($g.killed | sort) == ([$t.kill[].id] | sort)'
cross "d19 survived == mutants.json survive ids" '($t.survive | length) > 0 and ($g.survived | sort) == ([$t.survive[].id] | sort)'
cross "d19 components_killed == mutants.json components" '($g.components_killed | sort) == ([$t.kill[].component] | unique)'

out=$(bash "$here/verify-mutants.sh" "$demos/d18-two-agents-one-server" "$table" 2>&1); e=$?
if [ "$e" -eq 0 ] && grep -qx 'killed+named: 16/16' <<< "$out" && grep -qx 'survived: 5/5' <<< "$out"; then
  ok "d19 mutants 16/16 killed+named, 5/5 survived"
else bad "d19 mutants 16/16 killed+named, 5/5 survived" "exit $e: $(tail -n 2 <<< "$out" | tr '\n' ' ')"; fi

echo "spec-proofs: $checks checks, $failed failing"
if [ "$checks" -ne "$EXPECTED_CHECKS" ]; then
  echo "spec-proofs: $checks checks, expected $EXPECTED_CHECKS: a gate over a different set is not this gate"
  failed=$((failed + 1))
fi
if [ "$failed" -eq 0 ]; then rm -rf "${root:?}"; exit 0; fi
echo "spec-proofs: FAILED; run directories kept in $root"
exit 1
