#!/usr/bin/env bash
# M6-99, the M6 exit: every check of BUILD.md's M6 exit text passes with
# its PASS line: M6-01 (the W table for targets 100 and 200, built by a
# committed command from the engine's own round outcomes); M6-04 (an SPRT
# on game-win rate in full games at 100 and at 200 against the current
# best, with its bounds and their reason fixed first in docs/PLAN.md,
# recorded either way); M6-03 (on held-out games, the Brier score, the
# Brier skill against the score-only W baseline, and a reliability table
# with counts per bin, per mode and target, by test T8); M6-05 (README.md
# and REPORT.md show them with their settings); with M6-02 (the win
# objective, which analyze shows) under them; and docs/progress.md records
# the fresh-subagent milestone review. --fast passes --fast to each part.
set -u
cd "$(dirname "$0")/../.."
flag=${1:-}
fail=0
parts=""
for id in M6-01 M6-02 M6-03 M6-04 M6-05; do
  out=$("scripts/verify/$id.sh" $flag 2>&1)
  code=$?
  last=$(printf '%s\n' "$out" | tail -1)
  if [ "$code" -ne 0 ] || case "$last" in "$id: PASS"*) false ;; *) true ;; esac; then
    printf '%s\n' "$out" | grep '^ERROR' | head -5
    echo "ERROR $id failed: $last"
    fail=1
  else
    parts="$parts $id ok;"
  fi
done
if ! grep -q '^- M6 milestone review' docs/progress.md; then
  echo "ERROR docs/progress.md does not record the M6 milestone review"
  fail=1
fi
if [ "$fail" -eq 0 ]; then
  echo "M6-99: PASS$parts M6 milestone review recorded in docs/progress.md"
else
  echo "M6-99: FAIL"
fi
exit "$fail"
