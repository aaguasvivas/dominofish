#!/usr/bin/env bash
# M2-01: the InfoSet. Over seeded random game records in both modes, at every
# ply for every seat: each field equals the true state's public part and the
# seat's own hand, each void and round-1 exclusion is true of the hidden
# hands, and the history matches the record. Records that differ only in
# tiles hidden from the seat, with the same public record (moves, draw
# counts, scores) and own hand, give equal InfoSets (no hidden tile reaches
# an InfoSet field). --fast runs the same tests (seconds).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/M2-01.log
RUST_TEST_THREADS=4 cargo test -p dominofish-core --test infoset -- --nocapture > "$log" 2>&1
code=$?
fail=0
[ "$code" -eq 0 ] || { echo "ERROR infoset tests failed (details in $log)"; fail=1; }
for t in infoset_matches_the_public_state_and_its_deductions_hold infoset_ignores_tiles_hidden_from_its_seat; do
  grep -q "test $t ... ok" "$log" || { echo "ERROR test $t did not pass"; fail=1; }
done
truth=$(grep -o 'infoset truth: .*' "$log")
hidden=$(grep -o 'infoset hidden: .*' "$log")
same=$(printf '%s' "$hidden" | sed 's/.*swaps [0-9]*, \([0-9]*\) with.*/\1/')
[ "${same:-0}" -gt 100 ] || { echo "ERROR only ${same:-0} indistinguishable swaps checked"; fail=1; }
if [ "$fail" -eq 0 ]; then
  echo "M2-01: PASS ${truth#infoset truth: }; ${hidden#infoset hidden: }"
else
  echo "M2-01: FAIL"
fi
exit "$fail"
