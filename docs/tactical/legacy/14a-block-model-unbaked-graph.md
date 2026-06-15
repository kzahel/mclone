# Tactical 14a — Block model parse and unbaked graph

Port the JSON/model-loading half of Minecraft's block model stack: `BlockModel`, `BlockElement`, `BlockElementFace`, `BlockFaceUV`, item-display metadata, and the parent/resource-resolution path from `ModelBakery.loadBlockModel(...)`. This slice stops before baking; it produces resolved unbaked model graphs and texture-slot lookup, not `BakedQuad` output.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/client/renderer/block/model/BlockModel.java` | `src/renderer/model/block-model.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/BlockElement.java` | `src/renderer/model/block-element.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/BlockElementFace.java` | `src/renderer/model/block-element-face.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/BlockFaceUV.java` | `src/renderer/model/block-face-uv.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/BlockElementRotation.java` | `src/renderer/model/block-element-rotation.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/ItemTransform.java` | `src/renderer/model/item-transform.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/ItemTransforms.java` | `src/renderer/model/item-transforms.ts` |
| `reference/.../src/net/minecraft/client/renderer/block/model/ItemOverride.java` | `src/renderer/model/item-override.ts` |
| `reference/.../src/net/minecraft/client/resources/model/UnbakedModel.java` | `src/renderer/model/unbaked-model.ts` |
| `reference/.../src/net/minecraft/client/resources/model/Material.java` | `src/renderer/model/material.ts` |
| `reference/.../src/net/minecraft/client/resources/model/ModelBakery.java` (`loadBlockModel(...)`, builtin markers) | `src/renderer/model/block-model-repository.ts` |
| `reference/.../extracted/assets/minecraft/models/**/*.json` | unit oracle inputs via `test/renderer/model/block-model-unbaked.test.ts` |

## What to port and what to skip

### Direct translation (no divergence)

- `BlockFaceUV` rotation rules, optional-UV handling, `getU/getV/getReverseIndex`, and missing-UV fill.
- `BlockElementFace`, `BlockElementRotation`, and `BlockElement`, including element bounds checks, per-face lookup, shade defaulting, and UV derivation from `from/to`.
- `BlockModel` parse surface: parent, textures, ambient occlusion, gui light, item display transforms, overrides, dependency listing, `getElements()`, `getTransforms()`, `getMaterial()`, and texture-reference chasing.
- `ItemTransform`, `ItemTransforms`, and `ItemOverride` as pure data/JSON parse types.
- `Material` and `UnbakedModel` as the minimal client-model-layer counterparts the unbaked graph uses.
- The relevant `ModelBakery.loadBlockModel(...)` behavior: builtin `generated` / `entity` markers, builtin missing-model mesh, cache-backed model loading, and recursive parent linking.

### Intentionally deferred to 14b

- `BlockModel.bake(...)`, `FaceBakery`, `BakedModel`, `SimpleBakedModel`, `ItemOverrides`, and any quad emission.
- `ModelResourceLocation`, blockstate variant parsing, multipart selectors, and `BlockModelShaper`.
- Item-model layer extrusion from `ItemModelGenerator`; this slice only preserves the `builtin/generated` marker and inherited texture/display metadata.

## Oracle / done-when

**Unit (Vitest, no browser):**

- Parsing `minecraft:block/cube` fills omitted face UVs to the expected `[0, 0, 16, 16]` cube layout.
- Parsing a real rotated model (`minecraft:block/small_dripleaf_top`) preserves the element rotation axis/angle/origin and face-UV rotation metadata.
- Resolving `minecraft:block/stone` through its extracted parent chain (`stone -> cube_all -> cube -> block`) yields inherited elements and resolves `particle` / face textures to `minecraft:block/stone`.
- Resolving `minecraft:block/oak_log` through `cube_column` resolves `particle`, side, and end textures through chained `#side` / `#end` references.
- Resolving `minecraft:item/cooked_porkchop` through `minecraft:item/generated -> builtin/generated` inherits gui light and display transforms, with translation scaling preserved.

## Done when

- `pnpm test` passes
- `pnpm typecheck` passes
- `docs/tactical/README.md` links this slice and marks tactical 14a as done

## Next

Tactical 14b: [`14b-block-model-baking.md`](14b-block-model-baking.md) — bake resolved block models into renderable quad data. Port the `ModelBakery` baking pass, `SimpleBakedModel`, `BakedQuad` int-array layout, and the `BlockModelShaper` lookup path so we can diff baked quads per blockstate before wiring them into the mesher.
