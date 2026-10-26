#!/usr/bin/env bash
# M3-02: dominofish bench solves fixtures/bench.txt (16 full-round
# positions per mode, checked by tiles left) at the default limits shown in
# its header (10,000,000 nodes per solve, a 2^20-entry table of 16 MB, one
# thread) and prints Bench: <nodes> last; two runs
# print the same number; check.sh runs the same comparison. --fast is the
# same check.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs/M3-02
fail=0
err() { echo "ERROR $*"; fail=1; }
cargo build -q --release -p dominofish 2> runs/M3-02/build.log || { echo "ERROR build failed"; echo "M3-02: FAIL"; exit 1; }
n2=$(grep -c ' mode=2v2 ' fixtures/bench.txt)
n1=$(grep -c ' mode=1v1 ' fixtures/bench.txt)
[ "$n2" -eq 16 ] && [ "$n1" -eq 16 ] || err "fixtures/bench.txt holds $n2 2v2 and $n1 1v1 positions, not 16 each"
target/release/dominofish bench > runs/M3-02/bench-1.txt || err "bench exited non-zero"
target/release/dominofish bench > runs/M3-02/bench-2.txt || err "bench exited non-zero"
a=$(tail -1 runs/M3-02/bench-1.txt)
b=$(tail -1 runs/M3-02/bench-2.txt)
case "$a" in "Bench: "[0-9]*) ;; *) err "the last line is not Bench: <nodes>: $a" ;; esac
[ "$a" = "$b" ] || err "two runs differ: $a and $b"
grep -q '^DominoFish .* bench   32 positions   node cap 10,000,000 per solve   table 16 MB$' runs/M3-02/bench-1.txt || err "the bench header does not show 32 positions, a 10,000,000-node cap, and a 16 MB table: $(head -1 runs/M3-02/bench-1.txt)"
shallow=$(awk '$1 ~ /^[0-9]+$/ && $2 ~ /v/ && $3 < 27' runs/M3-02/bench-1.txt | wc -l | tr -d ' ')
[ "$shallow" -eq 0 ] || err "$shallow bench positions are not full rounds (27 or 28 tiles left)"
capped=$(grep -o 'Capped solves   : [0-9]*' runs/M3-02/bench-1.txt)
ms=$(grep -o 'Total time (ms) : [0-9]*' runs/M3-02/bench-1.txt)
grep -q '^step bench bench_twice$' scripts/check.sh || err "check.sh does not run the bench comparison"
if [ "$fail" -eq 0 ]; then
  echo "M3-02: PASS $a on two runs ($n2 2v2 and $n1 1v1 positions, ${capped#*: } capped, ${ms#*: } ms, load $(uptime | sed 's/.*load averages*: *//; s/,* .*//')); check.sh compares two runs"
else
  echo "M3-02: FAIL"
fi
exit "$fail"
