#!/usr/bin/env bash
# M6-03: test T8 (docs/PLAN.md) on its committed files in
# docs/eval/calibration/: each row as registered (the header's mode, target,
# seeds, and limits, its W table's hash equal to the committed table's, its
# game count, and the run line with the registered command); every
# probability within 0 to 1,000,000; and the summary lines (the Brier
# scores, the skill, their intervals, and the reliability table) equal to
# the ones `dominofish calibrate --from` recomputes from the decision lines.
# A short run at 4 worlds gives the same file on 1 and 2 threads. Prints each
# row's Brier score, baseline, and skill. --fast runs the same checks.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
dir=docs/eval/calibration
out=runs/M6-03
mkdir -p "$out"
fail=0
err() { echo "ERROR $*"; fail=1; }
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; echo "M6-03: FAIL"; exit 1; }
fnv() { /usr/bin/python3 -c '
import sys
h = 0xcbf29ce484222325
for b in open(sys.argv[1], "rb").read():
    h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
print(f"{h:016x}")' "$1"; }
rows=""
for k in 0 1 2 3; do
  case $k in
    0) mode=1v1; target=100; games=1000 ;;
    1) mode=1v1; target=200; games=500 ;;
    2) mode=2v2; target=100; games=400 ;;
    3) mode=2v2; target=200; games=200 ;;
  esac
  f="$dir/t8-row-$k-$mode-$target.txt"
  [ -f "$f" ] || { err "row $k: $f is missing"; continue; }
  seed=$((3000000000 + 1000000 * k))
  engine_seed=$((5001100000 + k))
  table="docs/eval/equity/w-$mode-$target.txt"
  hash=$([ -f "$table" ] && fnv "$table")
  grep -qx "# calibration mode=$mode target=$target deals=$seed+game engines=$engine_seed worlds=64 nodes=10000000 table_bits=16 table=${hash:-missing}" "$f" || err "row $k: the header differs from T8 or from the committed table $table"
  [ "$(grep -c '^game ' "$f")" -eq "$games" ] || err "row $k: not $games games"
  grep -q "^# run run=T8 row=$k command=runs/bin/[0-9a-f]*/dominofish+calibrate+--mode+$mode+--target+$target+--games+$games+--seed+$seed+--engine-seed+$engine_seed+--threads+4+--out+$f " "$f" || err "row $k: no T8 run line with the registered command"
  bad=$(awk '/^d / && ($8 > 1000000 || $10 > 1000000)' "$f" | wc -l | tr -d ' ')
  [ "$bad" -eq 0 ] || err "row $k: $bad probabilities above 1"
  "$bin" calibrate --from "$f" > "$out/row-$k.summary" 2>&1 || err "row $k: calibrate --from failed"
  [ "$(grep -E '^# (summary|brier|bin) ' "$f")" = "$(cat "$out/row-$k.summary")" ] || err "row $k: the summary differs from the one recomputed from its decisions"
  rows="$rows row $k ($mode to $target, $(sed -n 's/^# summary games=\([0-9]*\) predictions=\([0-9]*\)$/\1 games, \2 predictions/p' "$f")): $(sed -n 's/^# \(brier .*\) (95% intervals.*$/\1/p' "$f");"
done
eq=docs/eval/equity
if [ -f "$eq/w-1v1-100.txt" ]; then
  for t in 1 2; do
    "$bin" calibrate --mode 1v1 --target 100 --games 3 --seed 3100000000 --engine-seed 5001100900 --worlds 4 --threads "$t" --out "$out/threads-$t.txt" > /dev/null 2>&1 || err "the calibration on $t threads failed"
  done
  cmp -s "$out/threads-1.txt" "$out/threads-2.txt" || err "the calibration file differs between 1 and 2 threads"
else
  err "$eq/w-1v1-100.txt is missing"
fi
if [ "$fail" -eq 0 ]; then
  echo "M6-03: PASS T8 as registered, summaries equal to their recomputation, and 1 and 2 threads give the same file;$rows"
else
  echo "M6-03: FAIL"
fi
exit "$fail"
