# 030: Native Streaming Publish And Render Budget

Status: active.

## Purpose

Reduce visible native desktop frame drops while walking across chunk boundaries
by making streaming publication, render-section rebuilds, and GPU uploads
bounded frame work instead of unbounded work performed inline with presentation.

At 120 Hz the frame budget is 8.33 ms. The headless frame-budget probe added in
029 shows that steady drawing is cheap, but streaming work routinely exceeds
that budget by one to two orders of magnitude.

## Current Measurement

Release-mode probe command:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --frame-budget-probe --frame-budget-frames 120 --target-hz 120
```

Baseline from commit `9093b70`:

- `53/120` frames over the 8.33 ms 120 Hz budget.
- `p95_frame_ms = 180.398`, `p99_frame_ms = 359.413`,
  `max_frame_ms = 376.741`.
- Average per-frame phases:
  - `poll_ms = 32.167`
  - `set_interest_ms = 4.323`
  - `remesh_ms = 5.012`
  - `upload_ms = 1.306`
  - `render_ms = 0.073`

Interpretation: basic rendering is not the current bottleneck. The hitches come
from chunk-interest updates, completed worldgen publication, CPU render-section
rebuilds, and GPU buffer upload/allocation.

## Native Findings

Current native paths:

- `native/apps/mclone-native-client/src/main.rs`: spectator chunk movement calls
  `WindowSceneRuntime::set_interest_center`, then may immediately call
  `upload_runtime_sections`.
- `WindowSceneRuntime::poll` calls `IntegratedServer::try_simulation_tick_report`
  on the frame path.
- `native/crates/mclone-server/src/lib.rs`: `ChunkScheduler::poll` drains all
  completed worldgen jobs through `publish_completed_worldgen_jobs`.
- `publish_completed_feature_job` computes provisional lighting and packs a
  `ChunkSnapshot` for every completed target chunk before returning.
- `snapshot_mesh_block_state_ids` unpacks packed chunk sections into dense
  arrays and clones light data for meshing.
- `CachedTexturedRenderSections::rebuild_dirty` rebuilds dirty render sections
  synchronously.
- `mclone-mesh::build_textured_render_sections_for_chunks` scans section blocks
  for visibility, scans again to emit faces, and does linear chunk/light lookup
  during neighbor sampling.
- `TexturedSectionDrawResources::apply_section_updates` creates new GPU buffers
  and serializes vertex/index bytes synchronously for every uploaded section.

This means "background worldgen" is not enough. The frame can still block on
the publication and GPU-prep side of completed work.

## Java 1.17.1 Reference Shape

Relevant reference files:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
  creates worldgen, main-thread, and light mailboxes and routes them through
  `ChunkTaskPriorityQueueSorter`.
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
  stores chunk-status futures instead of synchronously promoting every status
  in the caller.
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
  schedules light work through a mailbox and processes it in batches.
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java`
  applies chunk packet data and updates light-section state.
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`
  marks render sections dirty after chunk and light packets.
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java`
  keeps a reusable render-chunk grid and marks chunks dirty as the camera moves.
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
  owns a compile queue, worker buffer packs, and an upload queue.
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
  uses `compileChunksUntil` to schedule chunk compile work only while frame
  budget remains.

The shape to match is not Java's exact object model. The important contract is:
packet/snapshot publication marks render work dirty; CPU compile work is queued;
GPU upload work is queued; the frame loop consumes those queues under a budget.

## Implementation Plan

1. Slice completed chunk publication:
   - Do not drain completed worldgen publication from `set_chunk_interest`.
   - Queue completed worldgen jobs inside `ChunkScheduler`.
   - Publish only a small number of completed target chunks per server poll.
   - Keep dedicated/headless "wait until idle" helpers aware of pending
     publication so they do not sleep while already-completed work is queued.
2. Add render-section build queue:
   - Convert dirty chunk/section keys into queued CPU mesh work.
   - First bound the amount of queued work consumed per frame.
   - Then move queued builds to worker jobs that produce
     `TexturedRenderSectionMesh` values.
   - Keep the main thread responsible for publishing completed section meshes.
   - Keep a synchronous fallback for wasm if worker support is not introduced
     there yet.
3. Add GPU upload budget:
   - Queue completed section meshes for upload.
   - Consume by max section count or byte count per frame.
   - Keep old GPU buffers visible until replacements upload successfully.
4. Reduce copies and lookup costs:
   - Cache unpacked mesh inputs by chunk revision instead of unpacking all
     loaded snapshots for every dirty rebuild.
   - Replace linear neighbor/light input lookup during meshing with a compact
     coordinate map.
   - Store light sections for direct section-y lookup.
5. Tighten diagnostics and budgets:
   - Add pending publication, mesh-job, and upload-queue counters to debug/probe
     output.
   - Add release-mode comparison notes for `pnpm native:frame-budget:perf`.
   - Once stable, add budget thresholds that fail only on clear regressions.

## First Slice

Implement completed-worldgen publication slicing:

- Add a pending publication queue in `ChunkScheduler`.
- Set a conservative per-poll completed target-chunk publication budget.
- Keep `IntegratedServer::set_chunk_interest` from synchronously polling
  completed jobs.
- Expose pending publication count for runtime wait loops and diagnostics.
- Precompute provisional light sections inside completed worldgen jobs so
  frame-side publication attaches packed light data instead of running light
  propagation.
- Validate with the frame-budget probe before moving to render mesh threading.

Expected result: `set_interest_ms` and `poll_ms` spikes should drop. Frames can
still miss budget because CPU remesh and GPU upload remain synchronous.

## First Slice Result

Implemented in the native streaming publication budget slice:

- `ChunkScheduler` queues completed worldgen publications and publishes one
  completed target chunk per scheduler poll.
- `IntegratedServer::set_chunk_interest` only applies interest and schedules
  work; completed job publication happens in `try_poll`.
- wait-until-idle helpers avoid sleeping when completed publication work is
  already queued.
- completed worldgen jobs carry precomputed provisional light sections from the
  worker path.
- frame-budget probe JSON and live debug stats expose pending publication count.

Release-mode probe comparison against the 120-frame baseline from commit
`9093b70`:

| Metric | Before | After first slice |
|---|---:|---:|
| over-budget frames | 53 / 120 | 14 / 120 |
| over 2x budget | 48 / 120 | 7 / 120 |
| over 4x budget | 36 / 120 | 3 / 120 |
| average frame work | 44.635 ms | 4.975 ms |
| p95 frame | 180.398 ms | 20.606 ms |
| p99 frame | 359.413 ms | 54.383 ms |
| max frame | 376.741 ms | 59.848 ms |
| average `set_interest_ms` | 4.323 ms | 0.337 ms |
| average `poll_ms` | 32.167 ms | 1.404 ms |
| max `poll_ms` | 183.702 ms | 31.426 ms |

Interpretation: completed publication and provisional lighting are no longer the
dominant steady hitch. Remaining over-budget frames are concentrated in frames
that synchronously rebuild and upload many render sections, for example 192-240
rebuilt sections plus 82-93 uploaded sections in the worst frames. The next
slice should queue CPU render-section builds and then add a GPU upload budget.

## Second Slice

Implement bounded render-section mesh work on the frame path:

- Keep startup, screenshots, timedemo setup, and movement smoke paths able to
  fully drain render mesh work.
- Convert live/window and frame-budget probe streaming to a one dirty chunk per
  frame mesh-build budget.
- Process old-section removals alongside the budgeted rebuild so unloaded
  chunks do not linger just because CPU mesh work is capped.
- Consume mesh work once per redraw after chunk-interest updates and integrated
  server polling have both run, instead of rebuilding once after interest and
  again after poll in the same frame.
- Expose pending render chunk count in live debug stats and frame-budget probe
  JSON.

This is intentionally still single-threaded CPU mesh construction. It adds the
Java-style queue and frame budget boundary first; the later worker-thread slice
can move the queued build itself off the frame thread without changing the
high-level publication contract.

## Second Slice Result

Release-mode probe comparison against the first slice result:

| Metric | After first slice | After second slice |
|---|---:|---:|
| over-budget frames | 14 / 120 | 9 / 120 |
| over 2x budget | 7 / 120 | 4 / 120 |
| over 4x budget | 3 / 120 | 1 / 120 |
| average frame work | 4.975 ms | 3.949 ms |
| p95 frame | 20.606 ms | 9.064 ms |
| p99 frame | 54.383 ms | 32.124 ms |
| max frame | 59.848 ms | 34.671 ms |
| average `remesh_ms` | 1.117 ms | 0.394 ms |
| max `remesh_ms` | 19.006 ms | 1.673 ms |
| average `upload_ms` | 0.355 ms | 0.144 ms |
| max `upload_ms` | 5.513 ms | 0.863 ms |

Interpretation: render-section rebuild and upload are no longer the dominant
hitch. Budgeted mesh publishing spreads rebuild work across more frames
(`36/120` frames rebuilt one chunk each, `576` total rebuilt sections), but the
mesh/upload phase stayed small enough that rebuilt frames averaged `7.858 ms`.
No-rebuild frames averaged `2.274 ms`.

The remaining p99/max misses are now dominated by integrated server `poll_ms`
spikes, not renderer work. The worst release frames were:

- frame `118`: `34.671 ms` total, `30.694 ms poll_ms`, `1.410 ms remesh_ms`,
  `0.598 ms upload_ms`
- frame `113`: `32.124 ms` total, `28.399 ms poll_ms`, `1.436 ms remesh_ms`,
  `0.371 ms upload_ms`
- frame `108`: `30.208 ms` total, `26.634 ms poll_ms`, `1.272 ms remesh_ms`,
  `0.383 ms upload_ms`

The next slice should instrument and budget the integrated server poll path
under frame-budget probe, with particular attention to scheduler tick,
publication/event application, and any main-thread chunk tick work that runs
while worldgen backlog is high.

## Poll Instrumentation Result

Added frame-budget probe instrumentation for native integrated-server polling:

- split `poll_ms` into local command flush, server tick, server-reported tick
  total, scheduler report, scheduler event application, client update
  application, and render mesh/update work.
- split scheduler report into stale ticket purge, holder reconciliation,
  completed publication, and pending unload processing.
- split fluid tick work into due scan, due removal, fluid mutation, and
  block-set/snapshot publication timing.
- exposed per-frame counts for scheduler events, snapshot/unload updates, fluid
  due/executed/deferred ticks, mutated blocks, snapshot events, and scheduled
  fluid ticks.

Release-mode 120 Hz probe with the instrumentation:

| Metric | Value |
|---|---:|
| over-budget frames | 8 / 120 |
| over 2x budget | 4 / 120 |
| over 4x budget | 2 / 120 |
| average frame work | 3.934 ms |
| p95 frame | 9.053 ms |
| p99 frame | 33.424 ms |
| max frame | 35.750 ms |
| max `poll_ms` | 31.635 ms |
| max `poll_fluid_tick_ms` | 31.066 ms |
| max `poll_fluid_set_block_ms` | 30.859 ms |
| max `poll_scheduler_report_ms` | 0.637 ms |
| max `poll_apply_updates_ms` | 0.015 ms |

The worst frame (`118`) was `35.750 ms` total:

- `31.635 ms poll_ms`
- `31.066 ms poll_fluid_tick_ms`
- `30.859 ms poll_fluid_set_block_ms`
- `0.551 ms poll_scheduler_report_ms`
- `0.015 ms poll_apply_updates_ms`
- `18` mutated fluid blocks, `18` snapshot events, `18` snapshot updates

Across the probe, only `15/120` frames executed fluid ticks. Those frames
produced `80` mutated blocks and `80` snapshot events. The fluid-heavy frames
dominated every remaining over-budget miss:

- over-budget frames averaged `15.297 ms` in `poll_fluid_tick_ms`
- `poll_fluid_set_block_ms` averaged `15.221 ms` in those same frames
- frames without executed fluid ticks averaged `2.539 ms` and had `0` budget
  misses

Interpretation: the remaining hitch is not chunk scheduler publication,
pending unload processing, render mesh work, GPU upload, or client update
application. It is fluid simulation publishing full chunk snapshots one block
mutation at a time through `ChunkScheduler::set_block_at_world`. Each changed
fluid block calls `snapshot_from_mutable_buffer`, increments the chunk revision,
updates the holder's published snapshot, and emits a whole `ChunkSnapshot`
event when client-visible. This makes a cluster of water/lava ticks scale as
number of mutations times full chunk snapshot build/copy cost.

Next implementation slice was revised by
[`031-native-section-block-delta-updates.md`](031-native-section-block-delta-updates.md).
The Java-shaped fix is not one full snapshot per mutated chunk; it is section
block deltas for runtime mutations, with full snapshots reserved for initial
chunk publication, reload, and recovery.

## Validation

Targeted gates:

```bash
cargo fmt --check
cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:frame-budget:smoke
git diff --check
```

Performance comparison:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --frame-budget-probe --frame-budget-frames 120 --target-hz 120
```

Interpret the result by phase. This slice is successful if `set_interest_ms`
and large `poll_ms` spikes shrink, even if `remesh_ms` and `upload_ms` remain
over budget.
