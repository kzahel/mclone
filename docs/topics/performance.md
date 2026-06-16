# Performance Topic

Status: active living index.

This document is the current map for native performance work. Tactical docs
remain the implementation records; this topic doc records priority, Java
alignment, current measured state, and where to look next.

Update this file whenever a performance slice changes the priority order,
invalidates an older recommendation, or establishes a new baseline.

## Current Baseline

Current validated slice: native async render-section compile queue, on top of
section block deltas, section-precise dirtying, and neighbor-ready render
boundaries.

Latest 120 Hz release frame-budget probe was captured during that slice at:

```text
/tmp/mclone-frame-budget-release-async-render-compile.json
```

Summary:

| Metric | Value |
|---|---:|
| over-budget frames | `0 / 120` |
| p95 frame | `4.030 ms` |
| p99 frame | `4.260 ms` |
| max frame | `6.968 ms` |
| headless average frame | `2.360 ms` |
| max `poll_ms` | `1.611 ms` |
| max `remesh_ms` | `0.060 ms` |
| max `upload_ms` | `0.326 ms` |
| max pending render chunks | `9` |
| max rebuilt sections | `16` |
| total rebuilt sections | `42` |
| total uploaded sections | `31` |
| submitted compile sections | `78` |
| completed compile sections | `42` |
| stale compile sections | `33` |
| max pending compile jobs | `1` |
| max in-flight sections | `16` |
| total fluid mutated blocks | `80` |
| total snapshot updates | `0` |
| total section block updates | `27` |
| total fluid snapshot events | `0` |

Interpretation: the measured full-snapshot-per-fluid-block hitch is fixed.
Fluid mutations now publish section deltas, and section/block update frames now
dirty only the affected render section neighborhood. CPU render-section
compilation is now off the frame path; `remesh_ms` in the frame probe is
submission/drain bookkeeping rather than mesh generation. Nonzero pending render
chunks include retained deferred sections and in-flight worker sections. The
global compile epoch intentionally stales some worker output during continuous
updates; this is correct but conservative.

For durable historical trends, use [`../performance-records.md`](../performance-records.md).

## Priority Queue

| Priority | Work | Java-shaped | Tactical | Status | Why It Matters |
|---|---|---:|---|---|---|
| P0 | Movement-shaped release frame probe | Native policy | [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md), [`033`](../tactical/033-native-async-render-section-compile-queue.md), [`../performance-records.md`](../performance-records.md) | next recommended | The original issue is visible walking hitching. The current frame-budget probe is green, while `native:movement:smoke` fully drains work per step and is not a live frame-pacing model. Add a release probe that moves like the desktop client without forcing full drains. |
| P1 | Render compile queue stale/prioritization refinement | Yes | [`033`](../tactical/033-native-async-render-section-compile-queue.md), [`030`](../tactical/030-native-streaming-publish-and-render-budget.md) | recommended | Java prioritizes/cancels compile tasks. Native's first worker queue uses a conservative global epoch; the release probe saw `33` stale sections out of `78` submitted. Per-section/chunk revisions and distance priority would reduce wasted worker work. |
| P2 | GPU upload budgeting and buffer reuse | Broadly | [`024`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md), [`030`](../tactical/030-native-streaming-publish-and-render-budget.md) | conditional | Native uploads changed sections incrementally, and this probe's max upload was only `0.326 ms`. Do this when probes show upload/allocation cost is material again. |
| P3 | Live light deltas and light-section dirtying | Yes | [`026`](../tactical/026-lighting-pipeline.md), [`031`](../tactical/031-native-section-block-delta-updates.md) | pending larger subsystem | Section block deltas currently leave light payloads unchanged. Correct live lighting needs Java-shaped light propagation/deltas and render dirtying by changed light sections. |
| P4 | Release perf budgets and durable records | Native policy | [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md), [`030`](../tactical/030-native-streaming-publish-and-render-budget.md), [`033`](../tactical/033-native-async-render-section-compile-queue.md), [`../performance-records.md`](../performance-records.md) | ongoing | Once baselines stabilize, add budget thresholds that catch regressions without failing on normal host noise. |

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
git diff --check
```

Release comparison:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --frame-budget-probe --frame-budget-frames 120 --target-hz 120
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
