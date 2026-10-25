#!/usr/bin/env bash
# M1-08 (BUILD.md M1e): property tests over seeded random playouts in both
# modes: tranque invariant and geometry (locked iff both ends show k and all
# 7 tiles of k are on the board; then at least 10 board tiles and board pips
# at least 6k+42), tile conservation, end-mirror symmetry, seat rotation with
# the opener and automatic tile fixed, line counts unchanged under pip
# relabeling with the opener fixed, determinism, play and undo round trip,
# incremental hash equal to recomputed. --fast runs the same tests (seconds).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/M1-08.log
RUST_TEST_THREADS=4 cargo test -p dominofish-core --lib properties -- --nocapture --test-threads=4 > "$log" 2>&1
code=$?
fail=0
err() { echo "ERROR $*"; fail=1; }
[ "$code" -eq 0 ] || err "property tests failed (details in $log)"
for t in tranque_invariant_and_geometry tile_conservation end_mirror_symmetry seat_rotation_with_opener_fixed \
         line_counts_unchanged_under_pip_relabeling determinism play_undo_round_trip_and_incremental_hash; do
  grep -q "test properties::$t ... ok" "$log" || err "test $t did not pass"
done
line() { grep -o "property $1: .*" "$log" | head -1; }
tranque=$(line tranque)
read -r _ _ _ states _ locks _ _ _ tiles _ _ _ _ _ _ _ margin _ <<< "$tranque"
[ "${locks:-0}" -gt 0 ] || err "no lock reached"
[ "${tiles:-0}" -ge 10 ] || err "a lock with ${tiles:-?} board tiles, target at least 10"
[ "${margin:--1}" -ge 0 ] || err "a lock with board pips below 6k+42"
summary="tranque: $states states, $locks locks, fewest board tiles $tiles (target at least 10), least margin over 6k+42 $margin (target at least 0)"
for p in conservation mirror rotation relabel determinism play-undo; do
  l=$(line "$p")
  [ -n "$l" ] || err "no summary for $p"
  summary="$summary; ${l#property }"
done
if [ "$fail" -eq 0 ]; then
  echo "M1-08: PASS $summary"
else
  echo "M1-08: FAIL"
fi
exit "$fail"
