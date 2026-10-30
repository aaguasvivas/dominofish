#!/usr/bin/env bash
# M5-04: later results show raw beside corrected. (1) docs/PLAN.md
# (Pre-registered runs) states that every strength run from M5 on plays
# with --aivat 8 and reports its corrected result beside the raw one, and
# every strength run entry (R<n>) stated from 2026-10-27 on names --aivat.
# (2) docs/eval/REPORT.md is current (report --check) and its header
# explains the corrected column; every row whose result file holds aivat
# lines shows a corrected interval and an SD ratio, every other row shows
# "-" there, and every row of a strength run stated from M5 on is
# corrected. (3) README.md shows T7 (by the owner's Q12 answer it stands
# for T5 in this feature): every row of its table equals its REPORT.md row
# (mode, unit, sides, pairs, raw and corrected intervals, SD ratio) and
# the t of its # aivat line; each summary number in the text equals the
# one computed from the 14 committed result files (the largest |t|, the
# sum of t^2, the null rows' largest corrected |t|, the SD ratios and
# their squares, the per-round version's t in row 5 from
# tools/aivat-recompute.py, T6's row 5 t); and the settings are named
# (commit and its Bench line, --aivat 8, target, seeds, command, result
# directory, wall time within the bounds its rows' wall times give).
# (4) Every T7 README row has a --fast reproduction: scripts/readme-fast.sh
# --check T7 reproduces the summary lines of each row's first pairs
# (docs/eval/results/2026-10-30-aivat-t7-fast.txt), and those pairs' lines,
# with their terms, equal the first lines of the committed full rows.
# --fast runs the same checks. README= and PLAN= name other files, for the
# negative controls.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0
err() { echo "ERROR $*"; fail=1; }
cargo run -q -p dominofish -- report --check > /dev/null || err "docs/eval/REPORT.md is stale"
out=$(/usr/bin/python3 - "${README:-README.md}" "${PLAN:-docs/PLAN.md}" <<'PY'
import math, os, re, subprocess, sys
readme = open(sys.argv[1]).read()
plan = open(sys.argv[2]).read()
report = open("docs/eval/REPORT.md").read()
errors = []
err = errors.append

# (1) The PLAN.md rule and the strength runs stated from M5 on.
runs = plan.split("\n## Pre-registered runs\n")[1].split("\n## ")[0]
intro = runs.strip().split("\n")[0]
for want in ["From M5 on (entries stated from 2026-10-27)", "`--aivat 8`", "beside the raw one", "SPRT stays on the raw pair margin"]:
    if want not in intro:
        err(f"PLAN.md Pre-registered runs does not state: {want}")
m5_runs = []
for line in runs.splitlines():
    m = re.match(r"^- (R\d+),", line)
    if not m:
        continue
    date = re.search(r"stated (\d{4}-\d{2}-\d{2})", line)
    if not date:
        err(f"PLAN.md {m.group(1)} has no stated date")
    elif date.group(1) >= "2026-10-27":
        m5_runs.append(m.group(1))
        if "--aivat" not in line:
            err(f"PLAN.md {m.group(1)}, stated {date.group(1)}, does not run with --aivat")

# (2) REPORT.md: corrected beside raw wherever the files hold the terms.
if "The corrected result subtracts AIVAT control variates" not in report.split("\n## ")[0]:
    err("REPORT.md's header does not explain the corrected column")
sections = {}
corrected_rows = plain_rows = 0
for part in report.split("\n## ")[1:]:
    head = re.match(r"^(\S+) \((\w+)\)", part)
    if not head:
        err(f"REPORT.md section without a run id: {part.splitlines()[0]}")
        continue
    run_dir, run_id = head.groups()
    rows = [[c.strip() for c in l.strip().strip("|").split("|")] for l in part.splitlines() if re.match(r"^\| [12]v[12] \|", l)]
    sections[run_dir] = rows
    for r in rows:
        if len(r) != 16:
            err(f"REPORT.md {run_dir}: a row with {len(r)} cells")
            continue
        text = open(os.path.join("docs/eval/results", run_dir, r[12])).read()
        terms = re.search(r"^aivat \d+ ", text, re.M) is not None
        if terms:
            corrected_rows += 1
            if not re.fullmatch(r"-?\d+\.\d\d \[-?\d+\.\d\d, -?\d+\.\d\d\]", r[14]) or not re.fullmatch(r"\d+\.\d\d", r[15]):
                err(f"REPORT.md {run_dir} {r[12]}: the file holds aivat lines, the row shows '{r[14]}' and '{r[15]}'")
        else:
            plain_rows += 1
            if r[14] != "-" or r[15] != "-":
                err(f"REPORT.md {run_dir} {r[12]}: no aivat lines, the row shows '{r[14]}' and '{r[15]}'")
        if run_id in m5_runs and not terms:
            err(f"REPORT.md {run_dir} {r[12]}: {run_id} is stated from M5 on and has no corrected result")

# (3) README.md: T7's rows, numbers, and settings.
t7_dir = "2026-10-30-aivat-t7"
start = readme.find("\n### Variance reduction (AIVAT)\n")
end = readme.find("\n### Reproduce\n", start)
if start < 0 or end < 0:
    err("README.md has no section '### Variance reduction (AIVAT)' before '### Reproduce'")
    print("\n".join(errors)); sys.exit(1)
section = readme[start:end]
report_rows = {}
for r in sections.get(t7_dir, []):
    report_rows[int(re.match(r"row-(\d+)-", r[12]).group(1))] = r
if sorted(report_rows) != list(range(14)):
    err(f"REPORT.md {t7_dir} rows {sorted(report_rows)}, not 0 to 13")
lines = {}
for k, r in report_rows.items():
    text = open(os.path.join("docs/eval/results", t7_dir, r[12])).read()
    lines[k] = re.search(r"^# aivat .*\(AIVAT, cross-fitted\) mean (\S+) sd (\S+) .*; raw sd (\S+); corrected minus raw mean \S+ se \S+ t (\S+);", text, re.M).groups()
seen = []
for line in section.splitlines():
    m = re.match(r"^\| (\d+) \| (1v1|2v2) \| (round|game to 100) \| ([^|]+) \| ([^|]+) \| ([\d,]+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$", line)
    if not m:
        continue
    k, mode, unit, a, b, pairs, raw, corr, t, ratio = [g.strip() for g in m.groups()]
    k = int(k)
    seen.append(k)
    r = report_rows.get(k)
    if r is None:
        err(f"README T7 row {k} is not in REPORT.md")
        continue
    got = (mode, "game" if unit == "game to 100" else unit, a, b, pairs.replace(",", ""), raw, corr, ratio, t)
    want = (r[0], r[1], r[3], r[4], r[5], r[6], r[14], r[15], lines[k][3])
    if got != want:
        err(f"README T7 row {k}: {got}, REPORT.md and its # aivat line {want}")
if sorted(seen) != list(range(14)):
    err(f"README T7 table rows {sorted(seen)}, not 0 to 13 once each")

ts = {k: float(v[3]) for k, v in lines.items()}
null_rows = [k for k, r in report_rows.items() if r[3] == r[4]]
if null_rows != [0, 1, 6, 7, 8, 13]:
    err(f"T7 rows where a bot plays itself: {null_rows}")
pairs_of = {k: int(r[5]) for k, r in report_rows.items()}
null_t = max(abs(float(lines[k][0]) / (float(lines[k][1]) / math.sqrt(pairs_of[k]))) for k in null_rows)
ratios = {k: r[15] for k, r in report_rows.items()}
top = max(ratios.values())
top_rows = sorted(k for k in ratios if ratios[k] == top)
def square(k):
    """Row k's (raw SD / corrected SD)^2: how many times fewer pairs."""
    return f"{(float(lines[k][2]) / float(lines[k][1])) ** 2:.2f}"
top_square = square(top_rows[0])
if any(square(k) != top_square for k in top_rows):
    err(f"the rows at SD ratio {top} square to {[square(k) for k in top_rows]}")
def misses_zero(ci):
    lo, hi = map(float, re.search(r"\[(\S+), (\S+)\]", ci).groups())
    return lo > 0 or hi < 0
if not (misses_zero(report_rows[7][6]) and misses_zero(report_rows[7][14])):
    err("T7 row 7's raw or corrected interval contains 0")
per_round = subprocess.run(["/usr/bin/python3", "-I", "tools/aivat-recompute.py", os.path.join("docs/eval/results", t7_dir, report_rows[5][12])], capture_output=True, text=True).stdout
pr5 = re.search(r"per round: d \S+ se \S+ t (\S+)", per_round).group(1).lstrip("+")
t6 = open([os.path.join("docs/eval/results/2026-10-29-aivat-t6", f) for f in os.listdir("docs/eval/results/2026-10-29-aivat-t6") if f.startswith("row-5-")][0]).read()
t6_5 = float(re.search(r"^# aivat .*corrected minus raw mean \S+ se \S+ t (\S+);", t6, re.M).group(1))
if abs(t6_5) <= 3.29:
    err(f"T6 row 5 t {t6_5} did not fail (a)")
commit = open(os.path.join("docs/eval/results", t7_dir, "COMMIT")).read().strip()
bench = re.search(r"^Bench: (\d+)$", subprocess.run(["/usr/bin/git", "log", "-1", "--format=%B", commit], capture_output=True, text=True).stdout, re.M)
walls = [float(re.search(r"^# run run=T7 row=\d+ .*wall_s=(\S+)", open(os.path.join("docs/eval/results", t7_dir, r[12])).read(), re.M).group(1)) for r in report_rows.values()]
minutes = re.search(r"`scripts/run-t7.sh` \((\d+) minutes at 4 rows at once\)", section)
if not minutes or not sum(walls) / 4 <= 60 * int(minutes.group(1)) <= sum(walls):
    err(f"README's T7 wall time {minutes.group(1) if minutes else 'missing'} min is outside {sum(walls) / 240:.0f} to {sum(walls) / 60:.0f} min")
claims = [
    f"within {max(abs(t) for t in ts.values()):.3f} standard errors of 0 in every row",
    f"the sum of t² over the 14 rows is {sum(t * t for t in ts.values()):.2f}",
    "against a limit of 36.12 (the 0.999 quantile of chi-square with 14 df)",
    f"In rows {', '.join(map(str, null_rows[:-1]))}, and {null_rows[-1]} a bot plays itself",
    f"the corrected means lie within {null_t:.2f} standard errors of 0",
    f"at least one interval at 95% misses 0 with a probability of about {1 - 0.95 ** len(null_rows):.2f}",
    f"T6's row 5 exposed that bias, and on T7's games the same version gives t {pr5} in row 5",
    f"At {ratios[5]}, as in row 5, a corrected result needs {square(5)} times fewer pairs",
    f"at {top}, as in rows {' and '.join(map(str, top_rows))}, it needs {top_square} times fewer",
    f"both sides from commit {commit} (Bench {int(bench.group(1)):,})" if bench else f"commit {commit} has no Bench line",
    "PIMC at 8 worlds per move in rows 4 and 11", "`--aivat 8`", "target 100",
    "deal seeds 640,000,000 + 1,000,000 k + i for pair i of row k, from the test range", "engine seed base 5,000,900,000 + k",
    "no SPRT", f"Results: docs/eval/results/{t7_dir}/", "[docs/eval/REPORT.md](docs/eval/REPORT.md)",
    "Test T7, pre-registered in [docs/PLAN.md](docs/PLAN.md)", "The SPRT stays on the raw pair margin",
    "R1 to R6 came before M5 and report raw results only",
]
for want in claims:
    if want not in section:
        err(f"README's T7 section does not say: {want}")
if not all(report_rows[k][3] == "pimc (8 worlds)" for k in (4, 11)):
    err("T7 rows 4 and 11 are not PIMC at 8 worlds")
if f"the SD of a result falls by a factor of up to {top}" not in readme or f"corrected minus raw within {max(abs(t) for t in ts.values()):.3f} standard errors of 0" not in readme:
    err("README's Variance reduction status row differs from T7")
if errors:
    print("\n".join(errors)); sys.exit(1)
print(f"{len(m5_runs)} strength runs stated from M5 on, all with --aivat; REPORT.md shows the corrected result in {corrected_rows} rows with aivat terms and '-' in {plain_rows} rows without; README.md shows T7's 14 rows equal to REPORT.md and their files, {len(claims)} settings and numbers checked (largest |t| {max(abs(t) for t in ts.values()):.3f}, null rows' corrected |t| at most {null_t:.2f}, SD ratio up to {top}, per-round t {pr5} in row 5)")
PY
)
code=$?
if [ "$code" -ne 0 ]; then
  printf '%s\n' "$out" | sed 's/^ERROR //; s/^/ERROR /'
  fail=1
fi
# (4) T7's --fast reproduction (every README number has one): its summary
# lines reproduce from commit ff3cdea, and each fast row's pair and aivat
# lines equal the first lines of the committed full row.
fast=$(scripts/readme-fast.sh --check T7 2>&1 | tail -1)
case "$fast" in "readme-fast: PASS"*) ;; *) err "$fast" ;; esac
same_pairs=0
for f in docs/eval/results/2026-10-30-aivat-t7/row-*.txt; do
  k=$(basename "$f" | cut -d- -f2)
  g=runs/readme-fast/2026-10-30-aivat-t7-fast-row-$k.txt
  n=$(grep -c '^pair ' "$g" 2>/dev/null)
  [ "${n:-0}" -gt 0 ] || { err "row $k: no pairs in $g"; continue; }
  [ "$(grep -E '^(pair|aivat) ' "$g")" = "$(grep -E '^(pair|aivat) ' "$f" | awk -v n="$n" '$2 < n')" ] || { err "row $k: the fast run's first $n pairs differ from the full run's"; continue; }
  same_pairs=$((same_pairs + n))
done
if [ "$fail" -eq 0 ]; then
  echo "M5-04: PASS $out; $fast, whose $same_pairs pairs equal the full rows' first pairs with their terms"
else
  echo "M5-04: FAIL"
fi
exit "$fail"
