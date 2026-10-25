#!/usr/bin/env bash
# M2-04: baseline bots (random, heaviest, accompanied, rule) deciding only
# from their seat's InfoSet. Checks: InfoSet legal moves equal the referee's
# at every decision; every pairing plays full games in both modes at
# targets 100 and 200 with only legal moves (the in-process referee asserts
# legality) and replays identically; each bot's choice maximizes its
# documented criterion (docs/PLAN.md D12). --fast runs the same tests.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/M2-04.log
RUST_TEST_THREADS=4 cargo test -p dominofish-core --test bots -- --nocapture --test-threads=4 > "$log" 2>&1
code=$?
fail=0
[ "$code" -eq 0 ] || { echo "ERROR bot tests failed (details in $log)"; fail=1; }
for t in infoset_legal_moves_equal_the_referees every_pairing_plays_legal_full_games_and_replays_identically each_bot_maximizes_its_criterion; do
  grep -q "test $t ... ok" "$log" || { echo "ERROR test $t did not pass"; fail=1; }
done
summary=$(grep -o 'bots [a-z]*: .*' "$log" | sed 's/^bots //' | sort | paste -sd ';' -)
if [ "$fail" -eq 0 ]; then
  echo "M2-04: PASS $summary"
else
  echo "M2-04: FAIL"
fi
exit "$fail"
