#!/usr/bin/env bash
# Build and upload the zipped reference asset pack used by the browser runtime.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUCKET="mclone"
REFERENCE_DIR="$PROJECT_DIR/reference/minecraft-1.17.1"
ASSET_PACK_ZIP="$REFERENCE_DIR/extracted.zip"
ASSET_PACK_MANIFEST="$REFERENCE_DIR/extracted.zip.json"
WRANGLER="npx --prefix $PROJECT_DIR wrangler"

content_type() {
  case "$1" in
    *.json) echo "application/json" ;;
    *.zip)  echo "application/zip" ;;
    *)      echo "application/octet-stream" ;;
  esac
}

upload_one() {
  local file="$1"
  local key="$2"
  local ct
  ct=$(content_type "$file")
  echo "  $key ($ct)"
  $WRANGLER r2 object put "$BUCKET/$key" --file="$file" --content-type="$ct" --remote >/dev/null
}

cd "$PROJECT_DIR"
pnpm assets:pack

echo "==> Uploading reference asset pack to R2 bucket '$BUCKET'"
upload_one "$ASSET_PACK_ZIP" "reference/minecraft-1.17.1/extracted.zip"
upload_one "$ASSET_PACK_MANIFEST" "reference/minecraft-1.17.1/extracted.zip.json"

echo ""
echo "Reference asset pack sync complete."
