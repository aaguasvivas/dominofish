#!/usr/bin/env bash
# M4-11: the first README results table for PIMC. README.md shows R4
# (PIMC against the rule bot), the R5 curve, and the R6 gap, each number
# equal to its own docs/eval/REPORT.md row (R4 by mode and unit against the
# rule bot: pairs, points per round, game-win rate, SPRT; R5 by mode and
# worlds; R6 by mode), with the settings named (commits, Bench, worlds,
# target, seeds, command, result directory, a link to REPORT.md, and the
# load of the timings); every README row has a
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
section=$(awk '/^### PIMC, the strength-first engine$/ {p = 1} /^### The baselines$/ {p = 0} p' "${README:-README.md}")
[ -n "$section" ] || err "README.md has no section '### PIMC, the strength-first engine'"
for want in "R4" "R5" "R6" "64 worlds" "held-out" "SPRT" "Bench 6,121,081" "target 100" "2,020,000,000" "2,030,000,000" "2,040,000,000" "commit ebaa5fd" "commit e3f9a4f" "scripts/run-pimc.sh R4" "scripts/run-pimc.sh R5" "scripts/run-pimc.sh R6" "docs/eval/results/2026-10-26-pimc-r4" "docs/eval/REPORT.md" "load average"; do
  printf '%s' "$section" | grep -qF -- "$want" || err "the PIMC section does not give: $want"
done
# Each README number against its own REPORT.md row: R4 (mode, unit, side B
# rule: pairs, points per round, game-win rate, SPRT), every R5 point
# (mode and worlds), and both R6 gaps (mode).
rowcheck=$(/usr/bin/python3 - "${README:-README.md}" <<'PY'
import re, sys
readme = open(sys.argv[1] if len(sys.argv) > 1 else "README.md").read()
report = open("docs/eval/REPORT.md").read()
start = readme.index("### PIMC, the strength-first engine")
section = readme[start:readme.index("### The baselines", start)]

def rows(run):
    """REPORT.md rows of one run directory, as lists of cells."""
    out = []
    part = report.split("\n## " + run + " ")[1].split("\n## ")[0]
    for line in part.splitlines():
        if re.match(r"^\| [12]v[12] \|", line):
            out.append([c.strip() for c in line.strip("|").split("|")])
    return out

errors, checked = [], 0
# R4: README rows "| mode | unit | pairs | ppr | win | SPRT |" against the
# REPORT rows of side B rule.
r4 = rows("2026-10-26-pimc-r4")
for line in section.splitlines():
    m = re.match(r"^\| (1v1|2v2) \| (round|game to 100) \| ([0-9,]+) \| ([^|]+) \| ([^|]+) \| H1 at pair (\d+) \|$", line)
    if not m:
        continue
    mode, unit, pairs, ppr, win, n = m.groups()
    unit = "round" if unit == "round" else "game"
    match = [r for r in r4 if r[0] == mode and r[1] == unit and r[4] == "rule"]
    if len(match) != 1:
        errors.append(f"R4 {mode} {unit}: {len(match)} REPORT rows")
        continue
    r = match[0]
    want = (r[5], r[6], r[7], r[8].split(" (")[0])
    got = (pairs.replace(",", ""), ppr.strip(), win.strip(), f"H1 at {n}")
    if want != got:
        errors.append(f"R4 {mode} {unit}: README {got} REPORT {want}")
    checked += 1
# R5: "| worlds | 1v1 ppr | 2v2 ppr |" against "pimc (W worlds)" rows.
r5 = rows("2026-10-26-pimc-curve-r5")
for line in section.splitlines():
    m = re.match(r"^\| (\d+) \| (-?[0-9.]+ \[[^]]+\]) \| (-?[0-9.]+ \[[^]]+\]) \|$", line)
    if not m:
        continue
    w, one, two = m.groups()
    for mode, val in (("1v1", one), ("2v2", two)):
        match = [r for r in r5 if r[0] == mode and r[3] == f"pimc ({w} worlds)"]
        if len(match) != 1 or match[0][6] != val:
            errors.append(f"R5 {mode} {w} worlds: README {val} REPORT {[r[6] for r in match]}")
        checked += 1
# R6: the two gaps in the prose, 1v1 first.
r6 = rows("2026-10-26-pimc-gap-r6")
gaps = re.findall(r"(-?[0-9.]+ \[-?[0-9.]+, -?[0-9.]+\]) points per round in 1v1 and (-?[0-9.]+ \[-?[0-9.]+, -?[0-9.]+\]) in 2v2", section)
if len(gaps) != 1:
    errors.append(f"R6: {len(gaps)} gap sentences")
else:
    for mode, val in zip(("1v1", "2v2"), gaps[0]):
        match = [r for r in r6 if r[0] == mode]
        if len(match) != 1 or match[0][6] != val:
            errors.append(f"R6 {mode}: README {val} REPORT {[r[6] for r in match]}")
        checked += 1
for e in errors:
    print("ERROR " + e)
print(f"rows {checked} errors {len(errors)}")
sys.exit(1 if errors or checked < 4 + 14 + 2 else 0)
PY
)
[ $? -eq 0 ] || err "README rows differ from REPORT.md: $(printf '%s' "$rowcheck" | tr '\n' ' ')"
nums=$(printf '%s' "$rowcheck" | sed -n 's/^rows \([0-9]*\) .*/\1/p')
for f in docs/eval/results/2026-10-26-pimc-r4-fast.txt docs/eval/results/2026-10-26-pimc-curve-r5-fast.txt docs/eval/results/2026-10-26-pimc-gap-r6-fast.txt; do
  [ -s "$f" ] || err "$f is missing"
done
runs="R4 R5 R6"
[ "${1:-}" = "--fast" ] && runs=R6
# shellcheck disable=SC2086
fast=$(scripts/readme-fast.sh --check $runs 2>&1 | tail -1)
case "$fast" in "readme-fast: PASS"*) ;; *) err "$fast" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M4-11: PASS README.md shows R4, R5, and R6 for PIMC with their settings, command, and files, and $nums rows equal to their REPORT.md rows; $fast; REPORT.md current and checked by check.sh"
else
  echo "M4-11: FAIL"
fi
exit "$fail"
