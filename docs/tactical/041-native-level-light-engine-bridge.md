# 041: Native Level Light Engine Bridge

Status: completed first pass.

## Purpose

Build on
[`040-native-sky-light-engine-bridge.md`](040-native-sky-light-engine-bridge.md)
by adding the Java-shaped `LevelLightEngine` coordinator and routing initial
generated chunk lighting through one combined level-light bridge.

This slice keeps lighting at generated chunk publication time. It prepares the
native server for a real `ChunkStatus::Light` pass, but does not make that
status scheduled yet.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`

Important reference shape:

- `LevelLightEngine` owns separate block and sky engines.
- `checkBlock` delegates to both layers.
- `onBlockEmissionIncrease` delegates to block light.
- `runUpdates` splits the budget between block and sky light and gives unused
  sky budget back to block light when block light exhausted its first slice.
- `getRawBrightness` returns `max(block, sky - skyDarken)`.
- `ThreadedLevelLightEngine.lightChunk(...)` is the scheduling reference for
  the next slice: mark not light-correct, update non-empty section status,
  enable sources, scan emitters, run propagation, then mark light-correct.

## Scope

Land the coordinator boundary:

- add `mclone-light/src/level_engine.rs`
- expose `has_work`, `update_section_status`, and `enable_light_sources`
  boundaries on the layer engines
- preserve Java method names where practical on the coordinator
- add `LevelLightEngine::runUpdates`-style budget splitting
- add `LevelLightEngine::get_raw_brightness`
- add a server-side `level_light_bridge.rs` that adapts loaded raw chunk blocks
  into both `BlockLightWorld` and `SkyLightWorld`
- route `lighting_seed.rs` through the combined level-light bridge
- keep the older layer bridge modules as test-only coverage for block/sky
  solver behavior

## Out Of Scope

- real `ChunkStatus::Light` scheduling
- a `ThreadedLevelLightEngine`-like server wrapper
- `queueSectionData`, `retainData`, and full padded light-section lifecycle
- live block light/sky light deltas
- Java face-shape occlusion
- full vanilla `BlockState.getLightBlock(...)` and emission tables
- render `LightTexture`
- ambient occlusion

## Result

Landed:

- `LevelLightEngine` now coordinates `BlockLightEngine` and `SkyLightEngine`
  inside `mclone_light`.
- Layer engines expose the Java-shaped work/status hooks the coordinator needs.
- The server now has a combined `level_light_bridge.rs` that builds both sky
  and block `PackedLightSection` payloads from one raw chunk world adapter.
- Generated chunk publication still uses `lighting_seed.rs`, but that module is
  now a thin provisional wrapper over `LevelLightEngine`.
- The old block/sky bridge modules remain test-only and keep focused synthetic
  coverage for each layer.

Remaining parity gaps: the native server still publishes lit payloads as
`ChunkStatus::Features` with `light_correct=false`. A real light status needs a
server-owned job boundary that mirrors `ThreadedLevelLightEngine.lightChunk(...)`
and only publishes visible chunks after `ChunkStatus::Light` completes.

## Validation

Completed on 2026-06-18:

- `cargo test --manifest-path native/Cargo.toml -p mclone-light`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh`
- `cargo fmt --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -- --check`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `git diff --check`
- native headless capture:
  `/tmp/mclone-light-level-engine-bridge.png`

The capture was inspected and rendered a nonblank, correctly framed terrain
cube with no obvious missing light payload or section-boundary artifact.

## Next

Make `ChunkStatus::Light` real in the native scheduler.

That slice should use this coordinator as the propagation boundary, keep
scheduler/mailbox ownership in `mclone-server`, and follow
`ThreadedLevelLightEngine.lightChunk(...)` for initial chunk lighting order.
