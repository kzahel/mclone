# 03 — NoiseSampler And Settings

Continuing from [`02-remaining-synth.md`](02-remaining-synth.md). This slice ports the minimal `levelgen/` settings objects plus the **overworld** `NoiseSampler` path needed for the 1.17.1 target. It still does **not** pull in a real biome pipeline, end-island density, or any of the dormant C&C Part 1 cave stack — see [`AGENTS.md`](../../AGENTS.md).

## Goal

TS ports of:

- `NoiseSamplingSettings`
- `NoiseSlideSettings`
- `NoiseSettings`
- `NoiseModifier.PASSTHROUGH`
- `NoiseSampler` for the default 1.17.1 overworld path (`islandNoise == null`)

all validated against Java fixtures for canonical seeds, using a deterministic oracle biome source that exercises the 5x5 biome weighting math without dragging `OverworldBiomeSource` into scope ahead of tactical `05`.

## Current status

- `NoiseSamplingSettings`, `NoiseSlideSettings`, and `NoiseSettings.create(...)` now exist in TS with the Java `min_y` / `height` guard behavior
- `NoiseModifier.PASSTHROUGH` now exists in TS
- `NoiseSampler` is fixture-backed for the default 1.17.1 overworld path across canonical seeds `0`, `1`, `12345`, and `2151901553968352745`
- The oracle uses the real overworld `NoiseSettings` preset plus two synthetic repeating biome layouts:
  - `constantPlains`
  - `mixedOverworld`
- Tactical `03` is complete; next up is tactical `04` (integration oracle harness)

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | `NoiseSamplingSettings` | none | constructor/getter parity only |
| 2 | `NoiseSlideSettings` | none | constructor/getter parity only |
| 3 | `NoiseSettings` | items 1-2 | field plumbing + `create(...)` guard behavior |
| 4 | `NoiseModifier` | none | `PASSTHROUGH` only |
| 5 | `NoiseSampler` | `BlendedNoise`, `PerlinNoise`, settings above, minimal biome-source interface | seed `S` + overworld settings preset + repeating biome layout + sampled cell columns `(cellX,cellZ)` → full density columns |

## Why this subset

For the real 1.17.1 overworld target, `NoiseBasedChunkGenerator` constructs `NoiseSampler` with:

- overworld `NoiseSettings`
- `BlendedNoise`
- depth `PerlinNoise` `[-15..0]`
- `NoiseModifier.PASSTHROUGH` because `noise_caves_enabled` is `false`
- `islandNoise == null` because `island_noise_override` is `false`

That means tactical `03` can stay narrowly focused on the live overworld path:

- weighted biome depth/scale blending
- random density offset
- blended-noise integration
- top/bottom slide application

What stays out of scope here:

- real overworld biome generation (`OverworldBiomeSource`, cubiomes integration) — tactical `05`
- end-island density (`TheEndBiomeSource.getHeightValue(...)`) — not part of the target overworld path
- nether/end presets
- `Cavifier`, `NoodleCavifier`, `OreVeinifier`, active `Aquifer` noise — still deferred per [`AGENTS.md`](../../AGENTS.md)

## Oracle extension

Extend `oracle/java/OracleDumper.java` with:

```bash
pnpm --silent oracle:gen noise \
    --class NoiseSampler \
    --seed 12345 \
    --preset overworld \
    --samples2d test/fixtures/noise/_samples-cell-2d.json \
    > test/fixtures/noise/noise-sampler-overworld-seed-12345.json
```

Each fixture stores one seed plus two named sample sets over the shared integer cell grid:

- `constantPlains`
- `mixedOverworld`

Each sample set includes:

- a repeating 5x5 biome pattern serialized as biome keys plus depth/scale values
- sampled density columns flattened in `x-major,z-minor,y-minor` order

This keeps the oracle faithful to Java `NoiseSampler` while avoiding any dependency on the still-deferred biome-source port.

## Translation gotchas

- **Most of the biome blending loop stays in `float`.** The 5x5 weights array, biome accumulators, amplified adjustments, and the derived depth/scale terms all use Java `float` semantics until the final doubles are produced. Missing a `Math.fround(...)` on the TS side is enough to fail zero-epsilon fixtures.
- **`NoiseSampler` itself is overworld-only in this slice.** The `islandNoise != null` path calls `TheEndBiomeSource.getHeightValue(...)`; do not quietly half-port that. This slice should throw if someone tries to use it.
- **The biome source is a blocker only if you make it one.** Tactical `05` owns real overworld biome generation. Tactical `03` just needs a minimal `getNoiseBiome(x,y,z) -> { depth, scale }` contract plus oracle-backed repeating layouts.
- **`NoiseSettings.create(...)` does not fully mirror codec validation.** The Java helper only enforces the `min_y + height`, `height % 16`, and `min_y % 16` checks. Mirror that narrow behavior.
- **The overworld column path uses `biomeY = seaLevel`.** `NoiseBasedChunkGenerator` calls `sampler.fillNoiseColumn(..., this.getSeaLevel(), ...)`, not an arbitrary biome sample height.

## Concrete steps

1. Port `NoiseSamplingSettings`, `NoiseSlideSettings`, and `NoiseSettings.create(...)`. Done.
2. Port `NoiseModifier.PASSTHROUGH`. Done.
3. Extend the oracle for overworld `NoiseSampler` using repeating biome layouts and the integer cell sample grid. Done.
4. Port overworld `NoiseSampler` and fixture-test it across canonical seeds. Done.
5. Stop there. Real biome sourcing is still tactical `05`; end-island support is still out of scope for the target.

## Done when

- `NoiseSettings.create(...)` matches Java’s guard behavior
- `NoiseSampler` matches Java fixtures for canonical seeds on the overworld preset and both repeating biome layouts
- `pnpm test` and `pnpm typecheck` stay green

## Out of scope for this doc

- real `BiomeSource` / `OverworldBiomeSource` translation — tactical `05`
- integration chunk diffs — tactical `04` first, then `06`
- end-island density path
- dormant C&C Part 1 cave density composition
