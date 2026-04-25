# Tactical 50 - Beach/river full decorated parity

Expand the exact full-decorated parity harness from the simple spawn baseline to one legitimate sand/gravel boundary target.

Status: next parity target. Tactical [`46`](46-full-decorated-spawn-chunk-parity.md) proves seed `12345`, chunk `(0,0)` can match a scheduler-pinned official-server fixture at `65,536 / 65,536` blocks after the fixture-equivalent generated-liquid tick window. This slice should test whether that result survives a chunk where sand and gravel are expected, not just rejected as dry-land regressions.

## Target

Use seed `12345`, chunk `(5,115)`.

Why this chunk:

- `test/fixtures/integration/overworld-seed-12345-chunks-5-115-surface-only.json` already exists and is covered by the surface oracle tests.
- The current surface tests describe it as the narrow sand-and-gravel oracle, so it is a better second full-decorated target than another grassland/taiga spawn-adjacent chunk.
- It directly exercises the risk left by tactical 46: the runtime can reject unexpected dry-land sand in the spawn fixture, but it still needs full decorated proof that legitimate shoreline/river sand, gravel, fluids, soft disks, and nearby vegetation remain exact together.

## Source files

Read these before writing code:

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/chunk/ChunkGenerator.java` (`applyBiomeDecoration(...)`) | `src/worldgen/levelgen/noise-based-chunk-generator.ts`, `src/runtime/host/generated-world-host.ts` |
| `reference/.../src/net/minecraft/world/level/biome/Biome.java` (`generate(...)`) | `src/worldgen/biome/biome.ts` |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/DiskReplaceFeature.java` | `src/worldgen/levelgen/feature/` disk/soft-disk ports |
| `reference/.../src/net/minecraft/world/level/levelgen/feature/TreeFeature.java` and local vegetation/decorator sources reached by the target biomes | `src/worldgen/levelgen/feature/`, `src/worldgen/levelgen/placement/` |
| `reference/.../src/net/minecraft/world/level/levelgen/carver/` for any fluid-edge mismatch | `src/worldgen/carver/` |

Do not port disabled Caves & Cliffs Part 1 systems from [`../../AGENTS.md`](../../AGENTS.md).

## Oracle inputs

Start from the existing staged oracle:

```bash
pnpm test -- test/worldgen/levelgen/noise-based-chunk-generator.test.ts test/worldgen/levelgen/surface-oracle-fixture.test.ts
```

Generate the full decorated server fixture with the same run-shape discipline used by tactical 46:

```bash
./oracle/integration/gen-fixture.sh \
    --scheduler-pins \
    --seed 12345 \
    --chunks 5,115 \
    --out test/fixtures/integration/overworld-seed-12345-chunks-5-115.json
```

If the first decorated diff shows cross-chunk edge writes, record the same-run scheduler evidence before changing runtime order:

```bash
pnpm --silent oracle:gen scheduler-trace --seed 12345 --chunk-x 5 --chunk-z 115
pnpm --silent oracle:gen feature-order-trace --seed 12345 --chunk-x 5 --chunk-z 115
```

## Plan

1. Add the scheduler-pinned full decorated fixture for seed `12345`, chunk `(5,115)`.
2. Reuse the full-block diff helpers from tactical 46 and add a target-specific runtime test.
3. Measure both the static post-generation diff and the `liquidSimulationMode: vanilla17` diff after deterministic host ticks. Do not assume the spawn chunk's `10`-tick window is sufficient until the matrix says so.
4. Burn down mismatches in this order:
   - legitimate sand/gravel and soft-disk placement
   - fluid-edge and generated-liquid tick timing
   - tree/log/leaf and neighboring feature writes
   - plants, snow/freezing, underground helpers, and ores
5. Keep the existing spawn fixture exact while ratcheting the new fixture.

## Validation

Required:

- `pnpm test -- test/runtime/generated-world-boundary.test.ts`
- `pnpm test -- test/worldgen/levelgen/noise-based-chunk-generator.test.ts test/worldgen/levelgen/surface-oracle-fixture.test.ts`
- targeted feature/decorator tests for every code fix made during the slice
- `pnpm typecheck` if TypeScript files are touched
- `git diff --check`

Run browser validation only if the change materially affects rendered pixels:

- `pnpm test:browser`
- the smallest relevant `pnpm probe:browser -- test/browser/probes/<name>.probe.ts`, with screenshots saved to `/tmp`

## Done when

- the new full decorated runtime chunk `(5,115)` for seed `12345` matches the scheduler-pinned official-server fixture at every block position after the measured fixture-equivalent tick window
- the exact comparison is encoded in a normal test
- the existing seed `12345`, chunk `(0,0)` exact fixture test still passes
- the existing sand/gravel surface oracle stays exact
- docs record any fixture/run-shape caveats discovered during the burn-down
