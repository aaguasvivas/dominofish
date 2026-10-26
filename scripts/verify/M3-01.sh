#!/usr/bin/env bash
# M3-01: the exact solver (core/src/solver.rs). In release mode:
# - against naive minimax on every position of the first two rounds of
#   SOLVER_GAMES seeded games per mode (30; --fast 2), wherever naive
#   minimax finishes within 50,000,000 nodes, and on near-target score
#   variants up to 18 tiles left: 0 mismatches; the tiles-left range where
#   naive minimax finished at every position is printed;
# - with the table on and off at every other position: equal values;
# - state text round trip and malformed lines (core/src/position.rs);
# - the allocation test: solver nodes counted, 0 allocations.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs/M3-01
fail=0
err() { echo "ERROR $*"; fail=1; }
games=30
[ "${1:-}" = "--fast" ] && games=2
log=runs/M3-01/solver.log
SOLVER_GAMES=$games cargo test -q --release -p dominofish-core --test solver -- --include-ignored --nocapture > "$log" 2>&1 || err "solver tests failed ($log)"
grep '^ERROR' "$log" | head -5
line=$(grep -o 'solver: .*' "$log")
[ -n "$line" ] || err "no solver summary ($log)"
for mode in 2v2 1v1; do
  case "$line" in *"[$mode games $games: naive positions "*"mismatches 0]"*) ;; *) err "$mode: $line" ;; esac
done
cargo test -q -p dominofish-core --lib position > runs/M3-01/position.log 2>&1 || err "state text tests failed (runs/M3-01/position.log)"
alloc=$(cargo test -q -p dominofish-core --test alloc 2>&1 | grep -o 'alloc: .*')
case "$alloc" in *"solver nodes "[1-9]*"allocations 0 (hash check true)") ;; *) err "allocation test: $alloc" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M3-01: PASS ${line#solver: }; state text round trip and malformed lines ok; ${alloc#alloc: }"
else
  echo "M3-01: FAIL"
fi
exit "$fail"
