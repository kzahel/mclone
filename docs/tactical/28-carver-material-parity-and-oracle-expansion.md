# Tactical 28 — Carver material parity and oracle expansion

Finish the next real classic-carver parity slice now that the overworld AIR-step path is integrated and has spawn-plus-ocean carved fixtures. This slice widens the chunk/oracle block model for the first nontrivial carver-relevant surface materials, aligns `WorldCarver`'s replaceable-material behavior with the live 1.17.1 overworld surface-stage path, and broadens carved-stage fixture coverage with one more nontrivial land chunk.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/carver/WorldCarver.java` | `src/worldgen/carver/world-carver.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/carver/{CaveWorldCarver,CanyonWorldCarver}.java` | `src/worldgen/carver/{cave-world-carver,canyon-world-carver}.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java` (`applyCarvers(...)`) | `src/worldgen/levelgen/noise-based-chunk-generator.ts` |
| `oracle/java/OracleDumper.java` (`surface-chunk`, `carved-chunk`) | `oracle/java/OracleDumper.java` |
| `src/worldgen/chunk/chunk-block-buffer.ts` | `src/worldgen/chunk/chunk-block-buffer.ts` |
| `test/worldgen/levelgen/{surface-oracle-fixture,carver-oracle-fixture,noise-based-chunk-generator}.test.ts` | same |
| `docs/carver-status.md` | `docs/carver-status.md` |

## What landed

- `ChunkBlockId` / `CHUNK_BLOCK_NAMES` now cover the first nontrivial carver-relevant surface-stage material set beyond the old tactical-07 palette: stone variants, dirt variants, mycelium, terracotta families, sandstone / red sandstone, and packed ice.
- `WorldCarver` now matches that widened live overworld AIR-step replaceable-material set and treats `mycelium` as a surface-top block alongside `grass_block`.
- The local Java oracle harness now emits the widened palette for `surface-chunk` and `carved-chunk` fixtures instead of hardcoding the old 10-entry surface/carved palette.
- The surface path now ports the vanilla sandstone underlayer mutation from `DefaultSurfaceBuilder`, which lets the TS generator match a real desert chunk that reaches sandstone before the carver pass.
- The committed oracle matrix now includes one more nontrivial land chunk: a desert/sandstone pair at `(96, -64)` on top of the existing spawn and ocean carved fixtures.
- The test suite now has explicit `WorldCarver` material-parity coverage in addition to the chunk-level oracle diffs.

## Scope choice

Landed here on purpose:

- widen the chunk/oracle numeric model beyond the old tactical-07 palette for the first carver-relevant surface and near-surface materials needed by `WorldCarver`
- align `WorldCarver.canReplaceBlock(...)` and surface-top detection with the live 1.17.1 overworld AIR-step path for those materials
- regenerate the existing surface/carved oracle fixtures against the widened palette
- add one more committed carved-stage land fixture beyond spawn and ocean
- add explicit unit tests around the widened replaceable-material set and the sand/gravel-over-water guard

Still deferred on purpose:

- `GenerationStep.Carving.LIQUID` underwater cave/canyon parity
- aquifer-enabled carving and every disabled Caves & Cliffs Part 1 cave system called out in [`../../AGENTS.md`](../../AGENTS.md)
- full badlands / frozen-ocean / mushroom surface-builder follow-through when it still needs biome-specific surface mutations beyond this material-model slice
- broader biome-family carved fixtures once the remaining surface-stage material gaps are truly closed

## Why this slice comes before LIQUID carvers

The current repo already proves that the AIR-step carver path is integrated, but it still under-models what the carvers are allowed to cut through and still only has a tiny carved-fixture matrix. Fixing those two constraints first gives the LIQUID step a cleaner base:

- the oracle JSON no longer has to stay pinned to the tactical-07 palette
- unit tests can assert the actual replaceable-material contract instead of only chunk-shape smoke behavior
- the carved-fixture matrix grows along a land/ocean split before the underwater carvers land

## Oracle / done-when

**Unit (Vitest):**

- `WorldCarver` replaceable-material tests cover the widened live overworld set and the sand/gravel-over-water exception
- the widened chunk palette still round-trips through section serialization and fixture metadata checks
- carved-stage fixture tests pin three committed carved chunks: spawn, ocean, and one nontrivial land chunk

**Java oracle fixtures:**

- rerun `pnpm --silent oracle:gen surface-chunk ...` / `carved-chunk ...` for every committed surface/carved fixture whose palette changed
- add one new carved-stage land fixture generated from the existing oracle harness, not from handwritten JSON

## Done when

- `pnpm typecheck` passes
- targeted `pnpm test` passes for chunk-buffer, carver, surface-oracle, carved-oracle, and chunk-generator parity suites
- `docs/tactical/README.md` links this slice and stops pointing the "next" carver work at the stale `08-` placeholder / dark-forest follow-through
- `docs/carver-status.md` reflects the widened material model and the broader carved-fixture matrix

## Next

Tactical 29 is now [`29-underwater-liquid-carver-parity.md`](29-underwater-liquid-carver-parity.md): port `GenerationStep.Carving.LIQUID` underwater cave/canyon parity, add a dedicated AIR-plus-LIQUID ocean oracle fixture, and keep the AIR-only carved fixtures stable for step-scoped parity tests.
