#!/usr/bin/env bash
# M11-01: the web board (web/, crates/dominofish-wasm/src/board.rs; owner
# request of 2026-10-25, docs/PLAN.md decision 8). Checks:
# - scripts/build-web.sh builds web/dist/index.html, one self-contained page
#   (the wasm module inline, no fetch of a local file);
# - the board request tests in Rust pass (full games in both modes, analysis
#   equal to the solver, bad requests rejected);
# - tools/board-smoke.mjs loads the wasm embedded in the built page and
#   plays full games in both modes through the requests (5 per mode; --fast
#   1): every reply ok, 28 tiles in every view, every bot move legal, every
#   analysis covering every legal move with the best move's value;
# - every string key exists in both English and Spanish.
set -u
cd "$(dirname "$0")/../.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
NODE=${NODE:-/Users/Adelson/.nvm/versions/node/v20.20.0/bin/node}
mkdir -p runs/M11-01
fail=0
err() { echo "ERROR $*"; fail=1; }
games=5
[ "${1:-}" = "--fast" ] && games=1
build=$(scripts/build-web.sh 2>&1 | tail -1)
case "$build" in "web/dist/index.html "*" bytes") ;; *) echo "ERROR $build"; echo "M11-01: FAIL"; exit 1 ;; esac
grep -q 'const DOMINOFISH_WASM = "' web/dist/index.html || err "the page does not embed the wasm module"
if grep -q 'fetch(' web/dist/index.html; then err "the page fetches a file"; fi
cargo test -q -p dominofish-wasm > runs/M11-01/board-tests.log 2>&1 || err "board request tests failed (runs/M11-01/board-tests.log)"
grep -q 'test result: ok. 3 passed' runs/M11-01/board-tests.log || err "the 3 board request tests did not all run"
smoke=$("$NODE" tools/board-smoke.mjs "$games" 2>&1 | tail -1)
case "$smoke" in "board-smoke: games $((2 * games)) "*"errors 0") ;; *) err "smoke: $smoke" ;; esac
keys=$("$NODE" -e '
const src = require("fs").readFileSync("web/src/i18n.js", "utf8");
const STRINGS = new Function(src + "; return STRINGS;")();
const en = Object.keys(STRINGS.en), es = Object.keys(STRINGS.es);
const missing = en.filter((k) => !es.includes(k)).concat(es.filter((k) => !en.includes(k)));
console.log(missing.length ? "missing " + missing.join(",") : "keys " + en.length);
')
case "$keys" in "keys "*) ;; *) err "strings: $keys" ;; esac
if [ "$fail" -eq 0 ]; then
  echo "M11-01: PASS $build; ${smoke#board-smoke: }; board request tests 3 passed; English and Spanish ${keys}"
else
  echo "M11-01: FAIL"
fi
exit "$fail"
