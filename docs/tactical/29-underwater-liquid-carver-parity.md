# Tactical 29 — Underwater LIQUID carver parity

Finish the missing classic-overworld `GenerationStep.Carving.LIQUID` path now that the AIR-step carvers, widened material model, and broader carved-oracle base are all in place. This slice ports the real 1.17.1 underwater cave/canyon carvers, wires ocean biomes to run them, widens the shared chunk/oracle palette for their block outputs, and adds a dedicated Java oracle module for AIR-plus-LIQUID carved chunks without mutating the older AIR-only fixtures.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/carver/UnderwaterCaveWorldCarver.java` | `src/worldgen/carver/underwater-cave-world-carver.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/carver/UnderwaterCanyonWorldCarver.java` | `src/worldgen/carver/underwater-canyon-world-carver.ts` |
| `reference/.../src/net/minecraft/data/worldgen/{BiomeDefaultFeatures,Carvers}.java` | `src/worldgen/biome/overworld-biome-generation-settings.ts`, `src/worldgen/carver/overworld-configured-carvers.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java` (`applyCarvers(...)`) | `src/worldgen/levelgen/noise-based-chunk-generator.ts`, `src/worldgen/carver/overworld-carvers.ts` |
| `oracle/java/OracleDumper.java` (`carved-chunk`) | `oracle/java/OracleDumper.java` |
| `src/worldgen/chunk/chunk-block-buffer.ts` | `src/worldgen/chunk/chunk-block-buffer.ts` |
| `test/worldgen/carver/{overworld-carver-wiring,underwater-carver}.test.ts` | same |
| `test/worldgen/levelgen/{carver-oracle-fixture,noise-based-chunk-generator}.test.ts` | same |

## What landed

- translated [`UnderwaterCaveWorldCarver`](../../src/worldgen/carver/underwater-cave-world-carver.ts) and [`UnderwaterCanyonWorldCarver`](../../src/worldgen/carver/underwater-canyon-world-carver.ts)
- ocean biome wiring for real `GenerationStep.Carving.LIQUID` configured carvers in [`overworld-biome-generation-settings.ts`](../../src/worldgen/biome/overworld-biome-generation-settings.ts)
- generator/carver staging that can apply one explicit carver step for oracle tests or both AIR and LIQUID for normal runtime generation
- widened shared chunk/oracle numeric palette with `obsidian` and `magma_block`, the two underwater-floor outputs produced by vanilla underwater carvers
- a dedicated Java oracle module, `liquid-carved-chunk`, so AIR-only carved fixtures remain stable while LIQUID coverage gets its own committed baseline
- a committed AIR-plus-LIQUID ocean fixture at [`overworld-seed-12345-chunks-117--128-liquid-carved.json`](../../test/fixtures/integration/overworld-seed-12345-chunks-117--128-liquid-carved.json)
- explicit underwater-carver unit tests covering sea-level rejection, water-fill, lava-below-floor, magma/obsidian-at-floor, and canyon-vs-cave replaceability differences

## Scope choice

Landed here on purpose:

- classic-overworld LIQUID-step cave/canyon parity for the default 1.17.1 overworld target
- exact ocean-biome configured-carver wiring for AIR plus LIQUID
- a dedicated oracle path for full carved-stage ocean chunks
- the minimum render-palette follow-through needed so new underwater outputs no longer collapse to air in browser frames

Still deferred on purpose:

- scheduled liquid/block tick parity for underwater water-flow and magma-block updates
- a broader LIQUID-stage oracle matrix that includes an ocean chunk with committed `obsidian` / `magma_block` floor output
- the remaining long-tail replaceable-material families beyond the currently modeled live overworld path
- aquifer-enabled carving and every disabled Caves & Cliffs Part 1 cave system called out in [`../../AGENTS.md`](../../AGENTS.md)

## Oracle / done-when

**Unit + integration (Vitest):**

- ocean biome wiring proves `GenerationStep.Carving.LIQUID` only appears where vanilla adds it
- explicit underwater-carver tests prove the branch behavior that a single ocean fixture cannot cover by itself
- carved-stage generator parity keeps AIR-only fixtures stable and adds a new AIR-plus-LIQUID ocean fixture

**Java oracle fixtures:**

- existing `surface-chunk` and `carved-chunk` fixtures are regenerated against the widened 38-entry palette
- `liquid-carved-chunk` is a separate module and committed fixture, not a silent rewrite of `carved-chunk`

## Done when

- `pnpm typecheck` passes
- targeted `pnpm test` passes for chunk-buffer, carver, carved-oracle, and chunk-generator parity suites
- `pnpm test:browser -- test/browser/smoke.test.ts` passes and writes `/tmp/mclone-browser-smoke.png`
- `docs/carver-status.md` and `docs/worldgen-status.md` describe LIQUID carvers as implemented, while still calling out the remaining coverage and scheduled-tick gaps

## Next

Tactical 30 should broaden LIQUID-stage oracle coverage and scheduled-tick parity: add an ocean/frozen-ocean fixture that actually hits the `y=10` underwater floor branch, capture the underwater scheduled water/magma tick consequences explicitly, and use a browser frame aimed at a visible cave mouth or ravine instead of the generic terrain smoke.
