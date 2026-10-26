#!/usr/bin/env bash
# M3-04: the omniscient bot (engine --bot omniscient), labeled as cheating
# wherever it appears. With HEAD's runs/bin binary:
# - it plays every baseline in both modes, as rounds (10 pairs; --fast 4)
#   and full games to 100 (3 pairs; --fast 1): 0 illegal moves, crashes, or
#   timeouts; each header shows a.view=omniscient with its node cap, table,
#   and the Bench of its binary, and b.view=infoset;
# - what each side receives (scripts/test-engines/spy.sh, 1v1): the
#   omniscient engine gets a state line before its moves, and the baseline
#   gets none;
# - dominofish report labels the side "(cheating)" in the table and in the
#   settings line.
set -u
cd "$(dirname "$0")/../.."
fail=0
err() { echo "ERROR $*"; fail=1; }
pr=10; pg=3
[ "${1:-}" = "--fast" ] && { pr=4; pg=1; }
dir=runs/M3-04
rm -rf "$dir"; mkdir -p "$dir/results/2026-10-25-omniscient-check"
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; echo "M3-04: FAIL"; exit 1; }
bench=$("$bin" bench | tail -1)
matches=0; moves=0
for bot in random heaviest accompanied rule; do
  for mode in 1v1 2v2; do
    for unit in round:$pr game:$pg; do
      u=${unit%%:*}; pairs=${unit#*:}
      out="$dir/$bot-$mode-$u.txt"
      line=$("$bin" match --mode "$mode" --unit "$u" --target 100 --pairs "$pairs" --seed 60000 --engine-seed 5000000304 \
        --a "$bin engine --bot omniscient" --b "$bin engine --bot $bot" --out "$out" | tail -1)
      case "$line" in "match: PASS"*"illegal 0, crashes 0, timeouts 0"*) ;; *) err "omniscient against $bot, $mode $u: $line" ;; esac
      header=$(head -1 "$out")
      for want in "a.bench=${bench#Bench: }" "a.budgets=nodes:10000000" "a.tables=16MB" "a.view=omniscient" "b.view=infoset" "b.bench=-"; do
        case "$header" in *"$want"*) ;; *) err "$out header lacks $want" ;; esac
      done
      n=$(printf '%s' "$line" | sed -n 's/.* moves \([0-9]*\);.*/\1/p')
      moves=$((moves + ${n:-0})); matches=$((matches + 1))
    done
  done
done
"$bin" match --mode 1v1 --unit round --target 100 --pairs 3 --seed 61000 --engine-seed 5000000305 \
  --a "scripts/test-engines/spy.sh $dir/spy-a.log $bin engine --bot omniscient" \
  --b "scripts/test-engines/spy.sh $dir/spy-b.log $bin engine --bot rule" --out "$dir/spy.txt" > /dev/null || err "spy match failed"
sa=$(grep -c '^state ' "$dir/spy-a.log"); ga=$(grep -c '^go$' "$dir/spy-a.log")
sb=$(grep -c '^state ' "$dir/spy-b.log"); gb=$(grep -c '^go$' "$dir/spy-b.log")
[ "$sa" -eq "$ga" ] && [ "$sa" -gt 0 ] || err "the omniscient engine got $sa state lines for $ga moves"
[ "$sb" -eq 0 ] && [ "$gb" -gt 0 ] || err "the baseline got $sb state lines (must be 0) over $gb moves"
cp "$dir/rule-2v2-round.txt" "$dir/results/2026-10-25-omniscient-check/row-0-2v2-round-100-omniscient-rule.txt"
"$bin" report --results "$dir/results" --out "$dir/REPORT.md" > /dev/null || err "report failed"
grep -q '| omniscient (cheating) | rule |' "$dir/REPORT.md" || err "the report table does not mark the omniscient side"
grep -q 'view omniscient (sees every hand: cheating)' "$dir/REPORT.md" || err "the report settings do not mark the omniscient side"
if [ "$fail" -eq 0 ]; then
  echo "M3-04: PASS $matches matches of the omniscient bot against every baseline (1v1 and 2v2, rounds and games), $moves moves, illegal 0, crashes 0, timeouts 0; header view=omniscient with ${bench}; state lines: omniscient $sa for $ga moves, baseline $sb for $gb moves; report marks it cheating"
else
  echo "M3-04: FAIL"
fi
exit "$fail"
