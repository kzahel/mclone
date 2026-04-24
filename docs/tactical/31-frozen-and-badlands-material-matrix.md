# Tactical 31 — Frozen and badlands material matrix

Broaden the carved-stage material/state matrix beyond the spawn/desert/ocean/floor baseline from tacticals 28-30. This slice ports the frozen-ocean and badlands surface follow-through that vanilla 1.17.1 still feeds into classic carvers, widens the render/runtime/oracle palette for those materials, adds committed frozen and badlands fixtures, and verifies the new surface families visually in the browser.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/levelgen/surfacebuilders/{FrozenOceanSurfaceBuilder,BadlandsSurfaceBuilder,WoodedBadlandsSurfaceBuilder,ErodedBadlandsSurfaceBuilder}.java` | `src/worldgen/surface/surface-builders.ts` |
| `reference/.../src/net/minecraft/world/level/biome/Biome.java` (`TemperatureModifier.FROZEN`) | `src/worldgen/biome/{biome,biome-data}.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/carver/WorldCarver.java` | `src/worldgen/carver/world-carver.ts` |
| `oracle/java/OracleDumper.java` | `oracle/java/OracleDumper.java` |
| `src/worldgen/chunk/chunk-block-buffer.ts` | same |
| `src/world/level/generated-render-blocks.ts` | same |
| `test/worldgen/levelgen/{surface-oracle-fixture,carver-oracle-fixture,noise-based-chunk-generator}.test.ts` | same |
| `test/renderer/generated-render-level.test.ts`, `test/browser/probes/surface-material-matrix.probe.ts` | same |

## What landed

- `ChunkBlockId` / `CHUNK_BLOCK_NAMES` now include the remaining frozen/badlands surface and carved-stage materials needed by the live overworld path in this repo: `red_sand`, `ice`, and `snow_block` on top of the earlier terracotta / packed-ice expansion.
- `Biome` now ports the vanilla frozen-temperature modifier path, so frozen-ocean surface logic can use `Biome.getTemperature(...)` instead of the old fixed-temperature shortcut.
- `surface-builders.ts` now ports the frozen-ocean and badlands surface families that were still missing from the TS surface path: badlands, wooded badlands, eroded badlands, frozen ocean, and deep frozen ocean.
- The generated render palette now has real registered/runtime-renderable states for the widened frozen/badlands materials, including translucent `ice`.
- The Java oracle surface/carved palette now matches that widened material set, and the committed fixture matrix adds:
  - a frozen-ocean surface chunk at `(-247, -247)`
  - a badlands surface chunk at `(-320, 99)`
  - a badlands AIR-step carved chunk at `(-320, 99)`
- The frozen-ocean RNG branch now matches the Mojang source exactly by comparing the iceberg air/water fill branches against the integer-truncated iceberg bounds, which keeps the shared `WorldgenRandom` aligned for the later bedrock pass.
- `LakeFeature` now uses the registered `minecraft:air` block state instead of a private `AirBlock` singleton, so generated render-level snapshots can serialize scheduled block ticks through the same registry-backed block identity as the rest of the runtime.

## Scope choice

Landed here on purpose:

- port the missing frozen/badlands surface builders that still gate real carver-relevant material parity
- widen the runtime/oracle/render palette only for the frozen/badlands families needed by those ports
- add one frozen surface oracle and one badlands surface/carved oracle pair
- add browser validation aimed at those surface families instead of another generic terrain smoke

Still deferred on purpose:

- remaining carver-relevant surface families that still throw in `surface-builders.ts`, especially giant-tree taiga / shattered savanna / mushroom follow-through (`podzol`, `coarse_dirt`, `mycelium`)
- every remaining block-state distinction that vanilla tracks but this repo still flattens in the numeric chunk model
- broader biome-decoration parity work such as dark forest, jungle, and savanna feature tables

## Oracle / done-when

**Unit + integration (Vitest):**

- surface-oracle and chunk-generator parity suites cover the frozen-ocean and badlands surface fixtures exactly
- carved-oracle and chunk-generator parity suites cover the new badlands AIR-step carved fixture
- generated-render-level tests prove the widened render palette is registered and still survives decoration/snapshot paths

**Java oracle fixtures:**

- regenerate every committed surface/carved/liquid fixture whose palette changed because the widened IDs are shared
- add the new frozen and badlands fixtures from the Java oracle harness, not handwritten JSON

**Browser validation:**

- `pnpm probe:browser -- test/browser/probes/surface-material-matrix.probe.ts` passes
- `/tmp/mclone-debug-frozen-ocean.png` and `/tmp/mclone-debug-badlands.png` are manually inspected for the intended surface families

## Done when

- `pnpm typecheck` passes
- focused `pnpm test` passes for chunk-buffer, surface-oracle, carved-oracle, chunk-generator parity, and generated-render-level suites
- `pnpm probe:browser -- test/browser/probes/surface-material-matrix.probe.ts` passes
- `docs/tactical/README.md`, `docs/carver-status.md`, and `docs/worldgen-status.md` describe the widened frozen/badlands matrix accurately

## Next

Tactical 32 is now [`32-podzol-coarse-dirt-and-mycelium-matrix.md`](32-podzol-coarse-dirt-and-mycelium-matrix.md): port giant-tree taiga / shattered savanna / mushroom surface follow-through, add companion oracle fixtures for `podzol` / `coarse_dirt` / `mycelium`, restore the matching `LakeFeature` follow-through, and only then demote carver parity below the next biome-decoration slice.
