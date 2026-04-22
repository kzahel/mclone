# 01 — Octaved noise + Blended noise

Continuing from [`00-worldgen-ts-port.md`](00-worldgen-ts-port.md). Ports the three `synth/` classes that `NoiseSampler` directly consumes. After this doc, `NoiseSampler` (tactical 03) has all its noise primitives available.

## Goal

TS ports of `PerlinNoise`, `SimplexNoise`, and `BlendedNoise` — byte-identical (f64 exact) against Java fixtures for the same seeds and sample points.

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | `PerlinNoise` | `ImprovedNoise` (done), `RandomSource` (done) | seed `S` + octave set (e.g. `[-7..0]`) + sample points `(x,y,z)` → `f64` |
| 2 | `SimplexNoise` | `RandomSource` | seed `S` + sample points `(x,y,z)` → `f64` (also `getValue(x,z)` 2D) |
| 3 | `BlendedNoise` | `PerlinNoise` | seed `S` + `sampleAndClampNoise(x,y,z, hScale, vScale, mainH, mainV)` → `f64` |

Order matters: `PerlinNoise` is a prerequisite for `BlendedNoise`. `SimplexNoise` is independent and can be done in parallel.

## Current status

- Tactical 00 prerequisites are complete: oracle harness, `SimpleRandomSource`, and `ImprovedNoise`
- `PerlinNoise` is fixture-backed for both octave sets used downstream: `[-7..0]` and `[-15..0]`, across canonical seeds `0`, `1`, `12345`, and `2151901553968352745`
- `SimplexNoise` is now fixture-backed for both entry points: 2D `getValue(x,z)` on a dedicated shared `x/z` grid and 3D `getValue(x,y,z)` on the shared 3D grid, again across canonical seeds `0`, `1`, `12345`, and `2151901553968352745`
- `BlendedNoise` is now fixture-backed across the canonical seeds and the three real `NoiseSampler` sampling tuples: overworld/amplified, nether/caves, and end/floating-islands
- Tactical `01` is complete; next up is [`02-remaining-synth.md`](02-remaining-synth.md)

## Why these three

`NoiseSampler.java` imports exactly these three classes from `synth/` (verified). The other `synth/` classes — `SurfaceNoise`, `PerlinSimplexNoise`, `NormalNoise`, `NoiseUtils` — are consumed outside `NoiseSampler`. Tactical 02 picks up only the pieces alive in default 1.17.1 worldgen: `SurfaceNoise` (interface), `PerlinSimplexNoise` (overworld surface noise + badlands/frozen-ocean surface builders), and `NormalNoise` scoped to the `barrier`/`waterLevel`/`lava` allocations in `NoiseBasedChunkGenerator`. `NoiseUtils` and the rest of the C&C Part 1 consumers (`Cavifier`, `NoodleCavifier`, `OreVeinifier`, active `Aquifer`) stay deferred — see [`AGENTS.md`](../../AGENTS.md).

## Oracle extension

Extend the existing Java dumper with a `noise` subcommand. Shape:

```
pnpm --silent oracle:gen noise \
    --class PerlinNoise \
    --seed 12345 \
    --octaves -7,-6,-5,-4,-3,-2,-1,0 \
    --samples test/fixtures/noise/_samples-3d.json \
    > test/fixtures/noise/perlin-seed-12345-oct-m7-0.json
```

Fixture JSON (current shape):

```json
{
  "noiseClass": "net.minecraft.world.level.levelgen.synth.PerlinNoise",
  "seed": "12345",
  "octaves": [-7,-6,-5,-4,-3,-2,-1,0],
  "gridOrder": "x-major,y-major,z-minor",
  "x": [0.0, 0.5],
  "y": [64.0, 64.5],
  "z": [0.0, 0.5],
  "values": [0.123456789]
}
```

**f64 serialization gotcha:** serialize `double` as full-precision text (`Double.toString` or `%.17g`) so we round-trip bit-exact through JSON. Float `parseFloat` / `Number()` in TS preserves f64 precision — just don't truncate on the way out.

**Seed serialization:** same as PRNG fixtures — stringify `long` to avoid JSON number precision loss.

## Translation gotchas

- **`PerlinNoise` amplitudes.** The constructor takes an arbitrary set of octave integers (not just "N octaves"). Highest octave index gets amplitude 1.0; each step down halves the input frequency and doubles the value factor. Read `PerlinNoise.java:60-90` ish and mirror exactly — this is the most likely place for off-by-one in the octave range.
- **PRNG consumption order matters.** `PerlinNoise` constructs N `ImprovedNoise` instances sequentially from a single `RandomSource`. The order they consume PRNG state is significant — if we construct in a different order than Java, the per-octave permutation tables diverge, and downstream noise values are wrong. Oracle-test with the actual constructor paths, not just "a seed and a sample point."
- **`IntStream.rangeClosed(-15, 0)`** means the set `{-15, -14, ..., -1, 0}` (16 octaves). `BlendedNoise`'s default constructor uses `(-15, 0)` for both limit noises and `(-7, 0)` for main. Verify our TS translation handles these ranges identically.
- **`SimplexNoise` gradient table.** 1.17.1's `SimplexNoise` uses a 12-gradient scheme (Ken Perlin's classic). Don't accidentally use the 16- or 8-gradient variants that exist in some references.
- **`BlendedNoise.sampleAndClampNoise`** has specific clamping behavior once the min/max blend fraction crosses 0 or 1 — it short-circuits and skips computing the unused side. Mirror that short-circuit exactly; even though the *final* value would be the same mathematically, the PRNG isn't advanced (wait — no PRNG at sample time, only at construction). Correction: the short-circuit affects which octaves get summed; verify that. Test with sample points on both sides of the clamp boundary.
- **f64 tolerance: zero.** Same IEEE 754 on both sides. If values differ in the last ULP, we have a bug, not a rounding issue.

## Directory layout (additions)

```
src/worldgen/noise/
  perlin-noise.ts
  simplex-noise.ts
  blended-noise.ts
test/worldgen/noise/
  perlin-noise.test.ts
  simplex-noise.test.ts
  blended-noise.test.ts
test/fixtures/noise/
  perlin-seed-{seed}-oct-{octaves}.json
  simplex-seed-{seed}.json
  blended-seed-{seed}.json
oracle/java/
  OracleDumper.java   (extended with `noise` subcommand)
```

## Concrete steps

1. **Extend the Java dumper** with a `noise` subcommand. Support `--class PerlinNoise|SimplexNoise|BlendedNoise`, seed, octave list (for Perlin), and a reusable sample-grid file.
2. **Generate canonical sample grids.** One shared 3D grid file currently lives at `test/fixtures/noise/_samples-3d.json`. It covers positive / negative / zero coordinates with non-integer `x/y/z`, and expands to 1331 samples.
3. **Emit fixtures** for each class × canonical seeds (the same four seeds as PRNG: `0`, `1`, `12345`, `2151901553968352745`). `PerlinNoise` coverage is complete for both required octave sets: `[-7..0]` and `[-15..0]`.
4. **Port `PerlinNoise`** — done and fixture-backed for both octave ranges used by `NoiseSampler` and `BlendedNoise`.
5. **Port `SimplexNoise`** — done and fixture-backed for both the 2D and 3D `getValue(...)` overloads.
6. **Port `BlendedNoise`** — done and fixture-backed against the three built-in sampling tuples that `NoiseSampler` actually derives from `NoiseGeneratorSettings`.

## Done when

- `pnpm test` passes assertions:
  - `PerlinNoise` matches Java fixture at ≥1000 sample points × 4 seeds × 2 octave ranges (zero f64 epsilon)
  - `SimplexNoise` matches at ≥1000 points × 4 seeds (zero f64 epsilon)
  - `BlendedNoise` matches at ≥1000 points × 4 seeds × a handful of `(hScale, vScale, mainH, mainV)` tuples (cover the common range `NoiseSampler` actually calls with)
- `pnpm oracle:gen noise --class PerlinNoise ...`, `--class SimplexNoise ...`, and `--class BlendedNoise ...` regenerate fixtures from scratch
- `oracle/README.md` is updated with the new subcommand
- Next tactical doc is drafted as [`02-remaining-synth.md`](02-remaining-synth.md)

## Out of scope for this doc

- `NormalNoise`, `SurfaceNoise`, `PerlinSimplexNoise`, `NoiseUtils` — tactical 02
- `NoiseSampler` itself — tactical 03 (needs `NoiseSettings` scaffolding too)
- Integration oracle — tactical 04
- Any biome-related code

## Open questions

- **Sample-point grid:** resolved as "one shared 3D grid for `(x,y,z)` methods, plus dedicated per-signature grids when needed." `SimplexNoise` now uses `test/fixtures/noise/_samples-2d.json` for the 2D overload and the shared `test/fixtures/noise/_samples-3d.json` grid for 3D.
- **`BlendedNoise` parameter coverage:** resolved as the three unique tuples derived from built-in `NoiseGeneratorSettings`: overworld/amplified, nether/caves, and end/floating-islands. The oracle fixtures use those exact scale values plus a dedicated integer cell grid in `test/fixtures/noise/_samples-cell-3d.json`.
