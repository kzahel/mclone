# 045: Native Shared Initial Light Batch

Status: completed first pass.

## Purpose

Build on
[`044-native-light-status-worker-and-disable-flag.md`](044-native-light-status-worker-and-disable-flag.md)
by removing the worst duplicate work from native initial `ChunkStatus::Light`
generation.

The prior worker slice moved propagation off the foreground scheduler path, but
radius-5 startup still computed `169` independent temporary light worlds. This
slice batches the initial light inputs for one completed feature job and drains
the level light graph once for all batch targets.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java`

Important reference shape:

- Java does not recompute an isolated 3x3 light graph for every target chunk.
- `ThreadedLevelLightEngine` owns a shared light engine boundary and accepts
  queued pre-update/post-update work.
- `ChunkMap` and `ChunkHolder` keep chunk status publication separate from the
  light engine's propagation ownership.

## Scope

Land the smallest shared-work boundary:

- add `PendingLightStatusBatch` in `mclone-server/src/light_status.rs`
- merge all ready target chunks from a completed feature job plus their
  one-chunk sky-light halo into one raw light-world input map
- add multi-target collection in `mclone-server/src/level_light_bridge.rs`
- update `LightStatusMailbox` to accept one batch request and emit individual
  completed light statuses
- stage light inputs in `ChunkScheduler` while feature publication is sliced
- enqueue the light batch only after the feature job's target publication
  finishes
- preserve existing one-light-status publication budget and holder status
  events
- expose completed light batch count in scheduler metrics and perf JSON

## Out Of Scope

- a fully long-lived per-world `ThreadedLevelLightEngine`
- light task prioritization and cancellation
- light ticket retain/release policy
- live block-change `checkBlock` propagation
- light delta packets and render-section dirtying
- full vanilla opacity/emission tables and face-shape occlusion
- render `LightTexture` and ambient occlusion

## Result

Landed:

- Initial light work is now feature-job scoped instead of target-chunk scoped.
- Radius-5 startup creates `1` light batch for `169` light statuses.
- The scheduler still publishes completed `ChunkStatus::Light` snapshots one at
  a time, so foreground publication budgets and client-visible events remain
  bounded.
- Server tests cover the existing light-status scheduling/publication behavior.
- Headless screenshot validation rendered nonblank terrain with the new batched
  light payloads.

Remaining parity gaps: the worker still rebuilds a temporary raw light world per
feature-job batch. That is much cheaper than per-target recomputation, but it is
not yet Java's retained world light state.

## Validation

Completed on 2026-06-18:

- `cargo fmt --manifest-path native/Cargo.toml -p mclone-server -- --check`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo check --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke`
- native radius-5 lighting-enabled perf:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting`
- native radius-5 lighting-disabled perf:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --disable-lighting`
- native full-frame screenshot:
  `/tmp/mclone-light-batch-shared.png`

Screenshot result: `960x540`, `166` cached sections, `26` drawn sections,
nonblank terrain; visually inspected with no obvious missing chunks or
blue-sky-only failure.

Radius-5 perf result:

| Lane | Total |
|---|---:|
| Native scheduler, lighting disabled | `1,107.779 ms` |
| Native scheduler, lighting enabled before batch | `47,782.677 ms` |
| Native scheduler, lighting enabled after batch | `10,745.617 ms` |

Lighting-enabled breakdown after batch:

| Metric | Value |
|---|---:|
| feature batch timing | `790.832 ms` |
| completed light statuses | `169` |
| completed light batches | `1` |
| total light-status batch compute | `9,323.850 ms` |
| `LevelLightEngine.run_all_updates` | `9,281.490 ms` |

## Next

The next likely lighting throughput slice is a real retained world light owner:

- keep it outside `scheduler.rs` so the scheduler does not become a light-engine
  mega-module
- retain generated/loaded block facts and light sections across batches
- queue section status, light-source enablement, and block-emission work against
  the retained engine
- instrument graph queue sizes and per-layer drain cost inside
  `run_all_updates`

After initial light startup is affordable, return to live block-change light
deltas and render-section dirtying.
