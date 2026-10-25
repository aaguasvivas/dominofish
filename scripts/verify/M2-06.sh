#!/usr/bin/env bash
# M2-06: the match harness. With HEAD's binary built into runs/bin/<commit>/
# (scripts/build-bin.sh), it checks:
# - matches in both modes, as rounds and full games, at targets 100 and 200,
#   with every seat a separate engine process: 0 illegal moves, crashes, or
#   timeouts, and a header (commit, Bench, binary hash, budgets, threads,
#   table sizes, seeds) that each engine echoed;
# - duplicate pairs: identical deterministic bots score exactly 0 in every
#   pair, so both games share the deals and the sides really swap seats;
# - each fault engine (illegal move, crash, silence, wrong header echo) stops
#   the match with that fault and exit 1;
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
      line=$("$bin" match --mode "$mode" --unit "$unit" --target "$target" --pairs "$pairs" --seed 4000 \
        --a "$bin engine --bot accompanied" --b "$bin engine --bot rule" --out "$out" | tail -1)
      case "$line" in
        "match: PASS"*"illegal 0, crashes 0, timeouts 0"*) ;;
        *) err "$mode $unit $target: $line" ;;
      esac
      header=$(head -1 "$out")
      for want in "a.commit=$commit" "b.commit=$commit" "a.bench=" "a.hash=" "b.hash=" "budgets=" "threads=1" "tables=" "seeds=4000.."; do
        case "$header" in *"$want"*) ;; *) err "$out header lacks $want" ;; esac
      done
      [ "$(grep -c '^pair ' "$out")" -eq "$pairs" ] || err "$out has fewer than $pairs pairs"
      moves=$((moves + $(printf '%s' "$line" | sed 's/.* moves \([0-9]*\) .*/\1/')))
      matches=$((matches + 1))
    done
  done
done
for mode in 1v1 2v2; do
  for unit in round game; do
    out="runs/M2-06/null-$mode-$unit.txt"
    "$bin" match --mode "$mode" --unit "$unit" --target 100 --pairs "$pairs_game" --seed 5000 \
      --a "$bin engine --bot heaviest" --b "$bin engine --bot heaviest" --out "$out" > /dev/null || err "null $mode $unit failed"
    nonzero=$(awk '$1 == "pair" && $(NF-2) != 0' "$out" | wc -l | tr -d ' ')
    [ "$nonzero" -eq 0 ] || err "identical bots: $nonzero pairs with a nonzero margin in $out"
  done
done
for kind in illegal:Illegal crash:Crash silent:Timeout bad-header:Header; do
  line=$("$bin" match --mode 2v2 --unit round --target 100 --pairs 3 --seed 6000 --timeout-ms 500 \
    --a "$bin engine --bot random" --b "scripts/test-engines/${kind%%:*}.sh" --out "runs/M2-06/fault-${kind%%:*}.txt" | tail -1)
  code=$?
  case "$line" in "match: FAIL ${kind#*:} "*) ;; *) err "fault engine ${kind%%:*}: $line" ;; esac
done
if "$bin" match --mode 1v1 --unit round --target 100 --pairs 1 --seed 1 --a "target/release/dominofish engine --bot random" \
    --b "$bin engine --bot random" --out runs/M2-06/refused.txt > /dev/null 2>&1; then
  err "a binary under target/ was accepted"
fi
if [ "$fail" -eq 0 ]; then
  echo "M2-06: PASS $matches matches (1v1 and 2v2, rounds and games, targets 100 and 200) from $bin, $moves moves, illegal 0, crashes 0, timeouts 0, header echoed; identical bots 0 in every duplicate pair (4 configurations); faults Illegal, Crash, Timeout, Header each stop a match; target/ refused"
else
  echo "M2-06: FAIL"
fi
exit "$fail"
