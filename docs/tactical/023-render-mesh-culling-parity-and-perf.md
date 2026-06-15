# 023: Render Mesh Culling Parity And Perf

Status: completed first pass.

## Purpose

Follow the native movement/culling smoke with a concrete renderer-mesh correctness pass. The immediate goal is to prove that native meshing is doing vanilla-shaped model-face culling at useful scale, expose face-pressure diagnostics in the native app path, and avoid premature greedy meshing before lighting, AO, liquids, and render layers are better constrained.

This is not a lighting slice and not a persistence slice.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/ModelBlockRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/BlockRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/VisGraph.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/VisibilitySet.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`

Key Java facts for this slice:

- `ChunkRenderDispatcher.RenderChunk.RebuildTask.compile(...)` scans each block in a 16x16x16 render chunk, sends opaque blocks to `VisGraph`, and renders block models through `BlockRenderDispatcher.renderBatched(...)`.
- `ModelBlockRenderer` asks the baked model for directional quads and calls `Block.shouldRenderFace(...)` before emitting those face quads.
- Vanilla render chunk compilation emits model quads; it does not greedy-merge neighboring full-cube faces into larger quads in the path we are currently matching.
- `VisGraph` / `VisibilitySet` produce a compiled-chunk face-visibility graph used by Java's smarter render traversal. Native still only has frustum AABB culling.

## Landed In First Pass

- `SectionMeshStats` now exposes a derived quad face count from the existing index count.
- Debug and textured mesh tests now cover a fully solid 16x16x16 section. The expected result is exterior model quads only: `1,536` faces / `9,216` indices, instead of all internal block faces.
- Textured mesh tests now cover two adjacent fully solid chunks and assert that the shared chunk-boundary side is culled across chunk inputs.
- Native movement perf JSON now reports `loaded_faces` and `visible_faces` alongside section/index counts.
- Native window diagnostics now include drawn/loaded face counts so frame-visible draw pressure is visible while flying.

## Current Limits

- Native still performs model-face culling, not greedy meshing. Greedy meshing should remain an optional optimization decision after lighting/AO/liquid/render-layer constraints are clearer.
- Native still does frustum AABB culling only. It does not yet port Java's `VisGraph` / `VisibilitySet` smart traversal.
- Dirty-only section diffs landed in [`024-render-section-dirty-cache-and-upload-diffs.md`](024-render-section-dirty-cache-and-upload-diffs.md), but dirty scope is still chunk-wide and conservative.
- No fixed release-mode budget is enforced yet; movement perf still records diagnostics rather than failing on budget thresholds.

## Gates

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-mesh -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-render -- --nocapture
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:movement:smoke
```

## Follow-Ups

1. Port a native `VisGraph` / `VisibilitySet` data structure and add section-level occlusion traversal tests against Java's face-visibility semantics.
2. Tighten dirty rebuild scope from whole chunks to individual render sections once block/light/liquid deltas carry section coordinates.
3. Add release-mode movement/render perf budgets once debug counters stabilize and the release baseline is refreshed.
