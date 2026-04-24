#!/usr/bin/env bash
# Build the browser app, upload dist/ plus the zipped reference asset pack, and deploy the worker.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUCKET="mclone"
DIST_DIR="$PROJECT_DIR/dist"
REFERENCE_DIR="$PROJECT_DIR/reference/minecraft-1.17.1"
ASSET_PACK_ZIP="$REFERENCE_DIR/extracted.zip"
ASSET_PACK_MANIFEST="$REFERENCE_DIR/extracted.zip.json"
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
    *.zip)  echo "application/zip" ;;
    *.jpg|*.jpeg) echo "image/jpeg" ;;
    *.ico)  echo "image/x-icon" ;;
    *.woff) echo "font/woff" ;;
    *.woff2) echo "font/woff2" ;;
    *)      echo "application/octet-stream" ;;
  esac
}

upload_file() {
  local file="$1"
  local key="$2"
  local ct
  ct=$(content_type "$file")
  echo "  $key ($ct)"
  $WRANGLER r2 object put "$BUCKET/$key" --file="$file" --content-type="$ct" --remote >/dev/null
}

echo "==> Building (vite)"
cd "$PROJECT_DIR"
pnpm assets:pack
pnpm build

if [ ! -d "$DIST_DIR" ]; then
  echo "Error: dist/ not produced by build." >&2
  exit 1
fi

echo "==> Uploading dist/ to R2 bucket '$BUCKET'"
while IFS= read -r -d '' f; do
  key="${f#$DIST_DIR/}"
  upload_file "$f" "$key"
done < <(find "$DIST_DIR" -type f -print0)

echo "==> Uploading reference asset pack"
upload_file "$ASSET_PACK_ZIP" "reference/minecraft-1.17.1/extracted.zip"
upload_file "$ASSET_PACK_MANIFEST" "reference/minecraft-1.17.1/extracted.zip.json"

echo "==> Deploying worker"
cd "$PROJECT_DIR/worker"
$WRANGLER deploy

echo ""
echo "Deployed to https://mclone.kzahel.com/"
