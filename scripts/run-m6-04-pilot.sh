#!/usr/bin/env bash
# The M6-04 pilot on dev deals (docs/PLAN.md, Seed ranges), not a strength
# result: PIMC with the win objective against PIMC with the points
# objective, both at the default limits (64 worlds per move), in full games
# with --aivat 8, with HEAD's runs/bin binary. It measures what sizes R7:
# each row's per-pair variance of the game-win share (each pair's wins / 2),
# its share of pairs that do not split 1 to 1, and its wall time per pair.
# Four rows at once, one thread each: k = 0, 1v1 to 100, 200 pairs; k = 1,
# 1v1 to 200, 100 pairs; k = 2, 2v2 to 100, 100 pairs; k = 3, 2v2 to 200,
# 50 pairs. Pair i of row k deals from 1,200,000,000 + 1,000,000 k + i; the
# engine seed base is 5,001,200,000 + k. Prints one line per row.
set -u
cd "$(dirname "$0")/.."
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; exit 1; }
dir=runs/M6-04-pilot
mkdir -p "$dir"
row() {
  local k=$1 mode=$2 target=$3 pairs=$4
  local out="$dir/row-$k-$mode-$target.txt"
  /usr/bin/time -l "$bin" match --mode "$mode" --unit game --target "$target" --pairs "$pairs" \
    --seed $((1200000000 + 1000000 * k)) --engine-seed $((5001200000 + k)) --aivat 8 --timeout-ms 120000 \
    --a "$bin engine --bot pimc --objective win" --b "$bin engine --bot pimc" \
    --out "$out" > "$dir/row-$k.out" 2> "$dir/row-$k.err" ||
    { echo "ERROR row $k failed ($dir/row-$k.err)"; return 1; }
  local wall rss
  wall=$(awk '/ real / {print $1}' "$dir/row-$k.err")
  rss=$(awk '/maximum resident set size/ {print int($1 / 1024)}' "$dir/row-$k.err")
  awk -v k="$k" -v m="$mode" -v t="$target" -v wall="$wall" -v rss="$rss" '
    $1 == "pair" { x = $NF / 2; n++; s += x; ss += x * x; if ($NF != 1) u++ }
    END { printf "pilot row %d %s to %d: %d pairs, win share %.4f, variance %.5f, not split %.3f, %.1f s per pair, peak RSS %d KB\n", k, m, t, n, s / n, (ss - s * s / n) / (n - 1), u / n, wall / n, rss }' "$out"
}
pids=""
row 0 1v1 100 200 & pids="$pids $!"
row 1 1v1 200 100 & pids="$pids $!"
row 2 2v2 100 100 & pids="$pids $!"
row 3 2v2 200 50 & pids="$pids $!"
fail=0
for p in $pids; do wait "$p" || fail=1; done
exit "$fail"
