# 02 — Remaining synth utilities

Continuing from [`01-noise-octaves.md`](01-noise-octaves.md). This slice finishes the `synth/` classes that are not direct `NoiseSampler` inputs but are needed immediately afterwards by surface generation and the Caves & Cliffs Part 1 internals.

## Goal

TS ports of `SurfaceNoise`, `PerlinSimplexNoise`, `NormalNoise`, and `NoiseUtils`, plus whatever minimal `WorldgenRandom` / `PerlinNoise` API backfill those classes need, all validated against Java fixtures where behavior is stateful or numerically fragile.

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | `SurfaceNoise` | none | interface only; no direct oracle |
| 2 | `PerlinSimplexNoise` | `SimplexNoise`, `WorldgenRandom` backfill | seed `S` + octave set + 2D sample points + `useNoiseOffsets` → `f64`; also `getSurfaceNoiseValue(x,y,z,yMax)` |
| 3 | `NormalNoise` | `PerlinNoise.create(...)` backfill | seed `S` + first octave + amplitude list + sample points `(x,y,z)` → `f64` |
| 4 | `NoiseUtils` | `NormalNoise` | pure math wrappers over `NormalNoise` output and range mapping |

## Why these four

`NoiseBasedChunkGenerator` constructs:

- `surfaceNoise` as either `PerlinSimplexNoise` or `PerlinNoise` via the `SurfaceNoise` interface
- `barrierNoise`, `waterLevelNoise`, and `lavaNoise` as `NormalNoise`

The C&C Part 1 internals then lean on the same set:

- `Cavifier`, `NoodleCavifier`, `OreVeinifier`, `Aquifer`, and `GeodeFeature` all allocate `NormalNoise`
- `NoiseUtils` is shared helper math for `Cavifier` and `NoodleCavifier`
- biome surface builders such as badlands / frozen ocean allocate `PerlinSimplexNoise`

After this doc, tactical `03` can focus on `NoiseSampler` and settings instead of still backfilling `synth/`.

## Oracle extension

Extend `oracle/java/OracleDumper.java` again:

```bash
pnpm --silent oracle:gen noise \
    --class PerlinSimplexNoise \
    --seed 12345 \
    --octaves -3,-2,-1,0 \
    --samples2d test/fixtures/noise/_samples-2d.json \
    > test/fixtures/noise/perlin-simplex-seed-12345-oct-m3-0.json
```

Likewise for `NormalNoise`, keyed by:

- seed
- `firstOctave`
- amplitude list
- shared 3D sample grid

`NoiseUtils` should stay mostly test-only unless the wrapper math turns out to be more stateful than expected.

## Translation gotchas

- **`PerlinSimplexNoise` is not just "Perlin over simplex."** Positive octaves are reseeded from `new WorldgenRandom((long)(baseSimplex.getValue(base.xo, base.yo, base.zo) * 9.223372E18F))`. If that seed derivation or the skip order is off, every higher octave diverges.
- **Missing `WorldgenRandom` is now a blocker.** Tactical `00` laid the groundwork, but this slice needs the actual TS implementation or a tightly scoped equivalent with `next(...)`, `consumeCount(...)`, and seeded construction.
- **`PerlinNoise.create(...)` matters.** `NormalNoise` does not use the public `PerlinNoise(RandomSource, octaves)` constructor directly; it goes through `PerlinNoise.create(...)` with amplitude lists. That API needs to exist on the TS side before `NormalNoise` can be faithful.
- **`NormalNoise` uses two Perlin chains.** Construction consumes PRNG twice, sequentially, from one source; sample-time uses the hard-coded `INPUT_FACTOR = 1.0181268882175227` on the second chain and scales the sum by a derived `valueFactor`.
- **`valueFactor` depends on non-zero amplitudes only.** The expected deviation uses the min/max indices of non-zero amplitudes in the amplitude list, not simply the list length.
- **`PerlinSimplexNoise.getSurfaceNoiseValue(...)` ignores half its signature.** It calls `getValue(x, y, true) * 0.55`; `z` and `yMax` are unused. Mirror the Java shape anyway so downstream interface calls stay honest.
- **`NoiseUtils.sampleNoiseAndMapToRange(...)` is small but downstream-visible.** Keep it exact and give it direct unit tests so later cavifier bugs are not blamed on wrapper math.

## Concrete steps

1. Port `WorldgenRandom` if the current TS tree still lacks it.
2. Backfill `PerlinNoise.create(...)` and `PerlinNoise.getSurfaceNoiseValue(...)` on the existing port.
3. Extend the oracle for `PerlinSimplexNoise`.
4. Port and fixture-test `PerlinSimplexNoise`.
5. Extend the oracle for `NormalNoise`.
6. Port and fixture-test `NormalNoise`.
7. Add direct unit coverage for `NoiseUtils` and the trivial `SurfaceNoise` implementations.

## Done when

- `PerlinSimplexNoise` matches Java fixtures across canonical seeds and at least the octave sets used by:
  - `NoiseBasedChunkGenerator` surface noise: `[-3..0]`
  - badlands / frozen ocean single-octave cases: `[0]`
- `NormalNoise` matches Java fixtures for the canonical seeds and at least the amplitude sets used by:
  - `barrierNoise`
  - `waterLevelNoise`
  - `lavaNoise`
  - one representative cavifier/noodle/ore-vein configuration
- `NoiseUtils` pure functions have direct TS unit coverage
- `pnpm test` and `pnpm typecheck` stay green

## Out of scope for this doc

- `NoiseSampler` and its settings objects — tactical `03`
- cave density composition (`Cavifier`, `NoodleCavifier`, `OreVeinifier`, `Aquifer`) — later tactical docs once their noise primitives exist
- biome or chunk integration
