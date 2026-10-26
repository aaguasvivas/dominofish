#!/usr/bin/env bash
# M0-03: dominofish selfplay finishes a full game to target 100 in 2v2 and
# 1v1 from seed 1, with identical output on two runs, and the cargo test that
# runs both passes. --fast runs the same checks.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
fail=0

cargo build --release -q -p dominofish 2> runs/M0-03.build.log || { echo "ERROR release build failed (runs/M0-03.build.log)"; exit 1; }
bin=target/release/dominofish
report=""
for mode in 2v2 1v1; do
  a=$("$bin" selfplay --mode "$mode" --seed 1) || { echo "ERROR selfplay $mode seed 1 exited non-zero"; fail=1; continue; }
  b=$("$bin" selfplay --mode "$mode" --seed 1)
  if [ "$a" != "$b" ]; then
    echo "ERROR selfplay $mode seed 1: two runs differ"
    fail=1
  fi
  last=$(printf '%s\n' "$a" | tail -1)
  # Final <TEAM> <score> <TEAM> <score> <TEAM> wins in R rounds, M moves
  read -r word _ s0 _ s1 _ _ _ rounds _ moves _ <<< "$last"
  if [ "$word" != Final ] || { [ "$s0" -lt 100 ] && [ "$s1" -lt 100 ]; }; then
    echo "ERROR selfplay $mode seed 1 did not finish at target 100: $last"
    fail=1
  fi
  report="$report $mode $s0-$s1 in $rounds rounds, $moves moves;"
done

if ! RUST_TEST_THREADS=4 cargo test -q -p dominofish --test selfplay > runs/M0-03.test.log 2>&1; then
  echo "ERROR cargo test --test selfplay failed (runs/M0-03.test.log)"
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "M0-03: PASS seed 1:$report identical on two runs; cargo test selfplay ok"
else
  echo "M0-03: FAIL"
fi
exit "$fail"
