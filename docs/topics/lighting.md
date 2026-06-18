# Lighting Topic

Status: active living index.

This document is the current map for native Java 1.17.1-style lighting work.
The broad architecture reference remains [`../lighting.md`](../lighting.md);
the first concrete native slice is
[`../tactical/026-lighting-pipeline.md`](../tactical/026-lighting-pipeline.md).
This topic records where the subsystem stands now, how the next slices should
compose, and which Java/native boundaries to preserve.

Update this file whenever a lighting tactical lands, a validation result changes
the next recommendation, or the implementation shape needs correction.

## Current Baseline

The first storage/render handoff has landed. Light is now a real chunk fact
that can cross the native server/client/render boundary, but the actual Java
solver and status scheduling are still pending.

Landed pieces:

- `mclone_light` has Java-shaped `DataLayer`, `LightLayer`, packed-light
  helpers, and tests for nibble/index/constant parity.
- `ChunkSnapshot` carries `light_correct` plus optional sky/block
  `PackedLightSection` bytes.
- Native protocol and filesystem persistence roundtrip light payloads.
- The server publishes provisional light payloads during generated chunk
  publication and keeps `light_correct=false`.
- The provisional producer seeds sky light and lava block light, then flood-fills
  through transparent cells across the available 3x3 chunk neighborhood.
- Textured mesh vertices carry Java-packed light.
- Textured meshing samples the face-adjacent block for flat solid faces, matching
  the shape of `ModelBlockRenderer.tesselateWithoutAO`.
- The chunk shader decodes packed sky/block values and applies a simple
  brightness factor, with a native/headless fullbright toggle.

Known gaps:

- `ChunkStatus::Light` exists, but the native server does not run a real light
  status.
- There is no `DynamicGraphMinFixedPoint` port yet.
- There is no Java-shaped `DataLayerStorageMap`, `LayerLightSectionStorage`,
  `BlockLightSectionStorage`, `SkyLightSectionStorage`, `LayerLightEngine`,
  `BlockLightEngine`, `SkyLightEngine`, or `LevelLightEngine` port yet.
- Light sections are attached to chunk snapshots, but native does not yet model
  Java's padded light-section lifecycle as solver-owned storage.
- Provisional opacity is coarse (`material_blocks_motion`) and does not use
  full `BlockState.getLightBlock(...)`, emission tables, or face shape
  occlusion.
- Block emission is lava-only.
- Live block changes do not call `checkBlock`, do not publish light deltas, and
  do not dirty render sections by changed light sections.
- Rendering does not yet use Java's 16x16 `LightTexture`.
- Solid block ambient occlusion and packed-light blending from
  `ModelBlockRenderer.AmbientOcclusionFace` are not ported.
- Liquid light sampling is not ported.

## Latest Visual Probe

On 2026-06-18, a native headless comparison for seed `12345`, chunk `(0,0)`,
render distance `2`, `960x640` produced:

```text
/tmp/mclone-light-current.png
/tmp/mclone-light-fullbright.png
/tmp/mclone-light-diff.png
```

The current-light and forced-fullbright captures are not identical, but the
difference is small:

| Metric | Value |
|---|---:|
| differing pixels | `9,105 / 614,400` |
| differing pixels percent | `1.48%` |
| current mean brightness | `0.307135` |
| fullbright mean brightness | `0.308349` |

Interpretation: the render handoff works and occluded/cave pixels darken, but
most visible open terrain still reads close to fullbright. That is expected
while stored light is provisional and while the renderer lacks Java `LightTexture`
and model AO.

Validation run from the same check:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -p mclone-mesh
```

Result: pass.

## Java Reference Anchors

Read the relevant Java source before porting each piece:

| Concern | Java source |
|---|---|
| 4-bit storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/DataLayer.java` |
| layer enum | `reference/minecraft-1.17.1/src/net/minecraft/world/level/LightLayer.java` |
| storage map | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DataLayerStorageMap.java` |
| shared storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightSectionStorage.java` |
| block storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightSectionStorage.java` |
| sky storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightSectionStorage.java` |
| fixed-point graph | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DynamicGraphMinFixedPoint.java` |
| shared engine | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightEngine.java` |
| block engine | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightEngine.java` |
| sky engine | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightEngine.java` |
| coordinator | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java` |
| threaded wrapper | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java` |
| light status | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java` |
| status scheduling | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`, `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java` |
| client packed light | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java` |
| lightmap | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LightTexture.java` |
| solid block lighting/AO | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/ModelBlockRenderer.java` |
| liquid lighting | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/LiquidBlockRenderer.java` |

## Native Code Map

Current native entry points:

| Concern | Native path |
|---|---|
| light data helpers | `native/crates/mclone-light/src/lib.rs` |
| snapshot payload | `native/crates/mclone-core/src/chunk.rs` |
| provisional producer | `native/crates/mclone-server/src/lighting_seed.rs` |
| scheduler publication | `native/crates/mclone-server/src/scheduler.rs` |
| worldgen mailbox precompute | `native/crates/mclone-server/src/worldgen_mailbox.rs` |
| protocol roundtrip | `native/crates/mclone-protocol/src/lib.rs` |
| persistence roundtrip | `native/crates/mclone-server/src/persistence.rs` |
| mesh light sampling | `native/crates/mclone-mesh/src/builder.rs` |
| textured vertex payload | `native/crates/mclone-mesh/src/data.rs` |
| shader consumption | `native/crates/mclone-render/src/shaders/chunk_textured.wgsl` |
| fullbright toggle | `native/crates/mclone-render/src/chunk.rs`, `native/apps/mclone-native-client/src/cli.rs` |

## Module Shape Policy

Lighting should follow the reference Java separation closely. Do not grow
`mclone_light/src/lib.rs` or `mclone-server/src/lighting_seed.rs` into large
catch-all modules.

Preferred native shape as the real solver lands:

| Java class | Preferred native home |
|---|---|
| `DataLayer` | `mclone-light/src/data_layer.rs` |
| `LightLayer`, packed constants/helpers | `mclone-light/src/layer.rs`, `mclone-light/src/packed.rs` |
| section/block position helpers | `mclone-light/src/pos.rs` or shared core helpers when already generic |
| `DataLayerStorageMap` | `mclone-light/src/storage_map.rs` |
| `LayerLightSectionStorage` | `mclone-light/src/section_storage.rs` |
| `BlockLightSectionStorage` | `mclone-light/src/block_storage.rs` |
| `SkyLightSectionStorage` | `mclone-light/src/sky_storage.rs` |
| `DynamicGraphMinFixedPoint` | `mclone-light/src/dynamic_graph.rs` |
| `LayerLightEngine` | `mclone-light/src/layer_engine.rs` |
| `BlockLightEngine` | `mclone-light/src/block_engine.rs` |
| `SkyLightEngine` | `mclone-light/src/sky_engine.rs` |
| `LevelLightEngine` | `mclone-light/src/level_engine.rs` |
| `ThreadedLevelLightEngine`-like scheduling | `mclone-server`, as a scheduler/mailbox wrapper around `mclone_light` |
| render `LightTexture` | `mclone-render`, consuming packed light/lightmap inputs only |
| `ModelBlockRenderer` AO sampling | `mclone-mesh`, as renderer-facing mesh vertex/light data, not solver state |

Rules:

- Port parity-critical data and solver code directly, with the same field and
  method names where practical.
- Keep runtime scheduling in `mclone-server`; keep propagation/data ownership in
  `mclone_light`.
- Keep `mclone-mesh` and `mclone-render` consumers of stored light, not owners of
  light propagation.
- Keep provisional lighting isolated and removable. It should not become the
  permanent home for solver logic.
- Split modules before they become difficult to review. A direct Java class
  counterpart is enough reason to create a small Rust module.

## Recommended Next Slices

### P0: Solver Storage Foundation

Next implementation slice should start the real Java solver foundation without
trying to wire full chunk status scheduling in the same change.

Scope:

- Split `mclone_light` into small Java-shaped modules.
- Move existing `DataLayer`, `LightLayer`, and packed helpers behind those
  modules without changing behavior.
- Port `DataLayerStorageMap`.
- Port the non-scheduling parts of `LayerLightSectionStorage`.
- Add focused tests for queued sections, visible/updating map separation,
  copy-on-write data layers, changed sections, and padded light-section ranges.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light
cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-mesh
git diff --check
```

### P1: Fixed-Point Graph And Block Light

Port `DynamicGraphMinFixedPoint`, then `BlockLightEngine` enough to run
controlled synthetic block-light fixtures.

Scope:

- Internal inverted levels: `0` full light, `15` dark.
- Six-direction propagation.
- Emission source node behavior.
- Opacity and face-occlusion hooks, with any temporary coarse opacity explicitly
  marked as a parity gap.
- Torch/lava placement and removal repair behavior.
- Cross-chunk source near boundary fixture.

### P2: Sky Light Storage And Engine

Port `SkyLightSectionStorage` and `SkyLightEngine`.

Scope:

- Top-section/source-column bookkeeping.
- Missing data above stored sky data reads as sky-lit where Java does.
- Direct-down transparent sky can remain level `15`.
- Sideways/obstructed sky decays.
- Roofed cave, vertical shaft, overhang, and chunk-boundary side-spread fixtures.

### P3: LevelLightEngine And ChunkStatus::Light

Wire the two layer engines behind `LevelLightEngine`, then make
`ChunkStatus::Light` real.

Scope:

- Java-shaped light section padding: `minSection - 1` through `maxSection + 1`.
- `updateSectionStatus`, `enableLightSources`, `queueSectionData`,
  `onBlockEmissionIncrease`, `checkBlock`, and `runUpdates`.
- Scheduler-owned light work after `FEATURES` with Java `ChunkStatus.LIGHT`
  dependency radius.
- Publish chunks as `light_correct=true` only after real light status completes.

### P4: Live Deltas And Render Dirtying

Connect runtime mutations to the light engine.

Scope:

- Block changes call `checkBlock` when opacity, emission, or light-occlusion
  shape changes.
- Light deltas are revisioned and published to clients.
- Changed light sections dirty render sections and neighbor sections as needed.
- Section block deltas and light deltas are batched when practical.

### P5: Rendering Parity

Improve visual parity after stored light is correct.

Scope:

- Port Java `LightTexture` 16x16 lightmap behavior into `mclone-render`.
- Replace the simple shader `max(block, sky) / 15` factor with lightmap sampling.
- Port `ModelBlockRenderer.AmbientOcclusionFace` sampling/blending into
  `mclone-mesh`.
- Port liquid light sampling from `LiquidBlockRenderer`.

## Tactical Index

Primary native lighting docs:

- [`../lighting.md`](../lighting.md)
- [`../tactical/026-lighting-pipeline.md`](../tactical/026-lighting-pipeline.md)
- [`../tactical/031-native-section-block-delta-updates.md`](../tactical/031-native-section-block-delta-updates.md)
- [`../tactical/036-native-sky-and-day-night-cycle.md`](../tactical/036-native-sky-and-day-night-cycle.md)

Legacy/reference-only lighting docs:

- [`../tactical/legacy/L0-lighting-oracle-foundation.md`](../tactical/legacy/L0-lighting-oracle-foundation.md)
- [`../tactical/legacy/L1-light-data-foundation.md`](../tactical/legacy/L1-light-data-foundation.md)
- [`../tactical/legacy/L2-light-solver-foundation.md`](../tactical/legacy/L2-light-solver-foundation.md)
- [`../tactical/legacy/L3-initial-chunk-lighting.md`](../tactical/legacy/L3-initial-chunk-lighting.md)
- [`../tactical/legacy/L4-light-snapshot-consumption.md`](../tactical/legacy/L4-light-snapshot-consumption.md)
- [`../tactical/legacy/L5-live-light-deltas.md`](../tactical/legacy/L5-live-light-deltas.md)
- [`../tactical/legacy/L6-lighting-scheduler-and-status-integration.md`](../tactical/legacy/L6-lighting-scheduler-and-status-integration.md)

Treat legacy docs as prior art only. New implementation work belongs in the
native Rust crates unless an explicit legacy request says otherwise.

## Validation Lanes

Fast focused gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -p mclone-mesh
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Native movement/render smoke:

```bash
pnpm --silent native:movement:smoke
pnpm --silent native:timedemo:smoke
```

Pixel-affecting lighting work must capture and inspect a native screenshot in
`/tmp` before the slice is considered complete. Keep a forced-fullbright
comparison around when changing render consumption:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --headless-chunk /tmp/mclone-light-current.png \
  --width 960 --height 640 --seed 12345 --chunk-x 0 --chunk-z 0 \
  --render-distance 2 --disable-fullbright

cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --headless-chunk /tmp/mclone-light-fullbright.png \
  --width 960 --height 640 --seed 12345 --chunk-x 0 --chunk-z 0 \
  --render-distance 2 --force-fullbright
```

## Update Policy

- Keep the current recommendation in this file; keep detailed implementation
  notes in tacticals.
- When a solver/status/rendering slice lands, update Current Baseline, Known
  gaps, Native Code Map, and Recommended Next Slices.
- If a native module starts collecting multiple Java-class responsibilities,
  split it before extending the subsystem further.
