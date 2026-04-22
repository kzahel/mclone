# Tactical 15 — First block tesselation path

Port the first baked-block meshing path from Minecraft's client renderer: `ModelBlockRenderer`, `BlockRenderDispatcher`, the CPU-side `VertexConsumer.putBulkData(...)` path those classes rely on, and the minimal level/light helpers needed to emit textured block vertices through `BufferBuilder`. This slice intentionally stops at model blocks; `LiquidBlockRenderer` stays deferred.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/client/renderer/block/ModelBlockRenderer.java` | `src/renderer/block/model-block-renderer.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/BlockRenderDispatcher.java` | `src/renderer/block/block-render-dispatcher.ts` |
| `reference/.../src/com/mojang/blaze3d/vertex/VertexConsumer.java` | `src/renderer/vertex/vertex-consumer.ts` |
| `reference/.../src/net/minecraft/client/renderer/LevelRenderer.java` (`getLightColor`) | `src/renderer/level-renderer.ts` |
| `reference/.../src/net/minecraft/world/level/BlockAndTintGetter.java` | `src/world/level/block-and-tint-getter.ts` |
| `reference/.../src/net/minecraft/core/BlockPos.java` | `src/core/block-pos.ts` |
| `reference/.../src/net/minecraft/util/Mth.java` (`getSeed`) | `src/util/mth.ts` |

## What landed

- `ModelBlockRenderer` now ports both `tesselateWithAO(...)` and `tesselateWithoutAO(...)`, including the AO adjacency tables, quad-shape calculation, cached light/brightness lookups, and `putQuadData(...)` bulk vertex emission.
- `BlockRenderDispatcher` now resolves baked block models from `BlockModelShaper` and emits model-block geometry through the translated tesselation path.
- Minimal level/light support landed for the mesher: mutable `BlockPos`, `BlockAndTintGetter`, `StaticBlockAndTintGetter`, `LightLayer`, `LevelRenderer.getLightColor(...)`, and the `BlockState`/`BlockBehaviour` hooks `ModelBlockRenderer` calls.
- `VertexConsumer.putBulkData(...)` now matches the Java matrix-transform path closely enough for baked-quads to flow through `BufferBuilder` without handwritten vertex packing.
- The browser smoke scene now preloads extracted blockstate/model JSON, bakes vanilla `minecraft:orange_wool` and `minecraft:stone`, and renders them via `BlockRenderDispatcher -> BufferBuilder -> VertexBuffer -> WebGPU`.

## Scope choice

- Landed here: model-block tesselation, AO/flat-light branching, block-model browser smoke.
- Explicitly deferred: `LiquidBlockRenderer`, chunk-region iteration, and section/chunk compilation. Those remain for later slices once the real shader set is in place.

## Oracle / done-when

**Unit (Vitest):**

- An isolated `minecraft:stone` block tesselates into `24` BLOCK-format vertices and `36` indices.
- A neighboring solid block culls the shared face, reducing the emitted geometry to `20` vertices and `30` indices.
- Under asymmetric corner occlusion, AO writes per-vertex color variation on the exposed top face while an emissive block of the same model stays flat-lit.

**Browser smoke (Playwright + system Chrome):**

- The smoke scene renders a large centered orange wool block face and a smaller rotated stone cube through the real block tesselation path.
- The canvas center pixel matches the expected shaded orange-wool texel.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm test` passes
- `pnpm typecheck` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 15 as done

## Next

Tactical 16 is now documented in [`16-core-wgsl-shaders.md`](16-core-wgsl-shaders.md). The next slice after that is tactical 17: chunk/section compilation infrastructure (`RenderChunkRegion`, `ChunkBufferBuilderPack`, `VisGraph`, `ViewArea`, `ChunkRenderDispatcher`) so the renderer can move from one handcrafted smoke mesh to real section builds.
