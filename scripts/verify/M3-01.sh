#!/usr/bin/env bash
# M3-01: the exact solver (core/src/solver.rs). In release mode:
# - against naive minimax on every position of the first two rounds of
#   SOLVER_GAMES seeded games per mode (30; --fast 2), wherever naive
#   minimax finishes within 50,000,000 nodes, and on near-target score
#   variants up to 18 tiles left, every one of them against naive minimax:
#   0 mismatches; the tiles-left range where naive minimax finished at every
#   position is printed; the full run needs at least 1,000 naive positions
#   and 4,000 variants in 2v2, and 500 and 2,000 in 1v1;
# - with the table on and off at every other position: equal values;
# - the capped fallback: a legal move, the state unchanged, the greedy value;
# - state text round trip, malformed and unreachable lines (position.rs);
# - the allocation test: solver nodes and capped solves counted, 0
#   allocations.
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
grep -q 'test result: ok. 3 passed' "$log" || err "the solver tests did not all run ($log)"
line=$(grep -o 'solver: .*' "$log")
[ -n "$line" ] || err "no solver summary ($log)"
for spec in "2v2 1000 4000" "1v1 500 2000"; do
  set -- $spec
  part=$(printf '%s' "$line" | grep -o "\[$1 games [^]]*\]")
  naive=$(printf '%s' "$part" | sed -n 's/.*naive positions \([0-9]*\) .*/\1/p')
  variants=$(printf '%s' "$part" | sed -n 's/.*near-target variants \([0-9]*\) .*/\1/p')
  case "$part" in *"(0 beyond naive)"*"mismatches 0]") ;; *) err "$1: $part" ;; esac
  if [ "$games" -eq 30 ]; then
    [ "${naive:-0}" -ge "$2" ] || err "$1: $naive naive positions, at least $2 needed"
    [ "${variants:-0}" -ge "$3" ] || err "$1: $variants variants, at least $3 needed"
  else
    [ "${naive:-0}" -gt 0 ] && [ "${variants:-0}" -gt 0 ] || err "$1: nothing checked: $part"
  fi
done
cargo test -q -p dominofish-core --lib position > runs/M3-01/position.log 2>&1 || err "state text tests failed (runs/M3-01/position.log)"
grep -q 'test result: ok. 3 passed' runs/M3-01/position.log || err "the 3 state text tests did not all run"
alloc=$(cargo test -q -p dominofish-core --test alloc 2>&1 | grep -o 'alloc: .*')
case "$alloc" in *"solver nodes "[1-9]*"(capped solves "[1-9]*") allocations 0 (hash check true)") ;; *) err "allocation test: $alloc" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M3-01: PASS ${line#solver: }; capped fallback legal and greedy; state text round trip, malformed and unreachable lines rejected; ${alloc#alloc: }"
else
  echo "M3-01: FAIL"
fi
exit "$fail"
