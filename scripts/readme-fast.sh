#!/usr/bin/env bash
# The --fast reproductions of the README results (R2 rows 12, 13, 15, 16:
# the rule bot against the random bot): the first pairs of each row with the
# row's deal and engine seeds, from R2's commit. Writes their summary lines
# to docs/eval/results/2026-10-25-baselines-r2-fast.txt, or with --check
# compares a fresh run with that file. R1's version is at commit a3e108a.
set -u
cd "$(dirname "$0")/.."
check=${1:-}
commit=d276826
bin=$(scripts/build-bin.sh "$commit" | tail -1)
[ -x "$bin" ] || { echo "ERROR build of $commit failed"; exit 1; }
mkdir -p runs/readme-fast
lines=""
for spec in "12 1v1 round 100 30" "13 1v1 game 100 10" "15 2v2 round 100 30" "16 2v2 game 100 10"; do
  set -- $spec
  row=$1; mode=$2; unit=$3; target=$4; pairs=$5
  seed=$((2000000000 + 100000 * row))
  engine_seed=$((5000100000 + row))
  out=$("$bin" match --mode "$mode" --unit "$unit" --target "$target" --pairs "$pairs" --seed "$seed" --engine-seed "$engine_seed" --sprt 0,4 --sprt-track \
    --a "$bin engine --bot rule" --b "$bin engine --bot random" --out "runs/readme-fast/row-$row.txt" | head -1)
  lines="${lines}row $row ($mode $unit $target, first $pairs pairs): $out
"
done
file=docs/eval/results/2026-10-25-baselines-r2-fast.txt
if [ "$check" = "--check" ]; then
  if [ "$lines" = "$(cat "$file")
" ]; then echo "readme-fast: PASS 4 rows reproduce $file"; exit 0; fi
  echo "ERROR readme-fast: a fresh run differs from $file"
  exit 1
fi
printf '%s' "$lines" > "$file"
echo "readme-fast: wrote $file"
