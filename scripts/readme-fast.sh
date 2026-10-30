#!/usr/bin/env bash
# The --fast reproductions of the README results: the first pairs of each
# README row with the row's deal and engine seeds, from its run's commit.
# - R2 rows 12, 13, 15, 16 (the rule bot against the random bot), commit
#   d276826: docs/eval/results/2026-10-25-baselines-r2-fast.txt.
# - R3 rows 12 to 15 (the omniscient bot against the rule bot), commit
#   f77e9c2, with R3's stopping SPRT: the first 8 pairs, before any
#   decision is possible: docs/eval/results/2026-10-25-omniscient-r3-fast.txt.
# - R4 rows 12 to 15 (PIMC at 64 worlds against the rule bot), commit
#   ebaa5fd, with R4's stopping SPRT: the first 4 round pairs and 2 game
#   pairs: docs/eval/results/2026-10-26-pimc-r4-fast.txt.
# - R5, every point of the curve (PIMC at 1 to 64 worlds against the rule
#   bot), commit e3f9a4f: the first 4 pairs of each:
#   docs/eval/results/2026-10-26-pimc-curve-r5-fast.txt.
# - R6 (the omniscient bot against PIMC at 64 worlds), commit e3f9a4f: the
#   first 4 pairs per mode: docs/eval/results/2026-10-26-pimc-gap-r6-fast.txt.
# - T7, every row (the AIVAT test), commit ff3cdea, with --aivat 8: the
#   first 10 pairs of each round row (5 per fold of the cross-fit, enough to
#   fit its terms) and the first 4 of each game row (too few to fit, so its
#   corrected line equals the raw one; M5-04 compares every pair's terms
#   with the full run's): docs/eval/results/2026-10-30-aivat-t7-fast.txt.
# Writes the summary lines (with the corrected line of an --aivat run) to
# those files, or with --check compares a fresh run with them. Run ids after
# the flag limit it to those runs (M2-09 checks R2 and R3, M4-11 checks R4,
# R5, and R6, M5-04 checks T7). R1's version is at commit a3e108a.
set -u
cd "$(dirname "$0")/.."
check=${1:-}
[ $# -gt 0 ] && shift
only=" $* "
# want <run id>: whether this invocation covers the run.
want() { [ "$only" = "  " ] || case "$only" in *" $1 "*) true ;; *) false ;; esac; }
checked=""
mkdir -p runs/readme-fast
fail=0
total=0

# run <commit> <file> <deal base> <engine base> <match flags> <a bot> <b bot> <specs...>
# A spec is "<row> <mode> <unit> <target> <pairs> [<deal seed> [<a worlds>
# [<a bot> <b bot>]]]": the deal seed defaults to deal base + 100,000 * row
# (- keeps it), <a worlds> runs side A as PIMC at that many worlds (- keeps
# <a bot>), and a spec's bots replace the run's, with + for each space
# ("rule+--salt+1"). A bot may carry its options ("pimc --worlds 64").
run() {
  local commit=$1 file=$2 deal=$3 eng=$4 sprt=$5 a=$6 b=$7
  shift 7
  local bin
  bin=$(scripts/build-bin.sh "$commit" | tail -1)
  [ -x "$bin" ] || { echo "ERROR build of $commit failed"; fail=1; return; }
  local lines="" spec row mode unit target pairs seed worlds sa sb abot bbot out
  for spec in "$@"; do
    sa="" sb=""
    read -r row mode unit target pairs seed worlds sa sb <<< "$spec"
    [ -z "$seed" ] || [ "$seed" = "-" ] && seed=$((deal + 100000 * row))
    abot=${sa:-$a} bbot=${sb:-$b}
    abot=${abot//+/ } bbot=${bbot//+/ }
    [ -n "$worlds" ] && [ "$worlds" != "-" ] && abot="pimc --worlds $worlds"
    # shellcheck disable=SC2086
    out=$("$bin" match --mode "$mode" --unit "$unit" --target "$target" --pairs "$pairs" \
      --seed "$seed" --engine-seed $((eng + row)) $sprt \
      --a "$bin engine --bot $abot" --b "$bin engine --bot $bbot" --out "runs/readme-fast/$(basename "$file" .txt)-row-$row.txt" |
      awk 'NR == 1 { line = $0 } /^corrected / { line = line " | " $0 } END { print line }')
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

want R2 && checked="$checked docs/eval/results/2026-10-25-baselines-r2-fast.txt" && run d276826 docs/eval/results/2026-10-25-baselines-r2-fast.txt 2000000000 5000100000 "--sprt 0,4 --sprt-track" rule random \
  "12 1v1 round 100 30" "13 1v1 game 100 10" "15 2v2 round 100 30" "16 2v2 game 100 10"
want R3 && checked="$checked docs/eval/results/2026-10-25-omniscient-r3-fast.txt" && run f77e9c2 docs/eval/results/2026-10-25-omniscient-r3-fast.txt 2010000000 5000300000 "--sprt 0,4" omniscient rule \
  "12 1v1 round 100 8" "13 1v1 game 100 8" "14 2v2 round 100 8" "15 2v2 game 100 8"
want R4 && checked="$checked docs/eval/results/2026-10-26-pimc-r4-fast.txt" && run ebaa5fd docs/eval/results/2026-10-26-pimc-r4-fast.txt 2020000000 5000400000 "--sprt 0,4 --timeout-ms 120000" "pimc --worlds 64" rule \
  "12 1v1 round 100 4" "13 1v1 game 100 2" "14 2v2 round 100 4" "15 2v2 game 100 2"
want R5 && checked="$checked docs/eval/results/2026-10-26-pimc-curve-r5-fast.txt" && run e3f9a4f docs/eval/results/2026-10-26-pimc-curve-r5-fast.txt 0 5000500000 "--timeout-ms 120000" - rule \
  "0 1v1 round 100 4 2030000000 1" "1 1v1 round 100 4 2030000000 2" "2 1v1 round 100 4 2030000000 4" "3 1v1 round 100 4 2030000000 8" \
  "4 1v1 round 100 4 2030000000 16" "5 1v1 round 100 4 2030000000 32" "6 1v1 round 100 4 2030000000 64" \
  "7 2v2 round 100 4 2031000000 1" "8 2v2 round 100 4 2031000000 2" "9 2v2 round 100 4 2031000000 4" "10 2v2 round 100 4 2031000000 8" \
  "11 2v2 round 100 4 2031000000 16" "12 2v2 round 100 4 2031000000 32" "13 2v2 round 100 4 2031000000 64"
want R6 && checked="$checked docs/eval/results/2026-10-26-pimc-gap-r6-fast.txt" && run e3f9a4f docs/eval/results/2026-10-26-pimc-gap-r6-fast.txt 2040000000 5000600000 "--timeout-ms 120000" omniscient "pimc --worlds 64" \
  "0 1v1 round 100 4" "1 2v2 round 100 4"
want T7 && checked="$checked docs/eval/results/2026-10-30-aivat-t7-fast.txt" && run ff3cdea docs/eval/results/2026-10-30-aivat-t7-fast.txt 0 5000900000 "--aivat 8 --timeout-ms 120000" - - \
  "0 1v1 round 100 10 640000000 - rule+--salt+1 rule+--salt+2" "1 1v1 round 100 10 641000000 - random+--salt+1 random+--salt+2" \
  "2 1v1 round 100 10 642000000 - rule random" "3 1v1 round 100 10 643000000 - accompanied heaviest" \
  "4 1v1 round 100 10 644000000 8 - rule" "5 1v1 game 100 4 645000000 - rule random" \
  "6 1v1 game 100 4 646000000 - rule+--salt+1 rule+--salt+2" \
  "7 2v2 round 100 10 647000000 - rule+--salt+1 rule+--salt+2" "8 2v2 round 100 10 648000000 - random+--salt+1 random+--salt+2" \
  "9 2v2 round 100 10 649000000 - rule random" "10 2v2 round 100 10 650000000 - accompanied heaviest" \
  "11 2v2 round 100 10 651000000 8 - rule" "12 2v2 game 100 4 652000000 - rule random" \
  "13 2v2 game 100 4 653000000 - rule+--salt+1 rule+--salt+2"

if [ "$check" = "--check" ] && [ "$fail" -eq 0 ]; then
  echo "readme-fast: PASS $total rows reproduce$checked"
fi
exit "$fail"
