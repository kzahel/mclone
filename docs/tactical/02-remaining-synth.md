# 02 — Remaining synth utilities

Continuing from [`01-noise-octaves.md`](01-noise-octaves.md). This slice finishes the `synth/` classes still needed for **default Minecraft 1.17.1 terrain generation** after tactical `01`. It explicitly does **not** pull in the dormant Caves & Cliffs Part 1 cave stack that exists in the 1.17.1 jar but is disabled in the built-in generator settings we are targeting — see [`AGENTS.md`](../../AGENTS.md) for the canonical list of disabled flags and dead-code classes.

## Goal

TS ports of `SurfaceNoise`, `PerlinSimplexNoise`, and the minimal `NormalNoise` support still required for 1.17.1 generator construction and PRNG parity, plus whatever `WorldgenRandom` / `PerlinNoise` API backfill those classes need, all validated against Java fixtures where behavior is stateful or numerically fragile.

## Current status

- `WorldgenRandom` now exists in TS with Java-matching seed helpers and `next(...)` call counting
- `PerlinNoise` has the required tactical-02 backfill: `create(...)` and `getSurfaceNoiseValue(...)`
- First tactical-02 slice landed: `PerlinSimplexNoise` is fixture-backed for the downstream octave sets `[-3..0]` and `[0]`, across canonical seeds `0`, `1`, `12345`, and `2151901553968352745`, validating both `useNoiseOffsets=false` and `useNoiseOffsets=true`
- Next up: the minimal `NormalNoise` slice for default 1.17.1 generator parity
- Deferred: `NoiseUtils` and the dormant C&C Part 1 cave/aquifer/noodle/ore-vein stack, unless we explicitly expand the target beyond default 1.17.1 worldgen

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | `SurfaceNoise` | none | interface only; no direct oracle |
| 2 | `PerlinSimplexNoise` | `SimplexNoise`, `WorldgenRandom` backfill | seed `S` + octave set + 2D sample points + `useNoiseOffsets` → `f64`; also `getSurfaceNoiseValue(x,y,z,yMax)` |
| 3 | `NormalNoise` | `PerlinNoise.create(...)` backfill | seed `S` + first octave + amplitude list + sample points `(x,y,z)` → `f64`, scoped to the built-in 1.17.1 allocations that still happen in `NoiseBasedChunkGenerator` |

## Why this subset

For the actual 1.17.1 target, `NoiseBasedChunkGenerator` constructs:

- `surfaceNoise` as either `PerlinSimplexNoise` or `PerlinNoise` via the `SurfaceNoise` interface
- `barrierNoise`, `waterLevelNoise`, and `lavaNoise` as `NormalNoise`

That is enough to keep tactical `02` relevant for default 1.17.1 parity:

- biome surface builders such as badlands / frozen ocean allocate `PerlinSimplexNoise`
- `NoiseBasedChunkGenerator` allocates the three `NormalNoise` instances unconditionally, so their constructor behavior and PRNG consumption still matter even though the built-in 1.17.1 presets never enable aquifers / noodle caves / ore veins / noise caves

What is **not** needed for the 1.17.1 target:

- `NoiseUtils`
- `Cavifier`
- `NoodleCavifier`
- `OreVeinifier`
- active `Aquifer` noise evaluation

Those are all tied to the dormant C&C Part 1 path, not the default presets we are targeting.

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

Use only the amplitude sets that default 1.17.1 `NoiseBasedChunkGenerator` actually allocates:

- `barrierNoise`: `firstOctave=-3`, amplitudes `[1.0]`
- `waterLevelNoise`: `firstOctave=-3`, amplitudes `[1.0, 0.0, 2.0]`
- `lavaNoise`: `firstOctave=-1`, amplitudes `[1.0, 0.0]`

## Translation gotchas

- **`PerlinSimplexNoise` is not just "Perlin over simplex."** Positive octaves are reseeded from `new WorldgenRandom((long)(baseSimplex.getValue(base.xo, base.yo, base.zo) * 9.223372E18F))`. If that seed derivation or the skip order is off, every higher octave diverges.
- **Missing `WorldgenRandom` is now a blocker.** Tactical `00` laid the groundwork, but this slice needs the actual TS implementation or a tightly scoped equivalent with `next(...)`, `consumeCount(...)`, and seeded construction.
- **`PerlinNoise.create(...)` matters.** `NormalNoise` does not use the public `PerlinNoise(RandomSource, octaves)` constructor directly; it goes through `PerlinNoise.create(...)` with amplitude lists. That API needs to exist on the TS side before `NormalNoise` can be faithful.
- **`NormalNoise` uses two Perlin chains.** Construction consumes PRNG twice, sequentially, from one source; sample-time uses the hard-coded `INPUT_FACTOR = 1.0181268882175227` on the second chain and scales the sum by a derived `valueFactor`.
- **`valueFactor` depends on non-zero amplitudes only.** The expected deviation uses the min/max indices of non-zero amplitudes in the amplitude list, not simply the list length.
- **`PerlinSimplexNoise.getSurfaceNoiseValue(...)` ignores half its signature.** It calls `getValue(x, y, true) * 0.55`; `z` and `yMax` are unused. Mirror the Java shape anyway so downstream interface calls stay honest.
- **Default 1.17.1 does not actually enable the C&C Part 1 cave flags.** Don't let the presence of `Cavifier`, `NoodleCavifier`, `OreVeinifier`, `Aquifer`, or `NoiseUtils` in the decomp drag this tactical doc into 1.18-style scope.

## Concrete steps

1. Port `WorldgenRandom` if the current TS tree still lacks it. Done.
2. Backfill `PerlinNoise.create(...)` and `PerlinNoise.getSurfaceNoiseValue(...)` on the existing port. Done.
3. Extend the oracle for `PerlinSimplexNoise`. Done.
4. Port and fixture-test `PerlinSimplexNoise`. Done.
5. Extend the oracle for `NormalNoise`, limited to the built-in 1.17.1 allocations (`barrierNoise`, `waterLevelNoise`, `lavaNoise`). Next.
6. Port and fixture-test `NormalNoise` for those built-in allocations. Next.
7. Stop there for the 1.17.1 target. `NoiseUtils` and the dormant C&C Part 1 consumers only move back into scope if we explicitly choose to translate that disabled path.

## Done when

- `PerlinSimplexNoise` matches Java fixtures across canonical seeds and at least the octave sets used by:
  - `NoiseBasedChunkGenerator` surface noise: `[-3..0]`
  - badlands / frozen ocean single-octave cases: `[0]`
- `NormalNoise` matches Java fixtures for the canonical seeds and at least the amplitude sets used by:
  - `barrierNoise`
  - `waterLevelNoise`
  - `lavaNoise`
- `pnpm test` and `pnpm typecheck` stay green

## Out of scope for this doc

- `NoiseSampler` and its settings objects — tactical `03`
- dormant C&C Part 1 cave density composition (`Cavifier`, `NoodleCavifier`, `OreVeinifier`, `Aquifer`, `NoiseUtils`) — only revisit if we explicitly widen the target beyond default 1.17.1
- biome or chunk integration
