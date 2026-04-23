# Carver Status

Living status page for the overworld carver path.

This doc is narrower than [`worldgen-status.md`](./worldgen-status.md): it only tracks classic 1.17.1 overworld carvers and their oracle coverage.

## Scope

- Target: vanilla Java `1.17.1` overworld carver parity.
- In scope: `GenerationStep.Carving.AIR` cave/ravine behavior for the default overworld path, plus the biome/config/oracle plumbing needed to verify it.
- Still out of scope in the current TS port: `GenerationStep.Carving.LIQUID` underwater carvers, aquifer-enabled carving, and the disabled Caves & Cliffs Part 1 cave systems called out in [`../AGENTS.md`](../AGENTS.md).

## Current state

The repo has a real, integrated classic-air-carver path:

- translated [`WorldCarver`](../src/worldgen/carver/world-carver.ts), [`CaveWorldCarver`](../src/worldgen/carver/cave-world-carver.ts), and [`CanyonWorldCarver`](../src/worldgen/carver/canyon-world-carver.ts)
- translated built-in overworld configured carvers in [`overworld-configured-carvers.ts`](../src/worldgen/carver/overworld-configured-carvers.ts)
- chunk-generator integration in [`NoiseBasedChunkGenerator.applyCarvers(...)`](../src/worldgen/levelgen/noise-based-chunk-generator.ts)
- generated-world integration in [`GeneratedRenderLevel.generateChunk(...)`](../src/world/level/generated-render-level.ts)
- biome-driven AIR-step carver selection through [`BiomeGenerationSettings`](../src/worldgen/biome/biome-generation-settings.ts) and [`overworld-biome-generation-settings.ts`](../src/worldgen/biome/overworld-biome-generation-settings.ts)

That is enough to truthfully say carvers are implemented and integrated.

It is not enough to call the path full parity yet.

## Confirmed parity gaps

These are the highest-signal gaps between the current TS port and the 1.17.1 reference behavior.

- The TS path only applies `GenerationStep.Carving.AIR`. Vanilla ocean biomes also wire `GenerationStep.Carving.LIQUID` via underwater cave/canyon carvers.
- The replaceable-block set in [`world-carver.ts`](../src/worldgen/carver/world-carver.ts) is now much closer for the live overworld AIR-step surface families, but it is still narrower than full vanilla `WorldCarver`. Mojang also covers additional families such as later badlands/frozen follow-through materials, tuff, and several ore/deepslate variants that this repo still does not exercise or model exhaustively.
- The numeric chunk/oracle palette in [`chunk-block-buffer.ts`](../src/worldgen/chunk/chunk-block-buffer.ts) is now widened for the first live surface-stage carver families beyond tactical 07, but it still does not cover every remaining biome-specific surface mutation needed for exhaustive badlands / frozen / mushroom follow-through.
- The current carved-stage oracle matrix is still too small. Three committed carved chunks across spawn, ocean, and desert/sandstone coverage are useful smoke coverage, not exhaustive parity coverage.
- The TS path still collapses some vanilla block-state distinctions in the carved-stage numeric model, which is acceptable for narrow chunk diffs but not the final form of exhaustive parity work.

## Current oracle coverage

Current carver verification is real but still intentionally narrow:

- three committed carved-stage oracle fixtures:
  [`overworld-seed-12345-chunks-0-0-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-0-0-carved-only.json)
  , [`overworld-seed-12345-chunks-117--128-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-117--128-carved-only.json),
  and [`overworld-seed-12345-chunks-96--64-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-96--64-carved-only.json)
- chunk-level parity assertion in [`noise-based-chunk-generator.test.ts`](../test/worldgen/levelgen/noise-based-chunk-generator.test.ts)
- fixture-shape pinning in [`carver-oracle-fixture.test.ts`](../test/worldgen/levelgen/carver-oracle-fixture.test.ts)
- explicit replaceable-material parity tests in [`world-carver-material-parity.test.ts`](../test/worldgen/carver/world-carver-material-parity.test.ts)
- biome/carver wiring tests in [`overworld-carver-wiring.test.ts`](../test/worldgen/carver/overworld-carver-wiring.test.ts)

That supports “partially oracled,” not “exhaustively covered.”

## Recommended next slices

The highest-value sequence from here is:

1. Port the missing `GenerationStep.Carving.LIQUID` underwater cave/canyon path and add dedicated ocean fixtures for it.
2. Broaden carved-stage oracle coverage beyond the current spawn/ocean/desert trio into additional land material families once their pre-carve surface mutations are represented cleanly.
3. Keep widening the carved-stage numeric model where later surface parity still cannot be represented cleanly enough for carved diffs.
4. Add browser validation aimed at exposed cave mouths / ravines after each substantial carver change, not only vegetation-heavy smoke frames.

## Practical definition of “full parity”

For this repo, carvers should only be described as full-parity when all of the following are true:

- AIR and LIQUID carver steps are both ported for the 1.17.1 overworld target.
- Per-biome configured-carver selection matches vanilla through biome generation settings rather than ad hoc fallback logic.
- The carveable/replaceable material set matches vanilla for the current target.
- The carved-stage oracle suite covers multiple biome/material families, not just one chunk near spawn.
- Generated browser frames have been manually inspected for cave mouths/ravines after each substantial carver change.
