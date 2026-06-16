# 034: Native Render Compile Revisions And Priority

Status: completed first pass.

## Purpose

Reduce wasted render compile worker CPU after the first async compile queue
slice.

The movement-frame release probe is under the 120 Hz frame budget, but it still
shows stale worker output (`30 / 91` submitted sections). That stale work is no
longer a visible frame hitch, but it is wasted CPU and battery. For desktop this
is acceptable; for future standalone/XR targets it is the next Java-shaped
efficiency issue.

## Java 1.17.1 Reference Shape

Relevant source files:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
  - `compileChunksUntil(...)` drains pending uploads, then schedules dirty
    chunks only while its estimated time budget allows.
  - `setBlockDirty(...)`, `setBlocksDirty(...)`, and
    `setSectionDirtyWithNeighbors(...)` mark dirty sections at block/section
    granularity.
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
  - `toBatch` is a `PriorityQueue` of compile tasks ordered by distance at task
    creation.
  - `RenderChunk.createCompileTask()` cancels older tasks for the same render
    chunk before scheduling a new one.
  - `RebuildTask.doTask(...)` exits early if cancelled or if required neighbors
    are unavailable.
  - `toUpload` keeps GPU upload work on the render side.

Native does not yet need Java's exact executor/mailbox/buffer-pack structure.
The important near-term shape is:

- dirty render-section work is ordered by camera distance
- an update to one section should not stale unrelated in-flight sections
- queued stale work should be coalesced instead of building an unbounded backlog
- CPU mesh generation stays off the frame path
- GPU upload stays render-thread owned

## Implementation Plan

1. Replace the global render compile epoch with per-render-section input
   revisions.
2. Bump section revisions when chunk snapshots, chunk unloads, or section block
   updates mark render sections dirty.
3. Attach the submitted section revisions to each compile request and return
   them with the worker result.
4. On completion, accept sections whose current revision still matches the
   submitted revision and requeue only the sections whose revision changed.
5. Sort dirty chunk/section work by distance to the current camera before
   applying the existing per-frame chunk budget.
6. Avoid queueing additional worker requests while one compile request is still
   pending; this is a conservative single-worker equivalent of coalescing queued
   stale work.

Out of scope for this slice:

- multiple render compile workers
- interrupting an actively running worker task
- Java-style reusable buffer packs
- GPU upload byte budgets
- live light-section revisioning

## Expected Result

Release movement-frame and frame-budget probes should keep `remesh_ms` low while
reducing stale compile sections substantially. If section updates happen during
an in-flight compile, completed sections unrelated to those updates should still
be accepted.

## First Slice Result

Implemented:

- per-render-section compile input revisions
- revision bumps for chunk snapshot, chunk unload, and section block update
  dirtying
- partial worker result acceptance so unchanged sections from a multi-section
  request are published even when one section becomes stale
- camera-distance ordering before applying the existing compile chunk budget
- single-worker queue coalescing by avoiding additional compile submissions
  while a request is already pending
- tests for distance ordering and partial stale acceptance

Release movement-frame probe:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --movement-frame-probe --frame-budget-frames 240 --target-hz 120 --path-radius 4
```

| Metric | Before | After |
|---|---:|---:|
| over-budget frames | `0 / 240` | `0 / 240` |
| p95 frame | `3.317 ms` | `3.010 ms` |
| p99 frame | `4.198 ms` | `4.156 ms` |
| max frame | `7.066 ms` | `7.234 ms` |
| max `poll_ms` | `1.295 ms` | `1.453 ms` |
| max `remesh_ms` | `0.094 ms` | `0.130 ms` |
| max `upload_ms` | `0.399 ms` | `0.400 ms` |
| submitted compile sections | `91` | `107` |
| completed compile sections | `61` | `107` |
| stale compile sections | `30` | `0` |
| max pending compile jobs | `1` | `1` |
| max in-flight sections | `16` | `16` |

Release stress-orbit frame-budget probe:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --frame-budget-probe --frame-budget-frames 120 --target-hz 120
```

| Metric | Before | After |
|---|---:|---:|
| over-budget frames | `0 / 120` | `0 / 120` |
| p95 frame | `4.030 ms` | `3.724 ms` |
| p99 frame | `4.260 ms` | `4.361 ms` |
| max frame | `6.968 ms` | `7.112 ms` |
| max `remesh_ms` | `0.060 ms` | `0.080 ms` |
| max `upload_ms` | `0.326 ms` | `0.342 ms` |
| submitted compile sections | `78` | `115` |
| completed compile sections | `42` | `98` |
| stale compile sections | `33` | `14` |
| max pending compile jobs | `1` | `1` |
| max in-flight sections | `16` | `16` |

Interpretation: the movement-shaped efficiency target is fixed for this slice:
fluid/section updates no longer stale unrelated in-flight render sections. The
stress orbit still has stale sections because that path intentionally moves fast
enough to unload or supersede chunks while active compile work is running; even
there, stale output dropped substantially.

Remaining gap: active worker tasks are not interruptible. Native now avoids
building a queued backlog behind the single worker, but a request already being
compiled can only be ignored after it completes. Matching Java more closely
would require cancellable worker tasks or smaller compile task granularity.

## Follow-Ups

1. Add cancellable render compile tasks or split requests so fast streaming can
   stop work on sections that unload while a worker is active.
2. Consider multiple workers only after task cancellation and upload budgeting
   are in place.
3. Add GPU upload byte/section budgeting if probes show upload cost becoming
   material.

## Validation

Required commands:

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-server -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm --silent native:frame-budget:smoke
pnpm --silent native:movement-frame:smoke
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --movement-frame-probe --frame-budget-frames 240 --target-hz 120 --path-radius 4
git diff --check
```

Rendered-output validation:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --screenshot /tmp/mclone-render-compile-revisions-verify.png --width 640 --height 360 --chunk-radius 1
```

Inspect the PNG before considering the slice complete.

Validation completed:

- `cargo fmt --manifest-path native/Cargo.toml --all -- --check`
- `cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-server -p mclone-native-client`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `pnpm --silent native:frame-budget:smoke`
- `pnpm --silent native:movement-frame:smoke`
- `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --movement-frame-probe --frame-budget-frames 240 --target-hz 120 --path-radius 4`
- `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --frame-budget-probe --frame-budget-frames 120 --target-hz 120`
- `cargo test --manifest-path native/Cargo.toml`
- `git diff --check`

Rendered-output validation:

- `/tmp/mclone-render-compile-revisions-verify.png` captured and inspected.
