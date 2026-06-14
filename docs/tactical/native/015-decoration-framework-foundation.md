# 015: Decoration Framework Foundation

Status: completed.

## Purpose

Start native feature decoration after the renderer gained section-level ownership in `014-streaming-renderer-and-camera.md`. This slice moves beyond terrain/surface-only chunks by adding a Rust feature execution boundary that can mutate authoritative chunk buffers before snapshots reach the server/client/render path.

The durable shape is:

```text
surface chunk buffer
  -> decoration seed / feature seed
  -> PlacedFeature decorators
  -> ConfiguredFeature placement
  -> Features chunk snapshot
  -> ClientRuntime / renderer
```

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/Feature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/ConfiguredFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/DecoratedFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/SimpleBlockFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/RandomPatchFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/TreeFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/LakeFeature.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/GenerationStep.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/Features.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java`

## Scope

Landed:

- `mclone_worldgen::feature` module with Java-shaped `DecorationStep`, `ConfiguredFeature`, and `PlacedFeature`
- decorated feature execution that applies existing `ConfiguredDecorator`s before invoking the feature
- deterministic decoration/feature seeding through the existing `WorldgenRandom` helpers
- `SimpleBlock` and `RandomPatch` feature paths for first grass/flower placement
- constrained starter oak tree feature that places logs/leaves through the same configured feature boundary
- new generated block-state IDs for `oak_log[axis=y]`, `oak_leaves`, `grass`, `dandelion`, and `poppy`
- asset registry coverage for those blockstates so native rendering can mesh the new feature blocks
- `generate_overworld_features_chunk(...)`
- default server chunk interest now targets `ChunkStatus::Features`
- focused tests for simple blocks, random patches, decorated position fan-out, starter trees, decorated chunk generation, and server status progression

Kept out:

- full vanilla `TreeFeature`, `TrunkPlacer`, `FoliagePlacer`, `FeatureSize`, and tree decorators
- biome-specific feature tables
- full `LakeFeature`; water block facts already exist, but the current renderer still lacks a true liquid draw path
- feature spillover into neighboring chunks
- ore, mushroom, seagrass, spring, disk, and structure feature families
- oracle fixtures for decorated chunks

## Architecture

`ConfiguredFeature` is the feature payload, while `PlacedFeature` owns a decoration step plus a decorator chain. The execution flow intentionally mirrors Java's `ConfiguredFeature.place(...)` and `DecoratedFeature.place(...)`: decorators produce positions, and each position attempts to place the configured feature.

The first native feature set is deliberately small and visible:

- starter oak trees
- grass patches
- dandelion patches
- poppy patches

The starter tree is not a full Java tree port. It exists so the server/client/render path now carries feature blocks end to end. Full tree parity should replace this with vanilla trunk/foliage placers rather than expanding the placeholder.

## Divergence Notes

Java decoration runs with a `WorldGenLevel`, chunk generator, heightmaps, biome generation settings, and cross-chunk write behavior. This slice runs against a single `MutableChunkBlockBuffer`. That is a runtime-scope divergence: it proves the feature boundary and visible snapshot path, but it does not claim decorated-chunk parity yet.

Java `LakeFeature` was reviewed but not ported into the default feature list because water currently has no true liquid renderer in the native textured path. Adding invisible water lakes now would make validation less clear. Lake/water feature parity should follow after liquid rendering or in a dedicated water-feature slice with explicit visual expectations.

## Validation

Required checks:

- `cargo test --workspace`
- `cargo check -p mclone-worldgen --target wasm32-unknown-unknown`
- `cargo check -p mclone-mesh --target wasm32-unknown-unknown`
- `cargo check -p mclone-render --target wasm32-unknown-unknown`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- `cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-native-decorated-chunk.png --width 960 --height 640 --chunk-radius 1`
- `cargo run -p mclone-native-client -- --headless-chunk-scenarios /tmp/mclone-native-decorated-scenarios --width 960 --height 640 --chunk-radius 1`
- inspect `/tmp/mclone-native-decorated-chunk.png` and at least one scenario image
- `pnpm native:web:smoke`

## Follow-Up

Proceed to `016-biome-feature-breadth.md`: replace the starter feature list with biome-aware feature tables, then add tree-family breadth, visible vegetation families, ores/underground extras, and selected oracle fixtures.
