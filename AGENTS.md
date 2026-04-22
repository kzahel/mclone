See [`README.md`](README.md) for project context.

## Notes for agents

**Do not use the auto-memory system** for this project (the `~/.claude/projects/-home-kgraehl-code-mclone/memory/` directory). Persist project-relevant guidance in this file (`AGENTS.md`) instead.

## Porting methodology — applies to every slice, worldgen and renderer

Every class we port has a 1:1 counterpart in `reference/minecraft-1.17.1/src/`. **Always read the source file before writing the port.** The default is direct translation: same field names, same method names, same logic flow. Diverge only when the platform forces it.

### Sanctioned divergences (exhaustive list)

| Layer | Java | TypeScript / WebGPU replacement |
|---|---|---|
| Renderer only | `GlStateManager` + `Uniform` | WebGPU pipeline descriptors + bind groups |
| Renderer only | GLSL core shaders (`assets/minecraft/shaders/core/`) | WGSL equivalents |
| Language | `ByteBuffer`, Java primitives, `int` bit-twiddling | `DataView` / typed arrays; preserve byte layout exactly |
| Language | Java generics / checked exceptions | TypeScript generics / unchecked throws |

Everything else — vertex formats, buffer builders, atlas stitching, model baking, chunk meshing, noise math, PRNG, biome layering — is a straight port. If you find yourself writing logic that isn't in the source, stop and re-read the source.

### Callout convention

When a method diverges from its Java counterpart, add a one-line comment:
```ts
// WebGPU: GPUBuffer instead of GL VAO/VBO/IBO
```
Straight ports get no comment.

### Reference tree must be present

Run `./scripts/decompile-mc.sh` if `reference/minecraft-1.17.1/src/` is missing. Do not write a port without the source in hand.

## Visual validation — fail fast, don't batch

For any slice that produces pixels, **capture a screenshot and look at it before moving on.** Do not finish a whole slice and then check. Check at the first drawable milestone — even a solid-color quad or a clear-color frame — then keep checking as complexity increases.

Concretely:
- After wiring up a new draw path, run `pnpm test:browser` and take a screenshot. Read the image and describe what you see. Does it look right? Are the shapes, colors, and positions what you expect?
- If the output looks wrong (blank canvas, wrong color, garbage geometry, WebGPU validation errors in the console), stop and fix it before adding more code on top.
- Unit tests (`pnpm test`) catch data-structure correctness. They do not catch GPU submission errors, wrong buffer layouts, or misconfigured pipelines. Actually looking at the rendered output is the only way to catch those.

## Target: Minecraft Java 1.17.1 vanilla overworld

Seed parity against 1.17.1 vanilla overworld is the correctness bar. The decomp under `reference/minecraft-1.17.1/` contains Caves & Cliffs Part 1 systems that are **present in the source but disabled by default in 1.17.1**. Do not port them for MVP. Revisit only if we later commit to 1.18+ or enable C&C Part 1 experimentally — at that point the target has changed and this section should be reviewed.

### Disabled flags in `NoiseGeneratorSettings.overworld(...)`

File: `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseGeneratorSettings.java:241`. The `overworld(...)` factory passes `false` for all five C&C Part 1 booleans:

| Flag | Effect when `false` |
|---|---|
| `aquifers_enabled` | `Aquifer.createDisabled` is used; `barrier`/`waterLevel`/`lava` `NormalNoise` fields in `NoiseBasedChunkGenerator` are allocated but never sampled |
| `noise_caves_enabled` | `Cavifier` is replaced by `NoiseModifier.PASSTHROUGH` |
| `deepslate_enabled` | `DepthBasedReplacingBaseStoneSource` skips deepslate substitution |
| `ore_veins_enabled` | `OreVeinifier.fillStream` short-circuits |
| `noodle_caves_enabled` | all four `NoodleCavifier.fill*NoiseColumn` methods short-circuit |

### Dead code in our target — do not port

Transitively, the following classes exist in the 1.17.1 decomp but are never exercised in vanilla overworld:

- `net.minecraft.world.level.levelgen.Aquifer`
- `net.minecraft.world.level.levelgen.Cavifier`
- `net.minecraft.world.level.levelgen.NoodleCavifier`
- `net.minecraft.world.level.levelgen.OreVeinifier`
- `net.minecraft.world.level.levelgen.synth.NormalNoise` — only live consumers are the classes above, plus `GeodeFeature` (post-MVP feature) and `MultiNoiseBiomeSource` (nether/end — overworld uses the layered `OverworldBiomeSource` path)
- `net.minecraft.world.level.levelgen.synth.NoiseUtils` — only consumers are `Cavifier` / `NoodleCavifier`
- the deepslate path in `DepthBasedReplacingBaseStoneSource`

If asked to port any of these, push back and confirm the target has changed before writing code.

### Active MVP pipeline

Per [`docs/tactical/README.md`](docs/tactical/README.md): PRNG → octaved noise (`PerlinNoise`, `SimplexNoise`, `BlendedNoise`) → surface-path noise (`PerlinSimplexNoise` + `SurfaceNoise` interface) → `NoiseSampler` (with `NoiseModifier.PASSTHROUGH` since `Cavifier` is disabled) → `NoiseBasedChunkGenerator` (terrain only) → classic carvers (`CaveWorldCarver`, `CanyonWorldCarver`) → surface rules → features. Everything after classic carvers is post-MVP polish.
