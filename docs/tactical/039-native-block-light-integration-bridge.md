# 039: Native Block Light Integration Bridge

Status: completed first pass.

## Purpose

Build on
[`038-native-light-graph-and-block-engine.md`](038-native-light-graph-and-block-engine.md)
by routing runtime block-light publication through the new graph-driven
`BlockLightEngine` instead of the provisional per-chunk block flood fill.

This slice keeps sky light provisional and keeps `ChunkStatus::Light` out of
scope, but makes block light use the Java-shaped graph/storage foundation for
loaded chunk data.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightEngine.java`

## Scope

Land a first runtime bridge:

- add `mclone-light/src/block_storage.rs` as the native
  `BlockLightSectionStorage` counterpart
- make `BlockLightEngine` own block-specific storage rather than generic layer
  storage directly
- add a server-side raw chunk adapter that implements `BlockLightWorld`
- seed all emitting blocks from loaded target/dependency chunks into
  `BlockLightEngine`
- activate loaded chunk sections and emit target chunk block light sections from
  solver storage
- route the block-light half of generated chunk publication through the graph
  bridge while leaving provisional sky light unchanged
- centralize current generated-block light facts in `mclone-worldgen`

## Out Of Scope

- `SkyLightSectionStorage` and `SkyLightEngine`
- full `LayerLightEngine` chunk cache shape
- `LevelLightEngine`
- real `ChunkStatus::Light`
- live block light deltas
- Java face-shape occlusion
- full vanilla `BlockState.getLightBlock(...)` tables
- render `LightTexture`
- ambient occlusion

## Result

Landed:

- `BlockLightSectionStorage` wraps `LayerLightSectionStorage` and provides the
  Java block-light missing-section value of `0`.
- `BlockLightEngine` now owns `BlockLightSectionStorage`, keeping the block
  layer boundary separate from generic layer storage.
- `mclone-server/src/block_light_bridge.rs` adapts loaded raw generated block
  chunks into `BlockLightWorld` and runs the fixed-point graph to produce
  block-light `PackedLightSection`s for the target chunk.
- `lighting_seed.rs` now uses the graph bridge for block light; only sky light
  remains provisional in that module.
- `mclone-worldgen::block` now centralizes current block-light emission and
  opacity facts for generated block IDs. Emission covers lava, magma block, and
  glow lichen. Opacity still follows the current motion-blocking approximation
  until the fuller block-state/shape table lands.
- Existing synthetic Java block-light fixtures still match exactly through the
  new graph path, including the cross-chunk case.

## Validation

Fast focused gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light
cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-worldgen
cargo test --manifest-path native/Cargo.toml -p mclone-mesh
cargo fmt --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -p mclone-worldgen -- --check
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Because generated block light now reaches rendered chunk snapshots through the
graph path, capture a native headless frame before considering the slice done.

Completed on 2026-06-18:

- `cargo test --manifest-path native/Cargo.toml -p mclone-light`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-worldgen`
- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh`
- `cargo fmt --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -p mclone-worldgen -- --check`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `git diff --check`
- native headless capture:
  `/tmp/mclone-light-graph-block-bridge.png`
