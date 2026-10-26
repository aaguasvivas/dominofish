#!/usr/bin/env bash
# M4-09: run R5 (docs/PLAN.md) is complete and as registered: PIMC at each
# power of two from 1 to the default worlds (side A) against the rule bot,
# 1v1 then 2v2, rounds at target 100, 500 pairs each, no SPRT, every
# budget of a mode on the same deal seeds (2,030,000,000 + i in 1v1,
# 2,031,000,000 + i in 2v2), engine seed base 5,000,500,000 + k, the R5
# commit, no fault, a time line and a run line; REPORT.md is current and
# shows every point with its 95% CI. Prints the curve. --fast checks the
# same files (no run).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
dir=docs/eval/results/2026-10-26-pimc-curve-r5
commit=$(cat "$dir/COMMIT" 2>/dev/null)
[ -n "$commit" ] || err "$dir/COMMIT (the R5 commit) is missing"
worlds=$(/usr/bin/git show "$commit:crates/dominofish/src/solve.rs" 2>/dev/null | sed -n 's/^pub const PIMC_WORLDS: u32 = \([0-9]*\);/\1/p')
[ -n "$worlds" ] || err "no PIMC_WORLDS at $commit"
curve=""; w=1
while [ "$w" -le "${worlds:-0}" ]; do curve="$curve $w"; w=$((w * 2)); done
points=$(printf '%s\n' $curve | grep -c .)
n=$(ls "$dir"/row-*.txt 2>/dev/null | wc -l | tr -d ' ')
[ "$n" -eq $((2 * points)) ] || err "$dir holds $n result files, not $((2 * points))"
k=0
for mode in 1v1 2v2; do
  base=$([ "$mode" = 1v1 ] && echo 2030000000 || echo 2031000000)
  for w in $curve; do
    f=$(ls "$dir"/row-$k-*.txt 2>/dev/null | head -1)
    if [ -z "$f" ]; then err "row $k is missing"; k=$((k + 1)); continue; fi
    header=$(head -1 "$f")
    for want in "a.commit=$commit" "a.cmd=runs/bin/$commit/dominofish+engine+--bot+pimc+--worlds+$w " "a.budgets=worlds:$w,nodes:10000000" "b.cmd=runs/bin/$commit/dominofish+engine+--bot+rule " "mode=$mode" "unit=round" "pairs=500"; do
      case "$header " in *"$want"*) ;; *) err "row $k: header lacks $want" ;; esac
    done
    grep -qx "# seeds deals=$base..$((base + 499)) engines=$((5000500000 + k))" "$f" || err "row $k: seeds differ from the registration"
    [ "$(grep -c '^pair ' "$f")" -eq 500 ] || err "row $k: not 500 pairs"
    grep -q '^# fault' "$f" && err "row $k has a fault"
    grep -q '^# time a=' "$f" || err "row $k has no time line"
    grep -q "^# run run=R5 row=$k " "$f" || err "row $k has no R5 run line"
    k=$((k + 1))
  done
done
cargo run -q -p dominofish -- report --check > /dev/null || err "docs/eval/REPORT.md is stale"
rows=$(awk '/^## 2026-10-26-pimc-curve-r5/ {p = 1; next} /^## / {p = 0} p && /^\| [12]v[12] \|/' docs/eval/REPORT.md)
total=$(printf '%s\n' "$rows" | grep -c '^|')
[ "$total" -eq $((2 * points)) ] || err "REPORT.md shows $total R5 rows"
ci=$(printf '%s\n' "$rows" | awk -F'|' '$8 ~ /\[-?[0-9.]+, -?[0-9.]+\]/' | wc -l | tr -d ' ')
[ "$ci" -eq "$total" ] || err "$((total - ci)) R5 rows lack a CI"
line=$(printf '%s\n' "$rows" | awk -F'|' '{gsub(/^ +| +$/, "", $2); gsub(/^ +| +$/, "", $5); gsub(/^ +| +$/, "", $8); printf "%s %s %s; ", $2, $5, $8}')
if [ "$fail" -eq 0 ]; then
  echo "M4-09: PASS R5 as registered (commit $commit, 500 pairs per point, the same deals per mode, no fault): ${line% }"
else
  echo "M4-09: FAIL"
fi
exit "$fail"
