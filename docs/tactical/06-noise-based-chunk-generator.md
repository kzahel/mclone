# 06 — NoiseBasedChunkGenerator (Terrain-Only)

Continuing from [`05-overworld-biome-source.md`](05-overworld-biome-source.md). This slice ports the **default 1.17.1 overworld terrain fill path** from `NoiseBasedChunkGenerator`: `NoiseSampler` column sampling, 4x8x4 cell interpolation, disabled-aquifer block selection (`stone` / `air` / `water`), and bottom bedrock. Surface rules, carvers, features, and every disabled C&C Part 1 system remain out of scope per [`AGENTS.md`](../../AGENTS.md).

## Goal

Land a minimal TS `NoiseBasedChunkGenerator` that:

- uses the existing `OverworldBiomeSource` and `NoiseSampler`
- emits chunk sections with only `minecraft:stone`, `minecraft:air`, `minecraft:water`, and `minecraft:bedrock`
- preserves the native 1024-entry biome grid for the chunk
- validates chunk `(0, 0)` for seed `12345` against a committed terrain-only Java oracle fixture for the same seed/chunk pinned by the full integration fixture

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | minimal `NoiseGeneratorSettings` preset | tactical `03` settings types | overworld preset values match 1.17.1 defaults |
| 2 | terrain-only `NoiseBasedChunkGenerator` | `OverworldBiomeSource`, `NoiseSampler`, PRNG/noise ports | seed `S` + chunk `(x,z)` → chunk sections + terrain heightmaps |
| 3 | terrain-only oracle fixture | tactical `04` integration fixture + local Java oracle | pinned seed/chunk → committed terrain-only chunk fixture |

## Oracle split

The committed integration fixture from tactical `04` is still the source of truth for the pinned seed/chunk and the stored biome grid:

- `test/fixtures/integration/overworld-seed-12345-chunks-0-0.json`

But its block sections are **not** directly comparable to tactical `06`, because that fixture already includes later-stage mutations that this slice still excludes:

- surface materials / ores
- classic carvers
- decorations and vegetation
- bottom-layer block mutations that do not match the target overworld bedrock-only slice

So tactical `06` adds a second, narrower fixture generated from the local Java `NoiseBasedChunkGenerator` path for that exact same seed/chunk:

- `test/fixtures/integration/overworld-seed-12345-chunks-0-0-terrain-only.json`

The test keeps using the full integration fixture for chunk identity and biomes, and uses the terrain-only fixture for voxel parity.

## Translation gotchas

- `NoiseBasedChunkGenerator` seeds `BlendedNoise`, `surfaceNoise`, and `depthNoise` from the **same** `WorldgenRandom` stream. Even though tactical `06` does not use surface noise yet, we still instantiate it so the downstream `depthNoise` seed stays aligned with vanilla.
- The default overworld preset in 1.17.1 keeps `aquifers_enabled`, `noise_caves_enabled`, `deepslate_enabled`, `ore_veins_enabled`, and `noodle_caves_enabled` all `false`. Tactical `06` rejects any settings that try to turn those paths on.
- Disabled aquifers still matter for block choice: density `<= 0` becomes `air` at `y >= seaLevel` and `water` below sea level.
- Bedrock is not part of `fillFromNoise`; it is applied afterwards with the chunk-seeded `WorldgenRandom` path from vanilla.

## Done when

- chunk `(0, 0)` for seed `12345` matches `test/fixtures/integration/overworld-seed-12345-chunks-0-0-terrain-only.json`
- generated chunk biomes still match the committed fixture’s native 1024-entry biome grid
- emitted palettes contain only `stone` / `air` / `water` / `bedrock`
- `pnpm typecheck` and `pnpm test` stay green

## Out of scope for this doc

- `SurfaceBuilder` / surface rules
- `CaveWorldCarver` / `CanyonWorldCarver`
- features, structures, ores, or decorations
- aquifers, `Cavifier`, noodle caves, ore veins, or deepslate replacement
