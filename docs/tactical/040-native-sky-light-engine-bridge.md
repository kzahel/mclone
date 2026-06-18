# 040: Native Sky Light Engine Bridge

Status: completed first pass.

## Purpose

Build on
[`039-native-block-light-integration-bridge.md`](039-native-block-light-integration-bridge.md)
by adding Java-shaped sky-light storage/engine modules and routing initial
generated chunk sky light through the fixed-point graph path.

This slice removes the old provisional sky flood fill from `lighting_seed.rs`.
It does not make `ChunkStatus::Light` real yet.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightEngine.java`

## Scope

Land a first graph-backed sky path:

- add `mclone-light/src/sky_storage.rs` as the native
  `SkyLightSectionStorage` counterpart
- add `mclone-light/src/sky_engine.rs` with Java-style inverted levels,
  top-source seeding, direct-down transparent sky propagation, and side-spread
  decay
- add a server-side raw chunk adapter implementing `SkyLightWorld`
- activate loaded target/dependency chunk sections and seed top transparent
  cells as sky sources
- produce target chunk sky `PackedLightSection`s from solver storage
- keep existing synthetic Java sky fixtures passing through the graph path
- remove the duplicate provisional sky flood-fill from `lighting_seed.rs`

## Out Of Scope

- full `LevelLightEngine`
- real `ChunkStatus::Light`
- live block light/sky light deltas
- Java face-shape occlusion
- full vanilla `BlockState.getLightBlock(...)` tables
- render `LightTexture`
- ambient occlusion

## Result

Landed:

- `SkyLightSectionStorage` wraps `LayerLightSectionStorage`, tracks enabled
  source columns/top sections, and implements Java-style sky reads above stored
  data.
- `SkyLightEngine` runs through `DynamicGraphMinFixedPoint` and preserves the
  Java internal-level convention.
- `mclone-server/src/sky_light_bridge.rs` adapts loaded generated block chunks
  into `SkyLightWorld` and emits sky light sections for the target chunk.
- `lighting_seed.rs` now only orchestrates/merges graph-backed sky and block
  light sections; the old provisional sky flood-fill was removed.
- Existing synthetic Java sky fixtures still match exactly through the new
  graph path, including roofed, overhang, and cross-chunk cases.

Remaining parity gaps: source-column updates are enough for initial generated
sections, but full Java `LevelLightEngine` scheduling, live section status
changes, face shape occlusion, and complete block-state opacity tables are still
pending.

## Validation

Fast focused gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-mesh
cargo fmt --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -- --check
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Because generated sky light now reaches rendered chunk snapshots through the
graph path, capture a native headless frame before considering the slice done.

Completed on 2026-06-18:

- `cargo test --manifest-path native/Cargo.toml -p mclone-light`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh`
- `cargo fmt --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -- --check`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `git diff --check`
- native headless capture:
  `/tmp/mclone-light-graph-sky-bridge.png`
