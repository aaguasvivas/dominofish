#!/usr/bin/env bash
# M1-06 (BUILD.md M1c): perft. fixtures/perft.txt equals a fresh run of
# tools/capi-oracle/perft.mjs at the pin; it holds depth-limited counts and
# complete-line counts from late positions in both modes; Rust matches every
# total and every divide entry exactly. --fast runs the same checks (seconds).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
export CAPI_DIR="${CAPI_DIR:-/Users/Adelson/Desktop/personal/capi}"
node=/Users/Adelson/.nvm/versions/node/v20.20.0/bin/node
fail=0
err() { echo "ERROR $*"; fail=1; }
mkdir -p runs

tools/capi-oracle/build.sh > runs/M1-06.build.log 2>&1 || { cat runs/M1-06.build.log; echo "M1-06: FAIL oracle build"; exit 1; }
"$node" tools/capi-oracle/perft.mjs runs/perft-regen.txt > runs/M1-06.perft.log 2>&1 || err "perft.mjs failed (runs/M1-06.perft.log)"
cmp -s fixtures/perft.txt runs/perft-regen.txt || err "fixtures/perft.txt differs from a fresh perft.mjs run"
# Per mode: 10 deals, each at depths 1 to 9, and one complete-line count
# from a late position of each deal.
for mode in 1v1 2v2; do
  deep=$(grep -c "^Q $mode-[0-9]*-d9 .* depth=9 " fixtures/perft.txt)
  full=$(grep -c "^Q $mode-[0-9]*-late .* depth=all " fixtures/perft.txt)
  [ "$deep" -eq 10 ] || err "$mode: $deep deals at depth 9, want 10"
  [ "$full" -eq 10 ] || err "$mode: $full complete-line queries from late positions, want 10"
done
late=$(grep -c ' depth=all ' fixtures/perft.txt)

out=$(cargo test -q -p dominofish-core --test perft -- --nocapture 2>&1)
summary=$(printf '%s\n' "$out" | grep -o 'perft: queries .*')
printf '%s\n' "$out" | grep '^ERROR' | head -5
case "$summary" in *"mismatches 0") ;; *) err "perft: ${summary:-no summary}" ;; esac

if [ "$fail" -eq 0 ]; then
  echo "M1-06: PASS $summary (target 0), totals and divide; $late complete-line queries from late positions; fixtures/perft.txt identical to a fresh perft.mjs run at 1d8868a"
else
  echo "M1-06: FAIL"
fi
exit "$fail"
