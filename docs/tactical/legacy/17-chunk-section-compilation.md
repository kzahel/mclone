# Tactical 17 — Chunk/section compilation infrastructure

Port Minecraft's section compilation path so the renderer can build many block sections through the vanilla chunk pipeline instead of relying on a single handcrafted smoke mesh. This slice stops at section rebuilds and uploads; camera frustum submission and the full world render loop remain for tactical 18.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/client/renderer/chunk/RenderChunkRegion.java` | `src/renderer/chunk/render-chunk-region.ts` |
| `reference/.../src/net/minecraft/client/renderer/chunk/VisGraph.java` | `src/renderer/chunk/vis-graph.ts` |
| `reference/.../src/net/minecraft/client/renderer/chunk/VisibilitySet.java` | `src/renderer/chunk/visibility-set.ts` |
| `reference/.../src/net/minecraft/client/renderer/ChunkBufferBuilderPack.java` | `src/renderer/chunk-buffer-builder-pack.ts` |
| `reference/.../src/net/minecraft/client/renderer/ViewArea.java` | `src/renderer/view-area.ts` |
| `reference/.../src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java` | `src/renderer/chunk/chunk-render-dispatcher.ts` |
| `reference/.../src/net/minecraft/client/renderer/ItemBlockRenderTypes.java` | `src/renderer/item-block-render-types.ts` |
| `reference/.../src/net/minecraft/util/Mth.java` (`floor`, `intFloorDiv`) | `src/util/mth.ts` |
| `reference/.../src/net/minecraft/core/SectionPos.java` | `src/core/section-pos.ts` |
| `reference/.../src/net/minecraft/world/level/chunk/LevelChunk.java` | `src/world/level/chunk/level-chunk.ts` |

## What landed

- `VisibilitySet` and `VisGraph` now match the vanilla section-occlusion logic closely enough to resolve flood-filled face visibility for a 16x16x16 section.
- `RenderChunkRegion` now builds the padded region cache that `ChunkRenderDispatcher.RebuildTask` expects, using a minimal section-backed `StaticRenderLevel` and `LevelChunk` to expose chunk-local block access in the smoke harness.
- `ChunkBufferBuilderPack`, `ViewArea`, and the first `ChunkRenderDispatcher` task path are in place, including `RenderChunk`, rebuild tasks, visibility-set capture, per-layer `BufferBuilder` finalization, and immediate WebGPU uploads into `VertexBuffer`.
- The browser smoke scene now populates a small multi-chunk level, compiles it through `ViewArea -> ChunkRenderDispatcher`, and draws each compiled section with a per-chunk `ChunkOffset` bind group instead of uploading one handcrafted mesh.
- The smoke harness now uses `AirBlock` for empty cells, which keeps air out of the model path and prevents missing-texture checkerboards from being baked into empty space.

## Scope choice

- Landed here: solid block-model section rebuilds, section visibility graphs, padded render regions, per-layer buffer packs, and enough level/chunk scaffolding to exercise the real compile path.
- Explicitly deferred: fluids, block entities, true asynchronous worker orchestration, frustum culling, transparency resort in a live world, and the higher-level `LevelRenderer`/camera loop. Those stay with tactical 18.

## Oracle / done-when

**Unit (Vitest):**

- `VisGraph` reports full visibility for sparse sections and no inter-face visibility for fully opaque sections.
- `RenderChunkRegion.createIfNotEmpty(...)` skips empty chunk neighborhoods and caches padded block states for populated ones.
- `ChunkBufferBuilderPack` allocates one `BufferBuilder` per chunk render layer.
- `ChunkRenderDispatcher` compiles a populated section into a non-empty solid layer with the expected `BLOCK` vertex format and a populated `VisibilitySet`.

**Browser smoke (Playwright + system Chrome):**

- The smoke scene compiles multiple sections through `ChunkRenderDispatcher` and draws them via per-chunk `ChunkOffset` uniforms.
- The canvas center pixel matches the expected shaded orange-wool texel.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 17 as done

## Next

Tactical 18 is now documented in [`18-camera-driven-world-frame.md`](18-camera-driven-world-frame.md). The next slice after that is tactical 19: bridge worldgen and the renderer so the camera-driven frame path stops drawing the static smoke level and starts moving across real generated chunk data.
