# 05 — Overworld Biome Source

Continuing from [`04-integration-oracle-harness.md`](04-integration-oracle-harness.md). This slice ports the **vanilla layered overworld biome pipeline** that 1.17.1 actually uses: `OverworldBiomeSource`, the `newbiome/` layer stack underneath it, and the chunk-biome container logic that writes the native 4×64×4 biome grid into chunk NBT.

## Goal

TS ports of:

- the layered biome runtime (`LazyArea`, `LazyAreaContext`, transformer helpers, and the live `newbiome/layer/*` transforms)
- overworld biome metadata needed by `NoiseSampler` and later generator stages (`id`, resource key, `depth`, `scale`)
- `OverworldBiomeSource` for the default 1.17.1 overworld path
- `ChunkBiomeContainer`-equivalent chunk biome filling for the native 1024-entry biome array

all validated against Java fixtures for canonical seeds, plus a chunk-level parity check against the committed server integration fixture from tactical `04`.

## Current status

- Tactical `00` through `04` are complete
- The TS port now contains a real layered overworld biome source under `src/worldgen/biome/`
- The Java oracle now supports `biome --class OverworldBiomeSource`
- Tactical `05` is complete; next up is tactical `06` (`NoiseBasedChunkGenerator`, terrain-only)

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | layered biome runtime (`LazyArea*`, transformer helpers, `LinearCongruentialGenerator`) | `ImprovedNoise`, `SimpleRandomSource` | deterministic sampled quart-biome IDs/keys/depths/scales |
| 2 | overworld biome metadata | built-in 1.17.1 biome registry | oracle `possibleBiomes[]` metadata parity |
| 3 | `OverworldBiomeSource` | items 1-2 | seed `S` + sampled quart coords `(x,z)` → biome IDs + factors |
| 4 | chunk biome container | item 3 | chunk `(x,z)` + source → native 1024-entry biome grid |

## Why this subset

For the 1.17.1 overworld target, the live biome path is still the pre-1.18 layered stack:

- `OverworldBiomeSource` delegates to `Layers.getDefaultLayer(...)`
- `NoiseSampler` only needs `Biome.getDepth()` / `getScale()` from `getNoiseBiome(...)`
- chunk serialization stores the raw 1024-entry quart-biome grid via `ChunkBiomeContainer.writeBiomes()`

That makes tactical `05` the right place to land:

- the full 2D layered source
- exact biome metadata lookup
- chunk-biome filling parity

What stays out of scope here:

- `BiomeManager`, `FuzzyOffsetBiomeZoomer`, and block-position biome lookup
- nether/end `MultiNoiseBiomeSource`
- any `NoiseBasedChunkGenerator`, surface, or carver translation

## Oracle extension

Extend `oracle/java/OracleDumper.java` with:

```bash
pnpm --silent oracle:gen biome \
    --class OverworldBiomeSource \
    --seed 12345 \
    --samples2d test/fixtures/biome/_samples-quart-2d.json \
    > test/fixtures/biome/overworld-seed-12345.json
```

Each fixture stores:

- seed
- default `legacyBiomeInitLayer=false`, `largeBiomes=false`
- the Java `possibleBiomes()` list with exact registry IDs, resource keys, `depth`, and `scale`
- sampled quart biomes over an integer 2D grid, flattened in `x-major,z-minor` order

Chunk-grid parity is then checked against the committed integration fixture from tactical `04`:

- `test/fixtures/integration/overworld-seed-12345-chunks-0-0.json`

## Translation gotchas

- **`possibleBiomes()` is not exhaustive.** Java’s `OverworldBiomeSource.POSSIBLE_BIOMES` list omits bamboo jungle variants even though the layered pipeline can emit biome IDs `168` and `169`. Keep the lookup table exhaustive by biome ID, but keep `possibleBiomes()` parity with Java’s narrower list.
- **The layer cache is shared per `LazyAreaContext`.** Each context hands the same cache map to derived `LazyArea` instances; recreating isolated caches changes behavior and performance characteristics.
- **`LazyArea` eviction is insertion-order, not LRU-on-read.** Java uses `Long2IntLinkedOpenHashMap.removeFirstInt()` without access-order updates.
- **Biome metadata must come from the real registry, not guessed literals.** The exact `double` values exposed through `Biome.getDepth()` / `getScale()` are widened `float`s such as `0.4000000059604645`, not rounded source literals.
- **The chunk biome grid is quart-sampled and Y-major.** Match `ChunkBiomeContainer.writeBiomes()` order exactly: 4×64×4 cells, `y-major,z-major,x-minor`.

## Concrete steps

1. Port the layered biome runtime and transforms under `src/worldgen/biome/layered/`. Done.
2. Add exact overworld biome metadata lookup and a concrete `Biome` type. Done.
3. Port `OverworldBiomeSource` and fixture-test it across canonical seeds. Done.
4. Port `ChunkBiomeContainer`-style biome filling and check it against the committed integration fixture. Done.
5. Stop there. `BiomeManager` zooming and terrain generation stay for later tactical docs. Done.

## Done when

- sampled quart biome IDs/keys/depths/scales match the Java oracle for canonical seeds
- the runtime biome metadata matches Java registry output exactly
- the TS chunk-biome container reproduces the committed integration fixture’s `biomes` array
- `pnpm test` and `pnpm typecheck` stay green

## Out of scope for this doc

- `BiomeManager.obfuscateSeed(...)` and fuzzy block-position biome zooming
- nether/end biome sources
- `NoiseBasedChunkGenerator`, surface rules, carvers, features, or structures
