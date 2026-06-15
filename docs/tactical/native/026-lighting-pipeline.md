# 026: Lighting Pipeline

Status: active. The first storage/render handoff has landed, followed by a provisional chunk-local sky propagation pass. The full Java solver, light status scheduling, live deltas, and lightmap parity are still pending.

## Purpose

Introduce native lighting in the Java-shaped direction without trying to land the full solver, live light deltas, ambient occlusion, and shader/lightmap parity in one slice.

The first implementation should make light a real chunk fact owned by the runtime, carried through snapshots/protocol, consumed by meshing, and visible in native/headless rendering. It should not become a renderer-only brightness hack.

Keep [`../../lighting.md`](../../lighting.md) as the broad architecture reference. This tactical is the first concrete native slice.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/DataLayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/LightLayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DataLayerStorageMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LightTexture.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/ModelBlockRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/LiquidBlockRenderer.java`

Reference facts from the initial pass:

- `DataLayer` stores one 16x16x16 light section as 4096 nibbles in 2048 bytes. Indexing is `y << 8 | z << 4 | x`; byte index is `index >> 1`; nibble index is `index & 1`.
- `DataLayer` is lazy. Missing data reads as zero until `set(...)` or `getData()` allocates the backing bytes.
- Java light sections are block sections plus vertical padding: `minLightSection = minSection - 1`, `lightSectionCount = worldSectionCount + 2`.
- `LevelLightEngine` owns independent block and sky engines and combines raw brightness as `max(block, sky - skyDarken)`.
- `LayerLightSectionStorage` has separate updating and visible maps. Propagation mutates updating data, then `swapSectionMap()` publishes a copied visible map and notifies affected sections.
- `BlockLightSectionStorage` returns zero when no section data exists.
- `SkyLightSectionStorage` tracks top sections and sky-source columns. Missing section data above stored sky data can read as 15.
- `LayerLightEngine` uses inverted solver levels: internal `0` is full light, internal `15` is dark, stored value is `15 - internal`.
- `BlockLightEngine` uses block emission, opacity, and face occlusion shapes; air attenuates by one level.
- `SkyLightEngine` has the direct-down special case: vertical transparent sky can stay level 15, while sideways or obstructed propagation decays.
- `LevelRenderer.getLightColor(...)` returns fullbright for emissive rendering; otherwise it packs `sky << 20 | max(block, emission) << 4`.
- `LightTexture` turns packed block/sky light coordinates into final color through a 16x16 lightmap with sky darken, flicker, effects, gamma, and dimension brightness.
- `ModelBlockRenderer` keeps packed light separate from AO shade brightness. Flat rendering usually samples the adjacent face block; AO blends four packed light samples and four brightness samples.
- `LiquidBlockRenderer` has its own light sampling path and should not be accidentally covered by solid-block lighting work.

## Current Native State

- `native/crates/mclone-light` has Java-shaped `DataLayer`, `LightLayer`, and packed-light helpers.
- `ChunkSnapshot` carries provisional light payloads with optional sky/block `DataLayer` bytes per section.
- Protocol and filesystem persistence roundtrip the light payload.
- `ChunkStatus::Light` exists in `mclone_core`, but the server path does not yet run a native light status.
- The server currently publishes provisional sky facts from generated/live chunk blocks and leaves `light_correct=false`.
- The provisional sky pass seeds open-sky cells at 15 and does chunk-local propagation through transparent cells, including the Java-style direct-down exception where vertical transparent sky can remain level 15.
- There is still no cross-chunk light storage, async fixed-point solver, block emission propagation, or live light-delta publication.
- `TexturedChunkVertex` carries Java-packed light, and `chunk_textured.wgsl` decodes it into a simple brightness factor.
- The native client/headless path has a fullbright render toggle for before/after captures.

## First Slice

Implement the light storage and renderer handoff, with a deliberately small provisional producer.

### 1. Data Types In `mclone_light`

Port the small data-level pieces directly:

- `LightLayer` enum: `Sky`, `Block`
- `DataLayer`
- light-section coordinate helpers
- packed-light helpers equivalent to `LightTexture.pack`, `block`, `sky`, plus constants:
  - `FULL_BRIGHT = 15728880`
  - `FULL_SKY = 15728640`
  - `FULL_BLOCK = 240`

`DataLayer` should preserve Java semantics:

- lazy allocation
- exact index order
- exact nibble order
- `copy`
- `is_empty`
- explicit `get_data` allocation only when the caller needs bytes

Unit tests should cover Java-visible edge cases: empty reads as zero, first/second nibble byte packing, high coordinates, copy-on-write behavior, and the exact packed-light constants.

### 2. Snapshot And Protocol Shape

Add a light payload shape that can survive the client/server boundary.

Preferred logical shape:

```text
ChunkSnapshot
  light_correct: bool
  light_sections:
    section_y
    sky: optional 2048-byte DataLayer
    block: optional 2048-byte DataLayer
```

The exact Rust placement can be either `mclone_core` or a small core-owned byte payload with conversions in `mclone_light`. Avoid making `mclone_core` depend upward on `mclone_light`.

Requirements:

- protocol roundtrip tests for snapshots with no light, sky-only light, block-only light, and both layers
- persistence either stores the new fields or explicitly writes empty/default light fields with a version note
- old fullbright rendering remains possible when `light_correct=false` or no light data exists

### 3. Provisional Initial Light Facts

Do not port `DynamicGraphMinFixedPoint` in this first slice.

Add a clearly named provisional builder owned by the server/runtime path, not the renderer. Its job is to generate initial light facts good enough to exercise the storage and render handoff:

- block light can start as zero everywhere unless an emission table is already available
- sky light can start as direct top-down column light: `15` from the top until an opaque block, then `0` below
- mark the output as provisional or keep `light_correct=false` until the real solver exists

This is intentionally not vanilla-correct lighting. It exists to validate the data path, vertex layout, shader path, dirty-section interactions, and visual inspection loop before the solver port.

If this provisional path starts hiding real solver needs, stop and narrow it. The final target is still the Java solver.

Implementation note: the provisional producer has since been extended from direct-only columns to chunk-local sky propagation. It is still provisional because it does not use `SkyLightSectionStorage`, cross-chunk top-section/source-column data, `DynamicGraphMinFixedPoint`, or Java block shape opacity.

### 4. Mesh Consumption

Introduce a light input to textured section meshing.

Recommended shape:

```text
TexturedChunkMeshInput
  blocks
  optional light view / sampler
```

For the first pass:

- preserve the existing tint and face shade color
- add a packed light or compact sky/block field to `TexturedChunkVertex`
- for flat solid faces, sample the face-adjacent position in the same spirit as `ModelBlockRenderer.tesselateWithoutAO`
- if no light view is present, emit `FULL_BRIGHT`

Do not implement ambient occlusion in this slice. AO is a separate `ModelBlockRenderer.AmbientOcclusionFace` port after the storage/render contract is stable.

### 5. Render Consumption

Add a non-fullbright shader path while preserving a toggle.

Acceptable first shader path:

- decode per-vertex sky/block levels in WGSL and apply a simple brightness factor
- or upload a 16x16 lightmap texture and sample it with packed block/sky coordinates

The lightmap route is closer to Java, but it is acceptable to start with a simpler shader if the vertex payload preserves Java packed-light semantics and the tactical records that `LightTexture` parity remains pending.

Add client/headless options:

- default: use available light data
- fallback/toggle: force fullbright for before/after captures and quick regression isolation

### 6. Diagnostics

Add counters before performance tuning:

- chunks with light payloads
- light sections carried per chunk
- sky/block bytes carried
- render sections meshed with light data versus fullbright fallback
- vertex layout byte size before/after
- headless capture path and file name in `/tmp`

Record a smoke baseline in [`../../performance-records.md`](../../performance-records.md) once the first non-fullbright capture lands.

## Follow-Up Slices

### Real Solver

Port the Java solver after the first data/render handoff lands:

- `DynamicGraphMinFixedPoint`
- `LayerLightSectionStorage`
- `DataLayerStorageMap`
- `BlockLightSectionStorage`
- `SkyLightSectionStorage`
- `LayerLightEngine`
- `BlockLightEngine`
- `SkyLightEngine`
- `LevelLightEngine`

Validation should use oracle fixtures for small synthetic worlds: empty column, roofed cave, vertical shaft, torch placement/removal, chunk-boundary torch, and sky side-spread around an overhang.

### Status Scheduling

Wire `ChunkStatus::Light` as a real status after the solver exists:

- dependency radius follows Java `ChunkStatus.LIGHT`
- scheduler keeps light work off the frame path
- worker/threading budget is explicit for native and web
- light-correct chunks publish after light status completes

### Live Updates

Defer to `027-live-light-and-liquid-updates.md`:

- block mutations call `checkBlock`
- light deltas publish to clients
- render-section dirtying follows changed light sections
- liquid updates exercise block/light dirtiness together

### Rendering Parity

Later renderer follow-ups:

- Java-shaped `LightTexture` 16x16 lightmap
- sky darken / gamma / dimension brightness
- `ModelBlockRenderer` AO brightness and lightmap blending
- liquid light sampling
- emissive rendering/fullbright exceptions

## Validation Gates

Minimum first-slice gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light -p mclone-core -p mclone-protocol -p mclone-mesh -p mclone-render -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
pnpm --silent native:movement:smoke
pnpm --silent native:timedemo:smoke
```

If rendered pixels change, capture and inspect:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --headless-chunk /tmp/mclone-lighting-first-slice.png --width 640 --height 480
```

The image should not be blank, over-dark, or still indistinguishable from forced fullbright.

## Out Of Scope For First Slice

- full `DynamicGraphMinFixedPoint` propagation
- exact `ThreadedLevelLightEngine` task batching
- `ClientboundLightUpdatePacket` mask parity
- live block/liquid light deltas
- ambient occlusion
- liquid renderer light parity
- persisted trusted light data from disk
- night/gamma/effect-correct `LightTexture`
- GPU compute lighting
