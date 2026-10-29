#!/usr/bin/env bash
# Builds the web board (web/src) into one self-contained page,
# web/dist/index.html (and web/dist/artifact.html, the same page without
# document tags, for publishing as an artifact): the CSS, the scripts, the logo, and the
# dominofish-wasm module (base64) inline, so it opens from disk with no
# server. Prints the page's path and size.
set -u
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
cargo build -q --release --target wasm32-unknown-unknown -p dominofish-wasm || { echo "ERROR wasm build failed"; exit 1; }
mkdir -p web/dist
/usr/bin/python3 - <<'PY' || { echo "ERROR page assembly failed"; exit 1; }
import base64, re, urllib.parse
src = "web/src/"
version = re.search(r'^version = "([^"]+)"', open("Cargo.toml").read(), re.M).group(1)
wasm = base64.b64encode(open("target/wasm32-unknown-unknown/release/dominofish_wasm.wasm", "rb").read()).decode()
logo = open("docs/assets/dominofish.svg").read().strip()
favicon = urllib.parse.quote(re.sub(r"<title[^>]*>.*?</title>", "", logo, flags=re.S), safe=" =:/,;")
logo = logo.replace('<svg ', '<svg class="logo" aria-hidden="true" focusable="false" ', 1)
logo = re.sub(r'\s*role="img" aria-labelledby="title"', "", logo)
logo = re.sub(r"<title[^>]*>.*?</title>", "", logo, flags=re.S)
script = (f'const DOMINOFISH_VERSION = "{version}";\nconst DOMINOFISH_WASM = "{wasm}";\n'
          + "".join(open(src + f).read() + "\n" for f in ["i18n.js", "tiles.js", "engine.js", "app.js"]))
page = open(src + "index.html").read()
for key, value in [("__STYLE__", open(src + "style.css").read()), ("__LOGO__", logo), ("__FAVICON__", favicon), ("__SCRIPT__", script)]:
    assert page.count(key) == 1, key
    page = page.replace(key, value)
open("web/dist/index.html", "w").write(page)
# The artifact version: the same page without its own document tags, which
# the artifact host adds (docs/FORMATS.md has no claim on it; it is a copy).
head = page[page.index("<head>") + 6:page.index("</head>")]
head = re.sub(r'<meta charset="utf-8">\s*|<meta name="viewport"[^>]*>\s*', "", head)
body = page[page.index("<body>") + 6:page.rindex("</body>")]
head = '<meta name="claude:viewport-fit-sides" content="cover">\n' + head.strip()
open("web/dist/artifact.html", "w").write(head + "\n" + body.strip() + "\n")
PY
echo "web/dist/index.html $(wc -c < web/dist/index.html | tr -d ' ') bytes"
