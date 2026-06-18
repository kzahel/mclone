# 043: Native Loaded Light Hydration

Status: completed first pass.

## Purpose

Build on
[`042-native-light-status-scheduling.md`](042-native-light-status-scheduling.md)
by routing persisted `ChunkStatus::Light` payloads through Java-shaped
`queueSectionData` / `retainData` hooks before loaded chunks are published.

This slice does not make the native light solver a long-lived per-world object.
It establishes the loaded-data API shape and validates that stored light bytes
can hydrate solver-owned section storage instead of remaining only opaque
snapshot payloads.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`

Important reference shape:

- `LevelLightEngine.queueSectionData(...)` delegates loaded light data to the
  layer engine for `LightLayer.BLOCK` or `LightLayer.SKY`.
- `retainData(...)` is a per-column retention switch used around section removal
  and loaded data lifecycle.
- `LayerLightSectionStorage.createDataLayer(...)` prefers queued section data
  when a section becomes stored.

## Scope

Land the first loaded-data path:

- expose `queue_section_data`, `retain_data`, and queued-data acceptance through
  `BlockLightEngine`, `SkyLightEngine`, and `LevelLightEngine`
- expose the same hooks through block/sky section storage wrappers
- add server hydration for loaded `ChunkStatus::Light` snapshots
- queue persisted sky/block `PackedLightSection` bytes into a `LevelLightEngine`
- activate the loaded chunk's light sections and accept queued data into visible
  solver storage
- repack hydrated section storage before publishing the loaded snapshot
- keep stored chunks valid only when they are already `ChunkStatus::Light` and
  `light_correct=true`

## Out Of Scope

- a long-lived per-world light engine owned by the server
- asynchronous loaded-light hydration
- loaded neighbor light data stitching across multiple stored chunks
- live block light/sky light deltas
- Java face-shape occlusion
- full vanilla `BlockState.getLightBlock(...)` and emission tables
- render `LightTexture`
- ambient occlusion

## Result

Landed:

- `mclone_light` exposes Java-shaped queue/retain hooks on the level and layer
  engine boundaries.
- `mclone-server/src/light_status.rs` hydrates loaded light snapshots through a
  temporary `LevelLightEngine` before scheduler publication.
- The scheduler's stored-snapshot path now calls hydration before marking a
  loaded `Light` snapshot ready.
- Focused tests cover queued section data through `LevelLightEngine` and
  persisted light-byte roundtripping through loaded snapshot hydration.

Remaining parity gaps: solver state is still reconstructed for initial/generated
and loaded chunk paths rather than retained as a long-lived world light engine.
Live block changes still do not call `checkBlock`, publish light deltas, or dirty
render sections by changed light sections.

## Validation

Completed on 2026-06-18:

- `cargo test --manifest-path native/Cargo.toml -p mclone-light`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh`
- `cargo fmt --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -- --check`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `git diff --check`
- `pnpm --silent native:timedemo:smoke`
- native headless capture:
  `/tmp/mclone-light-loaded-hydration.png`

The capture was inspected and rendered a nonblank, correctly framed terrain
cube with visible blocks. The timedemo smoke also exercised the launch-like
native runtime path and reported nonzero rendered sections, vertices, and
indices.

## Next

The next likely lighting slice is live block-change light deltas:

- detect block opacity/emission changes from runtime mutations
- call `LevelLightEngine.checkBlock(...)` / `onBlockEmissionIncrease(...)`
- publish changed light sections to clients
- dirty render sections and affected neighbor sections when light changes
