#!/usr/bin/env bash
# M4-10: run R6 (docs/PLAN.md) is complete and as registered: the
# omniscient bot (side A, cheating) against PIMC at the default worlds
# (side B), 1v1 (row 0) and 2v2 (row 1), rounds at target 100, 500 pairs
# each, no SPRT, deal seeds 2,040,000,000 + 100,000 k, engine seed base
# 5,000,600,000 + k, the R6 commit, no fault, time and run lines; REPORT.md
# is current and shows the gap with its 95% CI. --fast checks the same
# files (no run).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
dir=docs/eval/results/2026-10-26-pimc-gap-r6
commit=$(cat "$dir/COMMIT" 2>/dev/null)
[ -n "$commit" ] || err "$dir/COMMIT (the R6 commit) is missing"
worlds=$(/usr/bin/git show "$commit:crates/dominofish/src/solve.rs" 2>/dev/null | sed -n 's/^pub const PIMC_WORLDS: u32 = \([0-9]*\);/\1/p')
[ -n "$worlds" ] || err "no PIMC_WORLDS at $commit"
for k in 0 1; do
  mode=$([ "$k" -eq 0 ] && echo 1v1 || echo 2v2)
  f=$(ls "$dir"/row-$k-*.txt 2>/dev/null | head -1)
  [ -n "$f" ] || { err "row $k is missing"; continue; }
  header=$(head -1 "$f")
  for want in "a.commit=$commit" "a.cmd=runs/bin/$commit/dominofish+engine+--bot+omniscient " "a.view=omniscient" "b.cmd=runs/bin/$commit/dominofish+engine+--bot+pimc+--worlds+$worlds " "b.view=infoset" "mode=$mode" "unit=round" "pairs=500"; do
    case "$header " in *"$want"*) ;; *) err "row $k: header lacks $want" ;; esac
  done
  deal=$((2040000000 + 100000 * k))
  grep -qx "# seeds deals=$deal..$((deal + 499)) engines=$((5000600000 + k))" "$f" || err "row $k: seeds differ from the registration"
  [ "$(grep -c '^pair ' "$f")" -eq 500 ] || err "row $k: not 500 pairs"
  grep -q '^# fault' "$f" && err "row $k has a fault"
  grep -q "^# run run=R6 row=$k " "$f" || err "row $k has no R6 run line"
done
cargo run -q -p dominofish -- report --check > /dev/null || err "docs/eval/REPORT.md is stale"
rows=$(awk '/^## 2026-10-26-pimc-gap-r6/ {p = 1; next} /^## / {p = 0} p && /^\| [12]v[12] \|/' docs/eval/REPORT.md)
[ "$(printf '%s\n' "$rows" | grep -c '^|')" -eq 2 ] || err "REPORT.md does not show both R6 rows"
gaps=$(printf '%s\n' "$rows" | awk -F'|' '{gsub(/^ +| +$/, "", $2); gsub(/^ +| +$/, "", $8); printf "%s %s; ", $2, $8}')
case "$gaps" in *"["*"]"*"["*"]"*) ;; *) err "the gaps lack their CIs: $gaps" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M4-10: PASS R6 as registered (commit $commit, 500 pairs per mode, no fault): the omniscient bot (cheating) minus PIMC ($worlds worlds) in points per round [95% CI]: ${gaps% }"
else
  echo "M4-10: FAIL"
fi
exit "$fail"
