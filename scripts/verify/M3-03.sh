#!/usr/bin/env bash
# M3-03: dominofish solve-stats, one thread, the default node cap and table.
# It solves every position of rounds 1 and 2 of 500 seeded 2v2 games (seed
# 3000) and 1,000 seeded 1v1 games (seed 3001) (--fast: 20 and 50) and
# reports, per mode and by tiles left, solve time (median, p90, max) and
# nodes (median, p90, max), the share of all solves that hit the node cap,
# and the share of round-start solves (27 or 28 tiles left) that did. The
# run goes to runs/M3-03/solve-stats.md with commit, thread count, load,
# and date; the committed record is
# docs/eval/results/2026-10-25-solve-stats.md, which must hold both modes.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs/M3-03
fail=0
err() { echo "ERROR $*"; fail=1; }
g2=500; g1=1000
[ "${1:-}" = "--fast" ] && { g2=20; g1=50; }
cargo build -q --release -p dominofish 2> runs/M3-03/build.log || { echo "ERROR build failed"; echo "M3-03: FAIL"; exit 1; }
out=runs/M3-03/solve-stats.md
{
  echo "# Solver statistics"
  echo
  echo "Commit $(/usr/bin/git rev-parse --short HEAD), one thread, 1-minute load average $(uptime | sed 's/.*load averages*: *//; s/,* .*//') at the start, $(date +%F). Times are wall-clock; node counts are exact for the commit."
  echo
  echo "Commands: \`dominofish solve-stats --mode 2v2 --games $g2 --seed 3000\` and \`dominofish solve-stats --mode 1v1 --games $g1 --seed 3001\`."
  echo
  echo "## 2v2"
  echo
  target/release/dominofish solve-stats --mode 2v2 --games "$g2" --seed 3000
  echo
  echo "## 1v1"
  echo
  target/release/dominofish solve-stats --mode 1v1 --games "$g1" --seed 3001
} > "$out" 2>&1 || err "solve-stats failed ($out)"
s2=$(grep -o 'solve-stats: mode 2v2 .*' "$out")
s1=$(grep -o 'solve-stats: mode 1v1 .*' "$out")
[ -n "$s2" ] && [ -n "$s1" ] || err "a mode has no summary ($out)"
for m in 2v2 1v1; do
  rows=$(awk -v m="## $m" '$0 == m {p = 1; next} /^## / {p = 0} p && /^\| [0-9]+ \|/' "$out" | wc -l | tr -d ' ')
  [ "$rows" -ge 15 ] || err "$m: only $rows tiles-left rows"
  grep -q "^| 28 |" "$out" || err "no 28-tiles-left row"
done
for s in "$s2" "$s1"; do
  case "$s" in *"capped "[0-9]*" ("*"%); round starts"*"capped "[0-9]*" ("*"%)"*) ;; *) err "summary lacks a capped share: $s" ;; esac
done
rec=docs/eval/results/2026-10-25-solve-stats.md
if [ -f "$rec" ]; then
  grep -q 'solve-stats: mode 2v2 ' "$rec" && grep -q 'solve-stats: mode 1v1 ' "$rec" && grep -q '^Commit [0-9a-f]*, one thread' "$rec" || err "$rec lacks a mode or the settings line"
else
  err "$rec is missing"
fi
max1=$(awk '/^## 1v1/ {p = 1} p && /^\| [0-9]+ \|/ {split($0, f, "|"); if (f[6] + 0 > m) m = f[6] + 0} END {print m}' "$out")
if [ "$fail" -eq 0 ]; then
  echo "M3-03: PASS ${s2#solve-stats: }; ${s1#solve-stats: }; slowest 1v1 solve ${max1} ms; record $rec"
else
  echo "M3-03: FAIL"
fi
exit "$fail"
