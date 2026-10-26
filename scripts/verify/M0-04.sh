#!/usr/bin/env bash
# M0-04: the state files exist and follow BUILD.md, and regress.sh selects the
# fast command of every passing, non-superseded feature. --fast runs the same
# checks.
set -u
cd "$(dirname "$0")/../.."
fail=0
err() { echo "ERROR $*"; fail=1; }

lines=$(wc -l < CLAUDE.md 2> /dev/null | tr -d " " || echo 999)
[ "$lines" -lt 40 ] || err "CLAUDE.md has $lines lines, want under 40"

for h in "## Milestones" "## Architecture and API" "## Method decisions" "## Rejected techniques" \
         "## Pre-registered runs" "## Seed ranges" "## Resource caps" "## Dependencies" \
         "## Toolchain" "## Estimated sessions" "## Out of scope" "## Decision log"; do
  grep -qxF -- "$h" docs/PLAN.md || err "docs/PLAN.md lacks the heading: $h"
done
grep -qF "rustc 1.99.1" docs/PLAN.md || err "docs/PLAN.md lacks rustc -V"
grep -qF "cargo 1.99.1" docs/PLAN.md || err "docs/PLAN.md lacks cargo -V"
grep -qxF "## Top block" docs/progress.md || err "docs/progress.md lacks its top block"

features=$(/usr/bin/python3 - <<'EOF'
import json, re, sys
F = json.load(open("docs/features.json"))
keys = ["id", "milestone", "description", "verify", "fast", "requires", "passes", "evidence"]
ids = [f["id"] for f in F]
bad = []
if len(set(ids)) != len(ids):
    bad.append("duplicate ids")
for f in F:
    if [k for k in keys if k not in f]:
        bad.append(f"{f.get('id')}: missing fields")
    for r in f.get("requires", []):
        if r not in ids:
            bad.append(f"{f['id']}: unknown requirement {r}")
    if f["passes"] and not (f["evidence"] and {"date", "result", "log"} <= set(f["evidence"])):
        bad.append(f"{f['id']}: passes without evidence")
    if not re.fullmatch(r"M\d+-\d\d", f["id"]) or not f["id"].startswith(f["milestone"] + "-"):
        bad.append(f"{f['id']}: id does not match milestone {f['milestone']}")
per = {}
for f in F:
    per[f["milestone"]] = per.get(f["milestone"], 0) + 1
for m in range(12):
    if f"M{m}" not in per:
        bad.append(f"no M{m} entry")
for m in range(3):
    if per.get(f"M{m}", 0) < 3:
        bad.append(f"M{m} is not at feature grain")
    if f"M{m}-99" not in ids:
        bad.append(f"M{m} has no exit feature")
for b in bad:
    print("ERROR features.json " + b)
print(f"{len(F)} entries, M0 to M11")
sys.exit(1 if bad else 0)
EOF
) || fail=1
printf '%s\n' "$features" | grep '^ERROR' || true

[ -x scripts/regress.sh ] || err "scripts/regress.sh is not executable"
bash -n scripts/regress.sh || err "scripts/regress.sh has a syntax error"
want=$(/usr/bin/python3 -c '
import json
F = json.load(open("docs/features.json"))
s = {f["supersedes"] for f in F if f.get("supersedes")}
# Moved out of regress on 2026-10-26 (scripts/regress.sh header, BUILD.md:
# the slowest fast commands go to the milestone-exit run): the milestone
# exits and M0-02 (check.sh).
moved = lambda i: i.endswith("-99") or i == "M0-02"
print("\n".join(f["id"] + "\t" + f["fast"] for f in F if f["passes"] and f["id"] not in s and not moved(f["id"])))')
got=$(scripts/regress.sh --list)
[ "$want" = "$got" ] || err "regress.sh --list does not match the passing, non-superseded features"

if [ "$fail" -eq 0 ]; then
  echo "M0-04: PASS CLAUDE.md $lines lines; PLAN.md headings; features.json $(printf '%s\n' "$features" | tail -1); progress.md top block; regress.sh selection"
else
  echo "M0-04: FAIL"
fi
exit "$fail"
