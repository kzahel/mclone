# 012: Model Baking And Atlas

Status: completed.

## Purpose

Create the native renderer-asset boundary that section meshing can consume next: Java-shaped blockstate variants, block model parent resolution, texture-slot resolution, baked face facts, and a deterministic texture atlas plan.

This slice stops before generating chunk mesh vertices or submitting textured GPU draws. The output is CPU-side asset data that `013-vanilla-section-meshing.md` can turn into renderable section geometry.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/BlockModelDefinition.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/MultiVariant.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/Variant.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/BlockModel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/BlockElement.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/BlockElementFace.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/BlockFaceUV.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/resources/model/BlockModelRotation.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/resources/model/Material.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/resources/model/ModelBakery.java`

Relevant vanilla shape:

- blockstate variants are one `Variant` object or a non-empty array of variants
- a variant records model location, X/Y model rotation, UV lock, and positive weight
- model parents inherit elements and texture slots
- texture values are either direct resource locations or `#slot` references
- missing face UVs are filled from the element extents and face direction
- block texture materials point at the block atlas

## Scope

Landed:

- `BlockStateVariant` facts with Java defaults for `x`, `y`, `uvlock`, and `weight`
- blockstate asset parsing that preserves variant entries, not only model refs
- block model JSON parsing for parent, textures, elements, faces, cullface, tint index, UVs, UV rotation, shade, and ambient occlusion
- validated model face directions and Java-compatible bounds/default-UV checks
- recursive model parent loading with cycle detection
- texture-slot resolution through parent chains and `#slot` references with cycle detection
- `BakedBlockModel` / `BakedBlockModelFace` CPU facts with resolved `TextureMaterial`s
- texture PNG dimension probing and deterministic row-packed atlas planning
- real extracted-asset test that bakes the current terrain MVP model refs and builds an atlas plan from the resolved materials

Kept out:

- multipart selector evaluation
- element rotations and full quad transformation math
- weighted model selection at runtime
- render-layer classification beyond material collection
- actual atlas image stitching
- textured section mesh generation and GPU rendering

## Architecture

The renderer asset path is now:

```text
BlockStateRegistry + BlockStateAssetIndex
  -> BlockStateVariant model refs
  -> BlockModelLibrary
  -> BakedBlockModel
  -> TextureAtlasPlan
```

`TextureAtlasPlan` intentionally stores placement and normalized UV ranges without composing a final image yet. The next slice can choose whether to upload separate textures temporarily, stitch an actual CPU image, or build a GPU-side array/atlas implementation while still consuming the same material facts.

## Validation

Required checks:

- `cargo test --workspace`
- `cargo check -p mclone-assets --target wasm32-unknown-unknown`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- `pnpm native:web:smoke`

The asset tests include synthetic parent/texture-reference coverage and native filesystem coverage against `reference/minecraft-1.17.1/extracted` for the current terrain MVP block models and textures.

## Follow-Up

Proceed to `013-vanilla-section-meshing.md`: consume `BakedBlockModel` and `TextureAtlasPlan` from client replica snapshots, generate textured section meshes, split opaque/liquid/cutout facts as needed, and validate with headless native capture plus the browser/WASM smoke gate.
