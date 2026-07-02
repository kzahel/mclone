# Oracle Harness

Small Java fixtures live here so the native Rust parity code can compare against real Minecraft 1.17.1 behavior.

## PRNG oracle

`pnpm --silent oracle:gen prng --seed 12345 --count 10000 > test/fixtures/prng/seed-12345.json`

The dumper targets `net.minecraft.world.level.levelgen.SimpleRandomSource`, which is the classic 48-bit Java LCG used by Minecraft 1.17.1. In later Mojang mappings, this same legacy generator is named `LegacyRandomSource`.

Use `pnpm --silent` when redirecting to a file; otherwise `pnpm` prepends its own script banner ahead of the JSON stream.

Each output array is generated from a fresh `SimpleRandomSource(seed)` instance:

- `nextInt`: JSON numbers
- `nextLong`: signed decimal strings, to preserve full 64-bit precision
- `nextDouble`: JSON numbers from `Double.toString(...)`

The fixture metadata records the exact class and wire format so fixture consumers can load longs as `BigInt` or another lossless representation.

## ImprovedNoise oracle

`pnpm --silent oracle:gen noise --seed 12345 > test/fixtures/noise/seed-12345.json`

The dumper builds `net.minecraft.world.level.levelgen.synth.ImprovedNoise` from a seeded `SimpleRandomSource` and samples it on a fixed 11x11x11 dyadic grid:

- `x`: `-2.5` to `2.5` in steps of `0.5`
- `y`: `-1.875` to `1.875` in steps of `0.375`
- `z`: `-3.125` to `3.125` in steps of `0.625`

The fixture stores:

- constructor offsets `xo`, `yo`, `zo`
- the three coordinate axes
- flattened `values` in `x-major,y-major,z-minor` order

## PerlinNoise oracle

`pnpm --silent oracle:gen noise --class PerlinNoise --seed 12345 --octaves -7,-6,-5,-4,-3,-2,-1,0 --samples test/fixtures/noise/_samples-3d.json > test/fixtures/noise/perlin-seed-12345-oct-m7-0.json`

The shared sample grid in `test/fixtures/noise/_samples-3d.json` stores the `gridOrder` plus reusable `x`, `y`, and `z` axes. The dumper expands that into flattened `values` using the same `x-major,y-major,z-minor` order as the `ImprovedNoise` fixtures.

Current tactical/01 coverage includes both `PerlinNoise` octave sets used downstream: `[-7..0]` and `[-15..0]`.

## SimplexNoise oracle

`pnpm --silent oracle:gen noise --class SimplexNoise --seed 12345 --samples2d test/fixtures/noise/_samples-2d.json --samples3d test/fixtures/noise/_samples-3d.json > test/fixtures/noise/simplex-seed-12345.json`

The `SimplexNoise` fixture stores both overloads in one file:

- `samples2d`: `getValue(x,z)` on the dedicated `test/fixtures/noise/_samples-2d.json` grid, flattened in `x-major,z-minor` order
- `samples3d`: `getValue(x,y,z)` on the shared `test/fixtures/noise/_samples-3d.json` grid, flattened in `x-major,y-major,z-minor` order

The top-level metadata also records constructor offsets `xo`, `yo`, and `zo`, so consumers can assert that construction consumed the PRNG identically before checking sampled values.

## BlendedNoise oracle

`pnpm --silent oracle:gen noise --class BlendedNoise --seed 12345 --samples test/fixtures/noise/_samples-cell-3d.json > test/fixtures/noise/blended-seed-12345.json`

The `BlendedNoise` fixture uses a dedicated integer cell grid because `sampleAndClampNoise(...)` is called from `NoiseSampler` with cell coordinates, not arbitrary floating-point positions.

Each fixture stores three named sample sets keyed by the real built-in `NoiseGeneratorSettings` tuples:

- `overworld`: shared by overworld and amplified
- `nether`: shared by nether and caves
- `end`: shared by end and floating islands

Each sample set records:

- the exact `(limitHorizontalScale, limitVerticalScale, mainHorizontalScale, mainVerticalScale)` parameters derived from `NoiseSampler`
- the integer `x`, `y`, `z` axes
- flattened `values` in `x-major,y-major,z-minor` order
- `blendFactorRange` and `blendRegionCounts`, so the TS tests can assert the fixture surface exercises both clamp short-circuit branches and the interior lerp path

## PerlinSimplexNoise oracle

`pnpm --silent oracle:gen noise --class PerlinSimplexNoise --seed 12345 --octaves -3,-2,-1,0 --samples2d test/fixtures/noise/_samples-2d.json > test/fixtures/noise/perlin-simplex-seed-12345-oct-m3-0.json`

Each `PerlinSimplexNoise` fixture stores one octave set for one seed and includes two 2D sample sets over `test/fixtures/noise/_samples-2d.json`:

- `samplesWithoutOffsets`: `getValue(x,y,false)`
- `samplesWithOffsets`: `getValue(x,y,true)`

Both use `x-major,z-minor` flattening. The surface-noise wrapper is intentionally not serialized separately because `getSurfaceNoiseValue(x,y,z,yMax)` is just `getValue(x,y,true) * 0.55`; the TS tests assert that delegation directly.

## NormalNoise oracle

`pnpm --silent oracle:gen noise --class NormalNoise --seed 12345 --first-octave -3 --amplitudes 1.0,0.0,2.0 --samples test/fixtures/noise/_samples-3d.json > test/fixtures/noise/normal-water-level-seed-12345.json`

Each `NormalNoise` fixture stores one seed plus one explicit amplitude configuration over the shared `test/fixtures/noise/_samples-3d.json` grid:

- `firstOctave`
- `amplitudes`
- flattened `values` from `getValue(x,y,z)` in `x-major,y-major,z-minor` order

Current tactical/02 coverage is intentionally limited to the three `NormalNoise` allocations that still happen in default 1.17.1 `NoiseBasedChunkGenerator` construction:

- `barrierNoise`: `firstOctave=-3`, amplitudes `1.0`
- `waterLevelNoise`: `firstOctave=-3`, amplitudes `1.0,0.0,2.0`
- `lavaNoise`: `firstOctave=-1`, amplitudes `1.0,0.0`

## NoiseSampler oracle

`pnpm --silent oracle:gen noise --class NoiseSampler --seed 12345 --preset overworld --samples2d test/fixtures/noise/_samples-cell-2d.json > test/fixtures/noise/noise-sampler-overworld-seed-12345.json`

`NoiseSampler` fixtures are intentionally scoped to the default 1.17.1 overworld path:

- built-in overworld `NoiseSettings`
- `NoiseModifier.PASSTHROUGH`
- no end-island override (`islandNoise == null`)

Each fixture stores one seed plus two named sample sets:

- `constantPlains`
- `mixedOverworld`

Each sample set includes:

- a repeating 5x5 biome pattern serialized as biome keys plus depth/scale values
- sampled density columns over `test/fixtures/noise/_samples-cell-2d.json`
- column values flattened in `x-major,z-minor,y-minor` order

This lets consumers oracle-test biome weighting, random-density offset, blended-noise integration, and slide application before the real biome-source port lands.

## OverworldBiomeSource oracle

`pnpm --silent oracle:gen biome --class OverworldBiomeSource --seed 12345 --samples2d test/fixtures/biome/_samples-quart-2d.json > test/fixtures/biome/overworld-seed-12345.json`

Each fixture stores one seed for the default 1.17.1 overworld layered biome source:

- `legacyBiomeInitLayer=false`
- `largeBiomes=false`
- the Java `possibleBiomes()` list with exact registry IDs, resource keys, `depth`, and `scale`
- sampled quart biomes over `test/fixtures/biome/_samples-quart-2d.json`, flattened in `x-major,z-minor` order

This is the tactical/05 oracle for:

- exact biome metadata parity
- layered biome-source seed parity
- chunk-biome container parity against committed integration fixtures

## Staged chunk oracles

The Java oracle harness also emits pinned chunk snapshots for the translated terrain/surface/carver stages:

- `pnpm --silent oracle:gen surface-chunk --seed 12345 --chunk-x 0 --chunk-z 0 > test/fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json`
- `pnpm --silent oracle:gen carved-chunk --seed 12345 --chunk-x 117 --chunk-z -128 > test/fixtures/integration/overworld-seed-12345-chunks-117--128-carved-only.json`
- `pnpm --silent oracle:gen liquid-carved-chunk --seed 12345 --chunk-x 117 --chunk-z -128 > test/fixtures/integration/overworld-seed-12345-chunks-117--128-liquid-carved.json`
- `pnpm --silent oracle:gen liquid-carved-chunk --seed 12345 --chunk-x -129 --chunk-z -256 > test/fixtures/integration/overworld-seed-12345-chunks--129--256-liquid-carved.json`

`carved-chunk` stays intentionally AIR-step-only so the older carved fixtures remain stable and step-scoped. `liquid-carved-chunk` applies AIR and LIQUID sequentially and is the committed oracle path for full classic-ocean carved-stage parity checks. Carved and liquid-carved fixtures now also include `blockTicks` / `liquidTicks`, which pin the generation-stage scheduled tick consequences alongside the carved block palette.

## Feature-order trace oracle

`pnpm --silent oracle:gen feature-order-trace --seed 12345 --chunk-x 0 --chunk-z 0`

This diagnostic emits the vanilla `ChunkGenerator.applyBiomeDecoration(...)` ordering for one center chunk:

- the chunk primary biome selected by `OverworldBiomeSource.getPrimaryBiome(...)`
- the `WorldgenRandom.setDecorationSeed(...)` result for the chunk origin
- every configured feature slot in each `GenerationStep.Decoration`
- the exact `WorldgenRandom.setFeatureSeed(decorationSeed, featureIndex, stepIndex)` value used before that feature places

Pass `--generate-structures true` to include the structure seed slots that vanilla consumes before configured features in the same decoration step when `StructureFeatureManager.shouldGenerateFeatures()` is true. This is a trace of feature seed/order coordination, not a block snapshot; use it when a decorated-chunk diff looks like the right features are present but placed from the wrong random stream.

## Scheduler trace oracle

`pnpm --silent oracle:gen scheduler-trace --seed 12345 --chunk-x 0 --chunk-z 0`

This diagnostic starts a temporary deobfuscated 1.17.1 dedicated server with seed `12345`, instruments the vanilla `ChunkStatus.generate(...)` entry point through a narrow oracle-only shadow class, and stops when the target 3x3 neighborhood around `(0,0)` completes `FEATURES`.

The committed fixture is:

- `test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json`
- `test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-12345-chunk-0-0.json` — generated with `--target-radius 1 --record-radius 1 --stop-status features --generate-structures false --dump-chunks true --dump-only-target-chunk true`; this is the clean apples-to-apples chunk-state oracle after the target 3x3 has reached `FEATURES`, but before runtime liquid ticks can spread fluids.
- `test/fixtures/scheduler/vanilla-scheduler-light-snapshot-seed-12345-chunk-0-0.json` — generated with `--target-radius 1 --record-radius 1 --stop-status light --generate-structures false --dump-chunks true --dump-only-target-chunk true`; this adds status-aligned live Java light `DataLayer`s for strict light-section and byte parity without comparing against a later persisted Anvil snapshot.

The fixture records dependency-ready, task-start, and task-complete events for the target 3x3. The observed `FEATURES` completion/commit order in that fixture is:

```text
(-1,-1) -> (0,-1) -> (1,-1) -> (-1,0) -> (0,0) -> (1,0) -> (-1,1) -> (0,1) -> (1,1)
```

Options:

- `--stop-status full` continues the run until the target 3x3 reaches `FULL`.
- `--stop-status light` continues the run until the target 3x3 reaches `LIGHT`.
- `--dump-chunks true` adds palette-compressed chunk sections, optional live light payloads when the dumped chunk is light-correct, plus pending block/liquid ticks captured at the requested stop status. Use this for generation-status block and light parity; the Anvil integration fixture is a broader server-startup snapshot and can include later runtime effects.
- `--dump-only-target-chunk true` keeps `--target-radius` as the completion gate but stores only the requested center chunk in the snapshot.
- `--record-radius 12` records the wider dependency-window event stream instead of only the target 3x3.
- `--view-distance <n>` writes the temporary server `view-distance` property; default is `3`.
- `--timeout-seconds <n>` changes the server-process timeout.
- `--probe-target-tree-blocks true` switches `FEATURES` decoration to feature-by-feature probe mode and records all target-chunk `spruce_log` / `spruce_leaves` blocks after each configured feature. This is intended for focused taiga tree diagnostics; it is too verbose for the committed scheduler-order fixture.
- `--probe-tree-candidates true` records final selected `minecraft:tree` attempts from the decorated feature stack. Combine it with `--tree-probe-center-x`, `--tree-probe-center-z`, `--tree-probe-step-index`, and `--tree-probe-feature-index` to keep output focused. Each `tree_candidate` event includes origin, pine/spruce discriminator via foliage placer class, tree dimensions, placement result, random-count range, and rejection details such as `sapling_cannot_survive` or the first block that limited `max_free_tree_height`.
- `--probe-ore-placements true` records final selected `minecraft:ore` branches from the decorated feature stack. Combine it with `--ore-probe-center-x`, `--ore-probe-center-z`, `--ore-probe-step-index`, and `--ore-probe-feature-index` to keep output focused. Each `ore_placement` event includes the branch origin, ore targets, vein geometry, random-count range, and target-chunk writes.

The trace root includes `totalElapsedMs`, and each scheduler event includes
`elapsedMs` from the trace recorder's JVM start. These are intended for local
performance comparison only; committed parity fixtures should still avoid
asserting exact timings.

Example focused taiga candidate trace:

```bash
pnpm --silent oracle:gen scheduler-trace \
  --seed 12345 --chunk-x 0 --chunk-z 1 \
  --target-radius 1 --record-radius 1 --stop-status features \
  --generate-structures false \
  --probe-target-tree-blocks true \
  --probe-tree-candidates true \
  --tree-probe-center-x 0 \
  --tree-probe-center-z 1 \
  --tree-probe-step-index 8 \
  --tree-probe-feature-index 2
```

The child JVM is pinned with `-XX:ActiveProcessorCount=2` so vanilla's background executor has one worker, and `-XX:hashCode=3` so identity-hashed scheduler collections iterate reproducibly. In sandboxed environments the temporary server may need elevated permission to bind its localhost listener.

## Wrappers

### Vanilla client screenshot oracle

The first no-Prism vanilla client launcher scaffold lives in
`oracle/vanilla-client-launch.mjs`.

```bash
node oracle/vanilla-client-launch.mjs --print-command
node oracle/vanilla-client-launch.mjs --print-args
node oracle/vanilla-client-launch.mjs --hydrate
node oracle/vanilla-client-launch.mjs --launch
```

The default action only prints the Java command. `--launch` is explicit because
it can open a real LWJGL window. The harness uses the deobfuscated
`client-deobf.jar` plus Mojang's 1.17.1 libraries/assets and a dummy local
session, so Prism Launcher is not part of the normal oracle command path.
- `oracle/build.sh`: hydrates Mojang-declared runtime libraries into `reference/minecraft-1.17.1/libraries`, then compiles `oracle/java/*.java` into `oracle/classes`
- `oracle/run.sh`: builds if needed, then runs `OracleDumper`

## Integration oracle (server-side)

A second oracle tier runs the official 1.17.1 server jar headless against a pinned seed, then decodes the generated `region/*.mca` files into committed chunk fixtures. These fixtures let native parity work diff real Minecraft chunks against the translated `NoiseBasedChunkGenerator` path.

Pieces:

- `scripts/fetch-server-jar.sh 1.17.1` — SHA1-verified download of the server jar (Mojang manifest). Idempotent. Writes to `reference/minecraft-1.17.1/server.jar`.
- `oracle/integration/run-server.sh --seed <long>` — spawns the server with a pinned seed, waits for the `Done (` startup line, sends `stop`, and leaves the generated `world/region/*.mca` files on disk. Prints the working directory to stdout. Pass `--scheduler-pins` to use the same JVM scheduler pins as the scheduler-trace oracle.
- `oracle/integration/dump-chunks.ts` — Node CLI that reads region files and emits a committable JSON fixture. `gen-fixture.sh` runs it with Node transform-types support so it can import the oracle-owned TypeScript helpers.
- `oracle/integration/gen-fixture.sh --seed <long> --chunks <x,z,x,z,...> --out <path>` — end-to-end orchestration: ensures the server jar exists, runs the server, and decodes the requested chunks.

The reader and fixture builder live under `oracle/lib/anvil/` and `oracle/lib/integration/`. They are oracle-owned TypeScript tooling, not part of the retired browser engine; validate them through fixture generation or direct Node loader imports.

### Regenerating an integration fixture

```bash
./oracle/integration/gen-fixture.sh \
    --scheduler-pins \
    --seed 12345 \
    --chunks 0,0 \
    --out test/fixtures/integration/overworld-seed-12345-chunks-0-0.json
```

Fixtures are factual measurements and therefore safe to distribute even though the server jar itself isn't.
For full decorated fixtures, use `--scheduler-pins` unless the fixture is intentionally documenting an unpinned server run. Vanilla edge decoration writes are sensitive to scheduler run shape.

### Fixture format

Each fixture stores one seed + one generator configuration + one or more chunks. For each chunk:

- `status` — always `"full"` in committed fixtures (partially-generated border chunks are rejected)
- `isLightOn` — vanilla's persisted light-validity flag from `Level.isLightOn`; future light comparisons should require this to be `true`
- `sections[]` — each section records `y`, `palette` (resource keys only; 1.17.1 MVP drops block-state properties), and a decoded 4096-entry `blocks` array (palette indices, `y-major,z-major,x-minor`)
- `light` — optional persisted vanilla light payload. Present chunks store `light.block[]` and `light.sky[]` section records with signed section `y` and `dataBase64`, which encodes exactly one 2048-byte `DataLayer`.
- `heightmaps` — decoded `WORLD_SURFACE` / `OCEAN_FLOOR` / etc. as 16×16 arrays in `z-major,x-minor` row-major order
- `biomes` — the native 4×4×4 biome grid 1.17.1 writes as 1024 ints

See `docs/tactical/04-integration-oracle-harness.md` for the full shape and design rationale.

## Generation creature oracle (server-side)

Creature-generation fixtures use the same official 1.17.1 server runner, but dump entity chunk storage from `world/entities/*.mca` instead of block chunk sections. The normalized fixture shape is `module: "creature-generation"` and intentionally omits UUIDs, motion, attributes, brain data, equipment, passengers, and other full-NBT fields that are not yet owned by the native runtime.

Use scan mode first because generation-time passive mobs are probabilistic:

```bash
./oracle/integration/gen-creature-fixture.sh \
    --seed 12345 \
    --scan \
    --out /tmp/mclone-creature-scan-seed-12345.json
```

Then commit a selected non-empty chunk fixture:

```bash
./oracle/integration/gen-creature-fixture.sh \
    --seed 12345 \
    --chunks -7,-15 \
    --out test/fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json
```

Pieces:

- `oracle/integration/dump-creature-fixture.ts` — reads `world/entities/*.mca`, falls back to legacy/proto `Level.Entities` when needed, and emits fixture or scan JSON.
- `oracle/lib/anvil/entity-chunk.ts` — decodes vanilla entity chunk NBT (`DataVersion`, `Position`, `Entities`) and legacy chunk entity lists.
- `oracle/lib/integration/creature-fixture.ts` — normalizes stable entity facts, maps common mob categories, sorts records, and compares fixtures with readable diffs.

The first committed fixture is `test/fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json`, selected from scan output because it contains generated sheep.

## Dynamic liquid oracle (server-side)

Liquid simulation fixtures use the official 1.17.1 server too, but instead of dumping whole chunks at startup they inject a generated datapack, run a scripted scenario for an exact number of server ticks, save, stop, and dump only a bounded block-state region plus persisted `LiquidTicks`.

Regenerate the committed Liquid0 fixture with:

```bash
./oracle/integration/gen-liquid-fixture.sh \
    --scenario test/fixtures/liquid-scenarios/water-slope.json \
    --out test/fixtures/liquid/water-slope-10-ticks.json
```

Pieces:

- `test/fixtures/liquid-scenarios/*.json` — scenario name, seed, tick count, setup commands, dump bounds, and tick margin.
- `oracle/integration/prepare-liquid-server.ts` — writes `server.properties`, `eula.txt`, and a generated datapack with load/tick functions.
- `oracle/integration/run-liquid-server.sh` — starts the official server and waits for the datapack to `save-all flush` and `stop`.
- `oracle/integration/dump-liquid-fixture.ts` — reads the Anvil region files and emits `module: "liquid-sim"` JSON.
- `oracle/lib/integration/liquid-fixture.ts` — property-preserving fixture builder, persisted liquid tick decoder, and diff helpers.

Liquid fixtures flatten `blocks` in `y-major,z-major,x-minor` order, preserve palette entries as `{ name, properties }`, and sort pending `liquidTicks` by remaining delay, priority, target, then position. The first committed fixture is `test/fixtures/liquid/water-slope-10-ticks.json`.
