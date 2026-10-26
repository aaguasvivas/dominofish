#!/usr/bin/env bash
# M3-05: run R3 (docs/PLAN.md) is complete: 16 result files in
# docs/eval/results/2026-10-25-omniscient-r3/, each with its header (side A
# view=omniscient), no fault line, and a run line; dominofish report replays
# each row's SPRT from its pair lines, and every row accepts H1 (the
# omniscient bot beats every baseline in both modes, as rounds and as
# games); REPORT.md is current. --fast runs the same checks (they read the
# committed files).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
dir=docs/eval/results/2026-10-25-omniscient-r3
n=$(ls "$dir"/row-*.txt 2>/dev/null | wc -l | tr -d ' ')
[ "$n" -eq 16 ] || err "$dir holds $n result files, not 16"
for f in "$dir"/row-*.txt; do
  [ -f "$f" ] || continue
  head -1 "$f" | grep -q 'a.view=omniscient' || err "$f: side A is not the omniscient bot"
  grep -q '^# fault' "$f" && err "$f has a fault"
  grep -q '^# run run=R3 ' "$f" || err "$f has no R3 run line"
done
cargo run -q -p dominofish -- report --check > /dev/null || err "docs/eval/REPORT.md is stale"
rows=$(awk '/^## 2026-10-25-omniscient-r3/ {p = 1; next} /^## / {p = 0} p && /^\| [12]v[12] \|/' docs/eval/REPORT.md)
total=$(printf '%s\n' "$rows" | grep -c '^|')
h1=$(printf '%s\n' "$rows" | grep -c '| H1 at ')
[ "$total" -eq 16 ] || err "REPORT.md shows $total R3 rows"
[ "$h1" -eq 16 ] || err "only $h1 of $total R3 rows accept H1"
pairs=$(printf '%s\n' "$rows" | awk -F'|' '{s += $7} END {print s + 0}')
if [ "$fail" -eq 0 ]; then
  echo "M3-05: PASS R3: the omniscient bot (cheating upper bound) accepts H1 against every baseline in $h1 of 16 rows (1v1 and 2v2, rounds and games to 100), $pairs pairs, no fault; REPORT.md current"
else
  echo "M3-05: FAIL"
fi
exit "$fail"
