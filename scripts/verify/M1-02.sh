#!/usr/bin/env bash
# M1-02: the bitboard core plays and undoes moves exactly. Over 1,000 seeded
# random games per mode with raw moves (canonical ones plus flipped leads and
# right-end plays on equal ends): tile conservation, incremental hash equal
# to recomputed, and undo restoring the previous state after every move; the
# referee rejects illegal moves and takes Capi-ordered deals. --fast runs the
# same tests (about 3 seconds).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/M1-02.log
if ! RUST_TEST_THREADS=4 cargo test -p dominofish-core -- --nocapture --test-threads=4 > "$log" 2>&1; then
  grep -E 'panicked|FAILED|error' "$log" | head -5
  echo "M1-02: FAIL (details in $log)"
  exit 1
fi
summary=$(grep -o 'play-undo: games [0-9]* moves [0-9]* ok' "$log")
games=$(printf '%s' "$summary" | awk '{print $3}')
moves=$(printf '%s' "$summary" | awk '{print $5}')
for t in play_undo_round_trip_and_hash_in_both_modes from_tiles_rejects_a_repeated_tile rejects_what_capi_rejects; do
  grep -q "test .*$t ... ok" "$log" || { echo "ERROR test $t did not pass"; echo "M1-02: FAIL"; exit 1; }
done
if [ "${games:-0}" -lt 2000 ]; then
  echo "ERROR play-undo covered ${games:-0} games, target 2000"
  echo "M1-02: FAIL"
  exit 1
fi
echo "M1-02: PASS play, undo, and hash exact over $games seeded games (target 2000, 1000 per mode), $moves moves; referee rejects illegal moves; Capi-ordered deals"
