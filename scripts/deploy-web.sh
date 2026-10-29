#!/usr/bin/env bash
# Deploys the web board (docs/PLAN.md decision 9) to Cloudflare Workers static
# assets with wrangler.jsonc: https://dominofish.adelsonaguasvivas.workers.dev.
# Runs only from a clean, committed tree, builds web/dist, deploys it, and
# prints the commit and the page's SHA-256 to compare with the live page:
#   curl -s https://dominofish.adelsonaguasvivas.workers.dev/ | shasum -a 256
# Needs a wrangler login on this machine (npx wrangler login).
set -u
cd "$(dirname "$0")/.."
[ -z "$(/usr/bin/git status --porcelain)" ] || { echo "ERROR uncommitted changes; deploy only a committed tree"; exit 1; }
scripts/build-web.sh || exit 1
npx --yes wrangler@4.156.0 deploy || { echo "ERROR wrangler deploy failed"; exit 1; }
echo "deployed $(/usr/bin/git rev-parse --short HEAD); page sha256 $(shasum -a 256 web/dist/index.html | cut -d' ' -f1)"
