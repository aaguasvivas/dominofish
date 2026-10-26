#!/usr/bin/env bash
# M2-08: null tests, with HEAD's runs/bin binary.
# 1. Identical deterministic bots (each of the four, RNG keyed by deal and
#    seat) score exactly 0 in every duplicate pair, in both modes, as rounds
#    and as full games.
# 2. T2 (docs/PLAN.md, pre-registered): 200 null SPRT runs (100 per mode)
#    between independently seeded copies of the random bot accept H1 a number
#    of times in [3, 19], under 5% end inconclusive, and none has a fault.
# --fast plays fewer pairs in part 1 and runs 4 null SPRT runs (2 per mode)
# without judging the rate.
set -u
cd "$(dirname "$0")/../.."
fail=0
err() { echo "ERROR $*"; fail=1; }
pr=30; pg=10; runs=100
if [ "${1:-}" = "--fast" ]; then pr=8; pg=3; runs=2; fi
mkdir -p runs/M2-08
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; echo "M2-08: FAIL"; exit 1; }
zero_pairs=0
for bot in random heaviest accompanied rule; do
  for mode in 1v1 2v2; do
    for unit in round game; do
      pairs=$pr; [ "$unit" = game ] && pairs=$pg
      out="runs/M2-08/null-$bot-$mode-$unit.txt"
      "$bin" match --mode "$mode" --unit "$unit" --target 100 --pairs "$pairs" --seed 9500000 --engine-seed 5000000080 \
        --a "$bin engine --bot $bot" --b "$bin engine --bot $bot" --out "$out" > /dev/null || err "$bot $mode $unit: match failed"
      nonzero=$(awk '$1 == "pair" && $(NF-2) != 0' "$out" | wc -l | tr -d ' ')
      [ "$nonzero" -eq 0 ] || err "$bot $mode $unit: $nonzero pairs with a nonzero margin ($out)"
      zero_pairs=$((zero_pairs + $(grep -c '^pair ' "$out")))
    done
  done
done
h1=0; h0=0; open=0; total=0; pairs_played=0
for mode in 1v1 2v2; do
  for k in $(seq 0 $((runs - 1))); do
    kk=$k; [ "$mode" = 2v2 ] && kk=$((k + 100))
    out="runs/M2-08/t2-$mode-$k.txt"
    line=$("$bin" match --mode "$mode" --unit round --target 100 --pairs 2000 --seed $((9000000 + 10000 * kk)) --engine-seed 5000000081 --sprt 0,10 \
      --a "$bin engine --bot random --salt $((2 * kk + 1))" --b "$bin engine --bot random --salt $((2 * kk + 2))" --out "$out")
    case "$line" in *"match: PASS"*) ;; *) err "T2 run $mode $k: $(printf '%s' "$line" | tail -1)" ;; esac
    case "$line" in
      *": H1 after"*) h1=$((h1 + 1)) ;;
      *": H0 after"*) h0=$((h0 + 1)) ;;
      *) open=$((open + 1)) ;;
    esac
    total=$((total + 1))
    pairs_played=$((pairs_played + $(grep -c '^pair ' "$out")))
  done
done
if [ "${1:-}" != "--fast" ]; then
  [ "$h1" -ge 3 ] && [ "$h1" -le 19 ] || err "T2: $h1 of $total null runs accepted H1, band [3, 19]"
  [ $((open * 20)) -lt "$total" ] || err "T2: $open of $total runs inconclusive (limit under 5%)"
fi
if [ "$fail" -eq 0 ]; then
  echo "M2-08: PASS identical bots: $zero_pairs duplicate pairs (4 bots, both modes, rounds and games) all exactly 0; null SPRT runs: $total ($runs per mode), H1 $h1 (band [3, 19] at 200 runs), H0 $h0, inconclusive $open, $pairs_played pairs, no fault"
else
  echo "M2-08: FAIL"
fi
exit "$fail"
