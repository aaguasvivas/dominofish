#!/usr/bin/env bash
# M3-05: run R3 (docs/PLAN.md) is complete and as registered: 16 result
# files in docs/eval/results/2026-10-25-omniscient-r3/, row k with side A
# the omniscient bot at commit f77e9c2 and side B the k-th baseline (random,
# heaviest, accompanied, rule, four rows each), deal seeds 2,010,000,000 +
# 100,000 k with at most 1,000 round pairs or 300 game pairs, engine seed
# base 5,000,300,000 + k, the SPRT H0 0 H1 4 stopping at its decision (no
# --sprt-track), no fault, and a run line; dominofish report replays each
# row's SPRT from its pair lines, every row accepts H1, and REPORT.md is
# current. The full run also replays R3 with HEAD's binary, which counts
# capped solves: every pair line must equal the committed one, and no
# omniscient solve may reach its node cap. --fast skips the replay.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
dir=docs/eval/results/2026-10-25-omniscient-r3
n=$(ls "$dir"/row-*.txt 2>/dev/null | wc -l | tr -d ' ')
[ "$n" -eq 16 ] || err "$dir holds $n result files, not 16"
bots=(random heaviest accompanied rule)
for k in $(seq 0 15); do
  f=$(ls "$dir"/row-$k-*.txt 2>/dev/null | head -1)
  [ -n "$f" ] || { err "row $k is missing"; continue; }
  b=${bots[$((k / 4))]}
  unit=$([ $((k % 2)) -eq 0 ] && echo round || echo game)
  max=$([ "$unit" = round ] && echo 1000 || echo 300)
  deal=$((2010000000 + 100000 * k))
  header=$(head -1 "$f")
  for want in "a.commit=f77e9c2" "a.view=omniscient" "a.cmd=runs/bin/f77e9c2/dominofish+engine+--bot+omniscient " "b.cmd=runs/bin/f77e9c2/dominofish+engine+--bot+$b " "b.view=infoset" "unit=$unit" "pairs=$max"; do
    case "$header " in *"$want"*) ;; *) err "row $k: header lacks $want" ;; esac
  done
  grep -qx "# seeds deals=$deal..$((deal + max - 1)) engines=$((5000300000 + k))" "$f" || err "row $k: seeds differ from the registration"
  grep -q '^# summary .*sprt on the pair margin, H0 0 H1 4, alpha 0.05 beta 0.05: H1 after' "$f" || err "row $k: the SPRT is not H0 0 H1 4 with an H1 decision"
  grep -q -- '--sprt-track' "$f" && err "row $k: tracking mode, not stopping"
  grep -q '^# fault' "$f" && err "row $k has a fault"
  grep -q '^# run run=R3 row=' "$f" || err "row $k has no R3 run line"
done
cargo run -q -p dominofish -- report --check > /dev/null || err "docs/eval/REPORT.md is stale"
rows=$(awk '/^## 2026-10-25-omniscient-r3/ {p = 1; next} /^## / {p = 0} p && /^\| [12]v[12] \|/' docs/eval/REPORT.md)
total=$(printf '%s\n' "$rows" | grep -c '^|')
h1=$(printf '%s\n' "$rows" | grep -c '| H1 at ')
[ "$total" -eq 16 ] || err "REPORT.md shows $total R3 rows"
[ "$h1" -eq 16 ] || err "only $h1 of $total R3 rows accept H1"
pairs=$(printf '%s\n' "$rows" | awk -F'|' '{s += $7} END {print s + 0}')
replay="skipped (--fast)"
if [ "${1:-}" != "--fast" ]; then
  bin=$(scripts/build-bin.sh HEAD | tail -1)
  [ -x "$bin" ] || err "build failed: $bin"
  mkdir -p runs/M3-05
  capped=0
  for k in $(seq 0 15); do
    f=$(ls "$dir"/row-$k-*.txt | head -1)
    b=${bots[$((k / 4))]}
    mode=$([ $(((k / 2) % 2)) -eq 0 ] && echo 1v1 || echo 2v2)
    unit=$([ $((k % 2)) -eq 0 ] && echo round || echo game)
    max=$([ "$unit" = round ] && echo 1000 || echo 300)
    out=runs/M3-05/replay-$k.txt
    "$bin" match --mode "$mode" --unit "$unit" --target 100 --pairs "$max" --seed $((2010000000 + 100000 * k)) --engine-seed $((5000300000 + k)) --sprt 0,4 \
      --a "$bin engine --bot omniscient" --b "$bin engine --bot $b" --out "$out" > /dev/null || err "replay of row $k failed"
    [ "$(grep '^pair ' "$out")" = "$(grep '^pair ' "$f")" ] || err "replay of row $k differs from the committed pair lines"
    c=$(sed -n 's/^# capped a=\([0-9]*\) .*/\1/p' "$out")
    capped=$((capped + ${c:-1}))
  done
  [ "$capped" -eq 0 ] || err "$capped omniscient solves reached the node cap in the replay"
  replay="replayed with $bin: every pair line equal, $capped capped omniscient solves"
fi
if [ "$fail" -eq 0 ]; then
  echo "M3-05: PASS R3 as registered: the omniscient bot (cheating upper bound) accepts H1 against every baseline in $h1 of 16 rows (1v1 and 2v2, rounds and games to 100), $pairs pairs, seeds, bounds, maximums, and commit as registered, no fault; $replay; REPORT.md current"
else
  echo "M3-05: FAIL"
fi
exit "$fail"
