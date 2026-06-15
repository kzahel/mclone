# 016: Biome Feature Breadth

Status: completed.

## Purpose

Replace the global starter decoration list from `015-decoration-framework-foundation.md` with a biome-keyed overworld feature table. This is the first native step where the actual layered biome source affects visible feature placement after terrain/surface generation.

The durable shape is:

```text
surface chunk buffer
  -> layered biome source
  -> chunk-center biome feature profile
  -> placed feature list
  -> Features chunk snapshot
```

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/Features.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/placement/CountWithExtraChanceDecorator.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/placement/FrequencyWithExtraChanceDecoratorConfiguration.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/placement/RepeatingDecorator.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/placement/FeatureDecorator.java`

## Scope

Landed:

- Java-shaped `COUNT_EXTRA` decorator support for tree-style feature counts
- chunk-center biome sampling through `OverworldBiomeSource`
- biome-keyed native feature profiles for plains, forest, birch forest, taiga, snowy, mountain, desert, badlands, swamp, mushroom, and fallback land cases
- visible oak, birch, and spruce tree families through the existing constrained basic-tree placeholder
- fern and dead-bush patch support
- generated block IDs and asset registry entries for birch/spruce logs and leaves, fern, and dead bush
- unit tests proving distinct biome tables and feature-family placement

Kept out:

- full vanilla `TreeFeature`, trunk placers, foliage placers, decorators, and weighted random selectors
- exact Java biome generation settings and mixed-biome per-chunk decoration
- ores and underground extras
- water/lake/sugar-cane/cactus feature families
- double plants and tall grass two-block placement
- decorated-chunk oracle fixtures

## Divergence Notes

Minecraft decorates from biome generation settings and feature registries, with richer context than the current single-chunk buffer. This slice samples the chunk-center biome and selects a native feature profile from that key. That is a temporary runtime divergence to get real biome influence into the native server/client/render path.

The tree implementation remains the `015` basic-tree placeholder. Do not expand it into ad hoc vanilla-ish logic. Replace it with the real trunk/foliage placer stack when tree parity becomes the focus.

Counts are Java-inspired but not exact parity. The purpose is feature-family breadth and visible validation while the core feature executor is still incomplete.

## Validation

Required checks:

- `cargo test --workspace`
- `cargo check -p mclone-worldgen --target wasm32-unknown-unknown`
- `cargo check -p mclone-mesh --target wasm32-unknown-unknown`
- `cargo check -p mclone-render --target wasm32-unknown-unknown`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- `cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-native-biome-features.png --width 960 --height 640 --chunk-radius 1`
- inspect `/tmp/mclone-native-biome-features.png`
- `pnpm native:web:smoke`

## Follow-Up

Proceed to `017-structures-foundation.md` only after deciding whether to do one more worldgen follow-through slice first. The main open worldgen gaps are exact tree placers, ore/disk features, water/lake/liquid-visible features, and decorated-chunk oracle fixtures.
