#!/usr/bin/env bash
# spec-proofs.sh [--self-test] <course-5/demos dir>
#
# The pv contract proofs for the D18..D21 run records: exactly 17 checks.
#   validate  x4  pv validate accepts each demo's contract.
#   golden    x4  the golden record is Green: exit 0, verdict Pass, the positive control fired, no
#                 shape left unarmed, no unarmed violation, every W3C SHACL-Core case passed. An
#                 empty or unparseable pv output is not Green.
#   planted   x4  the planted record exits 1, and its sorted findings are byte-equal to
#                 fixtures/receipt.planted.expect.
#   d19       x4  D19's golden record agrees with what it summarises: planted_findings is the line
#                 count of D18's .expect; killed, survived and components_killed are exactly the ids
#                 and components of fixtures/mutants.json.
#   mutants   x1  verify-mutants.sh: 17/17 killed and named, 6/6 survived.
# Any other count fails: a gate over a different set of checks is not this gate.
#
# --self-test copies the four demos' specs and fixtures to a scratch tree and requires the copy to
# pass. It then plants one sabotage per fresh copy. Each SABOTAGES row states exactly what its
# sabotage must do: exit 1 with exactly the named set of FAIL checks, no more and no fewer, or exit 2
# with the named refusal as the last line. A sabotage that changes nothing, passes, or is refused in
# any other way fails the self-test. A gate that cannot fail is not a gate. A sabotage that makes a
# .bin directory in its copy has that directory put first on PATH: that is how a pv shim is planted.
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
KILL_N=17
SURVIVE_N=6
MUTANTS_CHECK="d19 mutants $KILL_N/$KILL_N killed+named, $SURVIVE_N/$SURVIVE_N survived"

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
  real_pv=$(command -v pv) || { echo "spec-proofs: no pv on PATH"; exit 2; }
  st=$(mktemp -d "${TMPDIR:-/tmp}/spec-proofs-selftest.XXXXXX") || exit 2
  outside_git "$st" || exit 2
  base=$st/base
  for d in "${DEMOS[@]}"; do
    mkdir -p "$base/$d"
    cp -r "$demos/$d/spec" "$demos/$d/fixtures" "$base/$d/" || { echo "spec-proofs: cannot copy $d"; exit 2; }
  done
  mkdir -p "$st/tmp"
  prove() { # prove <copy>: run this script on <copy>, with <copy>/.bin first on PATH when it exists
    local p=$PATH
    if [ -d "$1/.bin" ]; then p=$1/.bin:$PATH; fi
    PATH=$p TMPDIR=$st/tmp bash "$here/spec-proofs.sh" "$1" > "$st/last.txt" 2>&1
  }
  fail_set() { # the sorted names of every "PROOF <name> FAIL" line of the last run, joined by ";"
    awk 'index($0, "PROOF ") == 1 && substr($0, 59, 5) == " FAIL" { n = substr($0, 7, 52); sub(/ +$/, "", n); print n }' \
      "$st/last.txt" | LC_ALL=C sort | paste -sd ';' -
  }
  shim() { # shim <copy> <bash>: a pv in <copy>/.bin that runs <bash>, then hands the call to the real pv
    mkdir -p "$1/.bin" && printf '#!/usr/bin/env bash\n%s\nexec %q "$@"\n' "$2" "$real_pv" > "$1/.bin/pv" \
      && chmod +x "$1/.bin/pv"
  }
  edit() { jq "$2" "$1" > "$1.new" && mv "$1.new" "$1"; } # edit <json file> <jq program>
  sabotage() { # sabotage <name> <copy>: plant one defect in <copy>
    local c=$2 t=$2/d19-workflow-ontology/fixtures/mutants.json f
    case $1 in
      swap-d18-golden-planted)
        f=$c/d18-two-agents-one-server/fixtures
        mv "$f/receipt.golden.json" "$f/swap.json" && mv "$f/receipt.planted.json" "$f/receipt.golden.json" \
          && mv "$f/swap.json" "$f/receipt.planted.json" ;;
      d18-expect-empty)
        : > "$c/d18-two-agents-one-server/fixtures/receipt.planted.expect" ;;
      d18-checker-files-unpinned)
        f=$c/d18-two-agents-one-server/spec/d18-run-v1.yaml
        sed 's|{path: d18:checker_files, minCount: 1, in: \[out/verdicts.json\]}|{path: d18:checker_files, minCount: 1}|' \
          "$f" > "$f.new" && mv "$f.new" "$f" ;;
      d20-expect-extra-line)
        echo 'an extra line' >> "$c/d20-agy-app-fanout/fixtures/receipt.planted.expect" ;;
      d20-golden-no-insertText)
        edit "$c/d20-agy-app-fanout/fixtures/receipt.golden.json" '.app_evidence -= ["Input.insertText"]' ;;
      d21-golden-sends-F12)
        edit "$c/d21-agy-app-fanin/fixtures/receipt.golden.json" '.keys_sent += ["F12"]' ;;
      d21-spec-deleted)
        rm -f "${c:?}/d21-agy-app-fanin/spec/d21-run-v1.yaml" ;;
      empty-demos-dir)
        for f in "${DEMOS[@]}"; do rm -rf "${c:?}/${f:?}"; done ;;
      m01-moved-to-survive)
        edit "$t" '.survive += [.kill[] | select(.id == "m01") | {id, why: "sabotage", rust_assert: "none", patch}]
                   | .kill |= map(select(.id != "m01"))' ;;
      m03-vacuous-patch)
        edit "$t" '(.kill[] | select(.id == "m03") | .patch) = [{op: "replace", path: "/q1/decision", value: "accept"}]' ;;
      m07-wrong-component)
        edit "$t" '(.kill[] | select(.id == "m07") | .component) = "in"' ;;
      m07-wrong-property)
        edit "$t" '(.kill[] | select(.id == "m07") | .properties) = ["server_pid_start"]' ;;
      m07-duplicate-row)
        edit "$t" '.kill += [.kill[] | select(.id == "m07")]' ;;
      no-survivors)
        edit "$t" '.survive = []' ;;
      pv-version-0.70.2)
        shim "$c" 'if [ "${1:-}" = --version ]; then echo "pv 0.70.2"; exit 0; fi' ;;
      pv-lint-prints-nothing)
        shim "$c" 'if [ "${1:-}" = lint ]; then exit 0; fi' ;;
      *) return 1 ;;
    esac
  }
  # name|exit|for exit 1, exactly the checks that must FAIL, LC_ALL=C sorted and joined by ";";
  #           for exit 2, the refusal that must be the last line
  #     [|a fixed string the run's output must contain: the reason, not only the check, that failed]
  SABOTAGES=(
    "swap-d18-golden-planted|1|d18-two-agents-one-server golden Green;d18-two-agents-one-server planted == .expect;$MUTANTS_CHECK"
    "d18-expect-empty|1|d18-two-agents-one-server planted == .expect;d19 planted_findings == d18 .expect lines"
    "d18-checker-files-unpinned|1|d18-two-agents-one-server planted == .expect;$MUTANTS_CHECK"
    "d20-expect-extra-line|1|d20-agy-app-fanout planted == .expect"
    "d20-golden-no-insertText|1|d20-agy-app-fanout golden Green"
    "d21-golden-sends-F12|1|d21-agy-app-fanin golden Green"
    "d21-spec-deleted|1|d21-agy-app-fanin golden Green;d21-agy-app-fanin planted == .expect;d21-agy-app-fanin validate"
    "empty-demos-dir|1|d18-two-agents-one-server present;d19 components_killed == mutants.json components;d19 killed == mutants.json kill ids;$MUTANTS_CHECK;d19 planted_findings == d18 .expect lines;d19 survived == mutants.json survive ids;d19-workflow-ontology present;d20-agy-app-fanout present;d21-agy-app-fanin present"
    "m01-moved-to-survive|1|d19 killed == mutants.json kill ids;$MUTANTS_CHECK;d19 survived == mutants.json survive ids"
    "m03-vacuous-patch|1|$MUTANTS_CHECK|m03 PATCH-FAILED-OR-VACUOUS"
    "m07-wrong-component|1|$MUTANTS_CHECK|IMPURE(1 finding(s) not"
    "m07-wrong-property|1|$MUTANTS_CHECK|WRONG-PROPERTY(want [server_pid_start] got [server_pid_end])"
    "m07-duplicate-row|1|d19 killed == mutants.json kill ids;$MUTANTS_CHECK|has a duplicate row id"
    "no-survivors|1|$MUTANTS_CHECK;d19 survived == mutants.json survive ids"
    "pv-version-0.70.2|2|spec-proofs: pv 0.70.2, pinned $PV_PIN; refusing (not measured)"
    "pv-lint-prints-nothing|1|d18-two-agents-one-server golden Green;d18-two-agents-one-server planted == .expect;$MUTANTS_CHECK;d19-workflow-ontology golden Green;d19-workflow-ontology planted == .expect;d20-agy-app-fanout golden Green;d20-agy-app-fanout planted == .expect;d21-agy-app-fanin golden Green;d21-agy-app-fanin planted == .expect"
  )
  failures=0; refused=0
  if prove "$base"; then
    echo "SELF-TEST baseline passes ok"
  else
    echo "SELF-TEST baseline BASELINE-FAILED: $(tail -n 1 "$st/last.txt")"; failures=$((failures + 1))
  fi
  for row in "${SABOTAGES[@]}"; do
    s=${row%%|*}; rest=${row#*|}; wexit=${rest%%|*}; rest=${rest#*|}; want=${rest%%|*}; reason=""
    if [ "$rest" != "$want" ]; then reason=${rest#*|}; fi
    c=$st/$s
    if ! { [ "$wexit" = 1 ] || [ "$wexit" = 2 ]; } || [ -z "$want" ] || { [ "$rest" != "$want" ] && [ -z "$reason" ]; }; then
      echo "SELF-TEST $s BAD-ROW: exit must be 1 or 2, with what it must show, and a reason field, if any, non-empty"; failures=$((failures + 1)); continue
    fi
    cp -r "$base" "$c"
    if ! sabotage "$s" "$c"; then echo "SELF-TEST $s SABOTAGE-FAILED"; failures=$((failures + 1)); continue; fi
    if diff -rq "$base" "$c" > /dev/null 2>&1; then
      echo "SELF-TEST $s SABOTAGE-NOT-APPLIED: the copy is unchanged"; failures=$((failures + 1)); continue
    fi
    prove "$c"; e=$?
    if [ "$wexit" = 2 ]; then got=$(tail -n 1 "$st/last.txt"); else got=$(fail_set); fi
    if [ "$e" -eq 0 ]; then
      echo "SELF-TEST $s PASSED-WRONGLY"; failures=$((failures + 1))
    elif [ "$e" -ne "$wexit" ] || [ "$got" != "$want" ] || { [ -n "$reason" ] && ! grep -qF -- "$reason" "$st/last.txt"; }; then
      echo "SELF-TEST $s REFUSED-WRONGLY: exit $e, want $wexit"
      echo "  got:  [$got]"
      echo "  want: [$want]"
      [ -n "$reason" ] && echo "  reason: [$reason] $(grep -qF -- "$reason" "$st/last.txt" && echo present || echo ABSENT)"
      failures=$((failures + 1))
    else
      echo "SELF-TEST $s refused ok: exit $e [$got]${reason:+ because [$reason]}"; refused=$((refused + 1))
    fi
  done
  echo "self-test: $refused/${#SABOTAGES[@]} sabotages refused exactly as their row states, $failures failing"
  if [ "$failures" -eq 0 ] && [ "$refused" -eq "${#SABOTAGES[@]}" ]; then rm -rf "${st:?}"; exit 0; fi
  echo "spec-proofs: SELF-TEST FAILED; copies kept in $st"
  exit 1
fi

root=$(mktemp -d "${TMPDIR:-/tmp}/spec-proofs.XXXXXX") || exit 2
outside_git "$root" || exit 2
checks=0; failed=0
fits() { # a check name is at most 52 characters: the self-test reads FAIL lines by column
  if [ "${#1}" -gt 52 ]; then echo "spec-proofs: check name over 52 characters: $1"; exit 2; fi
}
ok() { fits "$1"; checks=$((checks + 1)); printf 'PROOF %-52s ok%s\n' "$1" "${2:+ ($2)}"; }
bad() { fits "$1"; checks=$((checks + 1)); failed=$((failed + 1)); printf 'PROOF %-52s FAIL %s\n' "$1" "$2"; }

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
  # An absent field is not a pass: every criterion must be present and hold. `input` makes an empty
  # out.json an error; a plain filter over an empty file prints nothing and exits 0.
  why=$(jq -nr 'input | if .verdict != "Pass" then "verdict \(.verdict)"
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
if [ "$e" -eq 0 ] && grep -qx "killed+named: $KILL_N/$KILL_N" <<< "$out" && grep -qx "survived: $SURVIVE_N/$SURVIVE_N" <<< "$out"; then
  ok "$MUTANTS_CHECK"
else
  # The first row that is not ok says WHY; the two summary lines alone say only how many.
  first=$(grep -m1 -E 'NOT-KILLED|IMPURE|WRONG-|PATCH-FAILED|SURVIVOR-KILLED|IDENTITY-FAILED|^verify-mutants:' <<< "$out")
  bad "$MUTANTS_CHECK" "exit $e: ${first:+$first | }$(tail -n 2 <<< "$out" | tr '\n' ' ')"
fi

echo "spec-proofs: $checks checks, $failed failing"
if [ "$checks" -ne "$EXPECTED_CHECKS" ]; then
  echo "spec-proofs: $checks checks, expected $EXPECTED_CHECKS: a gate over a different set is not this gate"
  failed=$((failed + 1))
fi
if [ "$failed" -eq 0 ]; then rm -rf "${root:?}"; exit 0; fi
echo "spec-proofs: FAILED; run directories kept in $root"
exit 1
