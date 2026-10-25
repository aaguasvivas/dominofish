#!/usr/bin/env bash
# M2-09: dominofish report generates docs/eval/REPORT.md from
# docs/eval/results/ in the strength-claim format, with a table of each
# baseline's results with CIs; README.md has the tagline, subtitle, and
# credit line exactly as decided. Checks: the report is current; it has one
# row per result file, each with pairs, a points-per-round CI, the SPRT
# bounds and outcome, wall time, peak RSS, seeds, and file, a game-win CI on
# every game row, and the settings line (commit, Bench, hash, budgets,
# threads, tables) and command; README.md has the three lines and its
# sections, its numbers appear in REPORT.md, and its fast reproductions
# match their logged summary lines; no LICENSE file, no license, repository,
# or homepage field, and publish = false. --fast runs the same checks.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
cargo run -q -p dominofish -- report --check > /dev/null || err "docs/eval/REPORT.md is stale"
report=docs/eval/REPORT.md
files=$(find docs/eval/results -mindepth 2 -name '*.txt' | wc -l | tr -d ' ')
rows=$(grep -c '^| [12]v[12] |' "$report")
[ "$files" -gt 0 ] && [ "$rows" -eq "$files" ] || err "REPORT.md has $rows rows for $files result files"
bad=$(grep '^| [12]v[12] |' "$report" | awk -F'|' '{
  ok = ($8 ~ /\[-?[0-9.]+, -?[0-9.]+\]/) && ($10 ~ /^ (H0 at|H1 at|inconclusive)/) && ($10 ~ /\(.*, .*\)/) && ($11 ~ /[0-9]/) && ($12 ~ /[0-9]/) && ($13 ~ /\.\./) && ($14 ~ /\.txt/)
  if ($3 ~ /game/ && $9 !~ /\[/) ok = 0
  if (!ok) n++ } END {print n + 0}')
[ "$bad" -eq 0 ] || err "$bad REPORT.md rows lack a claim-format field"
for want in "commit " "Bench " "binary hash " "budgets " "threads " "tables " "Pre-registered as " "Command of each row"; do
  grep -q "$want" "$report" || err "REPORT.md lacks: $want"
done
for line in "The Stockfish of Dominican Dominoes." "An open-source Dominican dominoes AI engine." "Built by the creator of Capi."; do
  grep -qxF "$line" README.md || err "README.md lacks the line: $line"
done
for h in "## What it does now" "## Results" "## Architecture" "## Testing" "## Limitations" "## Build and run"; do
  grep -qxF "$h" README.md || err "README.md lacks $h"
done
for word in "human benchmark" "relative" "equilibrium" "Capi parity" "Never-peeks" "SPRT"; do
  grep -q "$word" README.md || err "README.md does not mention $word"
done
nums=0
for ci in $(grep -o '[0-9.]* \[[0-9.]*, [0-9.]*\]' README.md | tr ' ' '_'); do
  grep -qF "$(printf '%s' "$ci" | tr '_' ' ')" "$report" || err "README number $ci is not in REPORT.md"
  nums=$((nums + 1))
done
[ "$nums" -ge 4 ] || err "README.md holds only $nums result numbers"
fast=$(scripts/readme-fast.sh --check 2>&1 | tail -1)
case "$fast" in "readme-fast: PASS"*) ;; *) err "$fast" ;; esac
[ ! -e LICENSE ] || err "a LICENSE file exists"
if grep -nE '^(license|license-file|repository|homepage) *=' Cargo.toml crates/*/Cargo.toml; then err "a license, repository, or homepage field exists"; fi
grep -q '^publish = false' Cargo.toml || err "the workspace does not set publish = false"
for c in crates/*/Cargo.toml; do grep -q '^publish.workspace = true' "$c" || err "$c does not inherit publish = false"; done
if [ "$fail" -eq 0 ]; then
  echo "M2-09: PASS REPORT.md current with $rows rows for $files result files, every claim-format field present; README.md has the three decided lines, its sections, and $nums numbers found in REPORT.md; $fast; no license, publish = false"
else
  echo "M2-09: FAIL"
fi
exit "$fail"
