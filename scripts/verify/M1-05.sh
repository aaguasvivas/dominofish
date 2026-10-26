#!/usr/bin/env bash
# M1-05 (BUILD.md M1b): every edge case of capi-rules-spec[2] and the three
# Parra and Trucco deals are named tests. Checks: fixtures/edges.txt equals a
# fresh run of tools/capi-oracle/edges.mjs at the pin; every Capi test cite in
# capi-rules-spec[2] has a row in docs/RULES.md (Edge cases) whose tests exist
# and pass; the three published deals and the opener tie-break tests pass; the
# edge replay has 0 mismatches. --fast runs the same checks (seconds).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
export CAPI_DIR="${CAPI_DIR:-/Users/Adelson/Desktop/personal/capi}"
node=/Users/Adelson/.nvm/versions/node/v20.20.0/bin/node
fail=0
err() { echo "ERROR $*"; fail=1; }
mkdir -p runs

tools/capi-oracle/build.sh > runs/M1-05.build.log 2>&1 || { cat runs/M1-05.build.log; echo "M1-05: FAIL oracle build"; exit 1; }
"$node" tools/capi-oracle/edges.mjs runs/edges-regen.txt > runs/M1-05.edges.log 2>&1 || err "edges.mjs failed (runs/M1-05.edges.log)"
cmp -s fixtures/edges.txt runs/edges-regen.txt || err "fixtures/edges.txt differs from a fresh edges.mjs run"

RUST_TEST_THREADS=4 cargo test -p dominofish-core --test edges -- --show-output --test-threads=4 > runs/M1-05.test.log 2>&1
code=$?
summary=$(grep -o 'edges: scenarios .*' runs/M1-05.test.log)
[ "$code" -eq 0 ] || err "cargo test --test edges failed (runs/M1-05.test.log)"
case "$summary" in *"mismatches 0") ;; *) err "replay: ${summary:-no summary}" ;; esac

checked=$(/usr/bin/python3 scripts/verify/M1-05-map.py) || fail=1
printf '%s\n' "$checked" | grep '^ERROR' | head -10

if [ "$fail" -eq 0 ]; then
  echo "M1-05: PASS $summary (target 0); $(printf '%s\n' "$checked" | tail -1); fixtures/edges.txt identical to a fresh edges.mjs run at 1d8868a"
else
  echo "M1-05: FAIL"
fi
exit "$fail"
