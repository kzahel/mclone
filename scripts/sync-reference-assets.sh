#!/usr/bin/env bash
# One-time (or rare) upload of reference/minecraft-1.17.1/extracted/ into R2.
# Slow: ~5s per file via wrangler, run in parallel (~15 min for a full first sync).
# Only re-run if the extracted asset tree changes.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUCKET="mclone"
ASSET_DIR="$PROJECT_DIR/reference/minecraft-1.17.1/extracted"
PARALLEL="${PARALLEL:-20}"
WRANGLER="npx --prefix $PROJECT_DIR wrangler"

if [ ! -d "$ASSET_DIR" ]; then
  echo "Error: $ASSET_DIR not found. Run scripts/extract-assets.sh first." >&2
  exit 1
fi

content_type() {
  case "$1" in
    *.json) echo "application/json" ;;
    *.png)  echo "image/png" ;;
    *.mcmeta) echo "application/json" ;;
    *.ogg)  echo "audio/ogg" ;;
    *.txt)  echo "text/plain" ;;
    *.nbt)  echo "application/octet-stream" ;;
    *)      echo "application/octet-stream" ;;
  esac
}
export -f content_type
export WRANGLER BUCKET ASSET_DIR

upload_one() {
  local f="$1"
  # Key is path relative to $PROJECT_DIR so it mirrors /reference/minecraft-1.17.1/extracted/...
  local key="reference/minecraft-1.17.1/extracted/${f#$ASSET_DIR/}"
  local ct
  ct=$(content_type "$f")
  $WRANGLER r2 object put "$BUCKET/$key" --file="$f" --content-type="$ct" --remote >/dev/null
  echo "  $key"
}
export -f upload_one

total=$(find "$ASSET_DIR" -type f | wc -l | tr -d ' ')
echo "==> Uploading $total files from $ASSET_DIR to R2 bucket '$BUCKET' ($PARALLEL parallel)"
find "$ASSET_DIR" -type f -print0 \
  | xargs -0 -n 1 -P "$PARALLEL" bash -c 'upload_one "$0"'

echo ""
echo "Reference asset sync complete."
