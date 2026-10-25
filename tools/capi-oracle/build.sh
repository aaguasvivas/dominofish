#!/usr/bin/env bash
# Extracts Capi's engine at the pinned commit from CAPI_DIR with git archive
# and bundles it with Capi's esbuild into tools/capi-oracle/build/ (gitignored).
# Reads CAPI_DIR only; never writes, installs, or builds inside it.
set -u
cd "$(dirname "$0")/../.."
PIN=1d8868a26ce930f478f98e64060c06b1d1a2c1f4
CAPI_DIR="${CAPI_DIR:-/Users/Adelson/Desktop/personal/capi}"
if ! /usr/bin/git -C "$CAPI_DIR" cat-file -e "$PIN^{commit}" 2> /dev/null; then
  echo "ERROR CAPI_DIR=$CAPI_DIR does not hold Capi commit $PIN"
  exit 1
fi
esbuild="$CAPI_DIR/node_modules/.bin/esbuild"
[ -x "$esbuild" ] || { echo "ERROR $esbuild not found"; exit 1; }
out=tools/capi-oracle/build
rm -rf "$out"
mkdir -p "$out"
/usr/bin/git -C "$CAPI_DIR" archive "$PIN" packages/engine/src | tar -x -C "$out" || { echo "ERROR git archive failed"; exit 1; }
"$esbuild" "$out/packages/engine/src/index.ts" --bundle --format=esm --platform=node \
  --outfile="$out/capi-engine.mjs" --log-level=error || { echo "ERROR esbuild failed"; exit 1; }
printf '%s\n' "$PIN" > "$out/PIN"
echo "capi-oracle: built $out/capi-engine.mjs from $PIN ($(wc -c < "$out/capi-engine.mjs" | tr -d ' ') bytes)"
