#!/usr/bin/env bash
# Usage: scripts/mark-pass.sh <feature-id> [committed-log-path]
# Runs the feature's verify command from docs/features.json and, only when it
# exits 0, sets passes to true and evidence to today's date, the command's
# last output line, and the log path (or null). Changes no other field.
set -u
cd "$(dirname "$0")/.."
id=${1:?usage: scripts/mark-pass.sh <feature-id> [log]}
log=${2:-}
cmd=$(/usr/bin/python3 -c '
import json, sys
f = [f for f in json.load(open("docs/features.json")) if f["id"] == sys.argv[1]]
print(f[0]["verify"] if f else "")' "$id")
[ -n "$cmd" ] || { echo "ERROR unknown feature $id"; exit 1; }
mkdir -p runs
bash -c "$cmd" > "runs/verify-$id.log" 2>&1
code=$?
cat "runs/verify-$id.log"
last=$(tail -1 "runs/verify-$id.log")
if [ "$code" -ne 0 ]; then
  echo "ERROR $id verify exited $code: $last (details in runs/verify-$id.log)"
  exit 1
fi
/usr/bin/python3 - "$id" "$last" "$log" <<'EOF'
import json, sys, datetime
fid, result, log = sys.argv[1], sys.argv[2], sys.argv[3] or None
F = json.load(open("docs/features.json"))
for f in F:
    if f["id"] == fid:
        f["passes"] = True
        f["evidence"] = {"date": datetime.date.today().isoformat(), "result": result, "log": log}
with open("docs/features.json", "w") as fh:
    json.dump(F, fh, indent=1, ensure_ascii=False)
    fh.write("\n")
EOF
echo "marked $id: $last"
