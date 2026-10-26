#!/usr/bin/env bash
# M4-05: dominofish analyze. The unit tests (crates/dominofish/src/
# analyze.rs) rank every legal move at positions of selfplay records in
# both modes, the same way twice, and reject impossible requests. From the
# command line, with a release build: selfplay --record writes a record
# that analyze reads; the same command prints the same bytes twice; the
# output says the values are sampled, with the world count and the number
# of consistent worlds; and malformed or impossible requests (round 0, a
# ply past the round, a finished round, a malformed record, a missing
# --worlds) exit 2 with an ERROR line. --fast runs the same checks.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
dir=runs/M4-05
mkdir -p "$dir"
fail=0
err() { echo "ERROR $*"; fail=1; }
cargo test -q --release -p dominofish analyze > "$dir/test.log" 2>&1 || err "analyze tests failed ($dir/test.log)"
grep -q 'test result: ok. 2 passed' "$dir/test.log" || err "the analyze tests did not both run ($dir/test.log)"
cargo build -q --release -p dominofish || err "build failed"
bin=target/release/dominofish
checked=""
for mode in 1v1 2v2; do
  rec="$dir/game-$mode.txt"
  "$bin" selfplay --mode "$mode" --seed 5 --record "$rec" > /dev/null || err "selfplay --record failed ($mode)"
  "$bin" analyze --record "$rec" --round 1 --ply 4 --worlds 16 --seed 2 > "$dir/a-$mode.txt" || err "analyze failed ($mode)"
  "$bin" analyze --record "$rec" --round 1 --ply 4 --worlds 16 --seed 2 > "$dir/b-$mode.txt" || err "analyze failed twice ($mode)"
  cmp -s "$dir/a-$mode.txt" "$dir/b-$mode.txt" || err "$mode: two runs differ"
  grep -q '^PIMC, sampled: 16 worlds of [0-9,]* consistent' "$dir/a-$mode.txt" || err "$mode: no sampled label with the world counts"
  grep -q '^Best: ' "$dir/a-$mode.txt" || err "$mode: no best move"
  n=$(awk '/^Move/ {p = 1; next} p && /^$/ {exit} p' "$dir/a-$mode.txt" | wc -l | tr -d ' ')
  checked="$checked $mode: $n moves ranked;"
  moves=$(grep -c -E '^[nesw] (play|draw|pass)' <(awk '/^deal/ {r++} r == 1' "$rec"))
  for bad in "--round 0 --ply 0 --worlds 4" "--round 1 --ply $((moves + 1)) --worlds 4" "--round 1 --ply $moves --worlds 4" "--round 1 --ply 0"; do
    out=$("$bin" analyze --record "$rec" $bad 2>&1); code=$?
    [ "$code" -eq 2 ] || err "$mode: analyze $bad exited $code, not 2"
    printf '%s\n' "$out" | head -1 | grep -q '^ERROR ' || err "$mode: analyze $bad printed no ERROR line"
  done
done
printf 'dominofish-record 1\nmode 2v2\ntarget 100\ndeal 0-0\nend\n' > "$dir/bad.txt"
"$bin" analyze --record "$dir/bad.txt" --round 1 --ply 0 --worlds 4 > /dev/null 2>&1
code=$?
[ "$code" -eq 2 ] || err "a malformed record exited $code, not 2"
if [ "$fail" -eq 0 ]; then
  echo "M4-05: PASS unit tests 2 passed (every legal move ranked, the same twice, impossible requests rejected);$checked equal bytes on two runs; labeled sampled with world counts; round 0, a ply past the round, a finished round, a missing --worlds, and a malformed record exit 2 with an ERROR line"
else
  echo "M4-05: FAIL"
fi
exit "$fail"
