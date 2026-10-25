# Part of scripts/verify/M1-05.sh: maps every Capi test cite of
# capi-rules-spec[2] to the docs/RULES.md (Edge cases) row and checks that
# the row's tests passed in runs/M1-05.test.log.
import json, re, sys
lanes = {l["lane"]: l for l in json.load(open("docs/research/2026-10-25-research-lanes.json"))["lanes"]}
detail = lanes["capi-rules-spec"]["findings"][2]["detail"]
cites, current = [], None
for group in re.findall(r"\(([^)]*)\)", detail):
    for token in re.split(r"[;,]", group):
        token = token.strip()
        m = re.fullmatch(r"([\w.\-]+\.ts):(\d+)", token)
        if m:
            current = m.group(1)
            cites.append((current, int(m.group(2))))
        elif re.fullmatch(r":(\d+)", token) and current:
            cites.append((current, int(token[1:])))
        elif re.fullmatch(r"\d+", token) and current:
            cites.append((current, int(token)))
rules = open("docs/RULES.md").read().split("## Edge cases", 1)[1]
rows = {}
for line in rules.splitlines():
    m = re.match(r"^\| (.+?) \| ([a-z0-9_, ]+) \|", line)
    if m:
        rows[m.group(1)] = [t.strip() for t in m.group(2).split(",")]
log = open("runs/M1-05.test.log").read()
passed = set(re.findall(r"^test (\w+)(?: - should panic)? \.\.\. ok$", log, re.M))
errors, tests = [], set()
def need(row_key, names):
    for n in names:
        tests.add(n)
        if n not in passed:
            errors.append(f"{row_key}: test {n} did not pass")
for file, line in sorted(set(cites)):
    key = [k for k in rows if k.startswith(f"`{file}:{line}`")]
    if not key:
        errors.append(f"no Edge cases row for {file}:{line}")
        continue
    need(key[0], rows[key[0]])
for k in rows:
    if k.startswith("Parra") or k.startswith("`reducer.ts:"):
        need(k, rows[k])
for e in errors:
    print("ERROR " + e)
print(f"{len(set(cites))} Capi test cites from capi-rules-spec[2] mapped; {len(tests)} named tests passed, 3 published deals")
sys.exit(1 if errors else 0)
