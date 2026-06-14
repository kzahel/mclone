# 013: Vanilla Section Meshing

Status: completed.

## Purpose

Move native rendering from temporary flat block colors to textured section meshes built from client replica chunk snapshots, vanilla blockstate/model assets, and the block atlas facts from `012-model-baking-and-atlas.md`.

This slice keeps the renderer simple and synchronous. It proves the durable data path first:

```text
ClientRuntime chunk snapshots
  -> BlockStateId section blocks
  -> terrain MVP blockstate/model catalog
  -> textured section mesh
  -> stitched atlas upload
  -> native headless/window draw
```

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/BlockRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/ModelBlockRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/BakedQuad.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/FaceBakery.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ItemBlockRenderTypes.java`
- TypeScript reference: `src/renderer/model/block-model.ts`
- TypeScript reference: `src/renderer/model/face-bakery.ts`
- TypeScript reference: `src/renderer/model/face-info.ts`

Relevant vanilla shape:

- chunk rebuild walks 16x16x16 render sections from a client/world view
- fluids and model blocks are rendered through separate paths
- model blocks render baked quads selected from blockstate models
- culled faces are emitted only when the neighbor does not occlude them
- baked quad vertices carry position, UV, tint, shade, and sprite facts
- render-layer choice is separate from block simulation truth

## Scope

Landed:

- `TexturedMeshCatalog` mapping current terrain MVP `BlockStateId`s to baked model faces and atlas UV rectangles
- textured chunk mesh input that preserves full `BlockStateId`s instead of the prior temporary `u8` adapter
- textured section mesh generation from client snapshot facts
- cube-face vertex ordering compatible with the Java/TypeScript `FaceInfo` path
- Java-compatible block-face UV rotation mapping
- neighbor occlusion based on catalog full-cube occluder facts, so no-face blocks do not hide adjacent model faces
- basic face shade and block tint multiplication for model textures such as grass overlays
- native RGBA atlas stitching from the `TextureAtlasPlan`
- WebGPU textured chunk pipeline and shader with nearest atlas sampling and alpha discard
- native headless chunk capture and window mode now use the textured path

Kept out:

- full `FaceBakery` model-state rotations and element rotations
- weighted variant/random model selection
- multipart selector evaluation
- true liquid meshing via `LiquidBlockRenderer`
- separate opaque/cutout/translucent draw sorting
- ambient occlusion and packed light
- streaming section mesh cache and incremental GPU uploads

## Architecture

The new mesh/render boundary is:

```text
mclone-assets
  BlockStateRegistry
  BlockStateAssetIndex
  BlockModelLibrary
  TextureAtlasPlan

mclone-mesh
  TexturedMeshCatalog
  TexturedChunkMeshInput
  TexturedVisibleChunkMesh

mclone-render
  ChunkTextureAtlas
  TexturedChunkDrawResources
```

The native client stitches atlas pixels because it is the native presentation app and can decode PNGs with the native `image` dependency. `mclone-mesh` remains renderer-neutral: it only sees baked faces and normalized atlas UVs.

## Divergence Notes

Java runs model blocks and fluids through separate renderers inside `ChunkRenderDispatcher.RebuildTask.compile(...)`. This slice ports the model block path first. Water currently has no model faces in vanilla assets, so the true liquid renderer remains the next renderer-parity gap rather than being faked as a normal cube in the catalog.

Java `FaceBakery` applies model-state and element rotations. Current terrain MVP block models consumed by this slice are cube-style model elements. Rotation support is intentionally deferred until the next model family requires it, rather than adding untested math before a visible textured path exists.

## Validation

Required checks:

- `cargo test --workspace`
- `cargo check -p mclone-mesh --target wasm32-unknown-unknown`
- `cargo check -p mclone-render --target wasm32-unknown-unknown`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- `cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-native-textured-chunk.png --width 960 --height 640 --chunk-radius 1`
- inspect `/tmp/mclone-native-textured-chunk.png`
- `pnpm native:web:smoke`

## Follow-Up

Proceed to `014-streaming-renderer-and-camera.md`: introduce section-level mesh ownership, visible-section upload caching, invalidation, frustum/camera scenario validation, and a cleaner split between native app orchestration and renderer runtime state.
