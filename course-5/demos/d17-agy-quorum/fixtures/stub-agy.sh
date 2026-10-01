#!/usr/bin/env bash
# Stub agy for the D17 falsifier. STUB_MODES="model=mode,..." picks each lane's behavior:
# pass | fail | hang (outlives the lane timeout) | garbage (invalid JSON) | vanish (no output at all)
if [[ ${1:-} == --version ]]; then echo 1.2.14; exit 0; fi
model=
while (($#)); do [[ $1 == --model ]] && model=$2; shift; done
mode=pass
IFS=, read -ra pairs <<< "${STUB_MODES:-}"
for p in "${pairs[@]}"; do [[ ${p%%=*} == "$model" ]] && mode=${p#*=}; done
case $mode in
  pass) echo '{"status":"SUCCESS","structured_output":{"verdict":"PASS","findings":[]}}' ;;
  fail) echo '{"status":"SUCCESS","structured_output":{"verdict":"FAIL","findings":[{"line":3,"class":"off-by-one","why":"stub"}]}}' ;;
  hang) sleep 60 ;;
  garbage) echo 'not json {' ;;
  vanish) rm -f "$(readlink "/proc/$$/fd/1")" ;;
esac
