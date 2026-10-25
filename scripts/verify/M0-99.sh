#!/usr/bin/env bash
# M0-99, the M0 exit: check.sh exits 0 (M0-02, which also covers the wasm32
# core build), selfplay finishes in both modes with identical output on two
# runs (M0-03), the state files exist (M0-04), and the first commit is on
# main. --fast passes --fast to each part.
set -u
cd "$(dirname "$0")/../.."
fail=0
flag=${1:-}
lines=""
for id in M0-01 M0-02 M0-03 M0-04; do
  out=$("scripts/verify/$id.sh" $flag 2>&1)
  code=$?
  last=$(printf '%s\n' "$out" | tail -1)
  if [ "$code" -ne 0 ]; then
    printf '%s\n' "$out" | grep '^ERROR'
    echo "ERROR $id failed: $last"
    fail=1
  fi
  lines="$lines $id ok;"
done
branch=$(/usr/bin/git rev-parse --abbrev-ref HEAD 2> /dev/null)
commits=$(/usr/bin/git rev-list --count main 2> /dev/null || echo 0)
[ "$branch" = main ] || { echo "ERROR current branch is '$branch', want main"; fail=1; }
[ "$commits" -ge 1 ] || { echo "ERROR main has no commit"; fail=1; }

if [ "$fail" -eq 0 ]; then
  echo "M0-99: PASS$lines on main with $commits commits"
else
  echo "M0-99: FAIL"
fi
exit "$fail"
