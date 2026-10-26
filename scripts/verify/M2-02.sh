#!/usr/bin/env bash
# M2-02: the game record format and the engine line protocol, both in
# docs/FORMATS.md, with parse and print round-trip tests over seeded random
# games (both modes, targets 100 and 200, non-canonical moves included) and
# rejection of malformed input at its line. --fast runs the same tests.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/M2-02.log
RUST_TEST_THREADS=4 cargo test -p dominofish-core --test formats -- --show-output > "$log" 2>&1
code=$?
fail=0
[ "$code" -eq 0 ] || { echo "ERROR format tests failed (details in $log)"; fail=1; }
for t in records_round_trip malformed_records_are_rejected_with_their_line protocol_round_trips; do
  grep -q "test $t ... ok" "$log" || { echo "ERROR test $t did not pass"; fail=1; }
done
for h in "## Game records" "## Engine protocol"; do
  grep -qxF "$h" docs/FORMATS.md || { echo "ERROR docs/FORMATS.md lacks $h"; fail=1; }
done
summary=$(grep -o 'formats [a-z]*: .*' "$log" | sed 's/^formats //' | sort | paste -sd ';' -)
if [ "$fail" -eq 0 ]; then
  echo "M2-02: PASS $summary; documented in docs/FORMATS.md"
else
  echo "M2-02: FAIL"
fi
exit "$fail"
