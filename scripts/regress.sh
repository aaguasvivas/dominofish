#!/usr/bin/env bash
# Runs the fast command of every passing feature in docs/features.json that no
# later entry supersedes. Prints one summary line with the total wall time;
# details go to runs/regress.log. --list prints the commands without running
# them. Exit 0 only when every fast command exits 0.
set -u
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/regress.log

selected=$(/usr/bin/python3 - <<'EOF'
import json
features = json.load(open("docs/features.json"))
superseded = {f["supersedes"] for f in features if f.get("supersedes")}
for f in features:
    if f["passes"] and f["id"] not in superseded:
        print(f["id"] + "\t" + f["fast"])
EOF
) || { echo "ERROR regress: cannot read docs/features.json"; exit 1; }

if [ "${1:-}" = "--list" ]; then
  printf '%s\n' "$selected" | sed '/^$/d'
  exit 0
fi

: > "$log"
start=$(date +%s)
total=0
failed=0
while IFS=$'\t' read -r id cmd; do
  [ -z "$id" ] && continue
  total=$((total + 1))
  t0=$(date +%s)
  echo "== $id: $cmd" >> "$log"
  if bash -c "$cmd" >> "$log" 2>&1; then
    echo "   $id ok in $(( $(date +%s) - t0 ))s" >> "$log"
  else
    echo "ERROR $id fast command failed: $cmd (details in $log)"
    failed=$((failed + 1))
  fi
done <<< "$selected"

secs=$(( $(date +%s) - start ))
if [ "$failed" -eq 0 ]; then
  echo "regress: PASS $total of $total fast commands in ${secs}s"
else
  echo "regress: FAIL $failed of $total fast commands in ${secs}s"
fi
[ "$failed" -eq 0 ]
