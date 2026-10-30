#!/usr/bin/env bash
# M5-03: test T7 (docs/PLAN.md; by the owner's Q12 answer it replaces T5 and
# T6) on its 14 committed result files in
# docs/eval/results/2026-10-30-aivat-t7/: every row as registered (commit,
# engines, seeds, pairs, --aivat 8, no fault); (a) per row, the mean of
# corrected minus raw points per round has |t| <= 3.29; (b) the sum of t^2
# over the 14 rows is at most 36.12 (the 0.999 quantile of chi-square with
# 14 df); (c) in the 6 null rows (0, 1, 6, 7, 8, 13) the corrected mean's t
# is within 3.29 of 0; (d) tools/aivat-recompute.py reproduces the mean, se,
# and t of corrected minus raw in every T6 row with the terms per round (the
# harness before the fix of Q12) and in every T7 row with the terms as
# totals, within one unit of the last printed digit, and with the terms per
# round on T7's games it fails (a) in at least one game row (5, 6, 12, 13);
# the report's corrected mean of each T7 row is within 0.01 of its # aivat
# line's. Prints each row's SD ratio, raw over corrected. --fast runs the
# same checks.
set -u
cd "$(dirname "$0")/../.."
fail=0
err() { echo "ERROR $*"; fail=1; }
dir=docs/eval/results/2026-10-30-aivat-t7
t6=docs/eval/results/2026-10-29-aivat-t6
commit=$(cat "$dir/COMMIT" 2>/dev/null)
[ -n "$commit" ] || err "$dir/COMMIT is missing"
names=(rule-rule random-random rule-random accompanied-heaviest pimc8-rule rule-random rule-rule)
units=(round round round round round game game)
as=("rule+--salt+1" "random+--salt+1" "rule" "accompanied" "pimc+--worlds+8" "rule" "rule+--salt+1")
bs=("rule+--salt+2" "random+--salt+2" "random" "heaviest" "rule" "random" "rule+--salt+2")
# pairs <k>: row k's registered pair count.
pairs() { case $(($1 % 7)) in 0|1) echo 1000 ;; 2|3) echo 16000 ;; 4) echo 300 ;; *) if [ "$1" -lt 7 ]; then echo 4000; else echo 1000; fi ;; esac; }
row_file() { local i=$(($1 % 7)) mode; mode=$([ "$1" -lt 7 ] && echo 1v1 || echo 2v2); echo "row-$1-$mode-${units[$i]}-100-${names[$i]}.txt"; }
# dst <file>: mean, se, and t of corrected minus raw in the # aivat line.
dst() { sed -n 's/^# aivat .*corrected minus raw mean \([-0-9.]*\) se \([0-9.]*\) t \([-0-9.]*\);.*/\1 \2 \3/p' "$1"; }
# recompute <file> <per round|totals>: the same three numbers from the tool.
recompute() { /usr/bin/python3 -I tools/aivat-recompute.py "$1" | sed -n "s/.*$2: d \([-+0-9.]*\) se \([0-9.]*\) t \([-+0-9.]*\).*/\1 \2 \3/p"; }
# same <three numbers> <three numbers>: equal within one unit of the last
# printed digit (0.0001 for the mean and se, 0.001 for t).
same() { echo "$1 $2" | awk 'NF == 6 { d = $1 - $4; s = $2 - $5; t = $3 - $6; ok = d * d <= 1.21e-8 && s * s <= 1.21e-8 && t * t <= 1.21e-6 } END { exit !ok }'; }
for g in "$t6"/row-*.txt; do
  same "$(dst "$g")" "$(recompute "$g" "per round")" || err "T6 $(basename "$g"): the tool per round gives $(recompute "$g" "per round"), the harness $(dst "$g")"
done
report=$(sed -n "/^## $(basename "$dir") /,/^## /p" docs/eval/REPORT.md)
[ -n "$report" ] || err "docs/eval/REPORT.md has no $(basename "$dir") section"
sum_t2=0; ratios=""; caught=""
for k in $(seq 0 13); do
  i=$((k % 7)); mode=$([ "$k" -lt 7 ] && echo 1v1 || echo 2v2); n=$(pairs "$k")
  f="$dir/$(row_file "$k")"
  [ -f "$f" ] || { err "row $k: $f is missing"; continue; }
  header=$(head -1 "$f")
  for want in "a.cmd=runs/bin/$commit/dominofish+engine+--bot+${as[$i]}" "b.cmd=runs/bin/$commit/dominofish+engine+--bot+${bs[$i]}" "a.commit=$commit" "mode=$mode" "unit=${units[$i]}" "target=100" "pairs=$n"; do
    case "$header " in *" $want "*) ;; *) err "row $k: header lacks $want" ;; esac
  done
  deal=$((640000000 + 1000000 * k))
  grep -qx "# seeds deals=$deal..$((deal + n - 1)) engines=$((5000900000 + k))" "$f" || err "row $k: seeds differ"
  [ "$(grep -c '^pair ' "$f")" -eq "$n" ] || err "row $k: not $n pairs"
  [ "$(grep -c '^aivat ' "$f")" -eq "$n" ] || err "row $k: not one aivat line per pair"
  grep -q '^# fault' "$f" && err "row $k has a fault"
  grep -q "^# run run=T7 row=$k command=runs/bin/$commit/dominofish+match+--mode+$mode+--unit+${units[$i]}+--target+100+--pairs+$n+--seed+$deal+--engine-seed+$((5000900000 + k))+--aivat+8+--timeout-ms+120000+" "$f" || err "row $k: no T7 run line with the registered command"
  grep -q '^# aivat j=8 ' "$f" || err "row $k: no aivat summary"
  got=$(dst "$f"); t=$(echo "$got" | awk '{print $3}')
  [ -n "$t" ] || { err "row $k: no t"; continue; }
  awk -v t="$t" 'BEGIN { exit !(t <= 3.29 && t >= -3.29) }' || err "row $k: corrected minus raw t $t (target |t| <= 3.29)"
  sum_t2=$(awk -v s="$sum_t2" -v t="$t" 'BEGIN { print s + t * t }')
  raw=$(sed -n 's/^# aivat .*; raw sd \([0-9.]*\);.*/\1/p' "$f")
  cmean=$(sed -n 's/^# aivat .*(AIVAT, cross-fitted) mean \(-\{0,1\}[0-9.]*\) sd .*/\1/p' "$f")
  csd=$(sed -n 's/^# aivat .*(AIVAT, cross-fitted) mean -\{0,1\}[0-9.]* sd \([0-9.]*\) .*/\1/p' "$f")
  case $i in 0|1|6)
    ct=$(awk -v m="$cmean" -v s="$csd" -v n="$n" 'BEGIN { printf "%.3f", m / (s / sqrt(n)) }')
    awk -v t="$ct" 'BEGIN { exit !(t <= 3.29 && t >= -3.29) }' || err "null row $k: corrected mean t $ct (target |t| <= 3.29)"
    ;;
  esac
  ratios="$ratios $k:$(awk -v a="$raw" -v b="$csd" 'BEGIN { printf "%.2f", a / b }')"
  same "$got" "$(recompute "$f" totals)" || err "row $k: the tool with totals gives $(recompute "$f" totals), the harness $got"
  case $i in 5|6)
    pt=$(recompute "$f" "per round" | awk '{print $3}')
    awk -v t="${pt:-0}" 'BEGIN { exit !(t > 3.29 || t < -3.29) }' && caught="$caught row$k:t=$pt"
    ;;
  esac
  rmean=$(printf '%s\n' "$report" | grep "| $(row_file "$k") |" | awk -F'|' '{ split($16, w, " "); print w[1] }')
  awk -v a="$rmean" -v b="$cmean" 'BEGIN { exit !(a != "" && a - b <= 0.01 && b - a <= 0.01) }' || err "row $k: the report's corrected mean ${rmean:-none} differs from the # aivat line's $cmean"
done
awk -v s="$sum_t2" 'BEGIN { exit !(s <= 36.12) }' || err "sum of t^2 $sum_t2 over 14 rows (target at most 36.12)"
[ -n "$caught" ] || err "the per-round version passed (a) in every game row (target: |t| > 3.29 in one)"
if [ "$fail" -eq 0 ]; then
  echo "M5-03: PASS T7 as registered (commit $commit, 14 rows, no fault): |t| of corrected minus raw at most 3.29 in every row, sum of t^2 $sum_t2 (at most 36.12), null rows' corrected means within 3.29 se of 0; the tool reproduces T6 per round and T7 with totals, and the per-round version on T7's games fails (a):$caught; SD ratio raw / corrected by row:$ratios"
else
  echo "M5-03: FAIL"
fi
exit "$fail"
