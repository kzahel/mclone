# Carver Status

Living status page for the overworld carver path.

This doc is narrower than [`worldgen-status.md`](./worldgen-status.md): it only tracks classic 1.17.1 overworld carvers and their oracle coverage.

## Scope

- Target: vanilla Java `1.17.1` overworld carver parity.
- In scope: classic overworld `GenerationStep.Carving.AIR` plus `GenerationStep.Carving.LIQUID` behavior for the default 1.17.1 overworld path, plus the biome/config/oracle plumbing needed to verify both steps.
- Still out of scope in the current TS port: broader post-generation fluid simulation, aquifer-enabled carving, and the disabled Caves & Cliffs Part 1 cave systems called out in [`../AGENTS.md`](../AGENTS.md).

## Current state

The repo now has a real, integrated classic-overworld carver path for both live steps:

- translated [`WorldCarver`](../src/worldgen/carver/world-carver.ts), [`CaveWorldCarver`](../src/worldgen/carver/cave-world-carver.ts), and [`CanyonWorldCarver`](../src/worldgen/carver/canyon-world-carver.ts)
- translated underwater carvers in [`underwater-cave-world-carver.ts`](../src/worldgen/carver/underwater-cave-world-carver.ts) and [`underwater-canyon-world-carver.ts`](../src/worldgen/carver/underwater-canyon-world-carver.ts)
- translated built-in overworld configured carvers in [`overworld-configured-carvers.ts`](../src/worldgen/carver/overworld-configured-carvers.ts), including ocean `LIQUID` entries
- chunk-generator integration in [`NoiseBasedChunkGenerator.applyCarvers(...)`](../src/worldgen/levelgen/noise-based-chunk-generator.ts), with explicit per-step application for oracle tests and AIR+LIQUID application for runtime generation
- generated-world integration in [`GeneratedRenderLevel.generateChunk(...)`](../src/world/level/generated-render-level.ts)
- scheduled block/liquid tick capture across [`chunk-block-buffer.ts`](../src/worldgen/chunk/chunk-block-buffer.ts), [`level-chunk.ts`](../src/world/level/chunk/level-chunk.ts), and [`chunk-snapshot.ts`](../src/world/level/chunk-snapshot.ts)
- biome-driven AIR/LIQUID carver selection through [`BiomeGenerationSettings`](../src/worldgen/biome/biome-generation-settings.ts) and [`overworld-biome-generation-settings.ts`](../src/worldgen/biome/overworld-biome-generation-settings.ts)
- frozen-ocean and badlands surface follow-through in [`surface-builders.ts`](../src/worldgen/surface/surface-builders.ts), which finally makes those live vanilla surface families available to the carver material/oracle matrix instead of leaving them behind tactical-07 gaps

That is enough to truthfully say classic overworld carvers are implemented and integrated.

It is not enough to call the path full parity yet.

## Confirmed parity gaps

These are the highest-signal gaps between the current TS port and the 1.17.1 reference behavior.

- Underwater scheduled tick consequences are now captured and oracled at generation time, but the runtime still only records them; it does not execute the later fluid/block updates that a full server tick loop would consume.
- The replaceable-block set in [`world-carver.ts`](../src/worldgen/carver/world-carver.ts) now covers the live desert/ocean/frozen/badlands families that the repo can currently surface-build, but it is still narrower than full vanilla `WorldCarver`. The highest-signal remaining families are the still-unported giant-tree-taiga / shattered-savanna / mushroom follow-through materials, plus later tuff and ore/deepslate-era variants that this target does not yet model exhaustively.
- The numeric chunk/oracle palette in [`chunk-block-buffer.ts`](../src/worldgen/chunk/chunk-block-buffer.ts) now includes the frozen/badlands follow-through materials added in tactical 31 on top of the earlier underwater-floor outputs (`obsidian`, `magma_block`), but it still does not cover every remaining biome-specific surface mutation needed for exhaustive podzol / mycelium-era parity.
- The current carved-stage oracle matrix is still too small. Four committed AIR-only carved chunks plus two AIR-plus-LIQUID chunks are useful targeted coverage, not exhaustive parity coverage.
- The TS path still collapses some vanilla block-state distinctions in the carved-stage numeric model, which is acceptable for narrow chunk diffs but not the final form of exhaustive parity work.

## Current oracle coverage

Current carver verification is real but still intentionally narrow:

- four committed AIR-only carved-stage oracle fixtures:
  [`overworld-seed-12345-chunks-0-0-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-0-0-carved-only.json)
  , [`overworld-seed-12345-chunks-117--128-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-117--128-carved-only.json),
  [`overworld-seed-12345-chunks-96--64-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-96--64-carved-only.json),
  and [`overworld-seed-12345-chunks--320-99-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--320-99-carved-only.json)
- two committed AIR-plus-LIQUID fixtures:
  [`overworld-seed-12345-chunks-117--128-liquid-carved.json`](../test/fixtures/integration/overworld-seed-12345-chunks-117--128-liquid-carved.json)
  and [`overworld-seed-12345-chunks--129--256-liquid-carved.json`](../test/fixtures/integration/overworld-seed-12345-chunks--129--256-liquid-carved.json)
- companion surface-stage fixtures for the new material families:
  [`overworld-seed-12345-chunks--247--247-surface-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--247--247-surface-only.json)
  and [`overworld-seed-12345-chunks--320-99-surface-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks--320-99-surface-only.json)
- chunk-level parity assertion in [`noise-based-chunk-generator.test.ts`](../test/worldgen/levelgen/noise-based-chunk-generator.test.ts)
- fixture-shape pinning in [`carver-oracle-fixture.test.ts`](../test/worldgen/levelgen/carver-oracle-fixture.test.ts)
- explicit replaceable-material parity tests in [`world-carver-material-parity.test.ts`](../test/worldgen/carver/world-carver-material-parity.test.ts)
- explicit underwater branch tests in [`underwater-carver.test.ts`](../test/worldgen/carver/underwater-carver.test.ts)
- snapshot/tick round-trip coverage in [`chunk-snapshot.test.ts`](../test/world/chunk-snapshot.test.ts)
- biome/carver wiring tests in [`overworld-carver-wiring.test.ts`](../test/worldgen/carver/overworld-carver-wiring.test.ts)
- targeted ravine browser validation in [`test/browser/cave-mouth.test.ts`](../test/browser/cave-mouth.test.ts)
- targeted frozen/badlands surface validation in [`test/browser/surface-material-matrix.test.ts`](../test/browser/surface-material-matrix.test.ts)

That supports “partially oracled,” not “exhaustively covered.”

## Recommended next slices

The highest-value sequence from here is:

1. Finish the remaining carver-relevant surface families that are still blocked in `surface-builders.ts`: giant-tree taiga / shattered savanna / mushroom follow-through (`podzol`, `coarse_dirt`, `mycelium`) plus one or two new oracle chunks that actually exercise them.
2. Decide whether recorded scheduled underwater ticks should stay a measured generation artifact or grow into a later runtime simulation requirement.
3. Keep browser validation aimed at exposed cave mouths / ravines after each substantial carver change, and keep surface-family validation aimed at the new material families when the widened palette changes.
4. Once the remaining surface/material matrix is landed, reassess whether carver parity should stay above dark-forest / jungle biome-table work or finally drop behind it.

## Practical definition of “full parity”

For this repo, carvers should only be described as full-parity when all of the following are true:

- AIR and LIQUID carver steps are both ported for the 1.17.1 overworld target.
- Per-biome configured-carver selection matches vanilla through biome generation settings rather than ad hoc fallback logic.
- The carveable/replaceable material set matches vanilla for the current target.
- Underwater scheduled water/magma tick behavior is either executed later in runtime or intentionally measured, stored, and documented as an accepted generation-stage boundary.
- The carved-stage oracle suite covers multiple biome/material families, including the remaining podzol/mycelium-era surface families, not just one chunk near spawn.
- Generated browser frames have been manually inspected for cave mouths/ravines after each substantial carver change.
