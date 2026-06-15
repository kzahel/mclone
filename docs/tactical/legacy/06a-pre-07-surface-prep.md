# 06a — Pre-07 Surface Prep

Continuing from [`06-noise-based-chunk-generator.md`](06-noise-based-chunk-generator.md). This is a **prep slice** before tactical `07`, not surface rules themselves. The goal is to remove the avoidable friction we already know will slow down the surface pass.

## Goal

Do the full prep set before porting overworld surface builders:

- extract the terrain generator’s mutable chunk storage / section / heightmap helpers into their own module(s)
- add a dedicated **surface-stage Java oracle path** so tactical `07` does not need to infer surface behavior from the full integration fixture
- move bedrock placement out of terrain fill and into an explicit `buildSurfaceAndBedrock`-style step
- widen the internal block-id / chunk-buffer model just enough for the first surface materials (`grass_block`, `dirt`, `sand`, `gravel`, likely `snow`)

This slice should make tactical `07` mostly about porting vanilla surface logic, not refactoring around the current terrain-only scaffolding.

## Current status

- Tactical `06a` is complete.
- Mutable chunk storage, section serialization, and heightmap helpers now live under `src/worldgen/chunk/`.
- `NoiseBasedChunkGenerator.fillFromNoise(...)` now stops at terrain density fill; `buildSurfaceAndBedrock(...)` owns the explicit post-fill mutation stage.
- `oracle/java/OracleDumper.java` now supports a dedicated `surface-chunk` path.
- The committed surface-stage oracle fixture is `test/fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json`.
- The current pinned surface fixture exercises `grass_block` and `dirt`; tactical `07` may need an additional narrow oracle chunk if `sand`, `gravel`, or `snow` are not reached by the first parity target.

## Why this exists

Tactical `06` proved two things:

- the terrain-only `NoiseBasedChunkGenerator` port is viable and can be checked against a narrower Java oracle
- the committed full integration fixture is **not** a clean oracle for earlier pipeline stages, because later stages have already mutated the block data

If we go straight into surface builders without prep, we will immediately pay for:

- a mixed-responsibility generator file
- terrain-only block vocabulary leaking into surface code
- another round of awkward fixture projection instead of a direct oracle
- bedrock being staged in the wrong place relative to vanilla

That is exactly the kind of churn this prep slice should remove.

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | mutable chunk buffer extraction | tactical `06` terrain generator | same terrain output, cleaner storage boundary |
| 2 | explicit `buildSurfaceAndBedrock` staging | item 1 | terrain fill unchanged; bedrock moved to the surface-phase API |
| 3 | widened internal block ids | item 1 | chunk buffer supports initial surface-material writes without stringly block handling |
| 4 | Java oracle surface dump | local 1.17.1 deobf/oracle harness | seed `S` + chunk `(x,z)` → committed surface-stage oracle fixture |

## Concrete prep tasks

1. Extract chunk storage helpers out of `NoiseBasedChunkGenerator`.
   Suggested split:
   - one module for the mutable chunk block buffer and block-id constants
   - one module for section serialization / palette compaction
   - one module for terrain/surface-relevant heightmap computation

   The target is not “perfect abstraction”; it is “surface code can mutate chunk blocks without fighting the terrain generator file.”

2. Introduce an explicit staging API that matches vanilla better.
   Minimum shape:
   - `fillFromNoise(chunkX, chunkZ)` or equivalent: terrain density only
   - `buildSurfaceAndBedrock(chunk, ...)` or equivalent: post-fill mutation step

   Tactical `06` currently bakes bedrock into terrain fill. Move it so surface work lands on the right staging boundary.

3. Widen the internal block model only as far as tactical `07` needs.
   Add numeric ids for:
   - `minecraft:grass_block`
   - `minecraft:dirt`
   - `minecraft:sand`
   - `minecraft:gravel`
   - `minecraft:snow`

   Keep palette serialization separate from the in-memory ids. Do **not** jump to a general block-state system yet.

4. Add a surface-stage Java oracle path to `oracle/java/OracleDumper.java`.
   Desired output:
   - seed/chunk metadata
   - palette + 16x16x256 block ids in `y-major,z-major,x-minor`
   - only the stage we care about for tactical `07`: after `buildSurfaceAndBedrock`, still before carvers/features/structures

   The important point is to avoid another “project the full integration fixture down to an earlier stage” hack.

5. Commit at least one narrow surface-stage oracle fixture for the pinned seed/chunk.
   Suggested path:
   - `test/fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json`

6. Add tests that pin the prep boundary.
   Minimum coverage:
   - extracted chunk buffer helpers preserve tactical `06` behavior
   - terrain generation still matches the existing terrain-only oracle
   - `buildSurfaceAndBedrock` can be invoked separately
   - the new surface oracle fixture is readable and metadata-stable

## Translation constraints

- Stay on the 1.17.1 vanilla overworld target from [`AGENTS.md`](../../AGENTS.md).
- Do **not** pull in carvers, structures, features, aquifers, Cavifier, noodle caves, ore veins, or deepslate replacement.
- Do **not** build a full `ChunkAccess` clone unless the prep work proves it is strictly necessary.
- Do **not** introduce a general block-state/material system ahead of need.

## Done when

- chunk buffer / section / heightmap code is no longer embedded directly inside the terrain generator
- bedrock is applied in an explicit post-terrain step
- the in-memory block vocabulary can represent the first surface materials
- a committed surface-stage Java oracle fixture exists for seed `12345`, chunk `(0, 0)`
- tactical `06` parity remains green after the extraction
- `pnpm typecheck` and `pnpm test` stay green

## Out of scope for this doc

- the actual surface-builder translation
- carvers, features, ores, structures, or biome decoration
- generalized chunk/world APIs beyond what tactical `07` immediately needs
