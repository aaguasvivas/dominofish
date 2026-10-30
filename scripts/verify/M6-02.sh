#!/usr/bin/env bash
# M6-02: the win-probability objective (docs/PLAN.md D21). In release mode
# (WIN_FULL=1; --fast: the debug sizes): the solver with the win objective
# equals naive minimax with win values on positions where naive minimax
# finishes, at their own scores and near-target variants, with the table
# on and off agreeing; PIMC with it gives every move its exact win value
# where one world is consistent; it never peeks (rearranged hidden tiles
# behind the same public record give the same analysis); the points
# objective is unchanged (Bench 6121081, the M3 solver test and the PIMC
# tests pass). With HEAD's runs/bin binary and the committed W tables
# (docs/eval/equity): dominofish analyze --objective win ranks the moves by
# win probability in percent with a standard error, in both modes; and a
# short match of PIMC with the win objective against PIMC with points, in
# both modes, has no fault and its header names objective:win with the
# tables' hash.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
out=runs/M6-02
mkdir -p "$out"
fail=0
err() { echo "ERROR $*"; fail=1; }
full=1
[ "${1:-}" = "--fast" ] && full=0
if [ "$full" -eq 1 ]; then
  WIN_FULL=1 cargo test -q --release -p dominofish-core --test win -- --show-output > "$out/win.log" 2>&1 || err "the win tests failed ($out/win.log)"
else
  cargo test -q -p dominofish-core --test win -- --show-output > "$out/win.log" 2>&1 || err "the win tests failed ($out/win.log)"
fi
grep -q 'test result: ok. 2 passed' "$out/win.log" || err "the win tests did not all pass ($out/win.log)"
cargo test -q --release -p dominofish-core --test never_peeks -- --show-output > "$out/never_peeks.log" 2>&1 || err "the never-peeks tests failed ($out/never_peeks.log)"
grep -q 'never-peeks win: ' "$out/never_peeks.log" || err "no never-peeks result for the win objective"
cargo test -q --release -p dominofish-core --test solver --test pimc > "$out/points.log" 2>&1 || err "the points solver or PIMC tests failed ($out/points.log)"
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; echo "M6-02: FAIL"; exit 1; }
bench=$("$bin" bench | tail -1)
[ "$bench" = "Bench: 6121081" ] || err "bench changed: $bench (target Bench: 6121081)"
hash=$(/usr/bin/python3 -c '
import glob, sys
h = 0xcbf29ce484222325
for p in sorted(glob.glob("docs/eval/equity/w-*.txt")):
    for b in open(p, "rb").read():
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
print(f"{h:016x}")')
analyzed=""
for mode in 1v1 2v2; do
  for t in 100 200; do
    [ -f "docs/eval/equity/w-$mode-$t.txt" ] || err "docs/eval/equity/w-$mode-$t.txt is missing"
  done
  "$bin" selfplay --mode "$mode" --seed 11 --record "$out/game-$mode.txt" > /dev/null || err "selfplay $mode failed"
  "$bin" analyze --record "$out/game-$mode.txt" --round 2 --ply 2 --worlds 8 --objective win > "$out/analyze-$mode.txt" 2>&1 || err "analyze --objective win failed in $mode ($out/analyze-$mode.txt)"
  grep -q '^Move  *Win %  *SE$' "$out/analyze-$mode.txt" || err "$mode: analyze does not show win probabilities"
  bad=$(awk '/^play |^draw|^pass/ { v = $(NF - 1); if (v < 0 || v > 100) n++ } END { print n + 0 }' "$out/analyze-$mode.txt")
  [ "$bad" -eq 0 ] || err "$mode: a win probability outside 0 to 100"
  analyzed="$analyzed $mode:$(grep -c -E '^(play|draw|pass)' "$out/analyze-$mode.txt")"
  "$bin" match --mode "$mode" --unit game --target 100 --pairs 2 --seed 4200000000 --engine-seed 5001000960 --timeout-ms 120000 \
    --a "$bin engine --bot pimc --worlds 4 --objective win" --b "$bin engine --bot pimc --worlds 4" --out "$out/match-$mode.txt" > "$out/match-$mode.log" 2>&1 || err "$mode: the match with the win objective failed ($out/match-$mode.log)"
  grep -q '^# fault' "$out/match-$mode.txt" && err "$mode: the match has a fault"
  head -1 "$out/match-$mode.txt" | grep -q " a.budgets=worlds:4,nodes:10000000,objective:win:$hash " || err "$mode: the header does not name objective:win with the tables' hash $hash"
done
if [ "$fail" -eq 0 ]; then
  echo "M6-02: PASS $(grep -o 'win objective: .*' "$out/win.log" | sed 's/^win objective: //'); $(grep -o 'win pimc one world: .*' "$out/win.log" | sed 's/^win pimc one world: //'); $(grep -o 'never-peeks win: .*' "$out/never_peeks.log" | sed 's/^never-peeks win: //'); points unchanged ($bench, solver and PIMC tests pass); analyze shows win probabilities (moves by mode:$analyzed); matches with the win objective in both modes have no fault and name objective:win:$hash"
else
  echo "M6-02: FAIL"
fi
exit "$fail"
