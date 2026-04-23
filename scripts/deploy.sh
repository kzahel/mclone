#!/usr/bin/env bash
# Build + upload dist/ + deploy the worker. Fast: only touches built artifacts.
# Reference assets are uploaded separately by scripts/sync-reference-assets.sh.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUCKET="mclone"
DIST_DIR="$PROJECT_DIR/dist"
WRANGLER="npx --prefix $PROJECT_DIR wrangler"

content_type() {
  case "$1" in
    *.html) echo "text/html" ;;
    *.js|*.mjs) echo "application/javascript" ;;
    *.wasm) echo "application/wasm" ;;
    *.json) echo "application/json" ;;
    *.map)  echo "application/json" ;;
    *.css)  echo "text/css" ;;
    *.svg)  echo "image/svg+xml" ;;
    *.png)  echo "image/png" ;;
    *.jpg|*.jpeg) echo "image/jpeg" ;;
    *.ico)  echo "image/x-icon" ;;
    *.woff) echo "font/woff" ;;
    *.woff2) echo "font/woff2" ;;
    *)      echo "application/octet-stream" ;;
  esac
}

echo "==> Building (vite)"
cd "$PROJECT_DIR"
pnpm build

if [ ! -d "$DIST_DIR" ]; then
  echo "Error: dist/ not produced by build." >&2
  exit 1
fi

echo "==> Uploading dist/ to R2 bucket '$BUCKET'"
while IFS= read -r -d '' f; do
  key="${f#$DIST_DIR/}"
  ct=$(content_type "$f")
  echo "  $key ($ct)"
  $WRANGLER r2 object put "$BUCKET/$key" --file="$f" --content-type="$ct" --remote >/dev/null
done < <(find "$DIST_DIR" -type f -print0)

echo "==> Deploying worker"
cd "$PROJECT_DIR/worker"
$WRANGLER deploy

echo ""
echo "Deployed to https://mclone.kzahel.com/"
