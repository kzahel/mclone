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

## Wrappers

- `oracle/build.sh`: hydrates Mojang-declared runtime libraries into `reference/minecraft-1.17.1/libraries`, then compiles `oracle/java/*.java` into `oracle/classes`
- `oracle/run.sh`: builds if needed, then runs `OracleDumper`
