# Minecraft Reference Bootstrap

The repo uses a local, gitignored Minecraft Java 1.17.1 reference tree for
vanilla behavior, assets, oracle fixtures, and visual correctness checks.
Separately pinned Alpha, Beta, and current-stable trees are historical or
comparative research inputs; none changes the 1.17.1 parity target.

Root path:

```text
reference/minecraft-1.17.1/
```

The most important subtree is:

```text
reference/minecraft-1.17.1/src/
```

Read the Java source before porting parity-sensitive vanilla behavior. Use it for simulation, content, renderer-facing state, model baking, texture atlas behavior, mipmaps/filtering, render layers, lighting, fog, sky, particles, and client-visible state.

## Target

The native worldgen target is **seed parity** with Minecraft Java 1.17.1 overworld output.

1.17.1 is intentionally pre-Caves-and-Cliffs terrain. It avoids the 1.18+ density-function and spline terrain system while still giving enough modern client/rendering source to reference.

The MVP worldgen pipeline is:

1. PRNG and octaved noise
2. `OverworldBiomeSource` / `BiomeManager`
3. `NoiseSampler`
4. `NoiseBasedChunkGenerator`
5. classic carvers: `CaveWorldCarver`, `CanyonWorldCarver`
6. surface rules
7. features and structures

See [`strategy.md`](strategy.md), [`worldgen-deterministic-order.md`](worldgen-deterministic-order.md), [`worldgen-status.md`](worldgen-status.md), [`carver-status.md`](carver-status.md), and [`structures.md`](structures.md) for current implementation status and parity policy.

## Disabled Caves & Cliffs Part 1 Systems

The decomp under `reference/minecraft-1.17.1/` contains Caves & Cliffs Part 1 systems that are present in the source but disabled by default in 1.17.1 overworld generation. Do not port them for the MVP target. Revisit only if the project target changes to 1.18+ or explicitly enables Caves & Cliffs Part 1 behavior.

`NoiseGeneratorSettings.overworld(...)` passes `false` for all five Caves & Cliffs Part 1 booleans in `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseGeneratorSettings.java`.

| Flag | Effect when `false` |
|---|---|
| `aquifers_enabled` | `Aquifer.createDisabled` is used; `barrier`/`waterLevel`/`lava` `NormalNoise` fields in `NoiseBasedChunkGenerator` are allocated but never sampled |
| `noise_caves_enabled` | `Cavifier` is replaced by `NoiseModifier.PASSTHROUGH` |
| `deepslate_enabled` | `DepthBasedReplacingBaseStoneSource` skips deepslate substitution |
| `ore_veins_enabled` | `OreVeinifier.fillStream` short-circuits |
| `noodle_caves_enabled` | all four `NoodleCavifier.fill*NoiseColumn` methods short-circuit |

Transitively, the following classes or paths are never exercised in vanilla 1.17.1 overworld generation:

- `net.minecraft.world.level.levelgen.Aquifer`
- `net.minecraft.world.level.levelgen.Cavifier`
- `net.minecraft.world.level.levelgen.NoodleCavifier`
- `net.minecraft.world.level.levelgen.OreVeinifier`
- `net.minecraft.world.level.levelgen.synth.NormalNoise`, except post-MVP `GeodeFeature` and non-overworld biome sources
- `net.minecraft.world.level.levelgen.synth.NoiseUtils`
- the deepslate path in `DepthBasedReplacingBaseStoneSource`

## Local Layout

Expected generated layout:

```text
client.jar          original obfuscated client jar, SHA1-verified from Mojang
client.txt          official Mojang Proguard mappings
client-deobf.jar    remapped client jar
src/                decompiled Java source
tools/              SpecialSource and Vineflower jars
parchment/          Parchment zip cache, if --parchment was used
extracted/          filtered client.jar assets: textures, models, blockstates,
                    structure NBTs; see assets-plan.md
*.json              Mojang manifests for rerunning the pipeline
```

## Key Java Entry Points

Worldgen:

- `net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java` - top-level terrain pipeline, 3D density to blocks
- `net/minecraft/world/level/levelgen/NoiseSampler.java` - 3D density sampler
- `net/minecraft/world/level/levelgen/carver/CaveWorldCarver.java` - classic connected-tunnel caves
- `net/minecraft/world/level/levelgen/carver/CanyonWorldCarver.java` - ravines
- `net/minecraft/world/level/levelgen/carver/WorldCarver.java` - carver base class
- `net/minecraft/world/level/levelgen/surfacebuilders/` - grass, dirt, sand, and stone layering per biome
- `net/minecraft/world/level/levelgen/feature/` - trees, ore blobs, foliage, and other feature placement

Client rendering and assets:

- `net/minecraft/client/renderer/texture/TextureAtlas.java` - atlas preparation, upload, mip-level selection, and filter updates
- `net/minecraft/client/renderer/texture/TextureAtlasSprite.java` - sprite UVs, per-sprite mip generation, animation frame upload, and UV shrink ratio
- `net/minecraft/client/renderer/texture/MipmapGenerator.java` - gamma-aware mipmap generation and alpha-cutout handling
- `com/mojang/blaze3d/platform/NativeImage.java` and `TextureUtil.java` - texture upload/filtering and GL mip allocation behavior
- `net/minecraft/client/renderer/block/model/` - block model baking and baked quad UV behavior
- `net/minecraft/client/renderer/RenderType.java` and `RenderStateShard.java` - vanilla render-layer state, transparency, culling, and texture-state choices

1.17.1 also includes Caves & Cliffs Part 1 internals such as `Aquifer`, `Cavifier`, `NoodleCavifier`, and `OreVeinifier`. They are useful context when comparing versions, but they are disabled in the vanilla 1.17.1 overworld target and are not part of the MVP terrain port.

## Build Or Refresh The Reference Tree

Everything above can be rebuilt from scripts in [`../scripts/`](../scripts/). Outputs land under `reference/`, which is gitignored.

Prerequisites:

- JDK 17+
- `curl`
- `jq`
- `sha1sum`
- `unzip`
- `python3`, when using `--parchment`

Common commands:

```bash
# Minecraft 1.17.1 client: download, remap, decompile, apply Parchment,
# and extract client assets.
./scripts/decompile-mc.sh 1.17.1 --parchment

# Other versions / server jar / custom output dir.
./scripts/decompile-mc.sh 1.16.5 --parchment
./scripts/decompile-mc.sh 1.18.2 --server --parchment
./scripts/decompile-mc.sh 1.17.1 --out /tmp/mc-scratch
./scripts/decompile-mc.sh 1.17.1 --no-assets
./scripts/decompile-mc.sh 1.17.1 --no-assets \
  --only net/minecraft/world/level/newbiome/layer/ShoreLayer
./scripts/decompile-mc.sh 1.17.1 --force

# Standalone asset extraction, if client decompilation already ran.
./scripts/extract-assets.sh 1.17.1

# Apply Parchment standalone to an existing source tree.
./scripts/apply-parchment.py reference/minecraft-1.17.1/src --mc 1.17.1
```

The decompile script is idempotent. Manifest fetch, jar download, remap, decompile, Parchment application, and asset extraction skip when their expected outputs already exist. Use `--force` to redo the pipeline.

Pipeline stages:

1. Fetch `piston-meta.mojang.com/mc/game/version_manifest_v2.json` and find the version entry.
2. Fetch that version's per-version manifest.
3. Download and SHA-1-verify `client.jar` or `server.jar`.
4. When mappings exist, download `client.txt` or `server.txt` and remap with
   SpecialSource. When mappings are absent, accept the jar only if it contains
   the expected official unobfuscated class path.
5. Decompile with Vineflower, optionally limiting work to repeatable `--only`
   class/package prefixes.
6. Optionally download Parchment and rewrite mapped method parameters.
7. For client builds, extract filtered textures, models, blockstates, and structures into `extracted/`.

Selective decompilation writes `decompile-selection.txt`; a different
selection must use a different output directory so unrelated partial source
sets never silently mix. Every ordinary run writes `provenance.txt` with the
version, side, naming mode, input hashes, Vineflower version, release time,
and full/focused selection mode.

## Legacy Alpha And Beta Side References

Minecraft Alpha predates Mojang's official mappings, so it has a separate,
gitignored reference pipeline based on the CC0
[Ornithe Feather mappings](https://github.com/OrnitheMC/feather). The pinned
study specimen is Alpha v1.1.2_01, the final pre-Halloween-Update Alpha build;
it is a historical side reference and does not change the 1.17.1 vanilla
Overworld target above.

```bash
# Build reference/minecraft-a1.1.2_01/src and the mapped merged jar.
pnpm reference:alpha

# Exercise the real mapped generator through terrain, surfaces, and caves.
pnpm oracle:alpha:terrain -- --seed 12345 --chunk-x 0 --chunk-z 0

# Decompile a later Alpha explicitly for comparison.
./scripts/decompile-alpha-mc.sh a1.2.6 --out /tmp/minecraft-a1.2.6
```

The decompiler verifies the official Mojang client SHA-1, pins the Feather
revision, and records its provenance under the generated reference directory.
The full historical selection report, generator trace, probe fingerprint, and
future porting boundary live in
[`topics/alpha-era-reference.md`](topics/alpha-era-reference.md).
The deliberately close-but-not-perfect native profile contract and current
implementation status live separately in
[`topics/alpha-world-generation.md`](topics/alpha-world-generation.md).

The selected Beta comparison specimen is Beta 1.7.3, the mature old-Beta
terrain family immediately before the Beta 1.8 Adventure Update generator
change. It uses the same pinned Feather pipeline through a Beta-specific entry
point:

```bash
# Build reference/minecraft-b1.7.3/src and the mapped merged jar.
pnpm reference:beta
```

The generated tree remains gitignored. The research report traces Beta's
climate-shaped Alpha density skeleton, biome surface/population policy, cave
change, dimensions, features, chunk/storage evolution, and measured size. It
also records the decisions required before any Rust implementation:
[`topics/beta-1.7.3-reference.md`](topics/beta-1.7.3-reference.md).

## Modern Stable Side Reference

The current comparative specimen is pinned separately from the parity target:

```bash
# Build a focused current-stable worldgen tree under
# reference/minecraft-26.2/src/.
pnpm reference:modern

# Use a scratch destination without changing the pinned wrapper.
pnpm reference:modern -- --out /tmp/minecraft-modern-research
```

[`scripts/decompile-modern-mc.sh`](../scripts/decompile-modern-mc.sh) pins Java
26.2 and selects the Overworld biome builder, surface rules, terrain splines,
density functions, noise router, aquifer, and chunk-density entry points.
Run `decompile-mc.sh 26.2 --no-assets` directly only when a whole-client
decompile is genuinely required.

Mojang stopped obfuscating Java Edition after the Mounts of Mayhem release.
Current jars carry original class, method, field, parameter, and variable names
and publish no mappings in the version JSON. The bootstrap therefore verifies
an expected named class and decompiles the official jar directly. Parchment is
neither needed nor accepted on this branch.

The modern tree is a source-research convenience, not a new oracle, asset
source, runtime dependency, or seed-parity target. Its durable selection,
refresh, licensing, and interpretation boundary lives in
[`topics/modern-minecraft-reference.md`](topics/modern-minecraft-reference.md).

## Mappings

For mapped releases such as 1.17.1, Mojang mappings cover class, method, and
field names. They do not include method parameter names, javadocs, or true
local variable names. Current unobfuscated releases are a separate branch and
already include the original bytecode-visible names.

The main mapping sets:

| | Mojang | Yarn (Fabric) | Parchment |
|---|---|---|---|
| Class / method / field | official | replacement names | uses Mojang's |
| Parameter names | no | yes | yes |
| Javadoc | no | yes | yes |
| Format | Proguard `.txt` | Tiny v2 | layered JSON on top of Mojang |
| Standalone CLI | easy | moderate | awkward |

This repo uses Mojang names plus Parchment parameters. That preserves the ability to search Mojang-named classes such as `NoiseBasedChunkGenerator` while improving method signatures.

`scripts/apply-parchment.py` downloads the Parchment release for the requested Minecraft version, parses `parchment.json`, walks the decompiled tree, and rewrites `varN` method parameters by matching `(class, method_name, param_count)` positionally. Ambiguous overloads are skipped. The script is idempotent.

True local variables inside method bodies remain decompiler-generated `varN` names because the original bytecode does not contain those names.
