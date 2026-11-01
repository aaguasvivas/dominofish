#!/usr/bin/env bash
# Run R7 (docs/PLAN.md, M6-04): the game-win SPRT of PIMC with the win
# objective (side A) against PIMC with the points objective (side B, the
# current best), both at the default limits (64 worlds per move), with
# HEAD's runs/bin binary. Rows one at a time, each on 4 threads
# (`match --threads 4`): k = 0, 1v1 games to 100; k = 1, 1v1 to 200;
# k = 2, 2v2 to 100; k = 3, 2v2 to 200. Deal seeds 2,050,000,000 +
# 1,000,000 k + i, engine seed base 5,001,300,000 + k; --aivat 8; the SPRT
# on each pair's game-win share, H0 0.5 against H1 0.53, stopping at its
# decision, at most 2,700 pairs (a row that reaches it is inconclusive);
# --timeout-ms 1800000, a safety limit against a hung engine. Each match
# prints its progress to runs/R7/row-<k>.time once a minute. The owner's
# Q15 answer keeps the Mac awake: start it as
# nohup nice -n 10 caffeinate -i scripts/run-r7.sh > runs/R7.log 2>&1 &
# Usage: scripts/run-r7.sh [rows], for example scripts/run-r7.sh 2 3 for
# the 2v2 rows (default: all four); then run `dominofish report`.
set -u
cd "$(dirname "$0")/.."
H1=0.53
MAX=2700
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; exit 1; }
dir=docs/eval/results/2026-11-01-win-objective-r7
status=runs/R7
mkdir -p "$dir" "$status"
basename "$(dirname "$bin")" > "$dir/COMMIT"
rows=${*:-0 1 2 3}
fail=0
for k in $rows; do
  case $k in
    0) mode=1v1; target=100 ;;
    1) mode=1v1; target=200 ;;
    2) mode=2v2; target=100 ;;
    3) mode=2v2; target=200 ;;
    *) echo "ERROR no row $k"; exit 1 ;;
  esac
  seed=$((2050000000 + 1000000 * k)); engine_seed=$((5001300000 + k))
  a="$bin engine --bot pimc --objective win"; b="$bin engine --bot pimc --objective points"
  out="$dir/row-$k-$mode-game-$target-win-points.txt"
  load=$(uptime | sed 's/.*load averages*: *//; s/,* .*//')
  cmd="$bin match --mode $mode --unit game --target $target --pairs $MAX --seed $seed --engine-seed $engine_seed --aivat 8 --sprt 0.5,$H1 --sprt-on wins --threads 4 --timeout-ms 1800000 --a \"$a\" --b \"$b\" --out $out"
  /usr/bin/time -l "$bin" match --mode "$mode" --unit game --target "$target" --pairs "$MAX" --seed "$seed" --engine-seed "$engine_seed" --aivat 8 --sprt "0.5,$H1" --sprt-on wins --threads 4 --timeout-ms 1800000 \
    --a "$a" --b "$b" --out "$out" > "$status/row-$k.out" 2> "$status/row-$k.time"
  code=$?
  if [ "$code" -ne 0 ]; then
    echo "ERROR row $k: $(tail -1 "$status/row-$k.out")"
    fail=1
    continue
  fi
  wall=$(awk '/ real / {print $1}' "$status/row-$k.time")
  rss=$(awk '/maximum resident set size/ {print int($1 / 1024)}' "$status/row-$k.time")
  echo "# run run=R7 row=$k command=$(printf '%s' "$cmd" | tr ' ' '+') wall_s=$wall peak_rss_kb=$rss load=$load date=$(date +%F)" >> "$out"
  echo "R7 row $k $mode to $target: $(grep -o 'sprt on the game-win share[^;]*' "$out" | head -1)"
done
exit "$fail"
