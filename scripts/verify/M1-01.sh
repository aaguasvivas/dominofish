#!/usr/bin/env bash
# M1-01: docs/RULES.md states rules R1 to R28 and legacy branches L1 to L9,
# each with at least one cite; every cite's anchor appears in its line range
# at the pinned Capi commit; the moves, intent map, canonical state, and rule
# path sections exist, and the path table holds Capi's 14 fuzz paths. Reads
# Capi through CAPI_DIR with git show only. --fast runs the same checks.
set -u
cd "$(dirname "$0")/../.."
export CAPI_DIR="${CAPI_DIR:-/Users/Adelson/Desktop/personal/capi}"
/usr/bin/python3 - <<'EOF'
import os, re, subprocess, sys

PIN = "1d8868a26ce930f478f98e64060c06b1d1a2c1f4"
capi = os.environ["CAPI_DIR"]
text = open("docs/RULES.md").read()
errors = []
cache = {}

def pinned(name):
    sub = "__tests__" if name.endswith(".test.ts") else "src"
    path = f"packages/engine/{sub}/{name}"
    if path not in cache:
        r = subprocess.run(["/usr/bin/git", "-C", capi, "show", f"{PIN}:{path}"],
                           capture_output=True, text=True)
        cache[path] = r.stdout.split("\n") if r.returncode == 0 else None
    return path, cache[path]

norm = lambda s: " ".join(s.split())
cite_re = re.compile(r"`([A-Za-z0-9_.\-]+\.ts):(\d+)(?:-(\d+))?`(?:: `([^`]+)`)?")
cites = cite_re.findall(text)
found = 0
for name, a, b, anchor in cites:
    path, lines = pinned(name)
    where = f"{name}:{a}{'-' + b if b else ''}"
    if lines is None:
        errors.append(f"{where}: {path} not found at the pin")
        continue
    lo, hi = int(a), int(b or a)
    if not (1 <= lo <= hi <= len(lines)):
        errors.append(f"{where}: range outside the file ({len(lines)} lines)")
        continue
    if not anchor:
        errors.append(f"{where}: cite without an anchor")
        continue
    if norm(anchor) not in norm(" ".join(lines[lo - 1:hi])):
        errors.append(f"{where}: anchor not in range: {anchor}")
    else:
        found += 1

for prefix, count in (("R", 28), ("L", 9)):
    for i in range(1, count + 1):
        m = re.search(rf"^- {prefix}{i}\. (.*)$", text, re.M)
        if not m:
            errors.append(f"{prefix}{i} missing")
        elif not cite_re.search(m.group(1)):
            errors.append(f"{prefix}{i} has no cite")

for heading in ("## Rules", "## Legacy branches not reproduced", "## Moves",
                "### Capi intents to moves", "## Canonical state", "## Rule paths"):
    if not re.search(rf"^{re.escape(heading)}$", text, re.M):
        errors.append(f"missing section: {heading}")

paths = re.findall(r"^\| ([a-z0-9-]+) \| (1v1, 2v2|1v1|2v2) \|", text, re.M)
capi_paths = ["capicua", "capicua-equal-ends", "capicua-double", "domino-double-one-end",
              "tranque-blocker-wins", "tranque-rival-wins", "tranque-tie",
              "tranque-tie-opener-not-compared", "pase-corrido-paid", "pase-corrido-capped",
              "salida-paid", "salida-capped", "salida-cancelled", "salida-cancelled-then-corrido"]
names = [p for p, _ in paths]
for p in capi_paths:
    if p not in names:
        errors.append(f"rule path table lacks Capi's path {p}")
if len(set(names)) != len(names):
    errors.append("duplicate rule path ids")

for e in errors:
    print("ERROR M1-01 " + e)
status = "FAIL" if errors else "PASS"
print(f"M1-01: {status} R1-R28 and L1-L9 present; {len(cites)} cites, "
      f"{found} anchors found in range at {PIN[:7]}; "
      f"{len(names)} rule paths ({len(capi_paths)} from Capi's fuzz)")
sys.exit(1 if errors else 0)
EOF
