#!/usr/bin/env bash
# M1-99, the M1 exit: M1a to M1g all pass (the verify commands of M1-01 to
# M1-10, the M1-07 fuzzer at 1,000,000 rounds per mode), and docs/progress.md
# records the fresh-subagent milestone review with each gap fixed or listed.
# --fast passes --fast to each part (the fuzzer then runs 10,000 rounds per
# mode).
set -u
cd "$(dirname "$0")/../.."
flag=${1:-}
fail=0
parts=""
for id in M1-01 M1-02 M1-03 M1-04 M1-05 M1-06 M1-07 M1-08 M1-09 M1-10; do
  out=$("scripts/verify/$id.sh" $flag 2>&1)
  code=$?
  last=$(printf '%s\n' "$out" | tail -1)
  if [ "$code" -ne 0 ]; then
    printf '%s\n' "$out" | grep '^ERROR' | head -5
    echo "ERROR $id failed: $last"
    fail=1
  else
    parts="$parts $id ok;"
  fi
done
if ! grep -q '^- M1 milestone review' docs/progress.md; then
  echo "ERROR docs/progress.md does not record the M1 milestone review"
  fail=1
fi
if [ "$fail" -eq 0 ]; then
  echo "M1-99: PASS$parts M1 milestone review recorded in docs/progress.md"
else
  echo "M1-99: FAIL"
fi
exit "$fail"
