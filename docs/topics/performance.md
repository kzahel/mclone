# Performance Topic

Status: active living index.

This document is the current map for native performance work. Tactical docs
remain the implementation records; this topic doc records priority, Java
alignment, current measured state, and where to look next.

Update this file whenever a performance slice changes the priority order,
invalidates an older recommendation, or establishes a new baseline.

## Current Baseline

Current validated slice: native render compile revisions and priority, on top
of the movement-frame probe, async render-section compile queue, section block
deltas, section-precise dirtying, and neighbor-ready render boundaries.

Latest 120 Hz release movement-frame probe was captured during that slice at:

```text
/tmp/mclone-movement-frame-probe-release-render-compile-revisions.json
```

Summary:

| Metric | Value |
|---|---:|
| probe mode | `movement_walk` |
| target | `120 Hz` / `8.333 ms` |
| frames | `240` |
| movement speed | `32 blocks/sec` |
| over-budget frames | `0 / 240` |
| p95 frame | `3.010 ms` |
| p99 frame | `4.156 ms` |
| max frame | `7.234 ms` |
| max `poll_ms` | `1.453 ms` |
| max `remesh_ms` | `0.130 ms` |
| max `upload_ms` | `0.400 ms` |
| max pending render chunks | `9` |
| submitted compile sections | `107` |
| completed compile sections | `107` |
| stale compile sections | `0` |
| max pending compile jobs | `1` |
| max in-flight sections | `16` |
| total fluid mutated blocks | `299` |
| total snapshot updates | `5` |
| total section block updates | `102` |

Interpretation: the movement-shaped headless probe remains under budget, and
the Java-shaped per-section revision slice removed stale compile output from
the normal walking lane (`30` stale sections before, `0` after). Runtime
polling, render-section submission/drain bookkeeping, and GPU upload remain
comfortably below 120 Hz budget. Fast stress-orbit streaming can still stale
active worker tasks when chunks unload before a compile finishes, but queued
backlog is now coalesced.

For durable historical trends, use [`../performance-records.md`](../performance-records.md).

## Priority Queue

| Priority | Work | Java-shaped | Tactical | Status | Why It Matters |
|---|---|---:|---|---|---|
| P0 | Cancellable render compile tasks | Yes | [`034`](../tactical/034-native-render-compile-revisions-and-priority.md), [`033`](../tactical/033-native-async-render-section-compile-queue.md) | next recommended | Native now avoids stale queued backlog and accepts unchanged sections, but an already-running worker request cannot be interrupted. Java render chunk tasks have cancellation flags. Cancellable or smaller-granularity tasks would reduce wasted CPU during fast streaming/unloads, relevant to battery/thermal targets. |
| P1 | GPU upload budgeting and buffer reuse | Broadly | [`024`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md), [`030`](../tactical/030-native-streaming-publish-and-render-budget.md) | conditional | Native uploads changed sections incrementally, and this probe's max upload was only `0.400 ms`. Do this when probes show upload/allocation cost is material again, or before adding multiple compile workers. |
| P2 | Live light deltas and light-section dirtying | Yes | [`lighting topic`](lighting.md), [`026`](../tactical/026-lighting-pipeline.md), [`031`](../tactical/031-native-section-block-delta-updates.md) | pending larger subsystem | Section block deltas currently leave light payloads unchanged. Correct live lighting needs Java-shaped light propagation/deltas and render dirtying by changed light sections. |
| P3 | Desktop-present pacing attribution | Native policy | [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md), [`033`](../tactical/033-native-async-render-section-compile-queue.md), [`../performance-records.md`](../performance-records.md) | optional | Do this if visible desktop hitching returns or if we need end-to-end swapchain/present attribution. Current priority is worker efficiency because desktop/headless probes are under budget. |
| P4 | Release perf budgets and durable records | Native policy | [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md), [`030`](../tactical/030-native-streaming-publish-and-render-budget.md), [`033`](../tactical/033-native-async-render-section-compile-queue.md), [`034`](../tactical/034-native-render-compile-revisions-and-priority.md), [`../performance-records.md`](../performance-records.md) | ongoing | Once baselines stabilize, add budget thresholds that catch regressions without failing on normal host noise. |

## Java Reference Anchors

Use these local sources when implementing or reviewing performance work:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`
  - chunk packets mark all chunk sections dirty
  - section/block update packets apply changed blocks to the client level
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
  - `setBlockDirty`, `setBlocksDirty`, and `setSectionDirtyWithNeighbors`
    define section dirty granularity
  - `updateRenderChunks(...)` performs graph-aware visible-section traversal
  - `compileChunksUntil(...)` consumes render compile/upload work under a time
    budget
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java`
  - owns reusable render chunk slots and dirty flags
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
  - owns render chunk compile tasks, upload queue, and `hasAllNeighbors()`
  - creates `RenderChunkRegion` with a one-block halo for real neighbor block
    reads
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/VisGraph.java`
  and `VisibilitySet.java`
  - local section face-connectivity graph used by traversal
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
  - records changed blocks by section and broadcasts block/section update
    packets
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
  - mailbox-shaped light scheduling and batched publication

## Completed Or De-Risked

- Desktop frame pacing controls and headless frame-budget probe:
  [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md)
- Completed chunk publication slicing and initial streaming attribution:
  [`030`](../tactical/030-native-streaming-publish-and-render-budget.md)
- Bounded synchronous render-section rebuild/upload work:
  [`030`](../tactical/030-native-streaming-publish-and-render-budget.md)
- CPU render-section dirty cache and incremental GPU section updates:
  [`024`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md)
- Java `VisGraph` / `VisibilitySet` first pass:
  [`025`](../tactical/025-render-section-visibility-graph.md)
- Section block deltas replacing runtime full chunk snapshot publication:
  [`031`](../tactical/031-native-section-block-delta-updates.md)
- Neighbor-ready render boundary first pass:
  [`032`](../tactical/032-native-neighbor-stable-render-boundaries.md)
- Section-precise render dirtying/rebuilds for section block deltas:
  [`024`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md),
  [`031`](../tactical/031-native-section-block-delta-updates.md)
- Async CPU render-section compile queue first pass:
  [`033`](../tactical/033-native-async-render-section-compile-queue.md)
- Movement-shaped release frame probe:
  [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md),
  [`033`](../tactical/033-native-async-render-section-compile-queue.md)
- Render compile queue revision/priority refinement:
  [`034`](../tactical/034-native-render-compile-revisions-and-priority.md)

## Tactical Index

Primary performance tacticals:

- [`020-native-movement-perf-and-render-culling.md`](../tactical/020-native-movement-perf-and-render-culling.md)
- [`023-render-mesh-culling-parity-and-perf.md`](../tactical/023-render-mesh-culling-parity-and-perf.md)
- [`024-render-section-dirty-cache-and-upload-diffs.md`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md)
- [`025-render-section-visibility-graph.md`](../tactical/025-render-section-visibility-graph.md)
- [`029-native-frame-pacing-and-streaming-hitches.md`](../tactical/029-native-frame-pacing-and-streaming-hitches.md)
- [`030-native-streaming-publish-and-render-budget.md`](../tactical/030-native-streaming-publish-and-render-budget.md)
- [`031-native-section-block-delta-updates.md`](../tactical/031-native-section-block-delta-updates.md)
- [`032-native-neighbor-stable-render-boundaries.md`](../tactical/032-native-neighbor-stable-render-boundaries.md)
- [`033-native-async-render-section-compile-queue.md`](../tactical/033-native-async-render-section-compile-queue.md)
- [`034-native-render-compile-revisions-and-priority.md`](../tactical/034-native-render-compile-revisions-and-priority.md)

Related subsystem tacticals:

- [`021-scheduled-fluid-ticks.md`](../tactical/021-scheduled-fluid-ticks.md)
- [`026-lighting-pipeline.md`](../tactical/026-lighting-pipeline.md)
- [`028-headless-window-frame-unification.md`](../tactical/028-headless-window-frame-unification.md)

## Validation Lanes

Fast gates:

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm --silent native:frame-budget:smoke
pnpm --silent native:movement-frame:smoke
git diff --check
```

Release comparison:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --frame-budget-probe --frame-budget-frames 120 --target-hz 120
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --movement-frame-probe --frame-budget-frames 240 --target-hz 120
```

Movement and render pressure:

```bash
pnpm --silent native:movement:smoke
pnpm --silent native:timedemo:smoke
```

For pixel-affecting native work, capture screenshots to `/tmp` and inspect
them before considering the slice complete.

## Update Policy

- Keep this topic doc short enough to scan.
- Record detailed raw historical baselines in
  [`../performance-records.md`](../performance-records.md), not here.
- When a tactical recommendation is superseded, update the old tactical with a
  pointer to the newer tactical and update this priority queue.
- Prefer Java-shaped work when it addresses the measured bottleneck or prevents
  correctness/performance churn.
- Do not add performance thresholds until there is a stable release-mode
  baseline and the threshold would catch a real regression rather than host
  noise.
