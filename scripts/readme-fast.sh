#!/usr/bin/env bash
# The --fast reproductions of the README results: the first pairs of each
# README row with the row's deal and engine seeds, from its run's commit.
# - R2 rows 12, 13, 15, 16 (the rule bot against the random bot), commit
#   d276826: docs/eval/results/2026-10-25-baselines-r2-fast.txt.
# - R3 rows 12 to 15 (the omniscient bot against the rule bot), commit
#   f77e9c2, with R3's stopping SPRT: the first 8 pairs, before any
#   decision is possible: docs/eval/results/2026-10-25-omniscient-r3-fast.txt.
# Writes the summary lines to those files, or with --check compares a fresh
# run with them. R1's version is at commit a3e108a.
set -u
cd "$(dirname "$0")/.."
check=${1:-}
mkdir -p runs/readme-fast
fail=0
total=0

# run <commit> <file> <deal base> <engine base> <match flags> <a bot> <b bot> <specs...>
# A spec is "<row> <mode> <unit> <target> <pairs> [<deal seed>]": the deal
# seed defaults to deal base + 100,000 * row. A bot may carry its options
# ("pimc --worlds 64").
run() {
  local commit=$1 file=$2 deal=$3 eng=$4 sprt=$5 a=$6 b=$7
  shift 7
  local bin
  bin=$(scripts/build-bin.sh "$commit" | tail -1)
  [ -x "$bin" ] || { echo "ERROR build of $commit failed"; fail=1; return; }
  local lines="" spec row mode unit target pairs seed out
  for spec in "$@"; do
    read -r row mode unit target pairs seed <<< "$spec"
    seed=${seed:-$((deal + 100000 * row))}
    # shellcheck disable=SC2086
    out=$("$bin" match --mode "$mode" --unit "$unit" --target "$target" --pairs "$pairs" \
      --seed "$seed" --engine-seed $((eng + row)) $sprt \
      --a "$bin engine --bot $a" --b "$bin engine --bot $b" --out "runs/readme-fast/$(basename "$file" .txt)-row-$row.txt" | head -1)
    lines="${lines}row $row ($mode $unit $target, first $pairs pairs): $out
"
    total=$((total + 1))
  done
  if [ "$check" = "--check" ]; then
    if [ "$lines" != "$(cat "$file")
" ]; then
      echo "ERROR readme-fast: a fresh run differs from $file"
      fail=1
    fi
  else
    printf '%s' "$lines" > "$file"
    echo "readme-fast: wrote $file"
  fi
}

run d276826 docs/eval/results/2026-10-25-baselines-r2-fast.txt 2000000000 5000100000 "--sprt 0,4 --sprt-track" rule random \
  "12 1v1 round 100 30" "13 1v1 game 100 10" "15 2v2 round 100 30" "16 2v2 game 100 10"
run f77e9c2 docs/eval/results/2026-10-25-omniscient-r3-fast.txt 2010000000 5000300000 "--sprt 0,4" omniscient rule \
  "12 1v1 round 100 8" "13 1v1 game 100 8" "14 2v2 round 100 8" "15 2v2 game 100 8"

if [ "$check" = "--check" ] && [ "$fail" -eq 0 ]; then
  echo "readme-fast: PASS $total rows reproduce docs/eval/results/2026-10-25-baselines-r2-fast.txt and docs/eval/results/2026-10-25-omniscient-r3-fast.txt"
fi
exit "$fail"
