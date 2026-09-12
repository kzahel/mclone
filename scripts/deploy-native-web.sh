#!/usr/bin/env bash
# Build and deploy the native Rust/WASM web app to the existing Cloudflare Worker.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DEPLOY_DIR="${MCLONE_NATIVE_WEB_DIST_DIR:-$PROJECT_DIR/dist-native-web}"
NATIVE_ROOT="$PROJECT_DIR/native"
WEB_GLUE_BUILD_SCRIPT="$PROJECT_DIR/native/apps/mclone-web-client/scripts/build-web-glue.mjs"
WEB_ROOT="$NATIVE_ROOT/target/mclone-web-client-www"
ANIMAL_CATALOG_WEB_ROOT="$PROJECT_DIR/tools/asset-lab/dist/web"
STRUCTURE_CATALOG_WEB_ROOT="$PROJECT_DIR/tools/structure-lab/dist/web"
TERRAIN_LAB_WEB_ROOT="$PROJECT_DIR/tools/terrain-lab/dist/web"
TEXTURE_LAB_WEB_ROOT="$PROJECT_DIR/tools/texture-lab/dist/web"
WORLD_EXPLORER_WEB_ROOT="$NATIVE_ROOT/target/mclone-world-explorer-www"
WORLD_EXPLORER_BUILD_SCRIPT="$PROJECT_DIR/native/apps/mclone-world-explorer/scripts/build-web.mjs"
WASM_PATH="$NATIVE_ROOT/target/wasm32-unknown-unknown/debug/mclone_web_client.wasm"
WASM_BINDGEN_VERSION="0.2.125"
WASM_BINDGEN_ROOT="$NATIVE_ROOT/target/wasm-bindgen-cli-$WASM_BINDGEN_VERSION"
WASM_BINDGEN_BIN="$WASM_BINDGEN_ROOT/bin/wasm-bindgen"
BINDGEN_OUT_DIR="$NATIVE_ROOT/target/wasm32-unknown-unknown/debug/mclone-web-client-bindgen"
STATIC_ASSET_HEADERS="$PROJECT_DIR/worker/_headers"
WRANGLER=(npx --prefix "$PROJECT_DIR" wrangler)
BUNDLE_ONLY=0
GIT_REV="$(git -C "$PROJECT_DIR" rev-parse --short=12 HEAD 2>/dev/null || true)"
DEPLOY_VERSION="${MCLONE_NATIVE_WEB_ASSET_VERSION:-${GIT_REV:-nogit}-$(date -u +%Y%m%d%H%M%S)}"

usage() {
  cat <<EOF
Usage: $0 [--bundle-only]

Builds the native web/WASM app into:
  $DEPLOY_DIR

Without --bundle-only, deploys that directory with Cloudflare Workers Static
Assets and publishes the existing Worker for https://mclone.kzahel.com/.
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

ensure_terrain_lab_dependencies() {
  if [ -x "$PROJECT_DIR/tools/terrain-lab/node_modules/.bin/vite" ]; then
    return
  fi

  echo "==> Installing Terrain Lab web dependencies"
  pnpm --dir "$PROJECT_DIR/tools/terrain-lab" install --frozen-lockfile
}

ensure_texture_lab_dependencies() {
  if [ -x "$PROJECT_DIR/tools/texture-lab/node_modules/.bin/vite" ]; then
    return
  fi

  echo "==> Installing Texture Lab web dependencies"
  pnpm --dir "$PROJECT_DIR/tools/texture-lab" install --frozen-lockfile
}

echo "==> Building native web assets"
cd "$PROJECT_DIR"
pnpm --dir tools/texture-lab install --frozen-lockfile
if [[ "${MCLONE_USE_STAGED_ASSETS:-0}" != "1" ]]; then
  pnpm assets:pack:first-party
fi
cargo build --locked --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
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
ensure_terrain_lab_dependencies
pnpm terrain-lab:web:build
ensure_texture_lab_dependencies
pnpm texture-lab:web:build
node "$WORLD_EXPLORER_BUILD_SCRIPT"
rm -rf "$DEPLOY_DIR"
mkdir -p "$DEPLOY_DIR/pkg"
cp -R "$WEB_ROOT"/. "$DEPLOY_DIR"/
cp "$WEB_ROOT/index.html" "$DEPLOY_DIR/smoke.html"
cp "$WEB_ROOT/hub.html" "$DEPLOY_DIR/index.html"
mkdir -p "$DEPLOY_DIR/play" "$DEPLOY_DIR/Play"
cp "$WEB_ROOT/app.html" "$DEPLOY_DIR/play/index.html"
cp "$WEB_ROOT/app.html" "$DEPLOY_DIR/Play/index.html"
perl -0pi -e 's#src="\./mclone-web-app\.js#src="/mclone-web-app.js#g' \
  "$DEPLOY_DIR/play/index.html" \
  "$DEPLOY_DIR/Play/index.html"
perl -0pi -e "s/__MCLONE_NATIVE_WEB_ASSET_VERSION__/$DEPLOY_VERSION/g" \
  "$DEPLOY_DIR/app.html" \
  "$DEPLOY_DIR/play/index.html" \
  "$DEPLOY_DIR/Play/index.html"
cp "$BINDGEN_OUT_DIR/mclone_web_client.js" "$DEPLOY_DIR/pkg/mclone_web_client.js"
cp "$BINDGEN_OUT_DIR/mclone_web_client_bg.wasm" "$DEPLOY_DIR/pkg/mclone_web_client_bg.wasm"
mkdir -p "$DEPLOY_DIR/animals"
cp -R "$ANIMAL_CATALOG_WEB_ROOT"/. "$DEPLOY_DIR/animals"/
mkdir -p "$DEPLOY_DIR/structures"
cp -R "$STRUCTURE_CATALOG_WEB_ROOT"/. "$DEPLOY_DIR/structures"/
mkdir -p "$DEPLOY_DIR/terrain"
cp -R "$TERRAIN_LAB_WEB_ROOT"/. "$DEPLOY_DIR/terrain"/
mkdir -p "$DEPLOY_DIR/textures"
cp -R "$TEXTURE_LAB_WEB_ROOT"/. "$DEPLOY_DIR/textures"/
mkdir -p "$DEPLOY_DIR/explore"
cp -R "$WORLD_EXPLORER_WEB_ROOT"/. "$DEPLOY_DIR/explore"/
cp "$STATIC_ASSET_HEADERS" "$DEPLOY_DIR/_headers"

node "$PROJECT_DIR/scripts/run-python.mjs" "$PROJECT_DIR/scripts/check-public-assets.py" "$DEPLOY_DIR" --report "$DEPLOY_DIR/public-files.json"

echo "==> Native web bundle ready: $DEPLOY_DIR"
echo "  asset version: $DEPLOY_VERSION"
find "$DEPLOY_DIR" -type f | sed "s#^$DEPLOY_DIR/#  #"

if [ "$BUNDLE_ONLY" -eq 1 ]; then
  exit 0
fi

echo "==> Deploying Worker and manifest-diffed static assets"
cd "$PROJECT_DIR/worker"
"${WRANGLER[@]}" deploy --assets "$DEPLOY_DIR"

echo ""
echo "Deployed native web app to https://mclone.kzahel.com/"
