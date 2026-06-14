# 011: Block Registry And Asset Source

Status: completed.

## Purpose

Create the native asset/data boundary needed before model baking, texture atlases, and textured section meshing.

This slice does not bake models yet. It establishes the durable primitives that later renderer slices will consume: Minecraft resource locations, pack-relative asset paths, native/web-compatible asset sources, blockstate JSON indexing, and an initial block-state registry for the block ids currently emitted by native worldgen.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/resources/ResourceLocation.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Block.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Blocks.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/state/StateDefinition.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/state/BlockState.java`

Relevant vanilla shape:

- `ResourceLocation` validates namespace/path strings and defaults missing namespaces to `minecraft`.
- `Blocks.register(...)` creates the block registry.
- `StateDefinition` enumerates property combinations in sorted property-name order.
- `Block.BLOCK_STATE_REGISTRY` maps concrete `BlockState` values to global ids.

## Scope

Landed:

- `ResourceLocation` with Java-compatible namespace/path validation and ordering
- pack-relative `AssetPath` helpers for blockstates, models, and textures
- `AssetSource` trait with:
  - `MemoryAssetSource` for web/embedded pack manifests
  - native-only `FilesystemAssetSource` for extracted vanilla assets
- blockstate JSON loader that records variant keys and referenced model resource locations
- `BlockStateAssetIndex` for namespace-wide blockstate indexing
- `BlockStateRegistry::terrain_mvp()` covering the block-state ids native worldgen currently writes into chunk snapshots
- validation that the current terrain MVP registry is backed by the extracted Minecraft blockstate assets

Kept out:

- full Java global block-state id parity
- block/item registry generation from Java constructors
- blockstate variant condition evaluation
- model parent resolution, element parsing, texture variable resolution, and model baking
- texture PNG decoding or atlas construction

## Architecture

The first asset boundary is:

```text
AssetSource
  -> AssetPath
  -> ResourceLocation
  -> BlockStateAssetIndex
  -> BlockStateRegistry
```

The MVP registry intentionally mirrors the ids currently emitted by `mclone-worldgen` rather than claiming full Java `Block.BLOCK_STATE_REGISTRY` parity. That keeps existing snapshots, protocol tests, and renderer smokes stable while giving later slices a single place to replace temporary generated ids with true vanilla state ids.

## Validation

Required checks:

- `cargo test --workspace`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- `pnpm native:web:smoke`

The registry/asset tests include memory-source coverage for web-shaped packs and native filesystem coverage against `reference/minecraft-1.17.1/extracted/assets/minecraft/blockstates`.

## Follow-Up

Proceed to `012-model-baking-and-atlas.md`: parse blockstate/model JSON deeply enough to resolve parent chains, collect texture references, build an atlas boundary, and render a first textured block/chunk smoke.
