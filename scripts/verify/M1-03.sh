#!/usr/bin/env bash
# M1-03: the Capi oracle tooling. build.sh extracts and bundles the pinned
# engine through CAPI_DIR and fails on a CAPI_DIR without the pinned commit;
# CAPI_DIR's git status and HEAD are unchanged afterwards; the self-test
# passes (bundle exports, fixed deals, the copied mulberry32); docs/FORMATS.md
# records mulberry32 and the fixed-deal rng; tools/ has no package.json.
# --fast runs the same checks (a few seconds).
set -u
cd "$(dirname "$0")/../.."
export CAPI_DIR="${CAPI_DIR:-/Users/Adelson/Desktop/personal/capi}"
node=/Users/Adelson/.nvm/versions/node/v20.20.0/bin/node
fail=0
err() { echo "ERROR $*"; fail=1; }

before=$(/usr/bin/git -C "$CAPI_DIR" status --porcelain; /usr/bin/git -C "$CAPI_DIR" rev-parse HEAD)
build=$(tools/capi-oracle/build.sh 2>&1) || err "build.sh failed: $build"

empty=$(mktemp -d)
if bad=$(CAPI_DIR="$empty" tools/capi-oracle/build.sh 2>&1); then
  err "build.sh accepted a CAPI_DIR without the pinned commit"
elif ! printf '%s\n' "$bad" | grep -q '^ERROR'; then
  err "build.sh failed without an ERROR line: $bad"
fi
rmdir "$empty"
tools/capi-oracle/build.sh > /dev/null 2>&1 || err "rebuild failed"

selftest=$("$node" tools/capi-oracle/selftest.mjs 2>&1) || err "selftest failed: $selftest"
after=$(/usr/bin/git -C "$CAPI_DIR" status --porcelain; /usr/bin/git -C "$CAPI_DIR" rev-parse HEAD)
[ "$before" = "$after" ] || err "CAPI_DIR git status or HEAD changed"

grep -qF 'export function mulberry32(seed) {' docs/FORMATS.md || err "FORMATS.md lacks mulberry32"
grep -qF 'fixedDealRng' docs/FORMATS.md || err "FORMATS.md lacks the fixed-deal rng"
[ -z "$(find tools -name package.json)" ] || err "a package.json exists under tools/"

if [ "$fail" -eq 0 ]; then
  echo "M1-03: PASS $(printf '%s\n' "$build" | tail -1 | sed 's/^capi-oracle: //'); refuses a CAPI_DIR without the pin; CAPI_DIR unchanged; $(printf '%s\n' "$selftest" | tail -1)"
else
  echo "M1-03: FAIL"
fi
exit "$fail"
