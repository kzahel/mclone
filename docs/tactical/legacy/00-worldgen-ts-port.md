# 00 — Worldgen TS port: bootstrap

First tactical step toward Phase 1 of `docs/strategy.md`. Pin the language target (TypeScript), stand up the oracle harness, and port the two foundational numeric modules (PRNG + Perlin noise). Everything above noise is mechanical translation once these are green.

## Goal

A TS implementation of MC 1.17.1's worldgen that produces byte-identical output to the official server jar for the same seed, validated by automated unit tests. Scope of *this* doc: project bootstrap + PRNG + `ImprovedNoise`. Carvers, surface rules, features come in later tactical docs.

## Approach: two oracle tiers

| Tier | Source | Speed | Use |
|---|---|---|---|
| Unit | Custom Java program importing MC classes, called with pinned inputs | ~10ms per test | Per-module fixtures (PRNG sequences, noise samples) |
| Integration | Server jar with pinned seed → `world/region/*.mca` → JSON dumps | ~10s per region gen | End-to-end chunk parity, run sparingly |

Build unit oracles first. The integration oracle only matters once we have enough modules wired to compare a whole chunk.

## Translation order

| # | Module | Unit-oracle input → expected |
|---|---|---|
| 1 | `SimpleRandomSource` (`LegacyRandomSource` in later mappings; ≈ `java.util.Random`) | seed `S` → first 10k `nextInt()` / `nextLong()` |
| 2 | `WorldgenRandom` | seed + position → derived sequence |
| 3 | `ImprovedNoise` (3D Perlin) | seed + sample points `(x,y,z)` → `f64` |
| 4 | `PerlinNoise` (octaves) — *next tactical doc* | |
| 5 | `NoiseSampler` — *later* | |
| 6 | `NoiseBasedChunkGenerator` — *later, first integration-oracle check* | |

## Stack picks

- **Package manager:** `pnpm` (workspaces if/when we split renderer + worldgen)
- **Layout:** single package to start; `packages/*` if scope demands later
- **Test runner:** Vitest (Vite-native, fast watch, fixture-friendly)
- **TS:** strict mode, ES2022 target
- **Numerics:** hi/lo `Uint32` pair PRNG. If precision diverges or perf bites, WASM-PRNG is the fallback *and* a second reference impl.
- **Oracle dumper:** small Java program in `oracle/` directory, depends on the mapped `client-deobf.jar` we already produce. Compiled + run via `pnpm --silent oracle:gen`. Outputs JSON fixtures under `test/fixtures/`.
- **Fixture format:** JSON for small numeric (PRNG, noise samples). Binary `.bin` + sidecar JSON for chunk-level later.

## Directory layout (proposed)

```
mclone/
  src/
    worldgen/
      prng/
        legacy-random-source.ts
        worldgen-random.ts
      noise/
        improved-noise.ts
  test/
    fixtures/
      prng/
      noise/
    worldgen/
      prng.test.ts
      noise.test.ts
  oracle/
    java/                          # Java sources for the dumper
      OracleDumper.java
    build.sh                       # javac + classpath wiring
    run.sh                         # invokes dumper, writes fixtures
  package.json
  tsconfig.json
  vitest.config.ts
```

## Current status

- `5c201d1`: project scaffold landed (`pnpm`, Vitest, strict TS config, smoke test)
- `2b1f527`: Java oracle harness landed under `oracle/`
- Canonical PRNG fixtures now live under `test/fixtures/prng/` for seeds `0`, `1`, `12345`, and `2151901553968352745`
- TypeScript `SimpleRandomSource` now matches the Java oracle for `nextInt()`, `nextLong()`, and `nextDouble()` across those fixture seeds
- Canonical `ImprovedNoise` fixtures now live under `test/fixtures/noise/` for the same seed set, sampled on a fixed 11x11x11 grid
- TypeScript `ImprovedNoise` now matches the Java oracle across those fixed-grid samples
- For MC 1.17.1, the relevant legacy LCG class is `net.minecraft.world.level.levelgen.SimpleRandomSource`; later Mojang mappings rename this to `LegacyRandomSource`
- Tactical 00 is complete. Tactical 01 (`01-noise-octaves.md`) is now underway.

## Week 1 — concrete steps

1. **Project scaffold.** `pnpm init`, install `vitest` + `typescript`. Strict `tsconfig.json`. Single empty test that passes.
2. **Oracle dumper skeleton.** Java program that takes a seed + module name on CLI, emits JSON to stdout. Wired through `pnpm --silent oracle:gen prng --seed 12345 --count 10000 > test/fixtures/prng/seed-12345.json`. Classpath uses `reference/minecraft-1.17.1/client-deobf.jar`.
3. **Generate PRNG fixtures.** Dump first 10k `nextInt()`, `nextLong()`, `nextDouble()` for a handful of canonical seeds (`0`, `1`, `12345`, MC's own famous seed `2151901553968352745` — "Far Lands"-like coverage).
4. **Port `SimpleRandomSource` / legacy LCG.** Hi/lo `Uint32` pair. ~50 lines. Vitest assertion: byte-exact match against fixture.
5. **Generate noise fixtures.** Dump `ImprovedNoise.getValue(x,y,z)` at a grid of sample points for several seeds.
6. **Port `ImprovedNoise`.** Validate against fixture.

## Open questions to decide before starting

- **Single-package vs. workspace?** Recommend single now; split when renderer lands.
- **Oracle dumper in this repo or sibling repo?** Recommend in-repo under `oracle/` — keeps fixtures and generator co-located, simpler CI.
- **Fixture commit policy:** commit JSON fixtures (small, deterministic, factual measurements per `strategy.md`); regenerate via `pnpm oracle:gen` only when the dumper changes.
- **CI:** even a pre-commit hook running `vitest run` is enough for now. Skip GitHub Actions until we have a public-ish artifact.

## Done when

- `pnpm test` runs and asserts:
  - `SimpleRandomSource` / legacy LCG matches the Java fixture for ≥3 seeds × 10k draws each
  - `ImprovedNoise.getValue` matches the Java fixture at ≥1000 sample points × 3 seeds (allow zero `f64` epsilon — should be exact)
- `pnpm oracle:gen prng` and `pnpm oracle:gen noise` regenerate the fixtures from scratch
- README in `oracle/` documents how to add a new fixture type
- Next tactical doc (`01-…`) is drafted, covering octaved Perlin + `NoiseSampler`

## Out of scope for this doc

- Carvers, surface rules, features (later tactical docs)
- Renderer, meshing, lighting (separate work stream)
- Integration oracle harness (deferred to the doc that introduces full-chunk testing)
