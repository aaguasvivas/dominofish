#!/usr/bin/env bash
# Runs the fast command of every passing feature in docs/features.json that no
# later entry supersedes, except two kinds moved out on 2026-10-26, when the
# run reached 1,419 s against its 10-minute target (BUILD.md, check.sh and
# regress.sh): the milestone exits (M<k>-99), whose fast commands rerun
# their milestone's fast commands a second time and run at each milestone
# exit, and M0-02, whose fast command is scripts/check.sh, run before
# regress (CONTINUE.md) and by the pre-commit hook on every commit. Prints
# one summary line with the total wall time;
# details go to runs/regress.log. --list prints the commands without running
# them. Exit 0 only when every fast command exits 0 and ends with its
# "<id>: PASS" line (bash 3.2 can exit 0 after an error).
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
    moved = f["id"].endswith("-99") or f["id"] == "M0-02"
    if f["passes"] and f["id"] not in superseded and not moved:
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
  out=$(bash -c "$cmd" < /dev/null 2>&1)
  code=$?
  printf '%s\n' "$out" >> "$log"
  last=$(printf '%s\n' "$out" | tail -1)
  if [ "$code" -eq 0 ] && case "$last" in "$id: PASS"*) true ;; *) false ;; esac; then
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
