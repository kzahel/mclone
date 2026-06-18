# 046: Native Retained Initial Light World

Status: completed first pass.

## Purpose

Build on
[`045-native-shared-initial-light-batch.md`](045-native-shared-initial-light-batch.md)
by moving native initial light computation from a temporary per-batch raw world
into a worker-owned retained light world.

This is an architectural parity step toward Java `ThreadedLevelLightEngine`.
It does not claim to solve the cold-start graph-drain cost by itself; the cold
radius-5 path is still one large `LevelLightEngine.run_all_updates` drain.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java`

Important reference shape:

- `ThreadedLevelLightEngine` owns the light engine and task queue.
- `ChunkMap` / `ChunkHolder` own chunk-status scheduling and publication.
- Light pre-update work sets section status, enables light sources, and queues
  block emitters before the graph drain.
- Post-update chunk completion stays separate from propagation ownership.

## Scope

Land the retained owner without growing `scheduler.rs`:

- add `mclone-server/src/light_world.rs`
- keep generated/dependency raw block facts in a retained world map
- keep a persistent `LevelLightEngine` inside the light worker
- insert or update only newly seen/changed batch chunks
- run section-status/source-enable/emitter pre-update work for newly retained
  chunks
- collect target chunk light sections after the shared graph drain
- keep `LightStatusMailbox` as the scheduler-facing async boundary
- keep `ChunkScheduler` publication and holder status behavior unchanged

## Out Of Scope

- Java-equivalent task priority queues
- cancellation of stale light work
- light ticket retain/release policy
- unloading/releasing retained light-world chunks
- loaded-neighbor light stitching beyond current persisted snapshot hydration
- live `checkBlock` / block-emission update queues
- light delta packets and render-section dirtying
- full vanilla opacity/emission tables and face-shape occlusion
- render `LightTexture` and ambient occlusion

## Result

Landed:

- `RetainedInitialLightState` owns the retained initial light world and
  persistent `LevelLightEngine`.
- Native and WASM mailbox backends compute batches through retained state.
- `PendingLightStatusBatch` is now only an input carrier; retained computation
  lives outside `light_status.rs`.
- The old temporary level-light bridge is test-only.
- Cold radius-5 startup remains roughly unchanged, confirming the remaining
  bottleneck is the graph drain itself.
- Multi-step movement shows retained-state reuse on later batches.

Remaining parity gaps: the retained owner still lacks Java task priority,
release, cancellation, and live-update policy. It also still uses current
coarse generated opacity/emission facts.

## Validation

Completed on 2026-06-19:

- `cargo fmt --manifest-path native/Cargo.toml -p mclone-server -- --check`
- `cargo check --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- native radius-5 lighting-enabled perf:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting`
- native radius-3 two-step lighting-enabled perf:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 3 --steps 2 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting`
- native full-frame screenshot:
  `/tmp/mclone-light-retained-world.png`

Screenshot result: `960x540`, `166` cached sections, `26` drawn sections,
nonblank terrain; visually inspected with no obvious missing chunks or
blue-sky-only failure.

Radius-5 perf result:

| Metric | Value |
|---|---:|
| total elapsed | `11,292.102 ms` |
| feature batch timing | `1,148.652 ms` |
| completed light statuses | `169` |
| completed light batches | `1` |
| total light-status compute | `9,341.649 ms` |
| `LevelLightEngine.run_all_updates` | `9,301.529 ms` |

Radius-3 two-step perf result:

| Step | Total step time | Loaded snapshots | Light batches | Cumulative light compute | Incremental light compute |
|---|---:|---:|---:|---:|---:|
| `0` | `5,513.912 ms` | `81` | `1` | `4,652.194 ms` | `4,652.194 ms` |
| `1` | `510.729 ms` | `90` | `2` | `5,076.063 ms` | `423.869 ms` |

## Next

The next likely lighting throughput slice is graph-drain instrumentation and
optimization:

- expose block/sky queue sizes before and after `run_updates`
- count processed graph nodes per layer
- split block and sky drain timing
- identify duplicate queueing from section activation, sky sources, and block
  emitters
- keep this inside `mclone_light` / `light_world.rs`, not `scheduler.rs`

After the graph drain is explainable and cheaper, return to retained-state
release policy and live block-change light deltas.
