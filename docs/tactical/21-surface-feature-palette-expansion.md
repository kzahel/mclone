# Tactical 21 — Surface-feature palette expansion

Keep the generated-world bridge from tactical 20, but broaden the renderer-visible block palette beyond bare terrain and fluids. This slice ports the first vegetation/simple-feature block classes, tint/render-layer wiring, and smoke-scene placement so generated terrain can render trees and plants instead of reading as a heightmap with snow/water accents.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/block/BushBlock.java` | `src/world/level/block/bush-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/LeavesBlock.java` | `src/world/level/block/leaves-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/CactusBlock.java` | `src/world/level/block/cactus-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/SugarCaneBlock.java` | `src/world/level/block/sugar-cane-block.ts` |
| `reference/.../src/net/minecraft/client/color/block/BlockColors.java` | `src/renderer/block/block-colors.ts` |
| `reference/.../src/net/minecraft/client/renderer/ItemBlockRenderTypes.java` | `src/renderer/item-block-render-types.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/properties/BlockStateProperties.java` (`AGE_15`, `DISTANCE`, `PERSISTENT`) | `src/world/level/block/state/properties/block-state-properties.ts` |
| `reference/.../src/net/minecraft/world/level/block/Blocks.java` (`oak_log`, `oak_leaves`, `grass`, `fern`, `dandelion`, `oak_sapling`, `cactus`, `sugar_cane`) | `src/world/level/generated-render-blocks.ts` |
| extracted blockstate/model/texture assets for the same blocks | `src/world/level/generated-render-blocks.ts`, `src/renderer/main.ts` |

## What landed

- `BushBlock`, `LeavesBlock`, `CactusBlock`, and `SugarCaneBlock` now exist as translated renderer-facing block classes with the state fields this slice needs.
- `BlockStateProperties` grew the vanilla `AGE_15`, `DISTANCE`, and `PERSISTENT` properties so those classes can keep their real default states.
- `ItemBlockRenderTypes` now mirrors the vanilla leaves special-case via `setFancy(...)`, letting `LeavesBlock` resolve to `cutoutMipped()` in the generated-world path.
- `BlockColors.createDefault()` now covers the first plant/foliage consumers from vanilla: `grass`, `fern`, `oak_leaves`, and `sugar_cane`.
- `generated-render-blocks.ts` now registers the first simple-feature block palette (`oak_log`, `oak_leaves`, `grass`, `fern`, `dandelion`, `oak_sapling`, `cactus`, `sugar_cane`) and preloads the corresponding extracted models and atlas sprites.
- `main.ts` now decorates the generated smoke terrain with a small oak canopy cluster plus simple plants/cactus/sugar cane so the browser frame visibly exercises the expanded palette on top of real terrain.

## Scope choice

- Landed here: block/state/tint/render-layer support for the first vegetation/simple-feature blocks, plus browser-harness placement on generated terrain.
- Explicitly deferred: the real worldgen feature pipeline (`Feature`, `ConfiguredFeature`, `PlacedFeature`, tree decorators, biome decoration steps), plant survival/update logic, leaf decay, cactus/sugar-cane growth, and any collision/shape-accurate meshing work.

## Oracle / done-when

**Unit (Vitest):**

- `BlockColors.createDefault()` colors grass/foliage feature blocks the same way the translated helpers do.
- `ItemBlockRenderTypes` routes the new generated feature palette through the expected layers (`solid`, `cutout`, `cutoutMipped`).
- Existing renderer/worldgen suites still pass.

**Browser smoke (Playwright + system Chrome):**

- The final frame still submits `solid`, `cutout`, and `translucent` chunk layers.
- The screenshot now visibly includes feature blocks from the expanded palette on top of generated terrain.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 21 as done

## Next

Tactical 22: translated feature placement bridge. The renderer can now draw the first vegetation/simple-feature blocks, but the browser frame still places them manually in the smoke harness. The next slice should port the narrowest real worldgen feature path needed to replace those harness decorations with translated placement output, starting with a minimal `Feature` / `ConfiguredFeature` / `PlacedFeature` bridge for trees and simple vegetation.
