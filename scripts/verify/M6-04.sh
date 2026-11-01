#!/usr/bin/env bash
# M6-04: run R7 (docs/PLAN.md) is complete and as registered: four result
# files in docs/eval/results/2026-11-01-win-objective-r7/, row k (1v1 to
# 100, 1v1 to 200, 2v2 to 100, 2v2 to 200) with side A PIMC with the win
# objective and side B PIMC with the points objective, both at the default
# limits of the R7 commit (64 worlds, a node cap of 10,000,000, 1 MB
# tables), side A naming the hash of the commit's W tables; full games;
# deal seeds 2,050,000,000 + 1,000,000 k; engine seed base 5,001,300,000 +
# k; --aivat 8; --threads 4; --timeout-ms 1800000; the SPRT on the
# game-win share, H0 0.5 against H1 0.53, stopping at its decision, with at
# most 2,700 pairs (inconclusive only at the maximum); no fault; a time
# line; and a run line with the registered command, wall time, and peak
# RSS. Each row's outcome is recorded either way, and the engine's default
# objective follows the registered rule: win if all four rows accept H1,
# points otherwise (HEAD's `engine --bot pimc` reports it). A replay of the
# first 2 pairs of each row (row 0 only with --fast) with the R7 binary on
# 2 threads gives the committed pair and aivat lines. REPORT.md is current.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
H1=0.53
MAX=2700
fast=${1:-}
fail=0
err() { echo "ERROR $*"; fail=1; }
dir=docs/eval/results/2026-11-01-win-objective-r7
out=runs/M6-04
mkdir -p "$out"
commit=$(cat "$dir/COMMIT" 2>/dev/null)
[ -n "$commit" ] || { echo "ERROR $dir/COMMIT (the R7 commit) is missing"; echo "M6-04: FAIL"; exit 1; }
bin=$(scripts/build-bin.sh "$commit" | tail -1)
head_bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] && [ -x "$head_bin" ] || { echo "ERROR build failed: $bin $head_bin"; echo "M6-04: FAIL"; exit 1; }
# The FNV-1a hash of the R7 commit's W tables, in file name order, as the
# engine computes it.
hash=$(for f in $(/usr/bin/git ls-tree --name-only "$commit" docs/eval/equity/ | grep '/w-.*\.txt$' | sort); do /usr/bin/git show "$commit:$f"; done | /usr/bin/python3 -c '
import sys
h = 0xcbf29ce484222325
for b in sys.stdin.buffer.read():
    h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
print(f"{h:016x}")')
rows=""
h1_rows=0
for k in 0 1 2 3; do
  case $k in
    0) mode=1v1; target=100 ;;
    1) mode=1v1; target=200 ;;
    2) mode=2v2; target=100 ;;
    3) mode=2v2; target=200 ;;
  esac
  f="$dir/row-$k-$mode-game-$target-win-points.txt"
  [ -f "$f" ] || { err "row $k: $f is missing"; continue; }
  deal=$((2050000000 + 1000000 * k)); eng=$((5001300000 + k))
  header=$(head -1 "$f")
  for want in "a.cmd=runs/bin/$commit/dominofish+engine+--bot+pimc+--objective+win " "a.commit=$commit " "a.budgets=worlds:64,nodes:10000000,objective:win:$hash " "a.tables=1MB " "a.view=infoset " "b.cmd=runs/bin/$commit/dominofish+engine+--bot+pimc+--objective+points " "b.commit=$commit " "b.budgets=worlds:64,nodes:10000000 " "b.tables=1MB " "b.view=infoset " "mode=$mode " "unit=game " "target=$target " "pairs=$MAX"; do
    case "$header " in *"$want"*) ;; *) err "row $k: the header lacks $want" ;; esac
  done
  grep -qx "# seeds deals=$deal..$((deal + MAX - 1)) engines=$eng" "$f" || err "row $k: the seeds differ from R7"
  grep -q '^# fault' "$f" && err "row $k has a fault"
  n=$(grep -c '^pair ' "$f")
  summary=$(grep '^# summary ' "$f")
  sprt="sprt on the game-win share, H0 0.5 H1 $H1, alpha 0.05 beta 0.05:"
  case "$summary" in
    *"$sprt H1 after $n pairs"*) outcome=H1; h1_rows=$((h1_rows + 1)) ;;
    *"$sprt H0 after $n pairs"*) outcome=H0 ;;
    *"$sprt inconclusive after $MAX pairs"*) outcome=inconclusive ;;
    *) err "row $k: no R7 SPRT outcome at its last pair ($n pairs)"; outcome=none ;;
  esac
  [ "$outcome" != inconclusive ] || [ "$n" -eq "$MAX" ] || err "row $k: inconclusive before its maximum"
  grep -q '^# aivat j=8 ' "$f" || err "row $k has no aivat line with j=8"
  grep -q '^# time a=[0-9.]*,[0-9.]*,[0-9]* ' "$f" || err "row $k has no time line"
  grep -q "^# run run=R7 row=$k command=runs/bin/$commit/dominofish+match+--mode+$mode+--unit+game+--target+$target+--pairs+$MAX+--seed+$deal+--engine-seed+$eng+--aivat+8+--sprt+0.5,$H1+--sprt-on+wins+--threads+4+--timeout-ms+1800000+.* wall_s=[0-9.]* peak_rss_kb=[0-9]* " "$f" || err "row $k: no R7 run line with the registered command, wall time, and peak RSS"
  share=$(sed -n 's/.*game-win rate of A \([0-9.]*\) 95% CI \(\[[0-9., ]*\]\).*/\1 \2/p' "$f")
  rows="$rows row $k ($mode to $target): $outcome after $n pairs, A's game-win rate $share;"
  if [ "$fast" != --fast ] || [ "$k" -eq 0 ]; then
    r="$out/replay-$k.txt"
    "$bin" match --mode "$mode" --unit game --target "$target" --pairs 2 --seed "$deal" --engine-seed "$eng" --aivat 8 --sprt "0.5,$H1" --sprt-on wins --threads 2 --timeout-ms 1800000 \
      --a "$bin engine --bot pimc --objective win" --b "$bin engine --bot pimc --objective points" --out "$r" > "$out/replay-$k.log" 2>&1 || err "row $k: the replay failed ($out/replay-$k.log)"
    [ "$(grep -E '^(pair|aivat) ' "$r")" = "$(grep -E '^(pair|aivat) [01] ' "$f")" ] || err "row $k: the replay of its first 2 pairs differs from the committed lines"
  fi
done
ready=$(printf 'dominofish 1\n' | "$head_bin" engine --bot pimc | head -1)
if [ "$h1_rows" -eq 4 ]; then
  default=win
  case "$ready" in *"objective:win:"*) ;; *) err "all four rows accept H1, but the default objective is not win: $ready" ;; esac
else
  default=points
  case "$ready" in *"objective:"*) err "$h1_rows of 4 rows accept H1, but the default objective is not points: $ready" ;; esac
fi
cargo run -q -p dominofish -- report --check > /dev/null || err "docs/eval/REPORT.md is stale"
if [ "$fail" -eq 0 ]; then
  echo "M6-04: PASS R7 as registered (commit $commit, H0 0.5 H1 $H1, at most $MAX pairs, no fault);$rows the default objective is $default ($h1_rows of 4 rows accept H1); replays equal; REPORT.md current"
else
  echo "M6-04: FAIL"
fi
exit "$fail"
