#!/usr/bin/env bash
# Builds the dominofish binary of a commit (default HEAD) into
# runs/bin/<commit>/dominofish from a git archive of that commit, with the
# old-commit target tree target/old (BUILD.md M2). Matches run only such
# copies, never target/release. Prints the binary's path.
set -u
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
commit=$(/usr/bin/git rev-parse --short=7 "${1:-HEAD}") || { echo "ERROR unknown commit ${1:-HEAD}"; exit 1; }
bin="runs/bin/$commit/dominofish"
if [ -x "$bin" ]; then
  echo "$bin"
  exit 0
fi
free=$(df -g /Users/Adelson | awk 'NR==2 {print $4}')
[ "$free" -ge 5 ] || { echo "ERROR only ${free} GB free"; exit 1; }
mkdir -p "runs/src/$commit" "runs/bin/$commit" &&
  /usr/bin/git archive "$commit" | tar -x -C "runs/src/$commit" &&
  cargo build --release -q --manifest-path "runs/src/$commit/Cargo.toml" --target-dir target/old -p dominofish > runs/build-bin.log 2>&1 &&
  cp target/old/release/dominofish "runs/bin/$commit/" &&
  rm -rf "runs/src/$commit" || { echo "ERROR build of $commit failed (runs/build-bin.log)"; rm -rf "runs/src/$commit"; exit 1; }
echo "$bin"
