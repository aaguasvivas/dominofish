#!/usr/bin/env bash
# Test T7 (docs/PLAN.md, pre-registered after T6 by the owner's Q12 and Q13
# answers): T6's 14 rows on new seeds with HEAD's runs/bin binary, --aivat 8
# with per-pair term totals (D19 as amended), target 100, no SPRT, at most 4
# rows at once (one thread each). Row k, 1v1 for k = 0 to 6 and 2v2 for
# k = 7 to 13, in this order per mode: rule against rule (salts 1 and 2) and
# random against random (salts 1 and 2), rounds, 1,000 pairs; rule against
# random and accompanied against heaviest, rounds, 16,000 pairs; PIMC at 8
# worlds against rule, rounds, 300 pairs; rule against random and rule
# against rule (salts 1 and 2), games to 100, 4,000 pairs in 1v1 and 1,000
# in 2v2. Deal seeds 640,000,000 + 1,000,000 k + i, engine seed base
# 5,000,900,000 + k. The longest rows start first; the order changes no row.
# Usage: scripts/run-t7.sh; then run `dominofish report`.
# scripts/run-t7.sh row <k> <binary> <dir> runs one row.
set -u
cd "$(dirname "$0")/.."
status=runs/T7
if [ "${1:-}" != "row" ]; then
  bin=$(scripts/build-bin.sh HEAD | tail -1)
  [ -x "$bin" ] || { echo "ERROR build failed: $bin"; exit 1; }
  dir=docs/eval/results/2026-10-30-aivat-t7
  rows="5 6 12 13 2 9 3 11 8 0 10 4 1 7"
  mkdir -p "$dir" "$status"
  basename "$(dirname "$bin")" > "$dir/COMMIT"
  printf '%s\n' $rows | /usr/bin/xargs -P 4 -I{} "$0" row {} "$bin" "$dir"
  fail=0
  for k in $rows; do
    grep -q '^ok' "$status/row-$k.status" 2>/dev/null || { echo "ERROR row $k: $(cat "$status/row-$k.status" 2>/dev/null)"; fail=1; }
  done
  exit "$fail"
fi
k=$2; bin=$3; dir=$4
mode=$([ "$k" -lt 7 ] && echo 1v1 || echo 2v2)
i=$((k % 7))
games=$([ "$k" -lt 7 ] && echo 4000 || echo 1000)
case $i in
  0) u=round; pairs=1000; a="--bot rule --salt 1"; b="--bot rule --salt 2"; name=rule-rule ;;
  1) u=round; pairs=1000; a="--bot random --salt 1"; b="--bot random --salt 2"; name=random-random ;;
  2) u=round; pairs=16000; a="--bot rule"; b="--bot random"; name=rule-random ;;
  3) u=round; pairs=16000; a="--bot accompanied"; b="--bot heaviest"; name=accompanied-heaviest ;;
  4) u=round; pairs=300; a="--bot pimc --worlds 8"; b="--bot rule"; name=pimc8-rule ;;
  5) u=game; pairs=$games; a="--bot rule"; b="--bot random"; name=rule-random ;;
  6) u=game; pairs=$games; a="--bot rule --salt 1"; b="--bot rule --salt 2"; name=rule-rule ;;
esac
seed=$((640000000 + 1000000 * k)); engine_seed=$((5000900000 + k))
out="$dir/row-$k-$mode-$u-100-$name.txt"
cmd="$bin match --mode $mode --unit $u --target 100 --pairs $pairs --seed $seed --engine-seed $engine_seed --aivat 8 --timeout-ms 120000 --a \"$bin engine $a\" --b \"$bin engine $b\" --out $out"
/usr/bin/time -l "$bin" match --mode "$mode" --unit "$u" --target 100 --pairs "$pairs" --seed "$seed" --engine-seed "$engine_seed" --aivat 8 --timeout-ms 120000 \
  --a "$bin engine $a" --b "$bin engine $b" --out "$out" > "$status/row-$k.out" 2> "$status/row-$k.time"
code=$?
wall=$(awk '/ real / {print $1}' "$status/row-$k.time")
rss=$(awk '/maximum resident set size/ {print int($1 / 1024)}' "$status/row-$k.time")
load=$(uptime | sed 's/.*load averages*: *//; s/,* .*//')
echo "# run run=T7 row=$k command=$(printf '%s' "$cmd" | tr ' ' '+') wall_s=$wall peak_rss_kb=$rss load=$load date=$(date +%F)" >> "$out"
if [ "$code" -ne 0 ]; then
  echo "ERROR row $k: $(tail -1 "$status/row-$k.out")" > "$status/row-$k.status"
else
  echo "ok" > "$status/row-$k.status"
fi
echo "T7 row $k $mode $u $name: $(grep -o 't -\{0,1\}[0-9.]*;' "$out" | head -1)"
