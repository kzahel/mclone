# Tactical 20 — Surface-special blocks and biome tint

Keep the generated-world bridge from tactical 19, but stop dropping the first overworld surface-special cases. This slice ports the minimal biome-tint path plus liquid and snow-layer rendering so generated terrain can show tinted grass, water, and snow instead of only opaque cube-model terrain.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/client/color/block/BlockColors.java` | `src/renderer/block/block-colors.ts` |
| `reference/.../src/net/minecraft/client/renderer/BiomeColors.java` | `src/renderer/biome-colors.ts` |
| `reference/.../src/net/minecraft/client/renderer/ItemBlockRenderTypes.java` | `src/renderer/item-block-render-types.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/LiquidBlockRenderer.java` | `src/renderer/block/liquid-block-renderer.ts` |
| `reference/.../src/net/minecraft/world/level/block/LiquidBlock.java` | `src/world/level/block/liquid-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/SnowLayerBlock.java` | `src/world/level/block/snow-layer-block.ts` |
| `reference/.../src/net/minecraft/world/level/GrassColor.java` | `src/world/level/grass-color.ts` |
| `reference/.../src/net/minecraft/world/level/FoliageColor.java` | `src/world/level/foliage-color.ts` |
| `reference/.../src/net/minecraft/world/level/biome/Biome.java` | `src/worldgen/biome/biome.ts`, `src/worldgen/biome/biome-data.ts` |
| `reference/.../extracted/assets/minecraft/textures/colormap/grass.png` | browser colormap load in `src/renderer/main.ts` |
| `reference/.../extracted/assets/minecraft/textures/colormap/foliage.png` | browser colormap load in `src/renderer/main.ts` |

## What landed

- `BlockGetter`/`BlockAndTintGetter` now expose fluid states and typed color resolvers, and `GeneratedRenderLevel` resolves biome tint directly from the translated biome-zoom path.
- `Biome`, `biome-data.ts`, `GrassColor`, and `FoliageColor` now carry the visual metadata the renderer needs: temperature/downfall, water color, grass/foliage overrides, and the swamp/dark-forest grass modifiers from vanilla.
- `BlockColors.createDefault()` and `BiomeColors` now tint `grass_block` and water directly from the runtime biome instead of returning untinted fallback colors.
- `LiquidBlock`, `SnowLayerBlock`, `Fluid` / `FluidState` / `Fluids`, and the translated `LiquidBlockRenderer` landed as the first non-cube surface-special render path.
- `generated-render-blocks.ts` now registers `water`, `lava`, and `snow`, exposes the required atlas sprites, and maps those ids into the generated-world palette instead of collapsing them to air.
- `ChunkRenderDispatcher` now emits fluid geometry into the translated chunk-layer system, including the `translucent` layer for water.
- The browser harness now loads vanilla grass/foliage colormaps, uses `BlockColors.createDefault()`, and decorates the smoke terrain with explicit snow/water accents so `cutout` and `translucent` submission paths stay exercised.

## Scope choice

- Landed here: biome tint, water/lava fluid-state plumbing, snow-layer rendering, fluid chunk-layer submission, and smoke/test coverage proving `cutout` and `translucent` paths are alive.
- Explicitly deferred: biome blend radius > 0, flowing-fluid levels, voxel-shape occlusion for partial blocks, foliage/leaves tint consumers beyond the current generated palette, and the rest of the surface-feature block catalog.

## Oracle / done-when

**Unit (Vitest):**

- Existing renderer/worldgen suites stay green.
- Chunk compilation now proves water routes into `RenderType.translucent()` and snow routes into `RenderType.cutout()`.
- The generated-level bridge still matches the pinned chunk oracle after tint/fluid plumbing lands.

**Browser smoke (Playwright + system Chrome):**

- Grass colormaps are loaded before the renderer scene boots.
- The final frame submits `solid`, `cutout`, and `translucent` chunk layers.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 20 as done

## Next

Tactical 21 is now complete in [`21-surface-feature-palette-expansion.md`](21-surface-feature-palette-expansion.md). The generated-world smoke frame now renders the first vegetation/simple-feature block palette (`oak_log`, `oak_leaves`, plants, cactus, sugar cane) on top of generated terrain instead of only terrain blocks.
