#!/usr/bin/env bash
# Build and deploy the native Rust/WASM web app to the existing Cloudflare Worker.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUCKET="${MCLONE_DEPLOY_BUCKET:-mclone}"
DEPLOY_DIR="${MCLONE_NATIVE_WEB_DIST_DIR:-$PROJECT_DIR/dist-native-web}"
NATIVE_ROOT="$PROJECT_DIR/native"
WEB_GLUE_BUILD_SCRIPT="$PROJECT_DIR/native/apps/mclone-web-client/scripts/build-web-glue.mjs"
WEB_ROOT="$NATIVE_ROOT/target/mclone-web-client-www"
ANIMAL_CATALOG_WEB_ROOT="$PROJECT_DIR/tools/asset-lab/dist/web"
STRUCTURE_CATALOG_WEB_ROOT="$PROJECT_DIR/tools/structure-lab/dist/web"
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
GIT_REV="$(git -C "$PROJECT_DIR" rev-parse --short=12 HEAD 2>/dev/null || true)"
DEPLOY_VERSION="${MCLONE_NATIVE_WEB_ASSET_VERSION:-${GIT_REV:-nogit}-$(date -u +%Y%m%d%H%M%S)}"

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

ensure_asset_lab_dependencies() {
  if [ -x "$PROJECT_DIR/tools/asset-lab/node_modules/.bin/vite" ]; then
    return
  fi

  echo "==> Installing Asset Lab web dependencies"
  pnpm --dir "$PROJECT_DIR/tools/asset-lab" install --frozen-lockfile
}

ensure_structure_lab_dependencies() {
  if [ -x "$PROJECT_DIR/tools/structure-lab/node_modules/.bin/vite" ]; then
    return
  fi

  echo "==> Installing Structure Lab web dependencies"
  pnpm --dir "$PROJECT_DIR/tools/structure-lab" install --frozen-lockfile
}

echo "==> Building native web assets"
cd "$PROJECT_DIR"
pnpm assets:pack
pnpm assets:pack:first-party
cargo build --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
ensure_wasm_bindgen
# --typescript emits mclone_web_client.d.ts alongside the JS glue (070 Stage 2). It does not
# change what ships — deploy copies only mclone_web_client.js and _bg.wasm out of the bindgen
# dir below — but the .d.ts is the single source the web-glue type-check gate
# (native:web:typecheck) checks the wasm-return boundary against.
"$WASM_BINDGEN_BIN" \
  --target web \
  --typescript \
  --out-dir "$BINDGEN_OUT_DIR" \
  --out-name mclone_web_client \
  "$WASM_PATH"

echo "==> Bundling native web app"
node "$WEB_GLUE_BUILD_SCRIPT"
ensure_asset_lab_dependencies
pnpm asset-lab:web:build
ensure_structure_lab_dependencies
pnpm structure-lab:web:build
rm -rf "$DEPLOY_DIR"
mkdir -p "$DEPLOY_DIR/pkg"
cp -R "$WEB_ROOT"/. "$DEPLOY_DIR"/
cp "$WEB_ROOT/index.html" "$DEPLOY_DIR/smoke.html"
cp "$WEB_ROOT/app.html" "$DEPLOY_DIR/index.html"
perl -0pi -e "s/__MCLONE_NATIVE_WEB_ASSET_VERSION__/$DEPLOY_VERSION/g" \
  "$DEPLOY_DIR/app.html" \
  "$DEPLOY_DIR/index.html"
cp "$BINDGEN_OUT_DIR/mclone_web_client.js" "$DEPLOY_DIR/pkg/mclone_web_client.js"
cp "$BINDGEN_OUT_DIR/mclone_web_client_bg.wasm" "$DEPLOY_DIR/pkg/mclone_web_client_bg.wasm"
mkdir -p "$DEPLOY_DIR/animals"
cp -R "$ANIMAL_CATALOG_WEB_ROOT"/. "$DEPLOY_DIR/animals"/
mkdir -p "$DEPLOY_DIR/structures"
cp -R "$STRUCTURE_CATALOG_WEB_ROOT"/. "$DEPLOY_DIR/structures"/
mkdir -p "$DEPLOY_DIR/reference/minecraft-1.17.1"
cp "$ASSET_PACK_ZIP" "$DEPLOY_DIR/reference/minecraft-1.17.1/extracted.zip"
cp "$ASSET_PACK_MANIFEST" "$DEPLOY_DIR/reference/minecraft-1.17.1/extracted.zip.json"

echo "==> Native web bundle ready: $DEPLOY_DIR"
echo "  asset version: $DEPLOY_VERSION"
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
