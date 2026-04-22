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

## Why these three

`NoiseSampler.java` imports exactly these three classes from `synth/` (verified). Other `synth/` classes (`NormalNoise`, `SurfaceNoise`, `PerlinSimplexNoise`, `NoiseUtils`) aren't used by `NoiseSampler`; they're consumed by C&C Part 1 internals (`Cavifier`, `Aquifer`, `OreVeinifier`) and specific surface builders. Tactical 02 picks them up.

## Oracle extension

Extend the existing Java dumper with a `noise` subcommand. Shape:

```
pnpm oracle:gen noise \
    --class PerlinNoise \
    --seed 12345 \
    --octaves -7,-6,-5,-4,-3,-2,-1,0 \
    --samples grid.json \
    > test/fixtures/noise/perlin-seed-12345-oct-m7-0.json
```

Fixture JSON (proposed shape — finalize when writing the dumper):

```json
{
  "class": "PerlinNoise",
  "seed": "12345",
  "octaves": [-7,-6,-5,-4,-3,-2,-1,0],
  "samples": [
    { "x": 0.0, "y": 64.0, "z": 0.0, "value": 0.123456789 }
  ]
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

1. **Extend the Java dumper** with a `noise` subcommand. Support `--class PerlinNoise|SimplexNoise|BlendedNoise`, seed, octave list (for Perlin), and a sample-point file (JSON list of `{x,y,z}`).
2. **Generate canonical sample grids.** One small file per noise class: `test/fixtures/noise/_samples.json`. ≥1000 points per grid covering positive / negative / zero coordinates, including non-integer `x/y/z` (don't let integer-only samples hide bugs).
3. **Emit fixtures** for each class × canonical seeds (the same four seeds as PRNG: `0`, `1`, `12345`, `2151901553968352745`). For `PerlinNoise`, additionally cover both canonical octave ranges used by `BlendedNoise`: `[-15..0]` and `[-7..0]`.
4. **Port `PerlinNoise`** — validate against `[-7..0]` fixtures first (smaller, faster iteration), then `[-15..0]`.
5. **Port `SimplexNoise`** — independent, can be interleaved or parallel.
6. **Port `BlendedNoise`** — glue over `PerlinNoise`; fixture validates that the combination + clamping works end-to-end.

## Done when

- `pnpm test` passes assertions:
  - `PerlinNoise` matches Java fixture at ≥1000 sample points × 4 seeds × 2 octave ranges (zero f64 epsilon)
  - `SimplexNoise` matches at ≥1000 points × 4 seeds (zero f64 epsilon)
  - `BlendedNoise` matches at ≥1000 points × 4 seeds × a handful of `(hScale, vScale, mainH, mainV)` tuples (cover the common range `NoiseSampler` actually calls with)
- `pnpm oracle:gen noise --class PerlinNoise ...` regenerates fixtures from scratch
- `oracle/README.md` is updated with the new subcommand
- Next tactical doc (`02-…`) drafted — likely scope: `SurfaceNoise`, `PerlinSimplexNoise`, `NormalNoise`, `NoiseUtils`

## Out of scope for this doc

- `NormalNoise`, `SurfaceNoise`, `PerlinSimplexNoise`, `NoiseUtils` — tactical 02
- `NoiseSampler` itself — tactical 03 (needs `NoiseSettings` scaffolding too)
- Integration oracle — tactical 04
- Any biome-related code

## Open questions

- **Sample-point grid:** pin one shared grid across noise classes, or let each class define its own? Recommend one shared grid for `(x,y,z)`-based classes; fall back to per-class for oddball signatures (e.g. `SimplexNoise.getValue(x,z)` 2D overload).
- **`BlendedNoise` parameter coverage:** pick tuples from what `NoiseSampler` actually passes (read `NoiseSampler.fillNoiseColumn`). Don't test arbitrary scale combinations — test the ones downstream code will use.
