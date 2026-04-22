# Tactical 14b — Block model baking core

Port the bake-time half of Minecraft's block model stack: `BakedQuad`, `BakedModel`, `SimpleBakedModel`, `FaceBakery`, `BlockModelRotation`, the minimal `ModelBakery` bake path, and `BlockModelShaper` state-to-model lookup. This slice turns resolved unbaked block models into baked quad arrays, but it still stops short of blockstate-variant JSON resolution and meshing.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/client/renderer/block/model/BakedQuad.java` | `src/renderer/model/baked-quad.ts` |
| `reference/.../src/net/minecraft/client/resources/model/BakedModel.java` | `src/renderer/model/baked-model.ts` |
| `reference/.../src/net/minecraft/client/resources/model/SimpleBakedModel.java` | `src/renderer/model/simple-baked-model.ts` |
| `reference/.../src/net/minecraft/client/resources/model/BuiltInModel.java` | `src/renderer/model/built-in-model.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/FaceBakery.java` | `src/renderer/model/face-bakery.ts` |
| `reference/.../src/net/minecraft/client/renderer/FaceInfo.java` | `src/renderer/model/face-info.ts` |
| `reference/.../src/net/minecraft/client/resources/model/ModelState.java` | `src/renderer/model/model-state.ts` |
| `reference/.../src/net/minecraft/client/resources/model/BlockModelRotation.java` | `src/renderer/model/block-model-rotation.ts` |
| `reference/.../src/net/minecraft/client/resources/model/ModelResourceLocation.java` | `src/renderer/model/model-resource-location.ts` |
| `reference/.../src/net/minecraft/client/resources/model/ModelManager.java` | `src/renderer/model/model-manager.ts` |
| `reference/.../src/net/minecraft/client/resources/model/ModelBakery.java` (`bake(...)`, missing-model handling) | `src/renderer/model/model-bakery.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/BlockModelShaper.java` | `src/renderer/model/block-model-shaper.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/ItemOverrides.java` | `src/renderer/model/item-overrides.ts` |
| `reference/.../src/net/minecraft/core/Registry.java` (`Registry.BLOCK`, `getKey`, iteration only) | `src/core/registry.ts` |

## What to port and what to skip

### Direct translation (no divergence)

- `BakedQuad` vertex-int layout, direction/tint/shade bookkeeping, and sprite association.
- `BakedModel`, `BuiltInModel`, and `SimpleBakedModel.Builder` structure.
- `FaceInfo` vertex-order tables and `FaceBakery` vertex packing, element rotation, model rotation, facing calculation, and winding recalculation.
- `ModelState`, `BlockModelRotation`, and `ModelResourceLocation`.
- The minimal `ModelBakery` bake path that turns a resolved `BlockModel` into a `BakedModel`.
- `BlockModelShaper.stateToModelLocation(...)` and cached baked-model lookup by `BlockState`.

### Intentionally deferred to 14c

- `BlockModelDefinition`, variant JSON, multipart selectors, and the `ModelBakery.loadModel(...)` top-level blockstate pipeline.
- UV-lock handling from rotated blockstate variants.
- `ItemModelGenerator` and generated-item quad extrusion (`builtin/generated` remains parsed but not baked).

## Oracle / done-when

**Unit (Vitest, no browser):**

- Baking `minecraft:block/stone` produces six culled quads, zero unculled quads, and a `BakedQuad` vertex array of length `32`.
- The baked north face of the stone cube preserves Minecraft's packed vertex order: `(1,1,0)`, `(1,0,0)`, `(0,0,0)`, `(0,1,0)` with the corresponding full-face UVs.
- `ModelBakery` caches the baked result for the same model/state tuple.
- `BlockModelShaper.stateToModelLocation(...)` formats property strings exactly like vanilla (`axis=x`, `axis=y`, etc.).
- `BlockModelShaper.rebuildCache()` returns the baked model registered for each `BlockState`.

## Done when

- `pnpm test` passes
- `pnpm typecheck` passes
- `docs/tactical/README.md` links this slice and marks tactical 14b as done

## Next

Tactical 14c: blockstate variant and multipart resolution. Port `BlockModelDefinition`, `MultiVariant`, multipart selectors, and the relevant `ModelBakery.loadModel(...)` path so real `BlockState -> ModelResourceLocation -> baked model` resolution works without manual cache population.
