#!/usr/bin/env bash
# M3-99, the M3 exit: every M3 feature's verify command passes with its
# PASS line (M3-01: 0 mismatches against naive minimax on fixed positions
# per mode in the recorded tiles-left range, near-target scores included,
# and table on and off equal on deeper positions; M3-02: Bench identical
# across runs and checked in check.sh; M3-03: solve time (median, p90, max)
# and nodes by tiles left per mode, and the share of 1v1 solves that hit
# the node cap; M3-04: the omniscient bot through the harness, labeled as
# cheating; M3-05: the omniscient bot beats every baseline by SPRT), and
# docs/progress.md records the fresh-subagent milestone review.
# --fast passes --fast to each part.
set -u
cd "$(dirname "$0")/../.."
flag=${1:-}
fail=0
parts=""
for id in M3-01 M3-02 M3-03 M3-04 M3-05; do
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
if ! grep -q '^- M3 milestone review' docs/progress.md; then
  echo "ERROR docs/progress.md does not record the M3 milestone review"
  fail=1
fi
if [ "$fail" -eq 0 ]; then
  echo "M3-99: PASS$parts M3 milestone review recorded in docs/progress.md"
else
  echo "M3-99: FAIL"
fi
exit "$fail"
