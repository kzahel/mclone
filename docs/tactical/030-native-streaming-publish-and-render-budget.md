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
   - Convert dirty chunk/section keys into CPU mesh jobs.
   - Let worker jobs produce `TexturedRenderSectionMesh` values.
   - Main thread publishes completed section meshes only.
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
