# 042: Native Light Status Scheduling

Status: completed first pass.

## Purpose

Build on
[`041-native-level-light-engine-bridge.md`](041-native-level-light-engine-bridge.md)
by making native `ChunkStatus::Light` a real scheduler status for generated
runtime chunks.

This slice moves initial light completion out of the worldgen mailbox. Feature
generation still produces block facts, but client-visible snapshots are now
published only after a scheduler-owned light status completes.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java`

Important reference shape:

- `ThreadedLevelLightEngine.lightChunk(...)` marks the chunk not light-correct
  before light work.
- Non-empty chunk sections call `updateSectionStatus(..., false)`.
- Light sources are enabled for the chunk.
- New chunks scan emitting blocks and call `onBlockEmissionIncrease(...)`.
- Propagation drains before the post-update marks the chunk light-correct.

## Scope

Land the first native status boundary:

- make runtime/client-visible chunks target `ChunkStatus::Light`
- keep `Terrain`, `Surface`, and `Features` readiness in holder status slots
- add a scheduler-owned pending light status queue
- move completed-light computation out of `worldgen_mailbox.rs`
- publish generated feature snapshots to holders without light payloads
- run initial light as the next scheduler step using the existing
  `LevelLightEngine` bridge
- publish client `SnapshotReady` events only for `ChunkStatus::Light` snapshots
  with `light_correct=true`
- keep stored chunks valid only when persisted at `ChunkStatus::Light` or later

## Out Of Scope

- a fully threaded/native worker `ThreadedLevelLightEngine` equivalent
- async/budgeted light propagation independent from scheduler polling
- `queueSectionData`, `retainData`, and loaded light data hydration
- live block light/sky light deltas
- Java face-shape occlusion
- full vanilla `BlockState.getLightBlock(...)` and emission tables
- render `LightTexture`
- ambient occlusion

## Result

Landed:

- `mclone-server/src/light_status.rs` owns pending initial light inputs.
- `ChunkScheduler` targets `ChunkStatus::Light` for runtime chunks.
- `WorldgenMailbox` no longer precomputes completed light sections.
- Feature publication marks `Features` ready, then queues light work.
- Light publication creates `ChunkStatus::Light` snapshots with
  `light_correct=true`.
- Client-visible chunks are published only once the light snapshot is ready.
- Scheduler tests now assert `Light` scheduling, `Light` readiness, and
  light-correct client snapshots.

Remaining parity gaps: light work is still synchronous inside scheduler polling,
the solver still uses provisional opacity/emission facts, and loaded light
section hydration is not yet modeled through Java `queueSectionData` /
`retainData`.

## Validation

Completed on 2026-06-18:

- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo test --manifest-path native/Cargo.toml -p mclone-light`
- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh`
- `cargo fmt --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -- --check`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `git diff --check`
- `pnpm --silent native:timedemo:smoke`
- native headless capture:
  `/tmp/mclone-light-status-scheduler.png`

The capture was inspected and rendered a nonblank, correctly framed terrain
cube with visible blocks. It did not reproduce the blue-sky-only failure.
The timedemo smoke also exercised the launch-like native runtime path and
reported nonzero rendered sections, vertices, and indices.

## Next

The next likely lighting slice is loaded-light hydration and retained data:

- add the Java-shaped `queueSectionData` / `retainData` hooks needed for loaded
  chunk light payloads
- keep persisted `Light` snapshots from recomputing when stored light data is
  valid
- preserve the scheduler-owned light status boundary landed here

Live block-change deltas should follow after loaded light data has a proper
solver-owned lifecycle.
