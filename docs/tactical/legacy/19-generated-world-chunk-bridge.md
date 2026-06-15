# Tactical 19 — Generated world chunk bridge

Replace the static smoke scene with real chunk data from the translated overworld generator, but keep the bridge intentionally minimal: the renderer still consumes a `BlockAndTintGetter`/chunk cache, and the browser harness now moves that cache across generated terrain before drawing the final frame.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/client/multiplayer/ClientChunkCache.java` (`Storage`, `inRange`, camera-centered chunk window) | `src/world/level/generated-render-level.ts` |
| `reference/.../src/net/minecraft/world/level/ChunkPos.java` (conceptual camera-centered chunk addressing) | `src/world/level/generated-render-level.ts` |
| `reference/.../src/net/minecraft/world/level/block/SnowyDirtBlock.java` | `src/world/level/block/snowy-dirt-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/properties/BlockStateProperties.java` (`SNOWY`) | `src/world/level/block/state/properties/block-state-properties.ts` |
| `reference/.../src/net/minecraft/world/level/block/Blocks.java` (`STONE`, `GRASS_BLOCK`, `DIRT`, `BEDROCK`, `SAND`, `GRAVEL`) | `src/world/level/generated-render-blocks.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java` | existing worldgen port, consumed from `src/renderer/main.ts` and `src/world/level/generated-render-level.ts` |

## What landed

- `GeneratedRenderLevel` now owns a camera-centered chunk ring backed by the translated overworld generator, and lazily materializes `LevelChunk`s only inside that window.
- `StaticRenderLevel` grew the minimal chunk-management hooks the moving generator bridge needs: set/remove/iterate loaded chunks and report loaded-chunk counts.
- `SnowyDirtBlock` and `BlockStateProperties.SNOWY` landed so `minecraft:grass_block` resolves the vanilla `snowy=false` blockstate variant instead of baking to the missing-model path.
- `generated-render-blocks.ts` now registers the small model-backed terrain palette used by the bridge (`stone`, `grass_block`, `dirt`, `bedrock`, `sand`, `gravel`) and exposes the atlas/model preload lists that `main.ts` needs.
- `main.ts` now builds a generated overworld scene from `NoiseBasedChunkGenerator`, preloads the corresponding extracted block models and sprites, moves the camera/chunk cache across two chunk centers, and renders the final frame through the existing `GameRenderer -> LevelRenderer -> ChunkRenderDispatcher` path.
- The browser smoke now validates a lower-center terrain pixel instead of the exact canvas midpoint, because the generated scene intentionally leaves sky/fog in the upper half of the frame.

## Scope choice

- Landed here: generator-backed chunk ingestion, a moving chunk cache behind `LevelRenderer`, the grass-block state-definition fix, and a browser smoke that proves generated chunk geometry reaches the GPU after a camera move.
- Explicitly deferred: a full translated `ClientLevel`/`ClientChunkCache`, biome tint/color caches, fluids, snow layers, and any non-model block renderers. Those belong to follow-up slices.

## Oracle / done-when

**Unit (Vitest):**

- `GeneratedRenderLevel` loads the pinned surface oracle for chunk `(0, 0)` into the runtime chunk cache with matching top-surface block names and heights at sampled columns.
- Moving the camera window from chunk `(0, 0)` to `(2, 0)` unloads out-of-range chunks and loads the new edge chunks while keeping the cache size fixed.
- Existing worldgen oracle suites and renderer unit suites still pass unchanged.

**Browser smoke (Playwright + system Chrome):**

- The browser harness renders generated terrain instead of the old handcrafted smoke blocks.
- The chunk cache is refreshed for a second camera position before the final draw.
- A lower-center sample pixel differs from the fog clear color, proving generated chunk geometry reached the final frame.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 19 as done

## Next

Tactical 20 is now complete in [`20-surface-special-blocks-and-biome-tint.md`](20-surface-special-blocks-and-biome-tint.md). The generated-world bridge now keeps biome tint, snow layers, and the first liquid path instead of dropping those surface-special cases on the floor.
