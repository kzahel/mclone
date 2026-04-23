# Oracle Harness

Small Java fixtures live here so the TypeScript port can compare against real Minecraft 1.17.1 behavior.

## PRNG oracle

`pnpm --silent oracle:gen prng --seed 12345 --count 10000 > test/fixtures/prng/seed-12345.json`

The dumper targets `net.minecraft.world.level.levelgen.SimpleRandomSource`, which is the classic 48-bit Java LCG used by Minecraft 1.17.1. In later Mojang mappings, this same legacy generator is named `LegacyRandomSource`.

Use `pnpm --silent` when redirecting to a file; otherwise `pnpm` prepends its own script banner ahead of the JSON stream.

Each output array is generated from a fresh `SimpleRandomSource(seed)` instance:

- `nextInt`: JSON numbers
- `nextLong`: signed decimal strings, to preserve full 64-bit precision
- `nextDouble`: JSON numbers from `Double.toString(...)`

The fixture metadata records the exact class and wire format so the TypeScript side can load longs as `BigInt` or another lossless representation.

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

The top-level metadata also records constructor offsets `xo`, `yo`, and `zo`, so the TS side can assert that construction consumed the PRNG identically before checking sampled values.

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

This lets the TS side oracle-test biome weighting, random-density offset, blended-noise integration, and slide application before the real biome-source port lands.

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

## Wrappers

- `oracle/build.sh`: hydrates Mojang-declared runtime libraries into `reference/minecraft-1.17.1/libraries`, then compiles `oracle/java/*.java` into `oracle/classes`
- `oracle/run.sh`: builds if needed, then runs `OracleDumper`

## Integration oracle (server-side)

A second oracle tier runs the official 1.17.1 server jar headless against a pinned seed, then decodes the generated `region/*.mca` files into committed chunk fixtures. Used by upcoming tactical docs (starting with tactical `06`) to diff real Minecraft chunks against our TS port of `NoiseBasedChunkGenerator`.

Pieces:

- `scripts/fetch-server-jar.sh 1.17.1` — SHA1-verified download of the server jar (Mojang manifest). Idempotent. Writes to `reference/minecraft-1.17.1/server.jar`.
- `oracle/integration/run-server.sh --seed <long>` — spawns the server with a pinned seed, waits for the `Done (` startup line, sends `stop`, and leaves the generated `world/region/*.mca` files on disk. Prints the working directory to stdout.
- `oracle/integration/dump-chunks.ts` — Node CLI that reads region files and emits a committable JSON fixture. Requires Node 22.6+ (native TypeScript strip-types support).
- `oracle/integration/gen-fixture.sh --seed <long> --chunks <x,z,x,z,...> --out <path>` — end-to-end orchestration: ensures the server jar exists, runs the server, and decodes the requested chunks.

The reader + fixture builder live under `src/oracle/anvil/` and `src/oracle/integration/` so they get Vitest coverage (see `test/oracle/`).

### Regenerating an integration fixture

```bash
./oracle/integration/gen-fixture.sh \
    --seed 12345 \
    --chunks 0,0 \
    --out test/fixtures/integration/overworld-seed-12345-chunks-0-0.json
```

Fixtures are factual measurements and therefore safe to distribute even though the server jar itself isn't.

### Fixture format

Each fixture stores one seed + one generator configuration + one or more chunks. For each chunk:

- `status` — always `"full"` in committed fixtures (partially-generated border chunks are rejected)
- `sections[]` — each section records `y`, `palette` (resource keys only; 1.17.1 MVP drops block-state properties), and a decoded 4096-entry `blocks` array (palette indices, `y-major,z-major,x-minor`)
- `heightmaps` — decoded `WORLD_SURFACE` / `OCEAN_FLOOR` / etc. as 16×16 arrays in `z-major,x-minor` row-major order
- `biomes` — the native 4×4×4 biome grid 1.17.1 writes as 1024 ints

See `docs/tactical/04-integration-oracle-harness.md` for the full shape and design rationale.
