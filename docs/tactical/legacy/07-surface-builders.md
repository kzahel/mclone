# 07 — Surface Builders

Continuing from [`06a-pre-07-surface-prep.md`](06a-pre-07-surface-prep.md). This slice ports the **default 1.17.1 overworld surface mutation stage** that runs inside `NoiseBasedChunkGenerator.buildSurfaceAndBedrock(...)`: biome lookup at block positions, surface-noise sampling, `ConfiguredSurfaceBuilder` dispatch, and the first recognizably Minecraft overworld top materials.

## Goal

Land a TS surface pass that:

- mutates the extracted mutable chunk buffer in the explicit post-fill stage
- matches vanilla’s block-position biome lookup and surface-noise inputs
- writes the first overworld surface materials through the numeric block-id model from tactical `06a`
- validates against the committed **surface-stage** Java oracle fixture instead of projecting the full integration fixture backwards

This is the first slice where chunk tops should stop looking like raw stone columns and start looking like overworld terrain.

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | TS `buildSurfaceAndBedrock(...)` surface path | tactical `06a` staging + chunk buffer modules | terrain-filled chunk → surface-mutated chunk buffer |
| 2 | `SurfaceBuilder` subset needed by the oracle target | item 1 | biome + noise + column state → top/filler/underwater materials |
| 3 | surface-stage parity tests | `06a` surface oracle fixture | pinned seed/chunk surface output matches Java |

## Oracle target

Start with the surface-stage fixture already committed by tactical `06a`:

- `test/fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json`

That fixture is the direct oracle for:

- metadata: seed/chunk/minY/height/block order
- the post-`buildSurfaceAndBedrock(...)`, pre-carver voxel state

Important caveat:

- the current pinned fixture exercises `grass_block` and `dirt`, but not necessarily `sand`, `gravel`, or `snow`

If the first tactical `07` parity target does not naturally cover those materials, add one or more **narrow additional surface-stage fixtures** chosen specifically to hit them, rather than widening the implementation speculatively.

## Translation gotchas

- This is still the **1.17.1 vanilla overworld** target. Use `SurfaceBuilder` / `ConfiguredSurfaceBuilder`, not 1.18+ surface rules.
- `NoiseBasedChunkGenerator` seeds `surfaceNoise` from the same `WorldgenRandom` stream as terrain construction. Keep the seed path aligned with vanilla.
- Biome lookup during surface building is at block positions via overworld biome zooming, not the raw quart-grid serialization order used for chunk biomes.
- `buildSurfaceAndBedrock(...)` owns both the surface mutation and bedrock application; keep bedrock in that stage boundary.
- Do not introduce carvers, features, structures, aquifers, Cavifier, noodle caves, ore veins, or deepslate replacement.

## Concrete steps

1. Port the minimal surface-builder dispatch needed by the pinned surface-stage oracle.
2. Thread the existing surface-noise instance into the TS `buildSurfaceAndBedrock(...)` implementation instead of leaving it as constructor-only seed alignment.
3. Keep the chunk buffer numeric and mutate it directly; do not invent a general block-state system.
4. Update tests so the TS surface stage is compared against the surface-only Java oracle fixture.
5. If parity still leaves `sand`, `gravel`, or `snow` unexercised, add one more narrow committed oracle fixture that does.

## Done when

- `buildSurfaceAndBedrock(...)` in TS matches the committed surface-stage Java oracle for the pinned target chunk
- the surface path writes at least the first live overworld materials correctly (`grass_block`, `dirt`, and any additional materials covered by committed fixtures)
- terrain-only parity from tactical `06` remains green
- `pnpm typecheck` and `pnpm test` stay green

## Out of scope for this doc

- carvers (`CaveWorldCarver`, `CanyonWorldCarver`)
- features, ores, structures, or biome decoration
- generalized chunk/world APIs beyond what the surface pass immediately needs
- any Caves & Cliffs Part 1 systems disabled in default 1.17.1 overworld generation
