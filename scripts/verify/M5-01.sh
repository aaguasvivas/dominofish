#!/usr/bin/env bash
# M5-01: the AIVAT terms (core/src/aivat.rs; docs/PLAN.md D19) and the
# bots' exact policies. In release mode (AIVAT_FULL=1; --fast: the small
# sizes): each bot's decisions over 64 seeds use exactly its policy; tied
# decisions' action terms sum to 0; one-tile draws and a deal against itself
# give 0; values equal the solver's without a table, whatever ran before.
# With HEAD's runs/bin binary, the first pairs of committed R2 rows replay
# with equal pair lines (six rows: every bot, both modes, rounds and games;
# --fast: two). The allocation test still reports 0.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs/M5-01
fail=0
err() { echo "ERROR $*"; fail=1; }
full=1
[ "${1:-}" = "--fast" ] && full=0
log=runs/M5-01/aivat.log
AIVAT_FULL=$full cargo test -q --release -p dominofish-core --test aivat -- --show-output > "$log" 2>&1 || err "AIVAT tests failed ($log)"
grep -q 'test result: ok. 3 passed' "$log" || err "the AIVAT tests did not all pass ($log)"
lines=$(grep -o 'aivat [a-z]*: .*' "$log" | sed 's/^aivat //' | sort | paste -sd '|' - | sed 's/|/; /g')
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || err "build failed: $bin"
specs=("12 1v1 round 30 rule random" "15 2v2 round 30 rule random" "13 1v1 game 10 rule random" "34 2v2 game 10 rule accompanied" "6 1v1 round 30 accompanied random" "21 2v2 round 30 accompanied heaviest")
[ "$full" -eq 1 ] || specs=("12 1v1 round 10 rule random" "21 2v2 round 10 accompanied heaviest")
replayed=0
for spec in "${specs[@]}"; do
  set -- $spec
  f=$(ls docs/eval/results/2026-10-25-baselines-r2/row-$1-*.txt | head -1)
  "$bin" match --mode "$2" --unit "$3" --target 100 --pairs "$4" --seed $((2000000000 + 100000 * $1)) --engine-seed $((5000100000 + $1)) \
    --sprt 0,4 --sprt-track --a "$bin engine --bot $5" --b "$bin engine --bot $6" --out "runs/M5-01/r2-$1.txt" > /dev/null || err "replay of R2 row $1 failed"
  n=$(grep -c '^pair ' "runs/M5-01/r2-$1.txt")
  [ "$(grep '^pair ' "runs/M5-01/r2-$1.txt")" = "$(grep '^pair ' "$f" | head -"$n")" ] || err "R2 row $1 replays differently"
  replayed=$((replayed + 1))
done
alloc=$(cargo test -q -p dominofish-core --test alloc 2>&1 | grep -o 'alloc: .*')
case "$alloc" in *"allocations 0 (hash check true)") ;; *) err "allocation test: ${alloc:-no summary}" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M5-01: PASS $lines; $replayed committed R2 rows replay with equal pair lines at HEAD; 0 allocations"
else
  echo "M5-01: FAIL"
fi
exit "$fail"
