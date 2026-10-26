#!/usr/bin/env bash
# M2-99, the M2 exit: every M2 feature's verify command passes (M2-01 to
# M2-09: the InfoSet, the record format and protocol, the exact world
# counts with the pass deductions and the worked example, the bots,
# never-peeks for every bot, the harness with 0 illegal moves, crashes, or
# timeouts, the SPRT error rates on synthetic data, the identical-bot and
# null SPRT tests, and the baseline table in REPORT.md), and
# docs/progress.md records the fresh-subagent milestone review.
# --fast passes --fast to each part.
set -u
cd "$(dirname "$0")/../.."
flag=${1:-}
fail=0
parts=""
for id in M2-01 M2-02 M2-03 M2-04 M2-05 M2-06 M2-07 M2-08 M2-09; do
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
if ! grep -q '^- M2 milestone review' docs/progress.md; then
  echo "ERROR docs/progress.md does not record the M2 milestone review"
  fail=1
fi
if [ "$fail" -eq 0 ]; then
  echo "M2-99: PASS$parts M2 milestone review recorded in docs/progress.md"
else
  echo "M2-99: FAIL"
fi
exit "$fail"
