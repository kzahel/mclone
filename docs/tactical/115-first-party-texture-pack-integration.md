# 115 - First-Party Texture Pack Integration

Status: active; Slice 2 first-party overlay pack landed.

## Goal

Move original texture-lab output toward engine use without committing Mojang
assets or depending on texture-lab preview code at runtime.

The near-term target is a development override pack:

- original texture source comes from `tools/texture-lab/packs/mclone-default/`
- runtime-compatible outputs use vanilla resource paths
- local vanilla blockstate/model JSON can still come from the user's
  `reference/minecraft-1.17.1/` assets during development
- generated review sheets and runtime bridge outputs stay under `/tmp`

The longer-term target is a distributable first-party default pack. That pack
must also own or generate enough blockstate/model data, because Mojang JSON is
not distributable.

## Workstream

Shared assets and native runtime integration. Runtime behavior should remain
owned by shared crates such as `mclone-assets`, `mclone-mesh`,
`mclone-render-session`, and `mclone-render`. App crates should only own
platform-specific source discovery, staging, and user-facing selection.

## Current Runtime Shape

The native runtime already loads an ordered `AssetSourceChain`:

- first-party overlay packs from `MCLONE_ASSET_OVERLAY_PACK`, when set
- first-party loose `assets/` when present
- local loose extracted assets
- packed assets such as `reference/minecraft-1.17.1/extracted.zip`
- local sound overlays

The terrain mesh path loads `minecraft` blockstates and models, then resolves
model texture references to paths like:

```text
assets/minecraft/textures/block/dirt.png
assets/minecraft/textures/block/grass_block_top.png
assets/minecraft/textures/block/grass_block_side.png
assets/minecraft/textures/block/grass_block_side_overlay.png
```

Texture lab authoring currently exports namespaced source art under
`assets/mclone/textures/block/`. That is useful as an authored namespace, but it
will not affect terrain that still reads vanilla model JSON unless we also
produce compatibility paths or add model/blockstate overrides.

Tint handling is already partially present. Model `tintindex` values survive
asset baking and the mesh builder applies a hard-coded green tint for tinted
faces. This is enough for a first neutral grass source test, but later work
should make grass/foliage/water tint definitions data-driven.

The checked-in illustration source is TypeScript DSL, not PNGs:

```text
tools/texture-lab/packs/mclone-default/texture.ts
tools/texture-lab/packs/mclone-default/block/dirt.ts
tools/texture-lab/packs/mclone-default/block/grass-block.ts
```

Those files define palettes, tint roles, seeded procedural speckles, and ASCII
masks. `pnpm texture-lab:runtime-compat` renders deterministic PNGs from that
source into `/tmp/mclone-texture-lab/runtime-pack/`.

`pnpm texture-lab:pack-overlay` then packs that runtime-compatible tree into:

```text
/tmp/mclone-texture-lab/mclone-default-overlay.pbp
```

The native runtime can apply it as a mod-pack-style overlay:

```sh
MCLONE_ASSET_OVERLAY_PACK=/tmp/mclone-texture-lab/mclone-default-overlay.pbp pnpm native:timedemo:smoke
```

`pnpm texture-lab:coverage` compares that overlay pack against the texture
materials the native terrain atlas actually requests and writes:

```text
/tmp/mclone-texture-lab/mclone-default-overlay-coverage.md
```

## Slice 1 - Runtime-Compatible Texture-Lab Export

Add a texture-lab export mode that writes derived PNGs to:

```text
/tmp/mclone-texture-lab/runtime-pack/assets/minecraft/textures/block/
```

This mode maps current authored paths:

```text
assets/mclone/textures/block/<name>.png
```

to runtime-compatible paths:

```text
assets/minecraft/textures/block/<name>.png
```

This is intentionally an override pack, not a standalone first-party pack. The
native runtime still needs local vanilla blockstate/model JSON from the user's
reference asset tree or reference pack.

Validation target:

```sh
pnpm texture-lab:typecheck
pnpm texture-lab:runtime-compat
MCLONE_FIRST_PARTY_ASSET_ROOT=/tmp/mclone-texture-lab/runtime-pack pnpm native:timedemo:smoke
```

## Slice 2 - First-Party Overlay Pack Contract

Split first-party overlay pack production from the Mojang reference pack.

Landed shape:

- `tools/minecraft_assets/overlay_pack.py` packs any first-party root containing
  `assets/`.
- `pnpm texture-lab:pack-overlay` regenerates texture-lab runtime-compatible
  PNGs and writes `/tmp/mclone-texture-lab/mclone-default-overlay.pbp`.
- The overlay pack manifest records `source_kind: first_party_overlay` and
  fingerprints only the first-party source tree.
- Runtime `MCLONE_ASSET_OVERLAY_PACK` accepts a platform path-list of overlay
  packs and pushes them before the rest of the asset source chain.

This is still an overlay pack, not a standalone first-party pack. Local
reference blockstates/models still fill in missing assets during development.

The existing `tools/minecraft_assets/asset_pack.py` already writes
`mclone-pack.json` manifests and can include repo `assets/`, but it currently
couples first-party files and local extracted Mojang files into one asset set.
Do not use that mixed pack as the public distribution boundary.

## Slice 3 - Standalone Terrain Coverage

A real distributable default cannot depend on Mojang model or blockstate JSON.
The overlay coverage report now makes this work measurable by listing required,
covered, missing, and unused block texture paths. Before calling the first-party
pack standalone, add one of:

- repo-owned generated blockstate/model JSON for the terrain MVP set
- a native fallback model catalog for simple block shapes
- a hybrid, with generated JSON for simple cube/cross/overlay cases and shared
  native fallbacks for missing low-value states

The first standalone milestone should be small and explicit. Dirt and
grass-block are enough for proving source-chain behavior, but not enough for a
normal generated overworld because `BlockStateRegistry::terrain_mvp()` includes
stone, logs, leaves, plants, fluids, snow, ores, and other states.

## Non-Goals For Slice 1

- No committed generated PNGs.
- No runtime UI for choosing packs.
- No web asset-pack URL migration.
- No standalone first-party pack claim.
- No dependency from native runtime crates on `tools/texture-lab`.

## Status

2026-06-30:

- Investigated the existing asset source chain, terrain asset loading,
  texture path resolution, and tint handling.
- Added a texture-lab runtime-compatible export mode and root
  `pnpm texture-lab:runtime-compat` wrapper.
- Moved accepted dirt and grass-block sources into
  `tools/texture-lab/packs/mclone-default/`, with examples reduced to wrappers
  around the canonical pack modules.
- Runtime-compatible export writes original dirt/grass starter PNGs under
  `/tmp/mclone-texture-lab/runtime-pack/assets/minecraft/textures/block/`.
- Native timedemo passed with
  `MCLONE_FIRST_PARTY_ASSET_ROOT=/tmp/mclone-texture-lab/runtime-pack`.
- Native full-frame screenshot
  `/tmp/mclone-texture-runtime-compat.png` was inspected; it rendered nonblank
  terrain with the generated dirt/grass overrides visible on the hillside and
  ground.
- Re-ran typecheck, named overlay export, timedemo, and screenshot validation
  after moving source definitions into `packs/mclone-default`.
- Added `tools/minecraft_assets/overlay_pack.py` and
  `pnpm texture-lab:pack-overlay` for first-party-only `.pbp` generation.
- Added native runtime `MCLONE_ASSET_OVERLAY_PACK` support so packed overlays
  shadow later loose/reference sources.
- Verified `/tmp/mclone-texture-lab/mclone-default-overlay.pbp` directly and
  inspected `/tmp/mclone-texture-overlay-pack.png`; packed-overlay rendering
  matches the loose runtime-pack bridge.
- Added `terrain_texture_coverage` and `pnpm texture-lab:coverage`; the first
  generated report showed 4 of 87 native terrain atlas textures covered by the
  overlay pack, with `grass_block_bottom.png` unused by the current vanilla
  model path.

Validation:

```sh
pnpm texture-lab:typecheck
node -e "JSON.parse(require('fs').readFileSync('package.json','utf8')); JSON.parse(require('fs').readFileSync('tools/texture-lab/package.json','utf8'))"
pnpm texture-lab:runtime-compat
pnpm texture-lab:pack-overlay
MCLONE_FIRST_PARTY_ASSET_ROOT=/tmp/mclone-texture-lab/runtime-pack pnpm native:timedemo:smoke
MCLONE_FIRST_PARTY_ASSET_ROOT=/tmp/mclone-texture-lab/runtime-pack cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-texture-runtime-compat.png --width 1280 --height 720 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
MCLONE_ASSET_OVERLAY_PACK=/tmp/mclone-texture-lab/mclone-default-overlay.pbp pnpm native:timedemo:smoke
MCLONE_ASSET_OVERLAY_PACK=/tmp/mclone-texture-lab/mclone-default-overlay.pbp cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-texture-overlay-pack.png --width 1280 --height 720 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
pnpm texture-lab:coverage
```
