#!/usr/bin/env bash
# M5-03: test T5 (docs/PLAN.md) on its 14 committed result files in
# docs/eval/results/2026-10-27-aivat-t5/: every row as registered (commit,
# engines, seeds, pairs, --aivat 8, no fault); (a) per row, the mean of
# corrected minus raw points per round has |t| <= 3.29; (b) the sum of t^2
# over the 14 rows is at most 36.12 (the 0.999 quantile of chi-square with
# 14 df); (c) in the 6 null rows (0, 1, 6, 7, 8, 13) the corrected mean's t
# is within 3.29 of 0; (d) the full run builds the mutant (action terms
# against the policy's maximum, not its mean) from HEAD, replays rows 2, 3,
# 9, and 10 with it, and requires |t| > 3.29 in at least one; and prints
# each row's SD ratio, raw over corrected. --fast skips (d).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
dir=docs/eval/results/2026-10-27-aivat-t5
commit=$(cat "$dir/COMMIT" 2>/dev/null)
[ -n "$commit" ] || err "$dir/COMMIT is missing"
names=(rule-rule random-random rule-random accompanied-heaviest pimc8-rule rule-random rule-rule)
units=(round round round round round game game)
pairs=(1000 1000 1000 1000 300 300 300)
# field <file> <name>: a number after "<name> " in the # aivat line.
field() { grep '^# aivat' "$1" | sed -n "s/.* $2 \(-\{0,1\}[0-9.]*\).*/\1/p" | head -1; }
sum_t2=0; ratios=""
for k in $(seq 0 13); do
  i=$((k % 7)); mode=$([ "$k" -lt 7 ] && echo 1v1 || echo 2v2)
  f="$dir/row-$k-$mode-${units[$i]}-100-${names[$i]}.txt"
  [ -f "$f" ] || { err "row $k: $f is missing"; continue; }
  header=$(head -1 "$f")
  for want in "a.commit=$commit" "mode=$mode" "unit=${units[$i]}" "pairs=${pairs[$i]}"; do
    case "$header " in *"$want"*) ;; *) err "row $k: header lacks $want" ;; esac
  done
  deal=$((610000000 + 1000000 * k))
  grep -qx "# seeds deals=$deal..$((deal + ${pairs[$i]} - 1)) engines=$((5000700000 + k))" "$f" || err "row $k: seeds differ"
  [ "$(grep -c '^pair ' "$f")" -eq "${pairs[$i]}" ] || err "row $k: not ${pairs[$i]} pairs"
  [ "$(grep -c '^aivat ' "$f")" -eq "${pairs[$i]}" ] || err "row $k: not one aivat line per pair"
  grep -q '^# fault' "$f" && err "row $k has a fault"
  grep -q "^# run run=T5 row=$k .*--aivat+8+--timeout-ms+120000" "$f" || err "row $k: no T5 run line with --aivat 8"
  grep -q '^# aivat j=8 ' "$f" || err "row $k: no aivat summary"
  t=$(grep '^# aivat' "$f" | sed -n 's/.*corrected minus raw mean [-0-9.]* se [0-9.]* t \(-\{0,1\}[0-9.]*\);.*/\1/p')
  [ -n "$t" ] || { err "row $k: no t"; continue; }
  awk -v t="$t" 'BEGIN { exit !(t <= 3.29 && t >= -3.29) }' || err "row $k: corrected minus raw t $t (target |t| <= 3.29)"
  sum_t2=$(awk -v s="$sum_t2" -v t="$t" 'BEGIN { print s + t * t }')
  raw=$(field "$f" "raw sd"); csd=$(grep '^# aivat' "$f" | sed -n 's/.*(AIVAT, cross-fitted) mean -\{0,1\}[0-9.]* sd \([0-9.]*\) .*/\1/p')
  cmean=$(grep '^# aivat' "$f" | sed -n 's/.*(AIVAT, cross-fitted) mean \(-\{0,1\}[0-9.]*\) .*/\1/p')
  case $i in 0|1|6)
    ct=$(awk -v m="$cmean" -v s="$csd" -v n="${pairs[$i]}" 'BEGIN { printf "%.3f", m / (s / sqrt(n)) }')
    awk -v t="$ct" 'BEGIN { exit !(t <= 3.29 && t >= -3.29) }' || err "null row $k: corrected mean t $ct (target |t| <= 3.29)"
    ;;
  esac
  ratios="$ratios $k:$(awk -v a="$raw" -v b="$csd" 'BEGIN { printf "%.2f", a / b }')"
done
awk -v s="$sum_t2" 'BEGIN { exit !(s <= 36.12) }' || err "sum of t^2 $sum_t2 over 14 rows (target at most 36.12)"
control="skipped (--fast)"
if [ "${1:-}" != "--fast" ]; then
  src=runs/src/m503-mutant
  rm -rf "$src" runs/M5-03/mutant; mkdir -p "$src" runs/M5-03/mutant-bin
  /usr/bin/git archive HEAD | tar -x -C "$src"
  /usr/bin/python3 - "$src/crates/dominofish-core/src/aivat.rs" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
a = "f64::from(values[i]) - mean(values.into_iter())"
assert s.count(a) == 1, "the action term changed; update the T5 mutant"
open(p, "w").write(s.replace(a, "f64::from(values[i]) - f64::from(*values.iter().max().expect(\"moves\"))"))
PY
  find "$src" -type f -exec touch {} +
  (cd "$src" && cargo build -q --release -p dominofish --target-dir ../../../target/mutant) > runs/M5-03/mutant-build.log 2>&1 || err "mutant build failed"
  cp target/mutant/release/dominofish runs/M5-03/mutant-bin/dominofish
  rm -rf "$src" target/mutant
  scripts/run-t5.sh runs/M5-03/mutant-bin/dominofish > runs/M5-03/mutant.log 2>&1
  caught=""
  for f in runs/M5-03/mutant/row-*.txt; do
    t=$(grep '^# aivat' "$f" | sed -n 's/.*corrected minus raw mean [-0-9.]* se [0-9.]* t \(-\{0,1\}[0-9.]*\);.*/\1/p')
    awk -v t="${t:-0}" 'BEGIN { exit !(t > 3.29 || t < -3.29) }' && caught="$caught $(basename "$f" .txt | cut -d- -f1-2):t=$t"
  done
  [ -n "$caught" ] || err "the biased mutant passed every replayed row (runs/M5-03/mutant/)"
  control="the biased mutant fails:$caught"
fi
if [ "$fail" -eq 0 ]; then
  echo "M5-03: PASS T5 as registered (commit $commit, 14 rows, no fault): |t| of corrected minus raw at most 3.29 in every row, sum of t^2 $sum_t2 (at most 36.12), null rows' corrected means within 3.29 se of 0; $control; SD ratio raw / corrected by row:$ratios"
else
  echo "M5-03: FAIL"
fi
exit "$fail"
