#!/usr/bin/env bash
# M1-04 (BUILD.md M1a): golden traces. The committed fixtures equal a fresh
# run of tools/capi-oracle/traces.mjs at the pinned commit; they hold both
# modes at targets 100 and 200 and the four whole-suit seeds; they stay under
# 20 MB; they reach every rule path Capi's fuzz counts (each in its modes)
# and left-end leads in both modes; Rust replays them with 0 mismatches.
# --fast runs the same checks (seconds).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
export CAPI_DIR="${CAPI_DIR:-/Users/Adelson/Desktop/personal/capi}"
node=/Users/Adelson/.nvm/versions/node/v20.20.0/bin/node
fail=0
err() { echo "ERROR $*"; fail=1; }
mkdir -p runs

tools/capi-oracle/build.sh > runs/M1-04.build.log 2>&1 || { cat runs/M1-04.build.log; echo "M1-04: FAIL oracle build"; exit 1; }
regen=runs/golden-regen
rm -rf "$regen"
"$node" tools/capi-oracle/traces.mjs "$regen" > runs/M1-04.traces.log 2>&1 || err "traces.mjs failed (runs/M1-04.traces.log)"
diff -rq fixtures/golden "$regen" > /dev/null || err "fixtures/golden differ from a fresh traces.mjs run"

# Each file holds games of its own mode and target only: 150 seeded games
# per mode at 100 and at 200, and the four whole-suit seeds at both targets.
games() { grep -c "^G $2 " "fixtures/golden/$1.txt" 2> /dev/null || echo 0; }
all_games() { grep -c "^G " "fixtures/golden/$1.txt" 2> /dev/null || echo 0; }
for spec in "1v1-100:1v1 100:150" "1v1-200:1v1 200:150" "2v2-100:2v2 100:150" "2v2-200:2v2 200:150" \
            "2v2-whole-suit:2v2 100:4" "2v2-whole-suit-200:2v2 200:4"; do
  f=${spec%%:*}; rest=${spec#*:}; want=${rest#*:}; mt=${rest%:*}
  n=$(games "$f" "$mt"); total=$(all_games "$f")
  [ "$n" -eq "$want" ] && [ "$total" -eq "$want" ] || err "fixtures/golden/$f.txt: $n games of $mt and $total in all, want $want"
done
for f in 2v2-whole-suit:100 2v2-whole-suit-200:200; do
  for seed in 59313 83322 282144 645301; do
    grep -q "^G 2v2 ${f#*:} $seed capi-fuzz$" "fixtures/golden/${f%%:*}.txt" || err "whole-suit seed $seed missing at target ${f#*:}"
  done
done
kb=$(du -sk fixtures | awk '{print $1}')
[ "$kb" -lt 20480 ] || err "fixtures/ is ${kb} KB, target under 20 MB"

min=""
for want in "1v1 capicua" "1v1 capicua-equal-ends" "1v1 capicua-double" "1v1 domino-double-one-end" \
            "1v1 tranque-blocker-wins" "1v1 tranque-rival-wins" "1v1 tranque-tie" "1v1 lead-left" \
            "2v2 capicua" "2v2 capicua-equal-ends" "2v2 capicua-double" "2v2 domino-double-one-end" \
            "2v2 tranque-blocker-wins" "2v2 tranque-rival-wins" "2v2 tranque-tie" \
            "2v2 tranque-tie-opener-not-compared" "2v2 pase-corrido-paid" "2v2 pase-corrido-capped" \
            "2v2 salida-paid" "2v2 salida-cancelled" "2v2 salida-capped" "2v2 salida-cancelled-then-corrido" \
            "2v2 lead-left"; do
  n=$(awk -v k="$want" '$1 " " $2 == k {print $3}' fixtures/golden/coverage.txt)
  if [ -z "$n" ] || [ "$n" -le 0 ]; then
    err "path $want not reached"
  elif [ -z "$min" ] || [ "$n" -lt "${min%% *}" ]; then
    min="$n ($want)"
  fi
done

out=$(cargo test -q -p dominofish-core --test golden -- --nocapture 2>&1)
summary=$(printf '%s\n' "$out" | grep -o 'golden: files .*')
printf '%s\n' "$out" | grep '^ERROR' | head -5
case "$summary" in
  *"mismatches 0") ;;
  *) err "replay: ${summary:-no summary}" ;;
esac

if [ "$fail" -eq 0 ]; then
  echo "M1-04: PASS $summary (target 0); fixtures/ ${kb} KB (target under 20480); 150 games per mode at 100 and 200 and the whole-suit seeds at both; identical to a fresh traces.mjs run at 1d8868a; Capi's fuzz paths and left leads reached, fewest $min"
else
  echo "M1-04: FAIL"
fi
exit "$fail"
