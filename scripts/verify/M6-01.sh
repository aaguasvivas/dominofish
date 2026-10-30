#!/usr/bin/env bash
# M6-01: the game equity table W (docs/PLAN.md D20). For each mode, the
# committed docs/eval/equity/rounds-<mode>.txt holds 10,000 rounds after
# the first from self-play games of PIMC at its default limits on training
# deal seeds (4,000,000,000 + game, engine seed base 5,001,000,000), with
# its summary and run lines; the tables w-<mode>-100.txt and
# w-<mode>-200.txt rebuilt from it with `dominofish equity --from` equal the
# committed ones byte for byte; a simulation of 20,000 games from each of
# five round starts per target agrees with W (|z| < 4); the core tests pass:
# every round after the first of seeded bot games, re-scored and applied at
# its real start scores, equals the rules engine's end scores, the two
# teams' values sum to 1, and the recursion equals simulated games; and a
# short recording at 2 worlds gives the same files on 1 and 4 threads.
# Prints each table's half-split difference. --fast runs the same checks.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
dir=docs/eval/equity
out=runs/M6-01
mkdir -p "$out"
fail=0
err() { echo "ERROR $*"; fail=1; }
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; echo "M6-01: FAIL"; exit 1; }
cargo test -q -p dominofish-core --lib equity -- --show-output > "$out/tests.log" 2>&1 || err "the equity tests failed ($out/tests.log)"
grep -q 'test result: ok. 6 passed' "$out/tests.log" || err "the equity tests did not all pass ($out/tests.log)"
rescore=$(grep -o 'equity rescore: .*' "$out/tests.log" | sed 's/^equity rescore: //')
summary=""
halves=""
zmax=0
for mode in 1v1 2v2; do
  f="$dir/rounds-$mode.txt"
  [ -f "$f" ] || { err "$f is missing"; continue; }
  grep -qx "# equity rounds mode=$mode target=200 deals=4000000000+game engines=5001000000 worlds=64 nodes=10000000 table_bits=16" "$f" || err "$f: the header differs from D20"
  n=$(grep -c '^round ' "$f")
  [ "$n" -eq 10000 ] || err "$f: $n rounds (target 10,000)"
  grep -q "^# summary rounds=10000 " "$f" || err "$f: no summary line for 10,000 rounds"
  grep -q '^# run command=runs/bin/[0-9a-f]*/dominofish+equity+--mode+'"$mode"'+--rounds+10000+--seed+4000000000+--engine-seed+5001000000+' "$f" || err "$f: no run line with the registered command"
  [ "$(awk '/^round / && $3 < 2' "$f" | wc -l | tr -d ' ')" -eq 0 ] || err "$f: a round 1 is recorded"
  "$bin" equity --from "$f" --out-dir "$out/$mode" --check 20000 > "$out/check-$mode.log" 2>&1 || err "equity --from $f failed ($out/check-$mode.log)"
  for t in 100 200; do
    cmp -s "$dir/w-$mode-$t.txt" "$out/$mode/w-$mode-$t.txt" || err "$dir/w-$mode-$t.txt differs from the table rebuilt from $f"
    d=$(sed -n 's/^# halves: .* differ by at most \([0-9]*\) millionths$/\1/p' "$dir/w-$mode-$t.txt")
    halves="$halves $mode-$t:${d:-?}"
  done
  [ "$(grep -c '^equity check: ' "$out/check-$mode.log")" -eq 10 ] || err "$mode: not 10 simulated starts"
  bad=$(awk '/^equity check: / { z = $NF; if (z < 0) z = -z; if (z >= 4) n++ } END { print n + 0 }' "$out/check-$mode.log")
  [ "$bad" -eq 0 ] || err "$mode: $bad simulated starts with |z| >= 4 ($out/check-$mode.log)"
  zmax=$(awk -v m="$zmax" '/^equity check: / { z = $NF; if (z < 0) z = -z; if (z > m) m = z } END { print m }' "$out/check-$mode.log")
  summary="$summary $mode: $(sed -n 's/^# summary rounds=\([0-9]*\) games=\([0-9]*\) moves=[0-9]* decisions=\([0-9]*\) capped=\([0-9]*\)$/\1 rounds from \2 games (\3 searched decisions, \4 capped solves)/p' "$f");"
done
for t in 1 4; do
  "$bin" equity --mode 2v2 --rounds 12 --seed 4100000000 --engine-seed 5001000900 --worlds 2 --threads "$t" --out-dir "$out/threads-$t" > /dev/null || err "the recording on $t threads failed"
done
for name in rounds-2v2.txt w-2v2-100.txt w-2v2-200.txt; do
  cmp -s "$out/threads-1/$name" "$out/threads-4/$name" || err "$name differs between 1 and 4 threads"
done
if [ "$fail" -eq 0 ]; then
  echo "M6-01: PASS$summary tables rebuilt from the rounds equal the committed ones; simulated starts |z| at most $zmax (target < 4); halves differ by at most (millionths)$halves; rescore: $rescore; 1 and 4 threads give the same files"
else
  echo "M6-01: FAIL"
fi
exit "$fail"
