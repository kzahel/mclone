#!/usr/bin/env bash
# Build and deploy the native Rust/WASM web app to the existing Cloudflare Worker.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUCKET="${MCLONE_DEPLOY_BUCKET:-mclone}"
DEPLOY_DIR="${MCLONE_NATIVE_WEB_DIST_DIR:-$PROJECT_DIR/dist-native-web}"
WWW_DIR="$PROJECT_DIR/native/apps/mclone-web-client/www"
NATIVE_ROOT="$PROJECT_DIR/native"
REFERENCE_DIR="$PROJECT_DIR/reference/minecraft-1.17.1"
ASSET_PACK_ZIP="$REFERENCE_DIR/extracted.zip"
ASSET_PACK_MANIFEST="$REFERENCE_DIR/extracted.zip.json"
WASM_PATH="$NATIVE_ROOT/target/wasm32-unknown-unknown/debug/mclone_web_client.wasm"
WASM_BINDGEN_VERSION="0.2.125"
WASM_BINDGEN_ROOT="$NATIVE_ROOT/target/wasm-bindgen-cli-$WASM_BINDGEN_VERSION"
WASM_BINDGEN_BIN="$WASM_BINDGEN_ROOT/bin/wasm-bindgen"
BINDGEN_OUT_DIR="$NATIVE_ROOT/target/wasm32-unknown-unknown/debug/mclone-web-client-bindgen"
WRANGLER="npx --prefix $PROJECT_DIR wrangler"
BUNDLE_ONLY=0

usage() {
  cat <<EOF
Usage: $0 [--bundle-only]

Builds the native web/WASM app into:
  $DEPLOY_DIR

Without --bundle-only, uploads that bundle to R2 bucket '$BUCKET' and deploys
the existing Cloudflare Worker for https://mclone.kzahel.com/.
EOF
}

while (($#)); do
  case "$1" in
    --bundle-only)
      BUNDLE_ONLY=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

content_type() {
  case "$1" in
    *.html) echo "text/html; charset=utf-8" ;;
    *.js|*.mjs) echo "text/javascript; charset=utf-8" ;;
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

ensure_wasm_bindgen() {
  if [ -x "$WASM_BINDGEN_BIN" ]; then
    return
  fi

  cargo install \
    wasm-bindgen-cli \
    --version "$WASM_BINDGEN_VERSION" \
    --locked \
    --root "$WASM_BINDGEN_ROOT"
}

echo "==> Building native web assets"
cd "$PROJECT_DIR"
pnpm assets:pack
cargo build --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
ensure_wasm_bindgen
"$WASM_BINDGEN_BIN" \
  --target web \
  --out-dir "$BINDGEN_OUT_DIR" \
  --out-name mclone_web_client \
  "$WASM_PATH"

echo "==> Bundling native web app"
rm -rf "$DEPLOY_DIR"
mkdir -p "$DEPLOY_DIR/pkg"
cp -R "$WWW_DIR"/. "$DEPLOY_DIR"/
cp "$WWW_DIR/index.html" "$DEPLOY_DIR/smoke.html"
cp "$WWW_DIR/app.html" "$DEPLOY_DIR/index.html"
cp "$BINDGEN_OUT_DIR/mclone_web_client.js" "$DEPLOY_DIR/pkg/mclone_web_client.js"
cp "$BINDGEN_OUT_DIR/mclone_web_client_bg.wasm" "$DEPLOY_DIR/pkg/mclone_web_client_bg.wasm"
mkdir -p "$DEPLOY_DIR/reference/minecraft-1.17.1"
cp "$ASSET_PACK_ZIP" "$DEPLOY_DIR/reference/minecraft-1.17.1/extracted.zip"
cp "$ASSET_PACK_MANIFEST" "$DEPLOY_DIR/reference/minecraft-1.17.1/extracted.zip.json"

echo "==> Native web bundle ready: $DEPLOY_DIR"
find "$DEPLOY_DIR" -type f | sed "s#^$DEPLOY_DIR/#  #"

if [ "$BUNDLE_ONLY" -eq 1 ]; then
  exit 0
fi

echo "==> Uploading native web bundle to R2 bucket '$BUCKET'"
while IFS= read -r -d '' f; do
  key="${f#$DEPLOY_DIR/}"
  upload_file "$f" "$key"
done < <(find "$DEPLOY_DIR" -type f -print0)

echo "==> Deploying worker"
cd "$PROJECT_DIR/worker"
$WRANGLER deploy

echo ""
echo "Deployed native web app to https://mclone.kzahel.com/"
