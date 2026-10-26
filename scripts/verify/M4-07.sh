#!/usr/bin/env bash
# M4-07: dominofish pimc-stats and the default worlds per move (docs/PLAN.md
# D18). Checks:
# - the calibration record docs/eval/results/2026-10-26-pimc-calibration.md
#   holds two runs per mode of `pimc-stats --games 20 --seed 1100000000` at
#   each tried W, and the default PIMC_WORLDS (crates/dominofish/src/
#   solve.rs) is the largest of 32, 64, 128 tried in order whose p90 time
#   per move is at most 1,000 ms in both modes in both runs;
# - pimc-stats at the default, two runs per mode (--fast: 2 games instead
#   of 20), with commit, thread count, load, and date, written to
#   runs/M4-07/pimc-stats.md; node counts identical between the two runs;
#   0 inconsistent worlds and 0 fallbacks; the full run's record is
#   docs/eval/results/2026-10-26-pimc-stats.md (written by the first full
#   run), which must hold both runs of both modes;
# - the peak RSS of every PIMC engine process of a 2v2 and a 1v1 match
#   (scripts/test-engines/rss.sh, /usr/bin/time -l) stays under 256 MB.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs/M4-07
fail=0
err() { echo "ERROR $*"; fail=1; }
games=20
[ "${1:-}" = "--fast" ] && games=2
worlds=$(sed -n 's/^pub const PIMC_WORLDS: u32 = \([0-9]*\);/\1/p' crates/dominofish/src/solve.rs)
[ -n "$worlds" ] || { echo "ERROR no PIMC_WORLDS"; echo "M4-07: FAIL"; exit 1; }
cal=docs/eval/results/2026-10-26-pimc-calibration.md
chosen=""
for w in 32 64 128; do
  lines=$(grep "^pimc-stats: mode [12]v[12] worlds $w " "$cal" 2>/dev/null)
  [ "$(printf '%s\n' "$lines" | grep -c .)" -eq 4 ] || break
  over=$(printf '%s\n' "$lines" | grep -o 'p90 [0-9.]*' | awk '$2 > 1000' | wc -l | tr -d ' ')
  [ "$over" -eq 0 ] || break
  chosen=$w
done
[ -n "$chosen" ] || chosen=16
[ "$chosen" = "$worlds" ] || err "the calibration record chooses $chosen worlds, PIMC_WORLDS is $worlds"
cargo build -q --release -p dominofish 2> runs/M4-07/build.log || { echo "ERROR build failed"; echo "M4-07: FAIL"; exit 1; }
out=runs/M4-07/pimc-stats.md
{
  echo "# PIMC time per move"
  echo
  echo "Commit $(/usr/bin/git rev-parse --short HEAD), one thread, 1-minute load average $(uptime | sed 's/.*load averages*: *//; s/,* .*//') at the start, $(date +%F). Times are wall-clock; node counts are exact for the commit. Positions: the decisions with more than one legal move in selfplay (random) games."
  echo
  echo "Command: \`dominofish pimc-stats --mode <m> --games $games --seed 1100000000 --worlds $worlds\`."
  echo
  for m in 2v2 1v1; do
    for run in 1 2; do
      echo "## $m, run $run"
      echo
      target/release/dominofish pimc-stats --mode "$m" --games "$games" --seed 1100000000 --worlds "$worlds"
      echo
    done
  done
} > "$out" 2>&1 || err "pimc-stats failed ($out)"
section() { awk -v m="## $1, run $2" '$0 == m {p = 1; next} /^## / {p = 0} p && /^\| [0-9]+ \|/' "$out"; }
nodes() { section "$1" "$2" | awk -F'|' '{print $2, $3, $7, $8, $9, $10}'; }
summaries=""
for m in 2v2 1v1; do
  [ -n "$(nodes "$m" 1)" ] || err "$m: no rows"
  [ "$(nodes "$m" 1)" = "$(nodes "$m" 2)" ] || err "$m: node counts differ between runs"
  s=$(grep -o "pimc-stats: mode $m .*" "$out" | head -1)
  case "$s" in *"inconsistent 0; fallback 0;"*) ;; *) err "$m: $s" ;; esac
  summaries="$summaries [${s#pimc-stats: }]"
done
if [ "$games" -eq 20 ]; then
  rec=docs/eval/results/2026-10-26-pimc-stats.md
  # The first full run writes the record (commit it after); later runs
  # check it.
  [ -f "$rec" ] || cp "$out" "$rec"
  for m in 2v2 1v1; do
    for run in 1 2; do grep -qx "## $m, run $run" "$rec" 2>/dev/null || err "$rec lacks $m run $run"; done
  done
fi
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || err "build-bin failed: $bin"
rm -f runs/M4-07/rss-*
for m in 1v1 2v2; do
  "$bin" match --mode "$m" --unit round --target 100 --pairs 2 --seed 65000 --engine-seed 5000000407 --timeout-ms 120000 \
    --a "scripts/test-engines/rss.sh runs/M4-07/rss-$m $bin engine --bot pimc --worlds $worlds" \
    --b "$bin engine --bot rule" --out "runs/M4-07/rss-$m.txt" > /dev/null || err "the $m memory match failed"
done
peak=$(cat runs/M4-07/rss-1v1.* runs/M4-07/rss-2v2.* 2>/dev/null | awk '/maximum resident set size/ {if ($1 > m) m = $1} END {print m + 0}')
procs=$(ls runs/M4-07/rss-1v1.* runs/M4-07/rss-2v2.* 2>/dev/null | wc -l | tr -d ' ')
[ "$procs" -eq 3 ] || err "$procs engine reports, 3 expected (1 in 1v1, 2 in 2v2)"
[ "$peak" -gt 0 ] && [ "$peak" -lt 268435456 ] || err "peak RSS $peak bytes (target under 256 MB)"
mb=$(awk -v b="$peak" 'BEGIN {printf "%.1f", b / 1048576}')
if [ "$fail" -eq 0 ]; then
  echo "M4-07: PASS default $worlds worlds per move by the D18 rule ($cal); two runs per mode, equal node counts:$summaries; peak RSS of $procs PIMC engine processes $mb MB (target under 256 MB)"
else
  echo "M4-07: FAIL"
fi
exit "$fail"
