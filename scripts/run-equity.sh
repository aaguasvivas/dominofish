#!/usr/bin/env bash
# The equity runs of docs/PLAN.md D20 (M6-01): 10,000 rounds after the
# first of self-play games between copies of PIMC at its default limits,
# with HEAD's runs/bin binary on 4 threads, deal seeds 4,000,000,000 + game
# and engine seed base 5,001,000,000; the rounds and the W tables for
# targets 100 and 200 go to docs/eval/equity/, and the run line (command,
# wall time, peak RSS, load, date) is appended to the rounds file, as
# scripts/verify/M6-01.sh expects. The output is the same at any thread
# count, so a stopped run is rerun from the start.
# Usage: scripts/run-equity.sh 1v1|2v2
set -u
cd "$(dirname "$0")/.."
mode=${1:?usage: scripts/run-equity.sh 1v1|2v2}
case $mode in 1v1 | 2v2) ;; *) echo "ERROR mode must be 1v1 or 2v2"; exit 1 ;; esac
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; exit 1; }
dir=docs/eval/equity
mkdir -p runs/equity
load=$(uptime | sed 's/.*load averages*: *//; s/,* .*//')
cmd="$bin equity --mode $mode --rounds 10000 --seed 4000000000 --engine-seed 5001000000 --threads 4 --out-dir $dir"
/usr/bin/time -l $cmd > "runs/equity/$mode.out" 2> "runs/equity/$mode.time"
code=$?
cat "runs/equity/$mode.out"
[ "$code" -eq 0 ] || { echo "ERROR the $mode equity run failed: $(tail -1 "runs/equity/$mode.time")"; exit 1; }
wall=$(awk '/ real / {print $1}' "runs/equity/$mode.time")
rss=$(awk '/maximum resident set size/ {print int($1 / 1024)}' "runs/equity/$mode.time")
echo "# run command=$(printf '%s' "$cmd" | tr ' ' '+') wall_s=$wall peak_rss_kb=$rss load=$load date=$(date +%F)" >> "$dir/rounds-$mode.txt"
echo "equity $mode: done in $wall s, peak RSS $rss KB"
