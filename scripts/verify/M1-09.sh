#!/usr/bin/env bash
# M1-09 (BUILD.md M1f): the allocation-counting test (crates/dominofish-core/
# tests/alloc.rs, run without the test harness) reports 0 heap allocations
# after setup over move generation, play and undo, hashing, perft, and random
# playouts in both modes. --fast runs the same test (seconds).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
out=$(cargo test -q -p dominofish-core --test alloc 2>&1)
code=$?
printf '%s\n' "$out" > runs/M1-09.log
summary=$(printf '%s\n' "$out" | grep -o 'alloc: .*')
case "$summary" in
  *"allocations 0 (hash check true)")
    [ "$code" -eq 0 ] && { echo "M1-09: PASS $summary (target 0 allocations)"; exit 0; } ;;
esac
echo "ERROR ${summary:-no summary} (exit $code, details in runs/M1-09.log)"
echo "M1-09: FAIL"
exit 1
