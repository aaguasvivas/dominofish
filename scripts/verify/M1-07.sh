#!/usr/bin/env bash
# M1-07 (BUILD.md M1d): the lockstep differential fuzzer against Capi's
# engine. Target: 0 mismatches over 1,000,000 random rounds per mode, every
# docs/RULES.md rule path of the mode above zero. CAPI_DIR is set explicitly;
# a SKIP (no pinned Capi commit) exits 1. --fast runs the first 10,000 rounds
# of the same seed per mode.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
export CAPI_DIR=/Users/Adelson/Desktop/personal/capi
rounds=1000000
[ "${1:-}" = "--fast" ] && rounds=10000
mkdir -p runs
cargo build --release -q -p dominofish 2> runs/M1-07.build.log || { echo "M1-07: FAIL build (runs/M1-07.build.log)"; exit 1; }
fail=0
summaries=""
for mode in 1v1 2v2; do
  start=$(date +%s)
  out=$(target/release/dominofish capi-fuzz --mode "$mode" --rounds "$rounds" --seed 1 2>&1)
  code=$?
  printf '%s\n' "$out" > "runs/M1-07.$mode.log"
  last=$(printf '%s\n' "$out" | tail -1)
  printf '%s\n' "$out" | grep '^ERROR' | head -5
  case "$last" in
    "capi-fuzz: PASS mode $mode seed 1 rounds $rounds "*"mismatches 0;"*) [ "$code" -eq 0 ] || fail=1 ;;
    *) echo "ERROR $mode: $last"; fail=1 ;;
  esac
  summaries="$summaries [$mode: ${last#capi-fuzz: } in $(( $(date +%s) - start ))s]"
done
if [ "$fail" -eq 0 ]; then
  echo "M1-07: PASS target 0 mismatches over $rounds rounds per mode:$summaries"
else
  echo "M1-07: FAIL$summaries"
fi
exit "$fail"
