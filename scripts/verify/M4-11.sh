#!/usr/bin/env bash
# M4-11: the first README results table for PIMC. README.md shows R4
# (PIMC against the rule bot), the R5 curve, and the R6 gap, each number
# also in docs/eval/REPORT.md (the strength-claim format holds there), with
# the PIMC commit, worlds, and budgets named; every README row has a
# --fast reproduction (scripts/readme-fast.sh: the first pairs of its run,
# same seeds and budgets) whose summary lines are committed in
# docs/eval/results/*-fast.txt and reproduce with --check (--fast: R6's
# only); REPORT.md is current and check.sh runs the report check.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
report=docs/eval/REPORT.md
cargo run -q -p dominofish -- report --check > /dev/null || err "$report is stale"
grep -q 'report --check' scripts/check.sh || err "check.sh does not check the report"
section=$(awk '/^### PIMC, the strength-first engine$/ {p = 1} /^### The baselines$/ {p = 0} p' README.md)
[ -n "$section" ] || err "README.md has no section '### PIMC, the strength-first engine'"
for word in "R4" "R5" "R6" "64 worlds" "held-out" "SPRT" "commit "; do
  printf '%s' "$section" | grep -q "$word" || err "the PIMC section does not mention $word"
done
nums=0
for ci in $(printf '%s' "$section" | grep -o -- '-\{0,1\}[0-9.]* \[-\{0,1\}[0-9.]*, -\{0,1\}[0-9.]*\]' | tr ' ' '_'); do
  grep -qF -- "$(printf '%s' "$ci" | tr '_' ' ')" "$report" || err "README number $ci is not in $report"
  nums=$((nums + 1))
done
[ "$nums" -ge 8 ] || err "the PIMC section holds only $nums numbers with CIs"
for f in docs/eval/results/2026-10-26-pimc-r4-fast.txt docs/eval/results/2026-10-26-pimc-curve-r5-fast.txt docs/eval/results/2026-10-26-pimc-gap-r6-fast.txt; do
  [ -s "$f" ] || err "$f is missing"
done
runs="R4 R5 R6"
[ "${1:-}" = "--fast" ] && runs=R6
# shellcheck disable=SC2086
fast=$(scripts/readme-fast.sh --check $runs 2>&1 | tail -1)
case "$fast" in "readme-fast: PASS"*) ;; *) err "$fast" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M4-11: PASS README.md shows R4, R5, and R6 for PIMC with $nums numbers found in REPORT.md; $fast; REPORT.md current and checked by check.sh"
else
  echo "M4-11: FAIL"
fi
exit "$fail"
