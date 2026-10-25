#!/usr/bin/env bash
# Every check main must pass: format, clippy, tests, the wasm32 build of the
# core, and the em dash scan. Prints one line per check and a summary line;
# details go to runs/check.log. Exit 0 only when every check passes.
set -u
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p runs
log=runs/check.log
: > "$log"
fail=0
start=$(date +%s)

step() {
  local name=$1
  shift
  echo "== $name: $*" >> "$log"
  if "$@" >> "$log" 2>&1; then
    echo "ok $name"
  else
    echo "ERROR $name failed (details in $log)"
    fail=1
  fi
}

step rustfmt cargo fmt --all --check
step clippy cargo clippy --workspace --all-targets -- -D warnings
step tests env RUST_TEST_THREADS=4 cargo test --workspace
step wasm32 cargo build -p dominofish-core --target wasm32-unknown-unknown
step report cargo run -q -p dominofish -- report --check

if grep -rIn "$(printf '\342\200\224')" --exclude-dir=target --exclude-dir=.git --exclude-dir=runs .; then
  echo "ERROR em dash found"
  fail=1
else
  echo "ok em-dash"
fi

secs=$(( $(date +%s) - start ))
if [ "$fail" -eq 0 ]; then
  echo "check: PASS in ${secs}s"
else
  echo "check: FAIL in ${secs}s"
fi
exit "$fail"
