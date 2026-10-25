#!/usr/bin/env bash
# M1-10 (BUILD.md M1g): the playout bench. Rounds per second per mode on one
# core, native and wasm32 under Node (tools/wasm-bench.mjs loads the
# dominofish-wasm cdylib with WebAssembly.instantiate and calls its one
# extern "C" export). Passes when the checksums agree in both modes; the
# speeds are a recorded baseline, not a gate. Each timing runs twice and the
# 1-minute load average is printed with it. --fast uses 20,000 rounds.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
node=/Users/Adelson/.nvm/versions/node/v20.20.0/bin/node
rounds=200000
[ "${1:-}" = "--fast" ] && rounds=20000
mkdir -p runs
cargo build --release -q -p dominofish 2> runs/M1-10.build.log &&
  cargo build --release -q -p dominofish-wasm --target wasm32-unknown-unknown 2>> runs/M1-10.build.log ||
  { echo "M1-10: FAIL build (runs/M1-10.build.log)"; exit 1; }
wasm=target/wasm32-unknown-unknown/release/dominofish_wasm.wasm
fail=0
report=""
for mode in 1v1 2v2; do
  for run in 1 2; do
    n=$(target/release/dominofish playout-bench --mode "$mode" --rounds "$rounds" --seed 1)
    w=$("$node" tools/wasm-bench.mjs "$wasm" "$mode" "$rounds" 1)
    load=$(uptime | sed 's/.*load averages*: *//; s/,* .*//')
    nc=$(printf '%s' "$n" | sed 's/.* checksum \([0-9a-f]*\) .*/\1/')
    wc=$(printf '%s' "$w" | sed 's/.* checksum \([0-9a-f]*\) .*/\1/')
    nr=${n##*rounds/s }
    wr=${w##*rounds/s }
    printf '%s\n%s\n' "$n" "$w" >> runs/M1-10.log
    if [ -z "$nc" ] || [ "$nc" != "$wc" ]; then
      echo "ERROR $mode run $run: native checksum $nc, wasm checksum $wc"
      fail=1
    fi
    report="$report [$mode run $run: checksum $nc native $nr rounds/s wasm $wr rounds/s load $load]"
  done
done
if [ "$fail" -eq 0 ]; then
  echo "M1-10: PASS checksums equal native and wasm, $rounds rounds per mode, commit $(/usr/bin/git rev-parse --short HEAD), 1 thread:$report"
else
  echo "M1-10: FAIL$report"
fi
exit "$fail"
