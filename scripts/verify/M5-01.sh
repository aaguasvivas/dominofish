#!/usr/bin/env bash
# M5-01: the AIVAT terms (core/src/aivat.rs; docs/PLAN.md D19) and the
# bots' exact policies. In release mode (AIVAT_FULL=1; --fast: the small
# sizes): each bot's decisions over 64 seeds use exactly its policy; tied
# decisions' action terms sum to 0; one-tile draws and a deal against itself
# give 0; values equal the solver's without a table, whatever ran before,
# and at the harness's node cap a used Valuer gives a fresh one's values
# (draws on reshuffled boneyards and later round starts), which fails when
# the table is not cleared. With HEAD's runs/bin binary: every pair of all
# 36 committed R2 rows replays with equal pair lines (--fast: the first 10
# pairs of two rows); and one deterministic engine on both sides with
# --aivat 4 plays mirror games, so each pair's margin and its deal, draw,
# and action totals are exactly 0 while the referee computes values: the
# same sampled deals and boneyard orders in both games of a pair. The
# allocation test still reports 0.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs/M5-01
fail=0
err() { echo "ERROR $*"; fail=1; }
full=1
[ "${1:-}" = "--fast" ] && full=0
log=runs/M5-01/aivat.log
AIVAT_FULL=$full cargo test -q --release -p dominofish-core --test aivat -- --show-output > "$log" 2>&1 || err "AIVAT tests failed ($log)"
grep -q 'test result: ok. 4 passed' "$log" || err "the AIVAT tests did not all pass ($log)"
lines=$(grep -o 'aivat [a-z]*: .*' "$log" | sed 's/^aivat //' | sort | paste -sd '|' - | sed 's/|/; /g')
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || err "build failed: $bin"
# Every R2 row, from its committed header and seeds line, with HEAD's bots.
replayed=0
pairs=0
for f in docs/eval/results/2026-10-25-baselines-r2/row-*.txt; do
  k=$(basename "$f" | cut -d- -f2)
  n=$(grep -c '^pair ' "$f")
  if [ "$full" -eq 0 ]; then
    case $k in 12 | 21) n=10 ;; *) continue ;; esac
  fi
  h=$(head -1 "$f")
  get() { printf '%s\n' "$h" | tr ' ' '\n' | sed -n "s/^$1=//p"; }
  grep -q '^# run run=R2 .*+--sprt+0,4+--sprt-track+' "$f" || err "R2 row $k was not run with --sprt 0,4 --sprt-track"
  deal=$(sed -n 's/^# seeds deals=\([0-9]*\)\.\..*/\1/p' "$f")
  eng=$(sed -n 's/^# seeds .* engines=\([0-9]*\)$/\1/p' "$f")
  abot=$(get a.cmd | sed 's/.*+--bot+//; s/+/ /g')
  bbot=$(get b.cmd | sed 's/.*+--bot+//; s/+/ /g')
  "$bin" match --mode "$(get mode)" --unit "$(get unit)" --target "$(get target)" --pairs "$n" --seed "$deal" --engine-seed "$eng" \
    --sprt 0,4 --sprt-track --a "$bin engine --bot $abot" --b "$bin engine --bot $bbot" --out "runs/M5-01/r2-$k.txt" > /dev/null || err "replay of R2 row $k failed"
  [ "$(grep '^pair ' "runs/M5-01/r2-$k.txt")" = "$(grep '^pair ' "$f" | head -"$n")" ] || err "R2 row $k replays differently"
  replayed=$((replayed + 1))
  pairs=$((pairs + n))
done
[ "$replayed" -eq "$([ "$full" -eq 1 ] && echo 36 || echo 2)" ] || err "replayed $replayed R2 rows"
# Mirror games: the same deterministic engine on both sides.
mirrored=0
for spec in "1v1 6" "2v2 3"; do
  set -- $spec
  out=runs/M5-01/mirror-$1.txt
  "$bin" match --mode "$1" --unit game --target 100 --pairs "$2" --seed 67000 --engine-seed 5000000501 --aivat 4 \
    --a "$bin engine --bot rule" --b "$bin engine --bot rule" --out "$out" > /dev/null || err "the $1 mirror match failed"
  [ "$(grep -c '^pair .* margin 0 ' "$out")" -eq "$2" ] || err "$1: the mirror games do not cancel"
  [ "$(grep -c '^aivat [0-9]* deal 0.000 draw 0.000 action 0.000$' "$out")" -eq "$2" ] || err "$1: a mirror pair's terms do not cancel: $(grep '^aivat ' "$out" | grep -v 'deal 0.000 draw 0.000 action 0.000$' | head -1)"
  v=$(sed -n 's/^# aivat .* values=\([0-9]*\) .*/\1/p' "$out")
  [ "${v:-0}" -gt 0 ] || err "$1: the mirror match computed no values"
  mirrored=$((mirrored + $2))
done
alloc=$(cargo test -q -p dominofish-core --test alloc 2>&1 | grep -o 'alloc: .*')
case "$alloc" in *"allocations 0 (hash check true)") ;; *) err "allocation test: ${alloc:-no summary}" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M5-01: PASS $lines; $replayed committed R2 rows ($pairs pairs) replay with equal pair lines at HEAD; $mirrored mirror pairs (1v1 and 2v2 games, --aivat 4) give margin 0 and terms exactly 0; 0 allocations"
else
  echo "M5-01: FAIL"
fi
exit "$fail"
