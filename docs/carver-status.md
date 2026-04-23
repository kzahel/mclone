# Carver Status

Living status page for the overworld carver path.

This doc is narrower than [`worldgen-status.md`](./worldgen-status.md): it only tracks classic 1.17.1 overworld carvers and their oracle coverage.

## Scope

- Target: vanilla Java `1.17.1` overworld carver parity.
- In scope: classic overworld `GenerationStep.Carving.AIR` plus `GenerationStep.Carving.LIQUID` behavior for the default 1.17.1 overworld path, plus the biome/config/oracle plumbing needed to verify both steps.
- Still out of scope in the current TS port: underwater scheduled tick parity, aquifer-enabled carving, and the disabled Caves & Cliffs Part 1 cave systems called out in [`../AGENTS.md`](../AGENTS.md).

## Current state

The repo now has a real, integrated classic-overworld carver path for both live steps:

- translated [`WorldCarver`](../src/worldgen/carver/world-carver.ts), [`CaveWorldCarver`](../src/worldgen/carver/cave-world-carver.ts), and [`CanyonWorldCarver`](../src/worldgen/carver/canyon-world-carver.ts)
- translated underwater carvers in [`underwater-cave-world-carver.ts`](../src/worldgen/carver/underwater-cave-world-carver.ts) and [`underwater-canyon-world-carver.ts`](../src/worldgen/carver/underwater-canyon-world-carver.ts)
- translated built-in overworld configured carvers in [`overworld-configured-carvers.ts`](../src/worldgen/carver/overworld-configured-carvers.ts), including ocean `LIQUID` entries
- chunk-generator integration in [`NoiseBasedChunkGenerator.applyCarvers(...)`](../src/worldgen/levelgen/noise-based-chunk-generator.ts), with explicit per-step application for oracle tests and AIR+LIQUID application for runtime generation
- generated-world integration in [`GeneratedRenderLevel.generateChunk(...)`](../src/world/level/generated-render-level.ts)
- biome-driven AIR/LIQUID carver selection through [`BiomeGenerationSettings`](../src/worldgen/biome/biome-generation-settings.ts) and [`overworld-biome-generation-settings.ts`](../src/worldgen/biome/overworld-biome-generation-settings.ts)

That is enough to truthfully say classic overworld carvers are implemented and integrated.

It is not enough to call the path full parity yet.

## Confirmed parity gaps

These are the highest-signal gaps between the current TS port and the 1.17.1 reference behavior.

- Underwater scheduled tick parity is still missing. Vanilla underwater carvers schedule magma-block ticks and some water-fluid updates; the TS chunk-buffer/oracle path currently verifies block output only.
- The replaceable-block set in [`world-carver.ts`](../src/worldgen/carver/world-carver.ts) is now much closer for the live overworld AIR-step surface families, but it is still narrower than full vanilla `WorldCarver`. Mojang also covers additional families such as later badlands/frozen follow-through materials, tuff, and several ore/deepslate variants that this repo still does not exercise or model exhaustively.
- The numeric chunk/oracle palette in [`chunk-block-buffer.ts`](../src/worldgen/chunk/chunk-block-buffer.ts) now includes underwater-floor outputs (`obsidian`, `magma_block`) on top of the earlier widened surface families, but it still does not cover every remaining biome-specific surface mutation needed for exhaustive badlands / frozen / mushroom follow-through.
- The current carved-stage oracle matrix is still too small. Three committed AIR-only carved chunks plus one AIR-plus-LIQUID ocean chunk are useful smoke coverage, not exhaustive parity coverage.
- The TS path still collapses some vanilla block-state distinctions in the carved-stage numeric model, which is acceptable for narrow chunk diffs but not the final form of exhaustive parity work.

## Current oracle coverage

Current carver verification is real but still intentionally narrow:

- three committed AIR-only carved-stage oracle fixtures:
  [`overworld-seed-12345-chunks-0-0-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-0-0-carved-only.json)
  , [`overworld-seed-12345-chunks-117--128-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-117--128-carved-only.json),
  and [`overworld-seed-12345-chunks-96--64-carved-only.json`](../test/fixtures/integration/overworld-seed-12345-chunks-96--64-carved-only.json)
- one committed AIR-plus-LIQUID ocean fixture:
  [`overworld-seed-12345-chunks-117--128-liquid-carved.json`](../test/fixtures/integration/overworld-seed-12345-chunks-117--128-liquid-carved.json)
- chunk-level parity assertion in [`noise-based-chunk-generator.test.ts`](../test/worldgen/levelgen/noise-based-chunk-generator.test.ts)
- fixture-shape pinning in [`carver-oracle-fixture.test.ts`](../test/worldgen/levelgen/carver-oracle-fixture.test.ts)
- explicit replaceable-material parity tests in [`world-carver-material-parity.test.ts`](../test/worldgen/carver/world-carver-material-parity.test.ts)
- explicit underwater branch tests in [`underwater-carver.test.ts`](../test/worldgen/carver/underwater-carver.test.ts)
- biome/carver wiring tests in [`overworld-carver-wiring.test.ts`](../test/worldgen/carver/overworld-carver-wiring.test.ts)

That supports “partially oracled,” not “exhaustively covered.”

## Recommended next slices

The highest-value sequence from here is:

1. Broaden LIQUID-stage oracle coverage beyond the current one-ocean-fixture water-fill case into a chunk that actually commits `obsidian` / `magma_block` floor output.
2. Capture scheduled underwater tick consequences explicitly if we want to describe the LIQUID path as full parity rather than block-output parity.
3. Broaden carved-stage oracle coverage beyond the current spawn/ocean/desert matrix into additional land material families once their pre-carve surface mutations are represented cleanly.
4. Add browser validation aimed at exposed cave mouths / ravines after each substantial carver change, not only vegetation-heavy smoke frames.

## Practical definition of “full parity”

For this repo, carvers should only be described as full-parity when all of the following are true:

- AIR and LIQUID carver steps are both ported for the 1.17.1 overworld target.
- Per-biome configured-carver selection matches vanilla through biome generation settings rather than ad hoc fallback logic.
- The carveable/replaceable material set matches vanilla for the current target.
- Underwater scheduled water/magma tick behavior is either modeled or intentionally measured and documented as an accepted divergence.
- The carved-stage oracle suite covers multiple biome/material families, not just one chunk near spawn.
- Generated browser frames have been manually inspected for cave mouths/ravines after each substantial carver change.
