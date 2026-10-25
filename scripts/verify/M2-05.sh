#!/usr/bin/env bash
# M2-05: never-peeks for every bot. Hidden tiles rearranged behind an
# identical public record (the opening, every move as seen, the seat's hand,
# the scores) give a byte-identical InfoSet text and the same decision of
# every bot at a fixed seed, in games of each bot in both modes; the InfoSet
# text round-trips. --fast runs the same test (seconds).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/M2-05.log
RUST_TEST_THREADS=4 cargo test -p dominofish-core --test never_peeks -- --nocapture > "$log" 2>&1
code=$?
summary=$(grep -o 'never-peeks: .*' "$log")
if [ "$code" -eq 0 ] && grep -q "test every_bot_never_peeks ... ok" "$log" && grep -qxF "## InfoSet text" docs/FORMATS.md; then
  echo "M2-05: PASS ${summary#never-peeks: }"
  exit 0
fi
echo "ERROR never-peeks failed (details in $log)"
echo "M2-05: FAIL"
exit 1
