# 033: Native Async Render Section Compile Queue

Status: completed first pass.

## Purpose

Move CPU render-section mesh compilation off the native desktop frame path.

The previous slices made publication, dirtying, and render-section rebuilds
bounded, but the frame that consumes dirty render work still performs CPU mesh
builds synchronously before GPU upload. That is no longer the dominant cost in
the 120 Hz frame-budget probe after section block deltas, but it remains the
most Java-shaped next risk for visible walking hitches when chunk snapshots,
streaming movement, or larger dirty groups arrive.

## Java 1.17.1 Reference Shape

Relevant source files:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
  - `compileChunksUntil(...)` drains pending uploads first, then schedules dirty
    render chunks only while estimated frame budget remains.
  - `setBlockDirty(...)`, `setBlocksDirty(...)`, and
    `setSectionDirtyWithNeighbors(...)` feed dirty render work.
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
  - owns `toBatch`, `toUpload`, free worker buffer packs, and an executor-backed
    mailbox.
  - `RenderChunk.rebuildChunkAsync(...)` creates a compile task and schedules it
    through the dispatcher.
  - `RebuildTask.doTask(...)` cancels if the task is stale or lacks required
    neighbors, compiles CPU buffers off the render caller, then queues uploads.
  - `uploadAllPendingUploads()` runs upload tasks on the render side.

Native does not need Java's exact buffer-pack object model yet. The important
contract is:

- dirty render work becomes queued compile work
- CPU mesh generation is not performed inside the frame's upload function
- stale compile results are ignored rather than publishing old chunk data
- GPU resource creation stays on the render thread

## Current Native Shape

Relevant paths:

- `native/apps/mclone-native-client/src/main.rs`
  - `WindowSceneRuntime::sync_render_sections_with_budget(...)` selects ready
    dirty sections.
  - `RenderSectionCompileWorker` owns a native compile thread and receives
    ready section targets plus cloned `ChunkSnapshot` inputs.
  - `CachedTexturedRenderSections::apply_build_report(...)` merges completed
    worker output into the CPU cache.
  - `ChunkApp::upload_runtime_sections(...)` drains completed worker output,
    submits newly ready work, and uploads completed section buffers.
- `native/crates/mclone-mesh/src/lib.rs`
  - `build_textured_render_sections_for_section_set_with_stats(...)` produces
    owned `TexturedRenderSectionMesh` values that can cross a worker boundary.
- `native/crates/mclone-render/src/chunk.rs`
  - `TexturedSectionDrawResources::apply_section_updates(...)` creates/updates
    GPU buffers and must remain on the render thread.

## First Slice Scope

Implement a conservative native-only worker queue:

- Add a render-section compile worker using `std::thread` and
  `std::sync::mpsc`, matching the existing server worker style.
- Submit ready dirty render sections to the worker instead of compiling them
  inline.
- Capture owned `ChunkSnapshot` inputs at submission time and let the worker
  unpack snapshots and build `TexturedRenderSectionMesh` values.
- Keep removals synchronous and cheap so unloaded sections disappear promptly.
- Drain completed worker results from the frame path and merge them into the CPU
  render-section cache.
- Use a render compile epoch so results compiled against older client chunk
  state are discarded and their target sections are re-dirtied if still loaded.
- Keep `sync_all_render_sections(...)` able to fully drain work for startup,
  screenshots, timedemo setup, and tests.
- Expose compile queue counters in debug/probe output.

Out of scope for this slice:

- multiple render compile workers
- priority ordering by distance
- cancellation of an actively running worker task
- GPU upload byte budgeting
- reusable/persistent GPU buffers
- chunk mesh input revision cache

## Expected Result

Interactive and frame-budget frames should show much lower synchronous
`remesh_ms` because the frame only submits compile work or drains completed
compile results. New metrics should show pending/submitted/completed compile
work so any remaining pacing misses can be attributed to server polling, GPU
upload, render submission, or worker backlog.

## First Slice Result

Implemented:

- native `RenderSectionCompileWorker` using `std::thread` and `std::sync::mpsc`
- frame-path submission of ready dirty sections instead of inline CPU mesh
  compilation
- worker-side snapshot unpacking and section mesh generation
- main-thread merge of completed `TexturedRenderSectionMesh` values
- stale-result handling through a render compile epoch
- retained synchronous `sync_all_render_sections(...)` drain behavior for
  startup, screenshots, timedemo setup, and tests
- debug/probe counters for submitted, completed, stale, pending, and in-flight
  compile work

Release 120 Hz frame-budget probe:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --frame-budget-probe --frame-budget-frames 120 --target-hz 120
```

| Metric | Value |
|---|---:|
| over-budget frames | `0 / 120` |
| p95 frame | `4.030 ms` |
| p99 frame | `4.260 ms` |
| max frame | `6.968 ms` |
| max `remesh_ms` | `0.060 ms` |
| max `upload_ms` | `0.326 ms` |
| submitted compile sections | `78` |
| completed compile sections | `42` |
| stale compile sections | `33` |
| max pending compile jobs | `1` |
| max in-flight sections | `16` |

Interpretation: CPU render-section compilation is no longer a meaningful
main-thread frame cost in the probe. The conservative global epoch discards
some worker output during continuous streaming/fluid updates; this is correct
for now, but a future per-section/chunk revision scheme would reduce wasted
worker work.

Movement smoke:

```bash
pnpm --silent native:movement:smoke
```

This still drains render work fully per movement step, so it is not the same as
interactive frame pacing. It remains useful for correctness and server/runtime
pressure. In this run, the largest step was dominated by polling/waiting for
worldgen publication (`poll_ms = 323.175`) rather than frame-path render
compilation.

Movement-frame release probe:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --movement-frame-probe --frame-budget-frames 240 --target-hz 120 --path-radius 4
```

| Metric | Value |
|---|---:|
| over-budget frames | `0 / 240` |
| p95 frame | `3.317 ms` |
| p99 frame | `4.198 ms` |
| max frame | `7.066 ms` |
| headless average frame | `2.054 ms` |
| max `poll_ms` | `1.295 ms` |
| max `remesh_ms` | `0.094 ms` |
| max `upload_ms` | `0.399 ms` |
| submitted compile sections | `91` |
| completed compile sections | `61` |
| stale compile sections | `30` |
| max pending compile jobs | `1` |
| max in-flight sections | `16` |

Interpretation: the movement-shaped headless probe does not reproduce the
visible 120 Hz desktop walking hitch on this host. The worst frame was frame
`0` at `7.066 ms`, mostly `device_poll_ms = 6.102`, with no remesh/upload work
on that frame. Runtime polling, render-section compile submission/drain, and GPU
upload stayed under budget.

Rendered validation:

- `/tmp/mclone-async-render-compile-verify.png` captured and inspected.

## Follow-Ups

1. Add desktop present/wait attribution for the visible walking hitch path; the
   movement-shaped headless frame probe is green, so desktop swapchain pacing is
   now the larger unknown.
2. Replace the global render compile epoch with per-chunk or per-section input
   revisions so unrelated updates do not stale otherwise valid compile output.
3. Add distance-prioritized compile scheduling and cancellation/coalescing for
   queued stale work, matching Java's priority/cancel shape more closely.
4. Add GPU upload byte/section budgeting only if probes show upload cost becomes
   material again.

## Validation

Required commands:

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-server -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm --silent native:frame-budget:smoke
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --frame-budget-probe --frame-budget-frames 120 --target-hz 120
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --movement-frame-probe --frame-budget-frames 240 --target-hz 120 --path-radius 4
git diff --check
```

Rendered-output validation:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --screenshot /tmp/mclone-async-render-compile-verify.png --width 640 --height 360 --chunk-radius 1
```

Inspect the PNG before considering the slice complete.
