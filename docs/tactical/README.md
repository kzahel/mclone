# Tactical docs

Numbered, short-lived implementation plans. Each covers a cohesive group of modules scoped to ~1–2 focused sessions of work. Strategy lives in [`../strategy.md`](../strategy.md); these are the sequenced "do this next" plans.

## Rule of thumb

- **Plan only the next one in detail.** What we learn porting module N changes what N+1 should look like.
- The arc below is *aspirational orientation*, not a commitment. Skip, merge, or reorder as reality dictates.
- Each tactical doc owns: scope (modules), unit-oracle inputs → expected outputs, "done when" criteria, and a pointer to the next.

## Arc (aspirational, toward MVP terrain gen)

| Doc | Modules | Oracle tier | Purpose |
|---|---|---|---|
| [`00-worldgen-ts-port.md`](00-worldgen-ts-port.md) | scaffold, `SimpleRandomSource`, `WorldgenRandom`, `ImprovedNoise` | unit | foundation numerics |
| [`01-noise-octaves.md`](01-noise-octaves.md) | `PerlinNoise`, `SimplexNoise`, `BlendedNoise` | unit | everything `NoiseSampler` consumes from `synth/` |
| [`02-remaining-synth.md`](02-remaining-synth.md) | `SurfaceNoise`, `PerlinSimplexNoise`, minimal `NormalNoise` | unit | remaining `synth/` pieces alive in default 1.17.1; `NoiseUtils` + C&C Part 1 consumers deferred (see [`../../AGENTS.md`](../../AGENTS.md)) |
| `03-` | `NoiseSampler` + settings (`NoiseSettings`, `NoiseSamplingSettings`) | unit | 3D density field from chunk coords |
| `04-` | **Integration oracle harness**: server-jar runner, MCA reader, chunk-level fixture format | infra | stand up the server-oracle *before* we need to diff chunks |
| `05-` | `BiomeSource` subset — translated `OverworldBiomeSource` + climate layers → biome IDs | unit + integration | |
| `06-` | `NoiseBasedChunkGenerator` (terrain-only: stone/air/water; no surface/caves/features) | **integration** | first end-to-end chunk diff — "does it look like MC?" moment |
| `07-` | `SurfaceRules` / `SurfaceBuilder` — grass/dirt/sand per biome | integration | visual recognizability |
| `08-` | Carvers: `CaveWorldCarver`, `CanyonWorldCarver` | integration | **MVP terrain gen reached here** |

Everything after 08 (features, ores, structures, aquifers, noodle caves) is post-MVP polish.

## MVP definition

"Recognizable Minecraft overworld" — terrain + carvers + surface rules, no features, no structures, no ores. Rendering a chunk should make you say "oh yeah, Minecraft."
