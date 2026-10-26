#!/usr/bin/env bash
# The PIMC runs of M4 (docs/PLAN.md, pre-registered): R4, R5, or R6, with
# HEAD's runs/bin binary, at most 4 rows at once (one thread each).
#   R4: PIMC (side A, the default worlds) against each baseline (random,
#       heaviest, accompanied, rule), in 1v1 and 2v2, as rounds (at most
#       1,000 pairs) and full games to 100 (at most 300): 16 rows, row k in
#       that order; deal seeds 2,020,000,000 + 100,000 k, engine seed base
#       5,000,400,000 + k; SPRT on the pair margin (H0 0, H1 4), stopping.
#   R5: PIMC at each world budget of the curve (the powers of two from 1 to
#       the default; side A) against the rule bot, 1v1 then 2v2 (row k:
#       mode, then budget), rounds, 500 pairs, no SPRT; the same deals for
#       every budget of a mode: seeds 2,030,000,000 (1v1) and 2,031,000,000
#       (2v2) + i; engine seed base 5,000,500,000 + k.
#   R6: the omniscient bot (side A, cheating) against PIMC (the default
#       worlds), 1v1 and 2v2, rounds, 500 pairs, no SPRT; deal seeds
#       2,040,000,000 + 100,000 k, engine seed base 5,000,600,000 + k.
# Usage: scripts/run-pimc.sh R4|R5|R6. Each row writes one file to its
# run's directory under docs/eval/results/ with a `# run` line; then run
# `dominofish report`. scripts/run-pimc.sh <run> row <k> runs one row.
set -u
cd "$(dirname "$0")/.."
run=${1:?usage: scripts/run-pimc.sh R4|R5|R6}
worlds=$(sed -n 's/^pub const PIMC_WORLDS: u32 = \([0-9]*\);/\1/p' crates/dominofish/src/solve.rs)
[ -n "$worlds" ] || { echo "ERROR no PIMC_WORLDS in crates/dominofish/src/solve.rs"; exit 1; }
curve=""; w=1
while [ "$w" -le "$worlds" ]; do curve="$curve $w"; w=$((w * 2)); done
points=$(printf '%s\n' $curve | wc -l | tr -d ' ')
case "$run" in
  R4) dir=docs/eval/results/2026-10-26-pimc-r4; rows=16 ;;
  R5) dir=docs/eval/results/2026-10-26-pimc-curve-r5; rows=$((2 * points)) ;;
  R6) dir=docs/eval/results/2026-10-26-pimc-gap-r6; rows=2 ;;
  *) echo "ERROR unknown run $run"; exit 2 ;;
esac
if [ "${2:-}" != "row" ]; then
  bin=$(scripts/build-bin.sh HEAD | tail -1)
  [ -x "$bin" ] || { echo "ERROR build failed: $bin"; exit 1; }
  mkdir -p "$dir" "runs/$run"
  basename "$(dirname "$bin")" > "$dir/COMMIT"
  seq 0 $((rows - 1)) | /usr/bin/xargs -P 4 -I{} "$0" "$run" row {} "$bin"
  fail=0
  for k in $(seq 0 $((rows - 1))); do
    grep -q '^ERROR' "runs/$run/row-$k.status" 2>/dev/null && { cat "runs/$run/row-$k.status"; fail=1; }
  done
  exit "$fail"
fi
k=$3
bin=$4
pimc="$bin engine --bot pimc --worlds $worlds"
sprt=""
case "$run" in
  R4)
    bots=(random heaviest accompanied rule)
    b=${bots[$((k / 4))]}
    mode=$([ $(((k / 2) % 2)) -eq 0 ] && echo 1v1 || echo 2v2)
    u=$([ $((k % 2)) -eq 0 ] && echo round || echo game)
    pairs=$([ "$u" = round ] && echo 1000 || echo 300)
    seed=$((2020000000 + 100000 * k)); engine_seed=$((5000400000 + k)); sprt="--sprt 0,4"
    a=$pimc; bcmd="$bin engine --bot $b"; name="$mode-$u-100-pimc-$b"
    ;;
  R5)
    # shellcheck disable=SC2206
    ws=($curve)
    w=${ws[$((k % points))]}
    mode=$([ "$k" -lt "$points" ] && echo 1v1 || echo 2v2)
    u=round; pairs=500
    seed=$([ "$mode" = 1v1 ] && echo 2030000000 || echo 2031000000); engine_seed=$((5000500000 + k))
    a="$bin engine --bot pimc --worlds $w"; bcmd="$bin engine --bot rule"; name="$mode-round-100-pimc$w-rule"
    ;;
  R6)
    mode=$([ "$k" -eq 0 ] && echo 1v1 || echo 2v2)
    u=round; pairs=500
    seed=$((2040000000 + 100000 * k)); engine_seed=$((5000600000 + k))
    a="$bin engine --bot omniscient"; bcmd=$pimc; name="$mode-round-100-omniscient-pimc"
    ;;
esac
out="$dir/row-$k-$name.txt"
cmd="$bin match --mode $mode --unit $u --target 100 --pairs $pairs --seed $seed --engine-seed $engine_seed $sprt --timeout-ms 120000 --a \"$a\" --b \"$bcmd\" --out $out"
# shellcheck disable=SC2086
/usr/bin/time -l "$bin" match --mode "$mode" --unit "$u" --target 100 --pairs "$pairs" --seed "$seed" --engine-seed "$engine_seed" $sprt --timeout-ms 120000 \
  --a "$a" --b "$bcmd" --out "$out" > "runs/$run/row-$k.out" 2> "runs/$run/row-$k.time"
code=$?
wall=$(awk '/ real / {print $1}' "runs/$run/row-$k.time")
rss=$(awk '/maximum resident set size/ {print int($1 / 1024)}' "runs/$run/row-$k.time")
load=$(uptime | sed 's/.*load averages*: *//; s/,* .*//')
echo "# run run=$run row=$k command=$(printf '%s' "$cmd" | tr ' ' '+') wall_s=$wall peak_rss_kb=$rss load=$load date=$(date +%F)" >> "$out"
if [ "$code" -ne 0 ]; then
  echo "ERROR row $k: $(tail -1 "runs/$run/row-$k.out")" > "runs/$run/row-$k.status"
else
  echo "ok" > "runs/$run/row-$k.status"
fi
echo "$run row $k $name: $(tail -1 "runs/$run/row-$k.out" | cut -c1-120)"
