#!/usr/bin/env bash
# R3 (docs/PLAN.md, pre-registered): the omniscient bot (side A, labeled as
# cheating) against each baseline (random, heaviest, accompanied, rule), in
# 1v1 and 2v2, as rounds (at most 1,000 pairs) and as full games to 100 (at
# most 300 pairs): 16 rows, row k in that order. Held-out deal seeds
# 2,010,000,000 + 100,000 * row, engine seed base 5,000,300,000 + row,
# HEAD's runs/bin binary, and the SPRT on the pair margin (H0 0, H1 4),
# stopping at a decision. Writes one file per row to
# docs/eval/results/2026-10-25-omniscient-r3/ with a `# run` line. Then run
# `dominofish report`.
set -u
cd "$(dirname "$0")/.."
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; exit 1; }
dir=docs/eval/results/2026-10-25-omniscient-r3
mkdir -p "$dir" runs/R3
row=0
fail=0
for b in random heaviest accompanied rule; do
  for mode in 1v1 2v2; do
    for unit in round:100:1000 game:100:300; do
      u=${unit%%:*}; rest=${unit#*:}; target=${rest%%:*}; pairs=${rest#*:}
      seed=$((2010000000 + 100000 * row))
      engine_seed=$((5000300000 + row))
      out="$dir/row-$row-$mode-$u-$target-omniscient-$b.txt"
      cmd="$bin match --mode $mode --unit $u --target $target --pairs $pairs --seed $seed --engine-seed $engine_seed --sprt 0,4 --a \"$bin engine --bot omniscient\" --b \"$bin engine --bot $b\" --out $out"
      /usr/bin/time -l "$bin" match --mode "$mode" --unit "$u" --target "$target" --pairs "$pairs" --seed "$seed" --engine-seed "$engine_seed" --sprt 0,4 \
        --a "$bin engine --bot omniscient" --b "$bin engine --bot $b" --out "$out" > "runs/R3/row-$row.out" 2> "runs/R3/row-$row.time"
      code=$?
      wall=$(awk '/ real / {print $1}' "runs/R3/row-$row.time")
      rss=$(awk '/maximum resident set size/ {print int($1 / 1024)}' "runs/R3/row-$row.time")
      load=$(uptime | sed 's/.*load averages*: *//; s/,* .*//')
      echo "# run run=R3 row=$row command=$(printf '%s' "$cmd" | tr ' ' '+') wall_s=$wall peak_rss_kb=$rss load=$load date=$(date +%F)" >> "$out"
      if [ "$code" -ne 0 ]; then
        echo "ERROR row $row: $(tail -1 "runs/R3/row-$row.out")"
        fail=1
      fi
      echo "row $row $mode $u $target omniscient-$b: $(tail -1 "runs/R3/row-$row.out" | cut -c1-120)"
      row=$((row + 1))
    done
  done
done
exit "$fail"
