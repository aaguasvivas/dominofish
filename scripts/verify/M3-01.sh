#!/usr/bin/env bash
# M3-01: the exact solver (core/src/solver.rs). In release mode:
# - against naive minimax on every position of the first two rounds of
#   SOLVER_GAMES seeded games per mode (30; --fast 2), wherever naive
#   minimax finishes within 50,000,000 nodes, and on near-target score
#   variants up to 18 tiles left, every one of them against naive minimax:
#   0 mismatches; the tiles-left range where naive minimax finished at every
#   position is printed; the full run needs at least 1,000 naive positions
#   and 4,000 variants in 2v2, and 500 and 2,000 in 1v1;
# - with the table on and off at every other position, and at salida
#   variants (scores 74 and 75) at 27 tiles left in 2v2: equal values; the
#   table stays warm across each round, as in the omniscient bot;
# - a negative control (full run only): a copy of HEAD whose table key
#   drops the score state must fail the same test with mismatches;
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
  case "$part" in *"(0 beyond naive)"*"warm table per round, mismatches 0]") ;; *) err "$1: $part" ;; esac
  if [ "$games" -eq 30 ]; then
    [ "${naive:-0}" -ge "$2" ] || err "$1: $naive naive positions, at least $2 needed"
    [ "${variants:-0}" -ge "$3" ] || err "$1: $variants variants, at least $3 needed"
  else
    [ "${naive:-0}" -gt 0 ] && [ "${variants:-0}" -gt 0 ] || err "$1: nothing checked: $part"
  fi
done
salida=$(printf '%s' "$line" | grep -o '\[2v2 games [^]]*\]' | sed -n 's/.*salida variants at 27 tiles left \([0-9]*\),.*/\1/p')
[ "${salida:-0}" -gt 0 ] || err "no salida variant was checked"
control="skipped (--fast)"
if [ "$games" -eq 30 ]; then
  mut=runs/src/m301-mutant
  rm -rf "$mut"; mkdir -p "$mut"
  /usr/bin/git archive HEAD | tar -x -C "$mut"
  /usr/bin/python3 - "$mut/crates/dominofish-core/src/solver.rs" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
a = "    state.position_hash()\n        ^ BONUS_KEYS[0][bonuses_left(state, 0)]\n        ^ BONUS_KEYS[1][bonuses_left(state, 1)]\n"
assert s.count(a) == 1, "the key function changed; update the negative control"
open(p, "w").write(s.replace(a, "    state.position_hash()\n"))
PY
  find "$mut" -name '*.rs' -exec touch {} +
  (cd "$mut" && SOLVER_GAMES=6 cargo test -q --release -p dominofish-core --test solver --target-dir ../../../target/old -- --ignored --nocapture) > runs/M3-01/mutant.log 2>&1
  rm -rf "$mut"
  caught=$(grep -o 'mismatches [0-9]*' runs/M3-01/mutant.log | awk '{s += $2} END {print s + 0}')
  [ "$caught" -gt 0 ] || err "negative control: a key without the score state passed (runs/M3-01/mutant.log)"
  control="a key without the score state gives $caught mismatches"
fi
cargo test -q -p dominofish-core --lib position > runs/M3-01/position.log 2>&1 || err "state text tests failed (runs/M3-01/position.log)"
grep -q 'test result: ok. 3 passed' runs/M3-01/position.log || err "the 3 state text tests did not all run"
alloc=$(cargo test -q -p dominofish-core --test alloc 2>&1 | grep -o 'alloc: .*')
case "$alloc" in *"solver nodes "[1-9]*"(capped solves "[1-9]*") allocations 0 (hash check true)") ;; *) err "allocation test: $alloc" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M3-01: PASS ${line#solver: }; negative control: $control; capped fallback legal and greedy; state text round trip, malformed and unreachable lines rejected; ${alloc#alloc: }"
else
  echo "M3-01: FAIL"
fi
exit "$fail"
