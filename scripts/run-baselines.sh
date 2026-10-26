#!/usr/bin/env bash
# R2 (docs/PLAN.md, pre-registered): the baseline table after the M2 review
# fixes. Every pairing of the four baseline bots, in 1v1 and 2v2, as rounds
# (1,000 pairs) and as full games at targets 100 and 200 (300 pairs each),
# on R1's held-out deal seeds 2,000,000,000 + 100,000 * row, engine seed
# base 5,000,100,000 + row, with HEAD's runs/bin binary and the SPRT on the
# pair margin (H0 0, H1 4) in tracking mode. Writes one file per row to
# docs/eval/results/2026-10-25-baselines-r2/ with a `# run` line (command,
# wall time, peak RSS of the match command, load, date). Then run
# `dominofish report`. R1's version of this script is at commit a3e108a.
set -u
cd "$(dirname "$0")/.."
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; exit 1; }
dir=docs/eval/results/2026-10-25-baselines-r2
mkdir -p "$dir" runs/R2
row=0
fail=0
for pairing in heaviest:random accompanied:random rule:random accompanied:heaviest rule:heaviest rule:accompanied; do
  a=${pairing%%:*}; b=${pairing#*:}
  for mode in 1v1 2v2; do
    for unit in round:100:1000 game:100:300 game:200:300; do
      u=${unit%%:*}; rest=${unit#*:}; target=${rest%%:*}; pairs=${rest#*:}
      seed=$((2000000000 + 100000 * row))
      engine_seed=$((5000100000 + row))
      out="$dir/row-$row-$mode-$u-$target-$a-$b.txt"
      cmd="$bin match --mode $mode --unit $u --target $target --pairs $pairs --seed $seed --engine-seed $engine_seed --sprt 0,4 --sprt-track --a \"$bin engine --bot $a\" --b \"$bin engine --bot $b\" --out $out"
      /usr/bin/time -l "$bin" match --mode "$mode" --unit "$u" --target "$target" --pairs "$pairs" --seed "$seed" --engine-seed "$engine_seed" --sprt 0,4 --sprt-track \
        --a "$bin engine --bot $a" --b "$bin engine --bot $b" --out "$out" > "runs/R2/row-$row.out" 2> "runs/R2/row-$row.time"
      code=$?
      wall=$(awk '/ real / {print $1}' "runs/R2/row-$row.time")
      rss=$(awk '/maximum resident set size/ {print int($1 / 1024)}' "runs/R2/row-$row.time")
      load=$(uptime | sed 's/.*load averages*: *//; s/,* .*//')
      echo "# run run=R2 row=$row command=$(printf '%s' "$cmd" | tr ' ' '+') wall_s=$wall peak_rss_kb=$rss load=$load date=$(date +%F)" >> "$out"
      if [ "$code" -ne 0 ]; then
        echo "ERROR row $row: $(tail -1 "runs/R2/row-$row.out")"
        fail=1
      fi
      echo "row $row $mode $u $target $a-$b: $(tail -1 "runs/R2/row-$row.out" | cut -c1-120)"
      row=$((row + 1))
    done
  done
done
exit "$fail"
