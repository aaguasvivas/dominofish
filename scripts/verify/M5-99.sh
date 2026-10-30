#!/usr/bin/env bash
# M5-99, the M5 exit: every check of BUILD.md's M5 exit text passes with
# its PASS line: M5-03 (the correction is shown unbiased on null matches
# and against raw results by test T7, stated in docs/PLAN.md before its
# first run, and the variance reduction is measured per row); M5-04 (later
# results show raw beside corrected: the PLAN.md rule for strength runs,
# REPORT.md, and README.md); with M5-01 (the AIVAT terms and the bot
# policies) and M5-02 (match --aivat and the cross-fit) under them; and
# docs/progress.md records the fresh-subagent milestone review.
# --fast passes --fast to each part.
set -u
cd "$(dirname "$0")/../.."
flag=${1:-}
fail=0
parts=""
for id in M5-01 M5-02 M5-03 M5-04; do
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
if ! grep -q '^- M5 milestone review' docs/progress.md; then
  echo "ERROR docs/progress.md does not record the M5 milestone review"
  fail=1
fi
if [ "$fail" -eq 0 ]; then
  echo "M5-99: PASS$parts M5 milestone review recorded in docs/progress.md"
else
  echo "M5-99: FAIL"
fi
exit "$fail"
