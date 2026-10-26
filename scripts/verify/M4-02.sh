#!/usr/bin/env bash
# M4-02: PIMC analysis (core/src/pimc.rs; docs/PLAN.md D18), in release
# mode: equal analyses from equal seeds; where one world is consistent,
# every move's mean equals its exact value with standard error 0 (at least
# 50 decisions per mode); every move's per-world values equal the solver's
# without a table on the same worlds; equal values with 2^12 and 2^16 table
# entries; over every decision of 20 seeded games per mode (--fast: 2), a
# legal chosen move, every legal move ranked, 0 inconsistent worlds, and 0
# fallbacks; and the allocation test (solver nodes counted) reports 0.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs/M4-02
fail=0
err() { echo "ERROR $*"; fail=1; }
full=1
[ "${1:-}" = "--fast" ] && full=0
log=runs/M4-02/pimc.log
PIMC_FULL=$full cargo test -q --release -p dominofish-core --test pimc -- --nocapture --test-threads=4 > "$log" 2>&1 || err "PIMC tests failed ($log)"
grep -q 'test result: ok. 5 passed' "$log" || err "the PIMC tests did not all pass ($log)"
sweep=$(grep -o 'pimc sweep .*' "$log")
lines=$(grep -o 'pimc [a-z ]*: .*' "$log" | grep -v '^pimc sweep' | sed 's/^pimc //' | sort | paste -sd '|' - | sed 's/|/; /g')
[ -n "$sweep" ] || err "no sweep summary ($log)"
for mode in OneVsOne TwoVsTwo; do
  part=$(printf '%s' "$sweep" | grep -o "\[$mode: [^]]*\]")
  case "$part" in *"inconsistent 0 fallback 0]") ;; *) err "$mode: $part" ;; esac
  [ "$full" -eq 0 ] || case "$part" in *"games 20 "*) ;; *) err "$mode: the sweep did not play 20 games" ;; esac
done
alloc=$(cargo test -q -p dominofish-core --test alloc 2>&1 | grep -o 'alloc: .*')
case "$alloc" in *"allocations 0 (hash check true)") ;; *) err "allocation test: ${alloc:-no summary}" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M4-02: PASS ${sweep#pimc }; $lines; solver allocations 0"
else
  echo "M4-02: FAIL"
fi
exit "$fail"
