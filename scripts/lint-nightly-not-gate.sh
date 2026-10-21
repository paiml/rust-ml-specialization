#!/usr/bin/env bash
# nightly.yml must stay out of the merge gate (Refs #792): no `needs:` or
# workflow_run link from ci.yml to it, no hosted runner, and it must carry
# both `schedule:` and `workflow_dispatch:`.
set -u
wf=.github/workflows
fail=0
for need in 'schedule:' 'workflow_dispatch:'; do
  grep -q "$need" "$wf/nightly.yml" || { echo "FAIL: nightly.yml lacks $need"; fail=1; }
done
grep -Eq 'runs-on:.*(ubuntu|macos|windows)-' "$wf/nightly.yml" && { echo "FAIL: nightly.yml uses a hosted runner"; fail=1; }
grep -Eq 'continue-on-error' "$wf/nightly.yml" && { echo "FAIL: nightly.yml uses continue-on-error"; fail=1; }
grep -Eqi 'nightly' "$wf/ci.yml" && { echo "FAIL: ci.yml references nightly"; fail=1; }
[ "$fail" -eq 0 ] && echo "lint-nightly-not-gate: OK"
exit "$fail"
