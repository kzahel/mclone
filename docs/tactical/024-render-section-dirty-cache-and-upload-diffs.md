# 024: Render Section Dirty Cache And Upload Diffs

Status: completed first pass.

## Purpose

Stop treating every visible chunk update as a full render-section rebuild/upload. Keep a CPU-side render-section cache keyed by `RenderSectionKey`, rebuild only sections affected by changed chunks, and upload only changed/removed sections to the GPU.

This slice keeps persistence, lighting, greedy meshing, and Java `VisGraph` traversal deferred.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java`

Key Java facts for this slice:

- `ChunkRenderDispatcher.RenderChunk` owns compiled buffers for one 16x16x16 render section.
- Dirty render chunks are rebuilt independently; Java does not rebuild all visible chunks after every chunk update.
- Neighboring compiled chunks matter at boundaries: when a chunk loads/unloads or changes, adjacent chunk faces may become visible or hidden.
- `LevelRenderer` keeps a current visible render list separate from compiled chunk ownership.

## Landed In First Pass

- Native `WindowSceneRuntime` now owns a CPU-side textured render-section cache keyed by `RenderSectionKey`.
- `ServerUpdate::ChunkSnapshot` and `ServerUpdate::ChunkUnload` mark the changed chunk plus four horizontal neighbors dirty.
- Dirty rebuild uses all currently loaded chunk inputs for neighbor-aware face culling, but only emits sections for the dirty target chunks.
- Unloaded or newly empty dirty chunks remove stale cached render-section keys.
- `TexturedSectionDrawResources` now supports incremental section updates: changed section uploads plus explicit section removals.
- Native diagnostics now report rebuilt sections/vertices/faces/indices and uploaded sections/vertices/faces/indices.
- `mclone-native-client --movement-perf` records CPU rebuilt-section pressure separately from total loaded/visible draw pressure.

## Landed Follow-Up

- `ServerUpdate::SectionBlockUpdates` now marks the changed render section and
  direct block-boundary neighbor sections dirty, matching Java's
  `LevelRenderer.setBlockDirty(...)` / `setSectionDirtyWithNeighbors(...)`
  shape more closely than chunk-wide dirtying.
- Section-delta rebuild work is budgeted by affected chunk for now, but the
  rebuilt target set is section-precise.
- Removed or empty dirty sections can now be removed from the CPU cache and GPU
  draw resources without requiring a whole dirty chunk.

## Current Limits

- Full `ChunkSnapshot` and `ChunkUnload` updates still use conservative
  chunk-neighborhood dirtying.
- Live block deltas are section-precise, but light deltas are not published yet,
  so light-section dirtying remains deferred to the lighting pipeline.
- GPU buffers are still recreated for changed sections rather than updated in-place or pooled.
- No release-mode perf budget is enforced yet.
- CPU render-section compilation has moved off the frame path in
  [`033`](033-native-async-render-section-compile-queue.md), but stale worker
  output is still handled conservatively.

## Optimized Dev Baseline

`pnpm native:movement:smoke` on June 15, 2026, after enabling native `profile.dev.opt-level = 2`, with seed `12345`, chunk radius `1`, and a 12-step circular path:

- initial load rebuilt 55 sections, 199,364 vertices, 49,841 faces, and 299,046 indices
- later chunk moves rebuilt 53-70 sections and removed 30-55 stale sections per step
- loaded draw pressure was 33,966-56,039 faces, while frustum-visible pressure was 6,863-22,315 faces
- remesh time was roughly 9.9-13.6 ms per movement step after the initial load
- total 12-step smoke time was 1,727 ms; post-bootstrap movement steps were roughly 45-79 ms each

Treat these as optimized-dev diagnostics, not final budgets. Refresh release-mode numbers before enforcing thresholds.

## Gates

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-mesh -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-render -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client -- --nocapture
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:movement:smoke
```

## Validation Notes

120 Hz release frame-budget probe after section-precise dirtying:

- command: `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --frame-budget-probe --frame-budget-frames 120 --target-hz 120`
- result: `0 / 120` over-budget frames, p95 `4.093 ms`, p99 `5.146 ms`,
  max `5.430 ms`
- section work: `27` section block update batches, `80` fluid-mutated blocks,
  `79` total rebuilt sections, max `16` rebuilt sections on a frame
- visual check: `/tmp/mclone-section-dirty-verify.png`

## Follow-Ups

1. Refresh release-mode movement/render baselines and decide budget thresholds
   for rebuilt/uploaded sections and vertices.
2. Reduce async compile stale churn with per-section/chunk input revisions and
   distance priority.
3. Add GPU buffer reuse/pooling if upload allocation becomes material after
   dirty diffs.
4. Add light deltas and light-section dirtying through the lighting pipeline.
