#!/usr/bin/env bash
# Test T8 (docs/PLAN.md, M6-03): the calibration of PIMC's win probability,
# with HEAD's runs/bin binary and the committed W tables, rows one at a time
# on 4 threads: k = 0, 1v1 to 100, 1,000 games; k = 1, 1v1 to 200, 500;
# k = 2, 2v2 to 100, 400; k = 3, 2v2 to 200, 200. Game g of row k deals from
# 3,000,000,000 + 1,000,000 k + g; the engine seed base is 5,001,100,000 + k.
# Each row's file goes to docs/eval/calibration/ with its run line appended.
# Usage: scripts/run-t8.sh [rows], for example scripts/run-t8.sh 0 1 for the
# 1v1 rows (default: all four).
set -u
cd "$(dirname "$0")/.."
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; exit 1; }
dir=docs/eval/calibration
mkdir -p "$dir" runs/T8
rows=${*:-0 1 2 3}
fail=0
for k in $rows; do
  case $k in
    0) mode=1v1; target=100; games=1000 ;;
    1) mode=1v1; target=200; games=500 ;;
    2) mode=2v2; target=100; games=400 ;;
    3) mode=2v2; target=200; games=200 ;;
    *) echo "ERROR no row $k"; exit 1 ;;
  esac
  out="$dir/t8-row-$k-$mode-$target.txt"
  seed=$((3000000000 + 1000000 * k))
  engine_seed=$((5001100000 + k))
  load=$(uptime | sed 's/.*load averages*: *//; s/,* .*//')
  cmd="$bin calibrate --mode $mode --target $target --games $games --seed $seed --engine-seed $engine_seed --threads 4 --out $out"
  /usr/bin/time -l $cmd > "runs/T8/row-$k.out" 2> "runs/T8/row-$k.time"
  code=$?
  if [ "$code" -ne 0 ]; then
    echo "ERROR row $k: $(grep -m1 '^ERROR' "runs/T8/row-$k.time")"
    fail=1
    continue
  fi
  wall=$(awk '/ real / {print $1}' "runs/T8/row-$k.time")
  rss=$(awk '/maximum resident set size/ {print int($1 / 1024)}' "runs/T8/row-$k.time")
  echo "# run run=T8 row=$k command=$(printf '%s' "$cmd" | tr ' ' '+') wall_s=$wall peak_rss_kb=$rss load=$load date=$(date +%F)" >> "$out"
  echo "T8 row $k $mode to $target: $(grep '^# brier' "$out")"
done
exit "$fail"
