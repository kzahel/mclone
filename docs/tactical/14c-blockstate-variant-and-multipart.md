# Tactical 14c — Blockstate variant and multipart resolution

Port the blockstate-loading half of Minecraft's model stack: `BlockModelDefinition`, `Variant`, `MultiVariant`, multipart `Condition`/`Selector`, `WeightedBakedModel`, `MultiPartBakedModel`, and the `ModelBakery.loadModel(...)` path that turns `ModelResourceLocation` blockstate entries into baked models. This slice connects real `BlockState` values to the baked-model output from tactical 14b.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/client/renderer/block/model/BlockModelDefinition.java` | `src/renderer/model/block-model-definition.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/Variant.java` | `src/renderer/model/variant.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/MultiVariant.java` | `src/renderer/model/multi-variant.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/multipart/Condition.java` | `src/renderer/model/multipart/condition.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/multipart/KeyValueCondition.java` | `src/renderer/model/multipart/key-value-condition.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/multipart/AndCondition.java` | `src/renderer/model/multipart/and-condition.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/multipart/OrCondition.java` | `src/renderer/model/multipart/or-condition.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/multipart/Selector.java` | `src/renderer/model/multipart/selector.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/multipart/MultiPart.java` | `src/renderer/model/multipart/multi-part.ts` |
| `reference/.../src/net/minecraft/client/resources/model/WeightedBakedModel.java` | `src/renderer/model/weighted-baked-model.ts` |
| `reference/.../src/net/minecraft/client/resources/model/MultiPartBakedModel.java` | `src/renderer/model/multi-part-baked-model.ts` |
| `reference/.../src/net/minecraft/client/resources/model/ModelBakery.java` (`loadModel(...)` blockstate path) | `src/renderer/model/model-bakery.ts` |
| `reference/.../src/net/minecraft/core/BlockMath.java` | `src/renderer/model/block-math.ts` |

## What landed

- Blockstate JSON now parses into vanilla-shaped `BlockModelDefinition`, `Variant`, `MultiVariant`, and multipart selector graphs.
- `ModelBakery.getModel(...)` now resolves `ModelResourceLocation` blockstate entries through extracted `blockstates/*.json`, including empty-variant keys and partial property predicates.
- `WeightedBakedModel` and `MultiPartBakedModel` are wired into the bake path so weighted variants and multipart selectors produce the same baked-model wrapper structure as vanilla.
- `FaceBakery` now handles rotated `uvlock` variants through a direct `BlockMath`-style UV recompute path, which multipart fence arms need immediately.
- `ModelBakery.bakeTopLevelBlockModels(...)` populates `ModelManager` from registered block states, so `BlockModelShaper` can resolve real `BlockState -> baked model` mappings without manual cache seeding.

## Oracle / done-when

**Unit (Vitest, no browser):**

- `minecraft:stone` resolves the empty `""` variant key into a `WeightedBakedModel`, and different random selections produce different baked UV layouts.
- `minecraft:oak_log` state variants (`axis=x`, `axis=y`) resolve through `ModelBakery` and populate `BlockModelShaper` with distinct baked quad layouts.
- `minecraft:oak_fence` multipart selectors honor boolean property predicates and return more geometry for connected states than isolated states.
- Rotated multipart fence arms bake without throwing on `uvlock: true`.

## Done when

- `pnpm test` passes
- `pnpm typecheck` passes
- `docs/tactical/README.md` links this slice and marks tactical 14c as done

## Next

Tactical 15: [first block tesselation path](15-block-tesselation-path.md). Port `ModelBlockRenderer` and `BlockRenderDispatcher`, emit AO/no-AO block vertices through `BufferBuilder`, and render a small baked-block arrangement in the browser harness for immediate visual inspection.
