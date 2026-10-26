#!/usr/bin/env bash
# M2-06: the match harness. With HEAD's binary built into runs/bin/<commit>/
# (scripts/build-bin.sh), it checks:
# - matches in both modes, as rounds and full games, at targets 100 and 200,
#   with every seat a separate engine process: 0 illegal moves, crashes, or
#   timeouts, and a header (each side's commit, Bench, binary hash, and the
#   budgets, threads, and tables its engines reported) that each engine
#   echoed; the header holds no seed, and the deal and engine seeds are in
#   the results file only;
# - what an engine receives (scripts/test-engines/spy.sh): the same game
#   seeds under two different deal seed bases, so no engine seed is a
#   function of a deal;
# - duplicate pairs: identical deterministic bots score exactly 0 in every
#   pair, so both games share the deals and the sides really swap seats;
# - in both modes, each fault engine stops the match with its fault and
#   exit 1: a draw on every turn and a pass on every turn (Illegal), a crash,
#   silence and a flood of info lines past the per-move deadline (Timeout),
#   a move line out of turn (Protocol), and a wrong header echo (Header);
# - a binary under target/ is refused.
# --fast plays fewer pairs.
set -u
cd "$(dirname "$0")/../.."
fail=0
err() { echo "ERROR $*"; fail=1; }
pairs_round=20
pairs_game=10
if [ "${1:-}" = "--fast" ]; then pairs_round=6; pairs_game=3; fi
mkdir -p runs/M2-06
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; echo "M2-06: FAIL"; exit 1; }
commit=$(basename "$(dirname "$bin")")
moves=0
matches=0
for mode in 1v1 2v2; do
  for unit in round game; do
    for target in 100 200; do
      pairs=$pairs_round
      [ "$unit" = game ] && pairs=$pairs_game
      out="runs/M2-06/$mode-$unit-$target.txt"
      line=$("$bin" match --mode "$mode" --unit "$unit" --target "$target" --pairs "$pairs" --seed 4000 --engine-seed 5000000060 \
        --a "$bin engine --bot accompanied" --b "$bin engine --bot rule" --out "$out" | tail -1)
      case "$line" in
        "match: PASS"*"illegal 0, crashes 0, timeouts 0"*) ;;
        *) err "$mode $unit $target: $line" ;;
      esac
      header=$(head -1 "$out")
      for want in "a.commit=$commit" "b.commit=$commit" "a.bench=" "b.bench=" "a.hash=" "b.hash=" \
        "a.budgets=- a.threads=1 a.tables=-" "b.budgets=- b.threads=1 b.tables=-"; do
        case "$header" in *"$want"*) ;; *) err "$out header lacks $want" ;; esac
      done
      case "$header" in *seed*) err "$out header holds a seed" ;; esac
      grep -qx "# seeds deals=4000..$((4000 + pairs - 1)) engines=5000000060" "$out" || err "$out lacks the seeds line"

      [ "$(grep -c '^pair ' "$out")" -eq "$pairs" ] || err "$out has fewer than $pairs pairs"
      n=$(printf '%s' "$line" | sed -n 's/.* moves \([0-9]*\);.*/\1/p')
      [ -n "$n" ] || { err "$mode $unit $target: no move count in: $line"; n=0; }
      moves=$((moves + n))
      matches=$((matches + 1))
    done
  done
done
for mode in 1v1 2v2; do
  for unit in round game; do
    out="runs/M2-06/null-$mode-$unit.txt"
    "$bin" match --mode "$mode" --unit "$unit" --target 100 --pairs "$pairs_game" --seed 5000 --engine-seed 5000000062 \
      --a "$bin engine --bot heaviest" --b "$bin engine --bot heaviest" --out "$out" > /dev/null || err "null $mode $unit failed"
    nonzero=$(awk '$1 == "pair" && $(NF-2) != 0' "$out" | wc -l | tr -d ' ')
    [ "$nonzero" -eq 0 ] || err "identical bots: $nonzero pairs with a nonzero margin in $out"
  done
done
for base in 4000 4100; do
  rm -f "runs/M2-06/spy-$base.log"
  "$bin" match --mode 1v1 --unit game --target 100 --pairs 3 --seed "$base" --engine-seed 5000000063 \
    --a "scripts/test-engines/spy.sh runs/M2-06/spy-$base.log $bin engine --bot rule" --b "$bin engine --bot rule" \
    --out "runs/M2-06/spy-$base.txt" > /dev/null || err "spy match at $base failed"
done
grep -q '^header ' runs/M2-06/spy-4000.log || err "the spy saw no header"
if grep '^header ' runs/M2-06/spy-4000.log | grep -q seed; then err "an engine received a header with a seed"; fi
seeds4000=$(grep '^game ' runs/M2-06/spy-4000.log)
[ "$(printf '%s\n' "$seeds4000" | wc -l | tr -d ' ')" -eq 6 ] || err "the spy saw $(printf '%s\n' "$seeds4000" | wc -l) games, not 6"
[ "$seeds4000" = "$(grep '^game ' runs/M2-06/spy-4100.log)" ] || err "engine seeds change with the deal seeds"
faults=0
for mode in 1v1 2v2; do
  for kind in illegal:Illegal pass:Illegal crash:Crash silent:Timeout flood:Timeout stray:Protocol bad-header:Header; do
    line=$("$bin" match --mode "$mode" --unit round --target 100 --pairs 3 --seed 6000 --engine-seed 5000000061 --timeout-ms 500 \
      --a "$bin engine --bot random" --b "scripts/test-engines/${kind%%:*}.sh" --out "runs/M2-06/fault-$mode-${kind%%:*}.txt" | tail -1)
    case "$line" in "match: FAIL ${kind#*:} "*) faults=$((faults + 1)) ;; *) err "fault engine ${kind%%:*} in $mode: $line" ;; esac
  done
done
if "$bin" match --mode 1v1 --unit round --target 100 --pairs 1 --seed 1 --engine-seed 5000000064 --a "target/release/dominofish engine --bot random" \
    --b "$bin engine --bot random" --out runs/M2-06/refused.txt > /dev/null 2>&1; then
  err "a binary under target/ was accepted"
fi
if [ "$fail" -eq 0 ]; then
  echo "M2-06: PASS $matches matches (1v1 and 2v2, rounds and games, targets 100 and 200) from $bin, $moves moves, illegal 0, crashes 0, timeouts 0, header echoed with each side's settings and no seed; engine seeds equal under two deal seed bases; identical bots 0 in every duplicate pair (4 configurations); $faults of 14 fault matches (7 engines, both modes) stopped with their fault; target/ refused"
else
  echo "M2-06: FAIL"
fi
exit "$fail"
