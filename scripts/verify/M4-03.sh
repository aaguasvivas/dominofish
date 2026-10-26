#!/usr/bin/env bash
# M4-03: the PIMC engine (engine --bot pimc). With HEAD's runs/bin binary:
# - PIMC with 8 worlds plays every baseline in both modes, as rounds (10
#   pairs; --fast 4) and full games to 100 (3 pairs; --fast 1): 0 illegal
#   moves, crashes, timeouts, or sampler faults (an inconsistent world or a
#   fallback stops the match); each header shows a.budgets=worlds:8 with
#   the node cap, a.tables=1MB, a.view=infoset, and the Bench of its binary;
# - two runs of one match give the same pair lines (PIMC is seeded);
# - what each side receives (scripts/test-engines/spy.sh, 1v1 and 2v2):
#   the PIMC engine and the baseline get no state line;
# - a negative control: an engine that reports an inconsistent world
#   (scripts/test-engines/inconsistent.sh) makes the match fail with a
#   Sampler fault.
set -u
cd "$(dirname "$0")/../.."
fail=0
err() { echo "ERROR $*"; fail=1; }
pr=10; pg=3
[ "${1:-}" = "--fast" ] && { pr=4; pg=1; }
dir=runs/M4-03
rm -rf "$dir"; mkdir -p "$dir"
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; echo "M4-03: FAIL"; exit 1; }
bench=$("$bin" bench | tail -1)
pimc="$bin engine --bot pimc --worlds 8"
matches=0; moves=0
for bot in random heaviest accompanied rule; do
  for mode in 1v1 2v2; do
    for unit in round:$pr game:$pg; do
      u=${unit%%:*}; pairs=${unit#*:}
      out="$dir/$bot-$mode-$u.txt"
      line=$("$bin" match --mode "$mode" --unit "$u" --target 100 --pairs "$pairs" --seed 62000 --engine-seed 5000000403 \
        --a "$pimc" --b "$bin engine --bot $bot" --out "$out" | tail -1)
      case "$line" in "match: PASS"*"illegal 0, crashes 0, timeouts 0"*) ;; *) err "PIMC against $bot, $mode $u: $line" ;; esac
      grep -q '^# fault' "$out" && err "$out has a fault"
      header=$(head -1 "$out")
      for want in "a.bench=${bench#Bench: }" "a.budgets=worlds:8,nodes:10000000" "a.tables=1MB" "a.view=infoset" "b.view=infoset" "b.bench=-"; do
        case "$header" in *"$want"*) ;; *) err "$out header lacks $want" ;; esac
      done
      n=$(printf '%s' "$line" | sed -n 's/.* moves \([0-9]*\);.*/\1/p')
      moves=$((moves + ${n:-0})); matches=$((matches + 1))
    done
  done
done
"$bin" match --mode 2v2 --unit round --target 100 --pairs "$pr" --seed 62000 --engine-seed 5000000403 \
  --a "$pimc" --b "$bin engine --bot rule" --out "$dir/again.txt" > /dev/null || err "the repeat match failed"
[ "$(grep '^pair ' "$dir/again.txt")" = "$(grep '^pair ' "$dir/rule-2v2-round.txt")" ] || err "two runs of one match differ"
spy=""
for mode in 1v1 2v2; do
  "$bin" match --mode "$mode" --unit round --target 100 --pairs 3 --seed 63000 --engine-seed 5000000404 \
    --a "scripts/test-engines/spy.sh $dir/spy-a-$mode.log $pimc" \
    --b "scripts/test-engines/spy.sh $dir/spy-b-$mode.log $bin engine --bot rule" --out "$dir/spy-$mode.txt" > /dev/null || err "spy match failed ($mode)"
  sa=$(grep -c '^state ' "$dir/spy-a-$mode.log"); ia=$(grep -c '^infoset ' "$dir/spy-a-$mode.log"); ga=$(grep -c '^go$' "$dir/spy-a-$mode.log")
  sb=$(grep -c '^state ' "$dir/spy-b-$mode.log")
  [ "$sa" -eq 0 ] && [ "$ia" -eq "$ga" ] && [ "$ga" -gt 0 ] || err "$mode: the PIMC engine got $sa state lines and $ia infosets for $ga moves"
  [ "$sb" -eq 0 ] || err "$mode: the baseline got $sb state lines"
  spy="$spy $mode: PIMC $ia infosets and $sa state lines for $ga moves, baseline $sb state lines;"
done
control=$("$bin" match --mode 2v2 --unit round --target 100 --pairs 2 --seed 64000 --engine-seed 5000000405 \
  --a "scripts/test-engines/inconsistent.sh $bin engine --bot rule" --b "$bin engine --bot rule" --out "$dir/control.txt" 2>&1 | tail -1)
grep -q '^# fault Sampler: ' "$dir/control.txt" || err "the inconsistent engine did not stop the match with a Sampler fault: $control"
if [ "$fail" -eq 0 ]; then
  echo "M4-03: PASS $matches matches of PIMC (8 worlds) against every baseline (1v1 and 2v2, rounds and games), $moves moves, illegal 0, crashes 0, timeouts 0, sampler faults 0; header a.budgets=worlds:8,nodes:10000000 a.tables=1MB a.view=infoset with ${bench}; a repeated match gives equal pair lines;$spy an engine reporting an inconsistent world stops the match with a Sampler fault"
else
  echo "M4-03: FAIL"
fi
exit "$fail"
