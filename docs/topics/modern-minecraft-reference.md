# Modern Minecraft Java Reference

Topic: `modern-minecraft-reference`

Status: **Active comparative side reference as of 2026-07-26. Minecraft Java
26.2 is the pinned current-stable specimen. Its official unobfuscated client
jar is SHA-1 verified and a focused 18-file worldgen source tree decompiles
reproducibly. This does not change the reference-locked Java 1.17.1 Overworld
target.**

## Scope

This topic owns:

- selection and refresh policy for a modern stable Java research specimen;
- the reproducible download, naming, decompilation, and provenance path;
- interpretation limits for decompiled current source;
- the curated default worldgen source surface; and
- durable findings that compare post-1.18 systems with the pinned 1.17.1
  reference or Mclone's original profile.

It does not own:

- Mclone terrain design, which remains in
  [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md);
- Java 1.17.1 parity, oracle fixtures, or vanilla implementation policy;
- modern Minecraft seed parity;
- a rolling snapshot checkout;
- Minecraft assets in Mclone's distributable content; or
- permission to port systems excluded by the 1.17.1 reference target.

## Selected Specimen

| Fact | Value |
|---|---|
| Minecraft version | Java 26.2 |
| Manifest type | `release` |
| Release time | `2026-06-16T12:03:33+00:00` |
| Official client size | 39,193,383 bytes |
| Official client SHA-1 | `2dc72797acbc1b63fc16a11c4ac393605f453754` |
| Naming mode | official unobfuscated names |
| Mapping artifact | absent by design |
| Decompiled default | 18 focused Java files |
| Vineflower | 1.12.0 |

The official manifest reported `26.2` as `latest.release` during the
2026-07-26 survey. The wrapper remains explicitly pinned rather than following
that mutable pointer. A refresh is an intentional research change with a
source comparison and documentation update.

Use a stable release for durable conclusions. Snapshots may be inspected in a
scratch directory for a specific question, but they do not silently replace
this specimen.

Primary publication evidence:

- [Minecraft Java Edition 26.2 release notes](https://www.minecraft.net/en-us/article/minecraft-java-edition-26-2);
- [Mojang's unobfuscated Java Edition announcement](https://www.minecraft.net/nl-nl/article/removing-obfuscation-in-java-edition);
- the SHA-1-verified Piston per-version manifest and official client jar
  recorded by the generated local `provenance.txt`.

## Reproducible Bootstrap

Run:

```bash
pnpm reference:modern
```

This invokes
[`scripts/decompile-modern-mc.sh`](../../scripts/decompile-modern-mc.sh),
which delegates to the shared bootstrap with:

- Java 26.2 client side;
- no asset extraction;
- a focused list of Overworld biome, surface, terrain spline, density,
  noise-router, aquifer, and chunk-generation classes; and
- the ordinary gitignored `reference/minecraft-26.2/` destination.

The shared bootstrap:

1. downloads Mojang's top-level and per-version manifests;
2. downloads and SHA-1 verifies the official client jar;
3. observes that no `client_mappings` artifact exists;
4. verifies `net/minecraft/client/Minecraft.class` to distinguish a current
   named jar from an old unmapped obfuscated release;
5. decompiles the selected class/package prefixes directly with Vineflower;
6. excludes assets and other non-class jar entries; and
7. writes selection and provenance receipts beside the generated source.

The focused tree is the default because it completed in roughly two seconds
after downloads on the July 2026 Linux host. A whole-client experiment at a
2 GiB Java heap exhausted memory in unrelated inventory classes before
completion. The shared script retains a 4 GiB full path for an explicit need,
but focused class/package decompilation is the normal research unit.

Different `--only` selections must use different output directories. This
prevents a source tree from becoming an undocumented union of unrelated
partial runs. Parchment is rejected because the official current jar already
contains original parameter and variable names.

## Default Source Surface

Start with:

- `net/minecraft/world/level/biome/OverworldBiomeBuilder.java`;
- `net/minecraft/data/worldgen/biome/OverworldBiomes.java`;
- `net/minecraft/data/worldgen/SurfaceRuleData.java`;
- `net/minecraft/data/worldgen/TerrainProvider.java`;
- `net/minecraft/world/level/levelgen/NoiseRouterData.java`;
- `net/minecraft/world/level/levelgen/DensityFunction.java`;
- `net/minecraft/world/level/levelgen/DensityFunctions.java`;
- `net/minecraft/world/level/levelgen/NoiseRouter.java`;
- `net/minecraft/world/level/levelgen/NoiseChunk.java`;
- `net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java`;
- `net/minecraft/world/level/levelgen/SurfaceRules.java`;
- `net/minecraft/world/level/levelgen/SurfaceSystem.java`; and
- `net/minecraft/world/level/levelgen/Aquifer.java`.

The biome package selection also includes its small data/bootstrap companions.
Add a focused class or package in a scratch output when a question reaches
beyond this surface. Do not expand the pinned default merely to approximate a
whole source tree.

## Interpretation Boundary

Current jars contain Mojang's original technical names, but the `.java` files
remain Vineflower reconstructions of bytecode:

- class, method, field, parameter, and retained local-variable names are much
  stronger evidence than in an obfuscated release;
- control flow, constants, descriptors, and bytecode-visible data remain
  direct program evidence;
- expression layout, recovered source constructs, comments, and some local
  structure remain decompiler output; and
- the jar's included license and the Minecraft EULA/usage rules still apply.

Generated jars and source remain gitignored local research inputs. Commit
findings, tiny clean-room fixtures, and Mclone-owned implementation—not the
Minecraft jar, decompiled tree, or assets.

## First Research Result: Coasts

Tactical
[`259`](../tactical/259-modern-and-historical-coast-reference-survey.md)
compares Alpha v1.1.2_01, Beta 1.7.3, Java 1.17.1, Java 26.2, and current
Mclone coasts.

The durable modern result is that Java 26.2 no longer inserts shore biomes
through a late four-neighbor `ShoreLayer`. Its `OverworldBiomeBuilder`
classifies a coherent continentalness coast band and combines it with erosion,
temperature, humidity, and weirdness:

- low erosion in low/mid terrain slices selects Stony Shore;
- other cells select Beach, Snowy Beach, Desert, a shattered coast family, an
  ordinary middle biome, or a river/frozen river;
- therefore beach is not the mandatory coast outcome; and
- `SurfaceRuleData` gives Stony Shore mostly stone with a narrow gravel-noise
  patch, while Beach and Snowy Beach receive sand/sandstone.

This is an economical terrain-aware classifier, not a wave, deposition, or
exposure simulation. It supports Mclone's plan to combine a coherent
alongshore selector with measured terrain and shelf facts rather than copying
the complete vanilla climate table.

The selected Mclone implications, cross-era evidence, current three-seed
baseline, topology/performance contract, and next-tactical acceptance criteria
live in Tactical 259 rather than this reference-lane topic.

## Second Research Result: Inland Landform Semantics

Tactical
[`263`](../tactical/263-cross-era-inland-landform-survey.md) compares Alpha
v1.1.2_01, Beta 1.7.3, Java 1.17.1, Java 26.2, and current Mclone inland
terrain.

The durable modern finding is the separation and later reunion of terrain
planning and biome interpretation:

- shifted continentalness, erosion, ridges, folded ridges, and weirdness are
  shared two-dimensional semantic coordinates;
- `TerrainProvider` maps them into separate offset, factor, and jaggedness
  splines;
- its erosion-offset routing contains low-erosion mountains, ordinary
  mountains, wide and narrow plateaus, plains, extreme hills, and swamps;
- `NoiseRouterData` combines vertical gradient plus offset into depth,
  modulates it by factor and jaggedness, and then adds base 3D blended noise;
  and
- `OverworldBiomeBuilder` consumes the same coordinates to interpret peaks,
  slopes, plateaus, middle terrain, lowlands, valleys, coasts, and rivers as
  ecology.

This is stronger than either “biomes choose terrain” or “terrain and biomes
use unrelated noise.” Terrain facts exist first, specialized shaping consumes
them, and biomes describe the result. Base 3D noise remains available for
local volumetric form.

For Mclone, the bounded lesson is a small original landform-intent vocabulary
over the existing cheap periodic 2D fields, followed by biome, surface,
vegetation, coast, water, and selective 3D consumers. It does not justify
copying the literal spline graph, modern world height, cave/aquifer stack, or
biome parameter table.

## Refresh Protocol

When intentionally moving to a newer stable release:

1. change the one pinned version in `decompile-modern-mc.sh`;
2. build into a fresh generated output directory;
3. record manifest type, release time, jar size/SHA-1, naming mode,
   Vineflower version, and selected class count;
4. diff the focused source questions that motivated the refresh;
5. update this topic and any affected research tactical;
6. retain 1.17.1 as the parity target unless a separate product decision
   explicitly changes it; and
7. commit only tooling and written findings.

## Related

- [`../reference-minecraft.md`](../reference-minecraft.md)
- [`alpha-era-reference.md`](alpha-era-reference.md)
- [`beta-1.7.3-reference.md`](beta-1.7.3-reference.md)
- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md)
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`../tactical/259-modern-and-historical-coast-reference-survey.md`](../tactical/259-modern-and-historical-coast-reference-survey.md)
- [`../tactical/263-cross-era-inland-landform-survey.md`](../tactical/263-cross-era-inland-landform-survey.md)
