# Performance Topic

Status: active living index.

This document is the current map for native performance work. Tactical docs
remain the implementation records; this topic doc records priority, Java
alignment, current measured state, and where to look next.

Update this file whenever a performance slice changes the priority order,
invalidates an older recommendation, or establishes a new baseline.

## Current Baseline

Current validated slice: native block render-facts parity after non-cubic model
AO render parity. The movement-frame probe remains under budget, cold startup
lighting is no longer dominated by the sky graph drain, the dark foliage-top
render bug is fixed, the textured chunk shader uses Java's default lightmap
curve, and model faces now get Java-shaped mesh-side AO including non-cubic
shape weighting plus current-block `BlockStateBase.Cache` render facts.

Latest 120 Hz release movement-frame probe was captured during that slice at:

```text
/tmp/mclone-render-facts-movement-frame-release.json
```

Summary:

| Metric | Value |
|---|---:|
| probe mode | `movement_walk` |
| target | `120 Hz` / `8.333 ms` |
| frames | `240` |
| movement speed | `32 blocks/sec` |
| over-budget frames | `0 / 240` |
| p95 frame | `4.128 ms` |
| p99 frame | `5.889 ms` |
| max frame | `8.250 ms` |
| initial face count | `121,222` |
| initial index count | `727,332` |
| average headless frame | `2.887 ms` |

Release timedemo from the same slice:

| Metric | Value |
|---|---:|
| frames | `240` |
| scene build | `1,491.873 ms` |
| section count | `1,296` |
| face count | `405,407` |
| index count | `2,432,442` |
| average frame | `2.739 ms` |
| max frame | `13.450 ms` |
| average drawn sections | `90.167` |
| max drawn sections | `125` |

Interpretation: Java `canOcclude=false` for leaves/glass-style blocks raises
mesh face pressure because leaves no longer cull adjacent hidden solid faces.
The release movement-frame lane remains within 120 Hz budget, and timedemo
stays in the existing renderer envelope. Treat the higher face count as the new
baseline for Java-style leaf non-occlusion.

For durable historical trends, use [`../performance-records.md`](../performance-records.md).

Standalone Quest Android XR is not covered by the desktop/headless
movement-frame, timedemo, or desktop-hosted OpenXR baselines. Track headset
frame pacing, real OpenXR refresh state, overlay diagnostics, and standalone
Quest baseline work in
[`096`](../tactical/096-android-xr-quest-performance.md). The first moving
Quest probe is automated no-clip flight at walking-like speed, with
render-distance scripts for 1, 5, and 10, current OpenXR refresh reporting,
and max-stage timing attribution. Durable headset rows live in
[`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md).
On-device `XR_META_performance_metrics` data initially suggested the RD10 frozen
lane had GPU headroom (`7ms` app GPU at `58%` utilization) while per-eye CPU wall
was `~21ms`. **E1 (2026-06-29, in-stream wgpu GPU timestamps) revised this:** the
blocking stereo poll wait is real GPU execution (`~10.7ms` both eyes, matching
the poll within `~0.5ms`), the Meta `app/gpu_frametime` counter under-reported,
and the frame is a balanced serial `CPU(~9ms) + GPU(~10.7ms)` — so CPU reductions
cannot hit 72Hz alone and CPU/GPU overlap plus GPU-side reduction become the key
levers. The attribution,
confidence levels, Playbox cross-check, and the landed diagnostic +
shared-records slices live in
[`099`](../tactical/099-android-xr-rd10-render-cost-attribution.md). The ordered
remaining plan — container-bound-cull diagnosis, cache prepared records across
frames, flatten cull/encode off `BTreeMap`, single shared dual-frustum cull,
stereo multiview encode, then batching / CPU-GPU overlap, plus the ship-distance
decision — lives in
[`106`](../tactical/106-android-xr-static-render-cpu-reduction.md). The unsafe
single-submit slice was reverted by `d0c5161` after headset validation exposed a
left-eye projection/uniform-lifetime bug; the new gating plan is
[`107`](../tactical/107-xr-stereo-uniform-ownership-and-multiview.md): orphaned
E1/E2 perf surfaces have been removed/fenced, and all current per-view
renderers now have slotted uniform/view-data ownership; next are headset visual
confirmation on the per-eye-submit baseline, one-submit correctness proof, then
multiview before optional frame pipelining.

## Priority Queue

| Priority | Work | Java-shaped | Tactical | Status | Why It Matters |
|---|---|---:|---|---|---|
| P0 | Startup loading/progress presentation | Native policy | [`027`](../tactical/027-mclone-ui-foundation.md), [`028`](../tactical/028-headless-window-frame-unification.md), [`044`](../tactical/044-native-light-status-worker-and-disable-flag.md) | next recommended for desktop feel | Radius-5 lighting-enabled startup dropped to `1,931.110 ms`, but the desktop window path still waits for the first light-ready scene. Even with faster lighting, presenting progress or a partial scene will make regressions diagnosable instead of looking like a frozen app. |
| P1 | Liquid renderer light sampling | Yes | [`054`](../tactical/054-native-block-render-facts-parity.md), [`lighting topic`](lighting.md) | next recommended for visual lighting parity | Stored sky/block values reach the renderer, the shader uses Java's default lightmap curve, mesh-side AO includes non-cubic shape weighting, and current terrain-MVP AO facts now follow Java `BlockStateBase.Cache` behavior. Liquid light sampling from `LiquidBlockRenderer` is still not ported. |
| P2 | Sky neighbor skip-through propagation | Yes | [`049`](../tactical/049-native-sky-source-section-ownership.md), [`lighting topic`](lighting.md) | pending solver parity | Source-section ownership now lives in `SkyLightSectionStorage`, but native still lacks Java `SkyLightEngine.checkNeighborsAfterUpdate(...)` behavior for vertical gaps in light-storage sections. This remains important before live deltas, but it is no longer the current foliage-top visual blocker. |
| P3 | Live light deltas and light-section dirtying | Yes | [`lighting topic`](lighting.md), [`026`](../tactical/026-lighting-pipeline.md), [`031`](../tactical/031-native-section-block-delta-updates.md) | pending larger subsystem | Section block deltas currently leave light payloads unchanged. Correct live lighting needs Java-shaped light propagation/deltas and render dirtying by changed light sections. |
| P4 | Cancellable render compile tasks | Yes | [`034`](../tactical/034-native-render-compile-revisions-and-priority.md), [`033`](../tactical/033-native-async-render-section-compile-queue.md) | deferred | Native now avoids stale queued backlog and accepts unchanged sections. This remains useful for stress-orbit streaming, but current radius-5 evidence no longer puts render compile cancellation ahead of startup presentation or lighting parity. |
| P5 | GPU upload budgeting and buffer reuse | Broadly | [`024`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md), [`030`](../tactical/030-native-streaming-publish-and-render-budget.md) | conditional | Native uploads changed sections incrementally, and movement probes show upload cost is small. Do this when probes show upload/allocation cost is material again. |
| P6 | Release perf budgets and durable records | Native policy | [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md), [`030`](../tactical/030-native-streaming-publish-and-render-budget.md), [`033`](../tactical/033-native-async-render-section-compile-queue.md), [`034`](../tactical/034-native-render-compile-revisions-and-priority.md), [`../performance-records.md`](../performance-records.md) | ongoing | Once baselines stabilize, add budget thresholds that catch regressions without failing on normal host noise. |
| PX | Quest RD10 serial CPU+GPU render frame | Native policy | [`099`](../tactical/099-android-xr-rd10-render-cost-attribution.md), [`106`](../tactical/106-android-xr-static-render-cpu-reduction.md), [`107`](../tactical/107-xr-stereo-uniform-ownership-and-multiview.md) | active; per-eye-submit correctness restored by `d0c5161`; Slice C code landed pending headset visual check | E1 (historical GPU timestamps) shows RD10 is a balanced serial `CPU(~9ms)+GPU(~10.7ms)` frame — the Meta `7ms` GPU counter under-reported and the poll wait is real GPU. The unsafe single-submit path was reverted after a deterministic left-eye uniform lifetime bug. Ordered next steps: confirm the slotted chunk, sky, actors, GUI, outline, and screen-effect paths in headset on the per-eye-submit baseline, prove one-submit left/right correctness, then pursue multiview as the real GPU-side reduction. CPU/GPU overlap remains optional and must be re-measured on a correct stereo path. Quest-only lane, separate from the desktop priority order above. |

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
- Shared initial light batch:
  [`045`](../tactical/045-native-shared-initial-light-batch.md)
- Retained initial light world:
  [`046`](../tactical/046-native-retained-initial-light-world.md)
- Light graph drain instrumentation and mixed graph map optimization:
  [`047`](../tactical/047-native-light-graph-drain-instrumentation.md)
- Empty-section light setup:
  [`048`](../tactical/048-native-sky-empty-section-light-setup.md)
- Sky source-section ownership:
  [`049`](../tactical/049-native-sky-source-section-ownership.md)
- Leaf sky render parity:
  [`050`](../tactical/050-native-leaf-sky-render-parity.md)
- LightTexture render parity:
  [`051`](../tactical/051-native-light-texture-render-parity.md)
- Model AO render parity:
  [`052`](../tactical/052-native-model-ao-render-parity.md)
- Non-cubic AO render parity:
  [`053`](../tactical/053-native-non-cubic-ao-render-parity.md)
- Block render-facts parity:
  [`054`](../tactical/054-native-block-render-facts-parity.md)

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
- [`096-android-xr-quest-performance.md`](../tactical/096-android-xr-quest-performance.md)
- [`099-android-xr-rd10-render-cost-attribution.md`](../tactical/099-android-xr-rd10-render-cost-attribution.md)
- [`106-android-xr-static-render-cpu-reduction.md`](../tactical/106-android-xr-static-render-cpu-reduction.md)
- [`107-xr-stereo-uniform-ownership-and-multiview.md`](../tactical/107-xr-stereo-uniform-ownership-and-multiview.md)

Related subsystem tacticals:

- [`021-scheduled-fluid-ticks.md`](../tactical/021-scheduled-fluid-ticks.md)
- [`026-lighting-pipeline.md`](../tactical/026-lighting-pipeline.md)
- [`028-headless-window-frame-unification.md`](../tactical/028-headless-window-frame-unification.md)
- [`044-native-light-status-worker-and-disable-flag.md`](../tactical/044-native-light-status-worker-and-disable-flag.md)
- [`045-native-shared-initial-light-batch.md`](../tactical/045-native-shared-initial-light-batch.md)
- [`046-native-retained-initial-light-world.md`](../tactical/046-native-retained-initial-light-world.md)
- [`047-native-light-graph-drain-instrumentation.md`](../tactical/047-native-light-graph-drain-instrumentation.md)
- [`048-native-sky-empty-section-light-setup.md`](../tactical/048-native-sky-empty-section-light-setup.md)
- [`049-native-sky-source-section-ownership.md`](../tactical/049-native-sky-source-section-ownership.md)
- [`050-native-leaf-sky-render-parity.md`](../tactical/050-native-leaf-sky-render-parity.md)
- [`051-native-light-texture-render-parity.md`](../tactical/051-native-light-texture-render-parity.md)
- [`052-native-model-ao-render-parity.md`](../tactical/052-native-model-ao-render-parity.md)
- [`053-native-non-cubic-ao-render-parity.md`](../tactical/053-native-non-cubic-ao-render-parity.md)

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
