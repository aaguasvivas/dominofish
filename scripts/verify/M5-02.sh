#!/usr/bin/env bash
# M5-02: match --aivat J (docs/PLAN.md D19; docs/FORMATS.md). With HEAD's
# runs/bin binary: the same matches with and without --aivat 4 give equal
# pair lines (1v1 rounds and 2v2 games, rule against random); the --aivat
# file has one aivat line per pair and a # aivat summary with draws=on, and
# a match with the omniscient bot says draws=off; the report shows the
# corrected points per round and the SD ratio for the --aivat file and -
# for the plain one; the cross-fit unit test passes; FORMATS.md documents
# both lines. --fast runs the same checks.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
dir=runs/M5-02
rm -rf "$dir"; mkdir -p "$dir/results/x"
fail=0
err() { echo "ERROR $*"; fail=1; }
bin=$(scripts/build-bin.sh HEAD | tail -1)
[ -x "$bin" ] || { echo "ERROR build failed: $bin"; echo "M5-02: FAIL"; exit 1; }
m() { "$bin" match --mode "$1" --unit "$2" --target 100 --pairs "$3" --seed 66000 --engine-seed 5000000502 "${@:6}" --a "$bin engine --bot $4" --b "$bin engine --bot $5" > /dev/null; }
m 1v1 round 30 rule random --out "$dir/plain-1v1.txt"
m 1v1 round 30 rule random --aivat 4 --out "$dir/results/x/row-0-1v1-round-100-rule-random.txt"
m 2v2 game 4 rule random --out "$dir/plain-2v2.txt"
m 2v2 game 4 rule random --aivat 4 --out "$dir/results/x/row-1-2v2-game-100-rule-random.txt"
[ "$(grep '^pair ' "$dir/plain-1v1.txt")" = "$(grep '^pair ' "$dir/results/x/row-0-1v1-round-100-rule-random.txt")" ] || err "1v1: --aivat changed the play"
[ "$(grep '^pair ' "$dir/plain-2v2.txt")" = "$(grep '^pair ' "$dir/results/x/row-1-2v2-game-100-rule-random.txt")" ] || err "2v2: --aivat changed the play"
[ "$(grep -c '^aivat ' "$dir/results/x/row-0-1v1-round-100-rule-random.txt")" -eq 30 ] || err "not one aivat line per pair"
grep -q '^# aivat j=4 .* draws=on corrected points per round (AIVAT, cross-fitted) mean' "$dir/results/x/row-0-1v1-round-100-rule-random.txt" || err "no # aivat summary with draws=on"
m 1v1 round 3 omniscient random --aivat 2 --out "$dir/omniscient.txt"
grep -q '^# aivat j=2 .* draws=off ' "$dir/omniscient.txt" || err "an omniscient match does not turn draw terms off"
cp "$dir/plain-1v1.txt" "$dir/results/x/row-2-1v1-round-100-rule-random.txt"
"$bin" report --results "$dir/results" --out "$dir/REPORT.md" > /dev/null || err "report failed"
r0=$(grep '| row-0-1v1-round-100-rule-random.txt |' "$dir/REPORT.md" | awk -F'|' '{print $17 "|" $18}')
r2=$(grep '| row-2-1v1-round-100-rule-random.txt |' "$dir/REPORT.md" | awk -F'|' '{print $17 "|" $18}')
case "$r0" in *"["*"]"*"|"*[0-9]*) ;; *) err "the report lacks the corrected columns for the --aivat file: $r0" ;; esac
case "$r2" in " - | - ") ;; *) err "the report shows corrected columns for a plain file: $r2" ;; esac
cargo test -q -p dominofish crossfit > "$dir/crossfit.log" 2>&1 && grep -q 'test result: ok. 1 passed' "$dir/crossfit.log" || err "the cross-fit test failed"
grep -q '^- `aivat <i> deal <d> draw <w> action <a>`' docs/FORMATS.md && grep -q '^- `# aivat j=<J>' docs/FORMATS.md || err "FORMATS.md does not document the AIVAT lines"
if [ "$fail" -eq 0 ]; then
  echo "M5-02: PASS --aivat leaves the pair lines unchanged (1v1 rounds, 2v2 games); one aivat line per pair; draws=on, and off with the omniscient bot; the report shows corrected$(printf '%s' "$r0" | sed 's/|/, SD ratio/') beside raw and - for a plain file; cross-fit test passes; FORMATS.md documents the lines"
else
  echo "M5-02: FAIL"
fi
exit "$fail"
