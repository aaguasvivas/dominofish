#!/usr/bin/env bash
# M4-99, the M4 exit: every check of BUILD.md's M4 exit text passes with
# its PASS line: M2-03 (the world counter reproduces the exact counts of
# Leads and the worked example); M4-06 (its own medians by board size
# beside the research medians, as a reference only); M4-01 (a chi-square
# test at a fixed seed matches exact enumeration on enumerable information
# sets, the true world appears at the uniform rate, and the fallback
# counter reads 0); M4-04 (never-peeks also compares node counts); M4-08
# (PIMC beats every baseline by SPRT on held-out pairs in both modes);
# M4-09 (strength against worlds per move as a curve); M4-10 (the gap to
# the omniscient bot with its CI); M4-07 (per-move time and peak RSS
# recorded); M4-05 (dominofish analyze ranks the legal moves); M4-11 (the
# first README results table for PIMC); with M4-02 and M4-03 under them;
# and docs/progress.md records the fresh-subagent milestone review.
# --fast passes --fast to each part.
set -u
cd "$(dirname "$0")/../.."
flag=${1:-}
fail=0
parts=""
for id in M2-03 M4-01 M4-02 M4-03 M4-04 M4-05 M4-06 M4-07 M4-08 M4-09 M4-10 M4-11; do
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
if ! grep -q '^- M4 milestone review' docs/progress.md; then
  echo "ERROR docs/progress.md does not record the M4 milestone review"
  fail=1
fi
if [ "$fail" -eq 0 ]; then
  echo "M4-99: PASS$parts M4 milestone review recorded in docs/progress.md"
else
  echo "M4-99: FAIL"
fi
exit "$fail"
