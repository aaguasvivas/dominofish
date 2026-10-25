#!/usr/bin/env bash
# M0-01: the toolchain is pinned and the build settings are committed.
# --fast runs the same checks (they take about a second).
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fail=0

need() {
  if ! grep -qxF -- "$2" "$1"; then
    echo "ERROR $1 lacks the line: $2"
    fail=1
  fi
}

# The value of key $3 inside TOML section [$2] of file $1.
section_value() {
  awk -v sec="[$2]" -v key="$3" '
    /^\[/ { in_sec = ($0 == sec); next }
    in_sec && $1 == key { sub(/^[^=]*=[[:space:]]*/, ""); print; exit }' "$1"
}

need rust-toolchain.toml 'channel = "1.99.1"'
need rust-toolchain.toml 'components = ["clippy", "rustfmt"]'
need rust-toolchain.toml 'targets = ["wasm32-unknown-unknown"]'
need rust-toolchain.toml 'profile = "minimal"'
need .cargo/config.toml 'jobs = 4'
for p in target/ runs/ tools/capi-oracle/build/; do need .gitignore "$p"; done
for prof in profile.dev profile.test; do
  v=$(section_value Cargo.toml "$prof" debug)
  if [ "$v" != '"line-tables-only"' ]; then
    echo "ERROR Cargo.toml [$prof] debug is '$v', want \"line-tables-only\""
    fail=1
  fi
done

rustc_v=$(rustc -V)
case "$rustc_v" in
  "rustc 1.99.1 "*) ;;
  *) echo "ERROR rustc -V is '$rustc_v', want 1.99.1"; fail=1 ;;
esac
if ! rustup target list --installed | grep -qx wasm32-unknown-unknown; then
  echo "ERROR wasm32-unknown-unknown target is not installed"
  fail=1
fi
cargo clippy -V > /dev/null 2>&1 || { echo "ERROR clippy missing"; fail=1; }
cargo fmt --version > /dev/null 2>&1 || { echo "ERROR rustfmt missing"; fail=1; }

if [ "$fail" -eq 0 ]; then
  echo "M0-01: PASS $rustc_v, clippy, rustfmt, wasm32 target, jobs 4, line-tables-only"
else
  echo "M0-01: FAIL"
fi
exit "$fail"
