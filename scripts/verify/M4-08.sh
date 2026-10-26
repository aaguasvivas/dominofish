#!/usr/bin/env bash
# M4-08: run R4 (docs/PLAN.md) is complete and as registered: 16 result
# files in docs/eval/results/2026-10-26-pimc-r4/, row k with side A PIMC at
# the default worlds (PIMC_WORLDS) at the R4 commit and side B the k-th
# baseline (random, heaviest, accompanied, rule, four rows each), deal
# seeds 2,020,000,000 + 100,000 k with at most 1,000 round pairs or 300
# game pairs, engine seed base 5,000,400,000 + k, the SPRT H0 0 H1 4
# stopping at its decision, --timeout-ms 120000, no fault, a `# time` line,
# and a run line with wall time and peak RSS; every row accepts H1 in
# REPORT.md, which is current. --fast checks the same files (no run).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
dir=docs/eval/results/2026-10-26-pimc-r4
commit=$(cat "$dir/COMMIT" 2>/dev/null)
[ -n "$commit" ] || err "$dir/COMMIT (the R4 commit) is missing"
worlds=$(/usr/bin/git show "$commit:crates/dominofish/src/solve.rs" 2>/dev/null | sed -n 's/^pub const PIMC_WORLDS: u32 = \([0-9]*\);/\1/p')
[ -n "$worlds" ] || err "no PIMC_WORLDS at $commit"
n=$(ls "$dir"/row-*.txt 2>/dev/null | wc -l | tr -d ' ')
[ "$n" -eq 16 ] || err "$dir holds $n result files, not 16"
bots=(random heaviest accompanied rule)
for k in $(seq 0 15); do
  f=$(ls "$dir"/row-$k-*.txt 2>/dev/null | head -1)
  [ -n "$f" ] || { err "row $k is missing"; continue; }
  b=${bots[$((k / 4))]}
  mode=$([ $(((k / 2) % 2)) -eq 0 ] && echo 1v1 || echo 2v2)
  unit=$([ $((k % 2)) -eq 0 ] && echo round || echo game)
  max=$([ "$unit" = round ] && echo 1000 || echo 300)
  deal=$((2020000000 + 100000 * k))
  header=$(head -1 "$f")
  for want in "a.commit=$commit" "a.cmd=runs/bin/$commit/dominofish+engine+--bot+pimc+--worlds+$worlds " "a.budgets=worlds:$worlds,nodes:10000000" "a.tables=1MB" "a.view=infoset" "b.cmd=runs/bin/$commit/dominofish+engine+--bot+$b " "b.view=infoset" "mode=$mode" "unit=$unit" "pairs=$max"; do
    case "$header " in *"$want"*) ;; *) err "row $k: header lacks $want" ;; esac
  done
  grep -qx "# seeds deals=$deal..$((deal + max - 1)) engines=$((5000400000 + k))" "$f" || err "row $k: seeds differ from the registration"
  grep -q '^# summary .*sprt on the pair margin, H0 0 H1 4, alpha 0.05 beta 0.05: H1 after' "$f" || err "row $k: the SPRT did not accept H1"
  grep -q '^# fault' "$f" && err "row $k has a fault"
  grep -q '^# time a=[0-9.]*,[0-9.]*,[0-9]* ' "$f" || err "row $k has no time line"
  grep -q "^# run run=R4 row=$k .*--timeout-ms+120000.* wall_s=[0-9.]* peak_rss_kb=[0-9]*" "$f" || err "row $k has no R4 run line with the timeout, wall time, and peak RSS"
done
cargo run -q -p dominofish -- report --check > /dev/null || err "docs/eval/REPORT.md is stale"
rows=$(awk '/^## 2026-10-26-pimc-r4/ {p = 1; next} /^## / {p = 0} p && /^\| [12]v[12] \|/' docs/eval/REPORT.md)
total=$(printf '%s\n' "$rows" | grep -c '^|')
h1=$(printf '%s\n' "$rows" | grep -c '| H1 at ')
pairs=$(printf '%s\n' "$rows" | awk -F'|' '{s += $7} END {print s + 0}')
[ "$total" -eq 16 ] || err "REPORT.md shows $total R4 rows"
[ "$h1" -eq 16 ] || err "only $h1 of $total R4 rows accept H1"
if [ "$fail" -eq 0 ]; then
  echo "M4-08: PASS R4 as registered: PIMC ($worlds worlds, commit $commit) accepts H1 against every baseline in $h1 of 16 rows (1v1 and 2v2, rounds and games to 100), $pairs pairs, seeds, bounds, maximums, and commit as registered, no fault; per-move time and peak RSS recorded; REPORT.md current"
else
  echo "M4-08: FAIL"
fi
exit "$fail"
