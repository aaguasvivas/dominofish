#!/usr/bin/env bash
# M2-03: exact count of consistent worlds per decision in both modes
# (core/src/worlds.rs; docs/PLAN.md D11). Checks: the counts of BUILD.md
# Leads (399,072,960 at a 2v2 free lead, 133,024,320 for a 2v2 round-1
# non-opener, C(21,7) = 116,280 for a 1v1 later round, C(20,6) = 38,760 for
# a 1v1 round-1 non-opener after the 6-6), the (7-s)(8-s)/2 void counts, the
# 4-6 worked example, the true world allowed at every decision of seeded
# games, and equality with a brute-force replay oracle on enumerable
# positions, 1v1 no-double openings included. --fast runs the same tests.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/M2-03.log
RUST_TEST_THREADS=4 cargo test -p dominofish-core --test worlds -- --nocapture --test-threads=4 > "$log" 2>&1
code=$?
fail=0
[ "$code" -eq 0 ] || { echo "ERROR world tests failed (details in $log)"; fail=1; }
for t in counts_from_the_leads void_counts_and_the_worked_example the_true_world_is_always_counted counts_equal_a_brute_force_oracle no_double_openings_are_counted_exactly; do
  grep -q "test $t ... ok" "$log" || { echo "ERROR test $t did not pass"; fail=1; }
done
summary=$(grep -o 'worlds [a-z]*: .*' "$log" | sed 's/^worlds //' | sort | paste -sd ';' -)
if [ "$fail" -eq 0 ]; then
  echo "M2-03: PASS $summary"
else
  echo "M2-03: FAIL"
fi
exit "$fail"
