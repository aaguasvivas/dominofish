#!/usr/bin/env bash
# Test T5 (docs/PLAN.md, pre-registered): the AIVAT correction (D19) on 14
# rows with HEAD's runs/bin binary, --aivat 8, target 100, no SPRT, at most
# 4 rows at once (one thread each). Row k, 1v1 for k = 0 to 6 and 2v2 for
# k = 7 to 13, in this order per mode: rule against rule (salts 1 and 2),
# random against random (salts 1 and 2), rule against random, accompanied
# against heaviest (rounds, 1,000 pairs each); PIMC at 8 worlds against
# rule (rounds, 300 pairs); rule against random and rule against rule
# (salts 1 and 2) (games to 100, 300 pairs each). Deal seeds
# 610,000,000 + 1,000,000 k + i, engine seed base 5,000,700,000 + k.
# Usage: scripts/run-t5.sh [<binary>]; with a binary (the T5 mutant), it
# replays rows 2, 3, 9, and 10 into runs/M5-03/mutant/. Then run
# `dominofish report`. scripts/run-t5.sh row <k> <binary> <dir> runs one row.
set -u
cd "$(dirname "$0")/.."
if [ "${1:-}" != "row" ]; then
  if [ -n "${1:-}" ]; then
    bin=$1; dir=runs/M5-03/mutant; rows="2 3 9 10"
  else
    bin=$(scripts/build-bin.sh HEAD | tail -1)
    [ -x "$bin" ] || { echo "ERROR build failed: $bin"; exit 1; }
    dir=docs/eval/results/2026-10-27-aivat-t5; rows=$(seq 0 13)
  fi
  mkdir -p "$dir" runs/T5
  [ -n "${1:-}" ] || basename "$(dirname "$bin")" > "$dir/COMMIT"
  printf '%s\n' $rows | /usr/bin/xargs -P 4 -I{} "$0" row {} "$bin" "$dir"
  fail=0
  for k in $rows; do
    grep -q '^ERROR' "runs/T5/row-$k.status" 2>/dev/null && { cat "runs/T5/row-$k.status"; fail=1; }
  done
  exit "$fail"
fi
k=$2; bin=$3; dir=$4
mode=$([ "$k" -lt 7 ] && echo 1v1 || echo 2v2)
i=$((k % 7))
case $i in
  0) u=round; pairs=1000; a="--bot rule --salt 1"; b="--bot rule --salt 2"; name=rule-rule ;;
  1) u=round; pairs=1000; a="--bot random --salt 1"; b="--bot random --salt 2"; name=random-random ;;
  2) u=round; pairs=1000; a="--bot rule"; b="--bot random"; name=rule-random ;;
  3) u=round; pairs=1000; a="--bot accompanied"; b="--bot heaviest"; name=accompanied-heaviest ;;
  4) u=round; pairs=300; a="--bot pimc --worlds 8"; b="--bot rule"; name=pimc8-rule ;;
  5) u=game; pairs=300; a="--bot rule"; b="--bot random"; name=rule-random ;;
  6) u=game; pairs=300; a="--bot rule --salt 1"; b="--bot rule --salt 2"; name=rule-rule ;;
esac
seed=$((610000000 + 1000000 * k)); engine_seed=$((5000700000 + k))
out="$dir/row-$k-$mode-$u-100-$name.txt"
cmd="$bin match --mode $mode --unit $u --target 100 --pairs $pairs --seed $seed --engine-seed $engine_seed --aivat 8 --timeout-ms 120000 --a \"$bin engine $a\" --b \"$bin engine $b\" --out $out"
/usr/bin/time -l "$bin" match --mode "$mode" --unit "$u" --target 100 --pairs "$pairs" --seed "$seed" --engine-seed "$engine_seed" --aivat 8 --timeout-ms 120000 \
  --a "$bin engine $a" --b "$bin engine $b" --out "$out" > "runs/T5/row-$k.out" 2> "runs/T5/row-$k.time"
code=$?
wall=$(awk '/ real / {print $1}' "runs/T5/row-$k.time")
rss=$(awk '/maximum resident set size/ {print int($1 / 1024)}' "runs/T5/row-$k.time")
load=$(uptime | sed 's/.*load averages*: *//; s/,* .*//')
echo "# run run=T5 row=$k command=$(printf '%s' "$cmd" | tr ' ' '+') wall_s=$wall peak_rss_kb=$rss load=$load date=$(date +%F)" >> "$out"
if [ "$code" -ne 0 ]; then
  echo "ERROR row $k: $(tail -1 "runs/T5/row-$k.out")" > "runs/T5/row-$k.status"
else
  echo "ok" > "runs/T5/row-$k.status"
fi
echo "T5 row $k $mode $u $name: $(grep -o 't -\{0,1\}[0-9.]*;' "$out" | head -1)"
