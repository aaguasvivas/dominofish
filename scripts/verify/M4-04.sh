#!/usr/bin/env bash
# M4-04: never-peeks for PIMC (crates/dominofish-core/tests/never_peeks.rs,
# pimc_never_peeks). At decisions of rule-bot games in both modes with at
# most 14 tiles left, hidden tiles rearranged behind an identical public
# record give a byte-identical InfoSet and the same PIMC Analysis at a fixed
# seed: the decision, every move's values, and the node counts; at least
# 100 rearranged worlds per mode. --fast runs the same test (seconds).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/M4-04.log
RUST_TEST_THREADS=4 cargo test -p dominofish-core --test never_peeks pimc_never_peeks -- --show-output > "$log" 2>&1
code=$?
summary=$(grep -o 'never-peeks pimc: .*' "$log")
if [ "$code" -eq 0 ] && grep -q "test pimc_never_peeks ... ok" "$log" && [ -n "$summary" ]; then
  echo "M4-04: PASS ${summary#never-peeks pimc: }"
  exit 0
fi
echo "ERROR PIMC never-peeks failed (details in $log)"
echo "M4-04: FAIL"
exit 1
