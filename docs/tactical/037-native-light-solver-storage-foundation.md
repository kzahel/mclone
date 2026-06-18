# 037: Native Light Solver Storage Foundation

Status: completed first pass.

## Purpose

Start the real native lighting solver foundation without wiring the whole
`ChunkStatus::Light` path or replacing the provisional snapshot producer in the
same slice.

This slice should make `mclone_light` look like the Java lighting package at
the storage boundary, so later `DynamicGraphMinFixedPoint`, `BlockLightEngine`,
`SkyLightEngine`, and `LevelLightEngine` ports have somewhere small and
recognizable to land.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/core/BlockPos.java`
- `reference/minecraft-1.17.1/src/net/minecraft/core/SectionPos.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/DataLayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DataLayerStorageMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/SectionTracker.java`

## Scope

Land the storage foundation only:

- split `mclone_light/src/lib.rs` into Java-shaped modules
- preserve the existing public `DataLayer`, `LightLayer`, and packed-light API
- add Java-compatible packed block/section position helpers needed by lighting
- port `DataLayerStorageMap`
- add block and sky storage-map variants
- add the non-scheduling part of `LayerLightSectionStorage`
- keep graph scheduling hooks explicit and small so `DynamicGraphMinFixedPoint`
  can attach in the next slice
- add focused unit tests for map copy/cache behavior, visible/updating map
  separation, queued section precedence, changed-section copy-on-write, affected
  section marking, section status lifecycle, sky top-section bookkeeping, and
  light-section padding

## Out Of Scope

- `DynamicGraphMinFixedPoint`
- `LayerLightEngine`
- `BlockLightEngine`
- `SkyLightEngine`
- `LevelLightEngine`
- `ThreadedLevelLightEngine` scheduling
- replacing provisional chunk lighting
- live light deltas
- render `LightTexture`
- ambient occlusion

## Implementation Notes

Keep names close to Java where practical. Prefer small modules over a large
crate root:

```text
mclone-light/src/data_layer.rs
mclone-light/src/layer.rs
mclone-light/src/packed.rs
mclone-light/src/pos.rs
mclone-light/src/storage_map.rs
mclone-light/src/section_storage.rs
```

`LayerLightSectionStorage` is coupled to Java's `SectionTracker`, which extends
`DynamicGraphMinFixedPoint`. This slice should not port the graph yet. Instead,
keep the storage-side methods that the graph will call (`get_level`,
`get_level_from_source`, and applying a graph-computed section level) as explicit
Rust methods with focused tests. The next slice can wire those methods into the
actual graph port.

## Validation

Fast focused gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light
cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-mesh
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

This slice should not affect rendered pixels. No screenshot is required unless
the mesh/render handoff changes unexpectedly.

## Completion Notes

Landed:

- split `mclone_light` into Java-shaped modules:
  - `data_layer`
  - `key`
  - `layer`
  - `packed`
  - `pos`
  - `section_storage`
  - `storage_map`
- preserved the public `DataLayer`, `LightLayer`, packed-light, and
  `packed_light_section_layer(...)` API used by server/mesh/render code
- added Java-compatible packed `BlockPos` and `SectionPos` helpers for the
  lighting solver
- added `LightSectionRange` for Java's one-section light padding above and
  below block sections
- ported `DataLayerStorageMap` behavior needed by the solver foundation,
  including copy-on-write layer copying and the small recent-section cache shape
- added `BlockDataLayerStorageMap` and `SkyDataLayerStorageMap` foundations
- added `LayerLightSectionStorage` storage/lifecycle state without graph
  scheduling:
  - updating vs. visible section maps
  - queued section data
  - trusted/untrusted queue tracking
  - section data/no-data lifecycle flags
  - graph-facing section-level methods
  - changed-section copy-on-write
  - affected-section tracking
  - retained queued data on section removal

Validation run:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light
cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-mesh
cargo fmt --manifest-path native/Cargo.toml -p mclone-light -- --check
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

All passed.

Workspace-wide `cargo fmt --manifest-path native/Cargo.toml --all -- --check`
was not used as an acceptance gate for this slice because it reports unrelated
pre-existing formatting diffs in native client/render files outside this work.

Next likely slice: port `DynamicGraphMinFixedPoint`, then use it to drive the
existing `LayerLightSectionStorage` graph-facing methods and the first
`BlockLightEngine` synthetic fixtures.
