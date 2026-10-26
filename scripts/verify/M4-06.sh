#!/usr/bin/env bash
# M4-06: dominofish world-stats. Over 300 seeded random games per mode
# (selfplay seeds 1 to 300), the median exact world count at decisions by
# board size, with the research medians beside it (2v2: 7,350 at 13 board
# tiles and 22 at 20) as a reference only; the committed record
# docs/eval/results/2026-10-26-world-stats.md must equal a fresh run.
# --fast runs the same commands (under a second).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
cargo build -q --release -p dominofish || err "build failed"
mkdir -p runs/M4-06
fresh=runs/M4-06/world-stats.md
{
  echo "# World counts by board size"
  echo
  echo "Written by scripts/verify/M4-06.sh: \`dominofish world-stats --mode <m> --games 300 --seed 1\`."
  for mode in 2v2 1v1; do
    echo
    echo "## $mode"
    echo
    target/release/dominofish world-stats --mode "$mode" --games 300 --seed 1 | sed '/^world-stats: /d'
  done
} > "$fresh"
committed=docs/eval/results/2026-10-26-world-stats.md
[ -f "$committed" ] || cp "$fresh" "$committed"
cmp -s "$fresh" "$committed" || err "$committed differs from a fresh run ($fresh)"
r13=$(grep -m1 '^| 13 |' "$fresh"); r20=$(grep -m1 '^| 20 |' "$fresh")
case "$r13" in *"| 7,350 |") ;; *) err "the 13-tile 2v2 row lacks the research median: $r13" ;; esac
case "$r20" in *"| 22 |") ;; *) err "the 20-tile 2v2 row lacks the research median: $r20" ;; esac
m13=$(printf '%s' "$r13" | awk -F'|' '{gsub(/ /, "", $4); print $4}')
m20=$(printf '%s' "$r20" | awk -F'|' '{gsub(/ /, "", $4); print $4}')
d=$(grep -o 'decisions [0-9]*$' "$fresh" | awk '{s += $2} END {print s + 0}')
[ "$d" -gt 0 ] || err "no decisions"
if [ "$fail" -eq 0 ]; then
  echo "M4-06: PASS 300 games per mode, $d decisions; 2v2 median worlds $m13 at 13 board tiles (research 7,350) and $m20 at 20 (research 22), reference only; $committed equals a fresh run"
else
  echo "M4-06: FAIL"
fi
exit "$fail"
