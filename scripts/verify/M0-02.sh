#!/usr/bin/env bash
# M0-02: scripts/check.sh passes on the repo and fails when a file holds an em
# dash. --fast runs the same checks (check.sh twice, seconds when warm).
set -u
cd "$(dirname "$0")/../.."
fail=0
probe=scripts/verify/.em-dash-probe.txt
trap 'rm -f "$probe"' EXIT

out=$(scripts/check.sh 2>&1)
code=$?
summary=$(printf '%s\n' "$out" | tail -1)
for step in rustfmt clippy tests wasm32 em-dash; do
  printf '%s\n' "$out" | grep -qx "ok $step" || { echo "ERROR check.sh step $step did not pass"; fail=1; }
done
[ "$code" -eq 0 ] || { echo "ERROR check.sh exited $code: $summary"; fail=1; }

printf 'probe %s\n' "$(printf '\342\200\224')" > "$probe"
probe_out=$(scripts/check.sh 2>&1)
probe_code=$?
rm -f "$probe"
if [ "$probe_code" -eq 0 ] || ! printf '%s\n' "$probe_out" | grep -qx "ERROR em dash found"; then
  echo "ERROR check.sh did not fail on an em dash (exit $probe_code)"
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "M0-02: PASS check.sh exit 0 ($summary); exit $probe_code with an em dash present"
else
  echo "M0-02: FAIL"
fi
exit "$fail"
