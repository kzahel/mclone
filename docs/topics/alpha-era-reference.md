# Alpha Era Reference Study

Topic: `alpha-era-reference`

Status: **Alpha v1.1.2_01 is the selected primary specimen. Its client/server
merged bytecode has been mapped and decompiled reproducibly, the terrain path
has been traced, and a deterministic headless terrain probe is live. No Alpha
generator profile has been added to the native engine.**

## Scope And Selection

The purpose of this study is to expose Minecraft's early core world-generation
system before biomes, dimensions, structures, registries, data-driven content,
and later collaborators made that system harder to see. It is both a historical
reference and a possible small clone target; it does not replace mclone's
Minecraft Java 1.17.1 parity target or the original mclone Overworld direction.
Its age and compactness make it a good proxy for an early core, but bytecode
archaeology does not establish class-by-class authorship; this report does not
claim that every recovered system had exactly one author.

The primary specimen is **Java Alpha v1.1.2_01**. It is the last Alpha build
before the Halloween Update introduced biomes and the Nether. That makes it a
cleaner expression of the terrain usually meant by "Alpha terrain" than the
last build carrying an Alpha label, v1.2.6. See the
[v1.1.2_01 history](https://minecraft.wiki/w/Java_Edition_Alpha_v1.1.2_01)
and the [Halloween Update history](https://minecraft.wiki/w/Halloween_Update).

Current community discussions are anecdotal rather than player telemetry, but
they consistently reveal the same split: v1.1.2_01 is recommended for the
pre-biome, "true Alpha" experience, while v1.2.6 is recommended when "last
Alpha" means the most features before Beta. Representative discussions include
[this v1.1.2_01 recommendation thread](https://www.reddit.com/r/GoldenAgeMinecraft/comments/1r5qlkz/what_is_the_best_version_of_alpha/)
and [this v1.1.2_01 versus v1.2.6 discussion](https://www.reddit.com/r/GoldenAgeMinecraft/comments/1gbeujz/whats_the_best_version_to_play_minecraft_alpha/).
The long-running Not So Seecret Saturday mod choosing v1.1.2_01 as its base is
[another useful sign of interest](https://www.minecraftforum.net/forums/mapping-and-modding-java-edition/minecraft-mods/2916817-the-not-so-seecret-saturday-mod-for-alpha-1-1-2_01)
in this particular cut, not proof of overall popularity.

## Preserved Historical Study Ladder

The earlier survey identified these useful cuts. They remain worth retaining
even though the current investigation is narrowed to Alpha.

| Cut | What it isolates | Role in this study |
|---|---|---|
| Classic 0.0.14 family | Very small finite Classic terrain and block-world baseline | Earliest compact comparison, but too early to represent survival-era Alpha |
| Indev 2010-02-23 | Late finite Indev maps and configurable map shape/theme | Best finite-world comparison |
| Infdev 2010-02-27 | Earliest public infinite-development checkpoint | Shows the initial finite-to-infinite transition |
| Infdev 2010-03-27 | Major infinite terrain-generator rewrite | Important ancestor of the Alpha density terrain; see the [version history](https://minecraft.wiki/w/Java_Edition_Infdev_20100327) |
| [Infdev 2010-06-18](https://minecraft.wiki/w/Java_Edition_Infdev_20100618) | First Seecret Friday-era Infdev and the launcher-preserved Infdev build | Convenient late-Infdev checkpoint with more surrounding game systems |
| Alpha v1.1.2_01 | Final pre-biome Alpha | **Primary specimen: smallest recognizable survival-era Alpha core** |
| Alpha v1.2.6 | Final Alpha-labelled release, after biomes and the Nether | Comparison specimen showing the first major complexity step |
| Beta 1.7.3 | Mature pre-Adventure-Update old terrain | Popular later baseline, but no longer an early single-core study |

The ladder should be understood as a set of code-archaeology checkpoints, not a
claim that every intermediate release is unimportant. If only one system is
ported, v1.1.2_01 is the right starting point. If the evolution is studied,
compare it first with Infdev 2010-03-27 and Alpha v1.2.6.

## Reproducible Decompilation

Mojang's official mappings begin much later than Alpha, so the existing
`scripts/decompile-mc.sh` cannot name this bytecode. The Alpha path instead uses
[Ornithe Feather](https://github.com/OrnitheMC/feather), a CC0 mapping set for
legacy Minecraft from Classic 0.0.12a_03 through 1.14.4. The alternative
[RetroMCP-Java](https://github.com/MCPHackers/RetroMCP-Java) is useful for
interactive legacy modding, but Feather provides a smaller command-line and
pin-friendly path for repeatable source study.

Run:

```bash
pnpm reference:alpha
```

The wrapper `scripts/decompile-alpha-mc.sh`:

1. resolves the requested version through Mojang's official version manifest;
2. downloads and SHA-1-verifies the official client jar;
3. checks out Feather at commit
   `f9c6723b76d00cfffd48f10de317a4646919bbfc`;
4. invokes Feather's Vineflower decompilation task; and
5. saves the decompiled source, named jar, Tiny mapping, and a provenance file
   under the gitignored `reference/minecraft-<version>/` tree.

For v1.1.2_01, Feather produces a named client/server merge identified as
`a1.1.2_01&a0.2.1`. The observed official client metadata is:

| Field | Value |
|---|---|
| Mojang type | `old_alpha` |
| Client SHA-1 | `daa4b9f192d2c260837d3b98c39432324da28e86` |
| Client size | 897,164 bytes |
| Feather mapping license | CC0-1.0 |

The generated reference source is private study material and remains ignored by
Git, just like the 1.17.1 reference tree. A decompilation is not the original
source distribution: control flow, constants, types, and bytecode-visible data
are reconstructed, while names come from community mappings and local-variable
structure comes from the decompiler.

## Code Map

After decompilation, begin with these generated paths under
`reference/minecraft-a1.1.2_01/src/net/minecraft/`:

- `world/gen/chunk/OverworldChunkGenerator.java`: terrain density, surface
  replacement, chunk seed, and all population attempts;
- `world/gen/noise/ImprovedNoise.java`: permutation-table gradient noise;
- `world/gen/noise/PerlinNoise.java`: the octave combiner;
- `world/gen/Generator.java`: neighbor-seeded generator base used by caves;
- `world/gen/carver/CaveWorldCarver.java`: rooms, branching tunnels, and lava;
- `world/chunk/ChunkCache.java`: the 32 by 32 chunk ring and population gate;
- `world/chunk/WorldChunk.java`: 16 by 128 by 16 block storage, height, and
  light arrays;
- `world/chunk/storage/AlphaChunkStorage.java`: per-chunk NBT persistence; and
- `world/World.java`: world seed, global winter flag, spawn selection, and the
  chunk-cache owner.

Only 16 Java files sit below `world/gen`: three core generator/carver classes,
ten feature classes, and three noise classes. This compactness is the main
reason the version is so instructive.

## Actual Generation Pipeline

```text
World
  -> ChunkCache: load saved chunk or call generator
     -> buildTerrain: 5 x 17 x 5 density lattice -> 16 x 128 x 16 blocks
     -> buildSurfaces: grass/dirt/sand/gravel/bedrock
     -> CaveWorldCarver: neighbor-seeded rooms and tunnels
     -> height map and initial light bookkeeping

2 x 2 neighboring chunks ready
  -> populateChunk: dungeons and ores
                   -> trees and plants
                   -> water/lava springs and optional snow
  -> mark terrainPopulated
```

This split matters. `getChunk` is a pure target-chunk construction path except
for height/light callbacks on the attached test world. Population is a later
world-mutating path: it intentionally places at offsets that can cross chunk
boundaries, so the cache waits for a 2 by 2 chunk neighborhood.

### Chunk representation and cache

- A chunk is 16 by 128 by 16: 32,768 one-byte block IDs.
- The raw block index is `x << 11 | z << 7 | y`; Y is contiguous.
- Metadata, sky light, and block light are separate nibble arrays. The 256-byte
  height map records the first transparent Y above each column.
- `ChunkCache` is a fixed 32 by 32 ring indexed with `chunk & 31`. An evicted
  slot is unloaded and saved before a different coordinate reuses it.
- `AlphaChunkStorage` writes individual compressed NBT files named from base-36
  chunk coordinates beneath a two-level 64 by 64 directory fanout.

### Random ownership

There is no single global seed stream. Reproducing output requires preserving
which random source owns each stage and the order in which it is consumed.

- The eight terrain-noise banks are constructed sequentially from one
  `java.util.Random(worldSeed)`. Constructor order therefore affects every
  later bank.
- Base terrain and surface randomness reseed with
  `chunkX * 341873128712 + chunkZ * 132897987541` and do **not** mix in the
  world seed directly. World-seed variation already lives in the constructed
  noise banks.
- Caves and population seed a random from the world seed, derive two odd `long`
  multipliers, then reseed a source chunk with
  `(chunkX * multiplierX + chunkZ * multiplierZ) ^ worldSeed`.
- The World's own `Random` is constructed without the world seed. On a new
  world it chooses `snowCovered` with probability 1/4 and random-walks the spawn
  until it finds a valid sand column. Both values are persisted, but creating
  the same terrain seed twice can produce different winter modes and spawns.

The last point is an original Alpha behavior, but it is undesirable as an
implicit reproducibility rule in mclone. A faithful profile should expose the
persisted winter bit explicitly and either fixture the spawn separately or
document an intentional deterministic adaptation.

### Noise core and density terrain

`ImprovedNoise` is the familiar 256-entry shuffled permutation table duplicated
to 512 entries, with random X/Y/Z offsets, quintic fade, hashed gradient dot
products, and trilinear interpolation. `PerlinNoise` constructs several of
these levels from the shared `Random`. Each successive level halves its
coordinate scale and is divided by that same factor, so the lower-frequency
levels receive progressively larger amplitude.

`OverworldChunkGenerator` owns eight octave banks:

| Bank | Levels | Purpose |
|---|---:|---|
| `minLimitPerlinNoise` | 16 | First 3D density boundary |
| `maxLimitPerlinNoise` | 16 | Second 3D density boundary |
| `perlinNoise1` | 8 | Chooses or blends the two density boundaries |
| `perlinNoise2` | 4 | Sand and gravel surface masks |
| `perlinNoise3` | 4 | Surface-layer depth |
| `scaleNoise` | 10 | Broad horizontal terrain scale |
| `depthNoise` | 16 | Broad vertical terrain offset |
| `forestNoise` | 8 | Tree attempt count during population |

Terrain is not sampled per block. It creates a 5 by 17 by 5 density lattice,
then trilinearly expands each 4 by 8 by 4 cell to the chunk's 16 by 128 by 16
blocks. At every lattice point:

1. two 16-level 3D fields produce lower and upper candidate densities;
2. an 8-level selector chooses one, the other, or a linear blend;
3. two 2D fields alter the vertical center and vertical scale;
4. an asymmetric vertical gradient penalizes distance from that center; and
5. the top three coarse layers fade toward density -10 to guarantee open sky.

Positive density becomes stone. Non-positive density becomes air above Y=64
and water below it; a global-winter world turns the water surface into ice.
There is no biome lookup, climate map, heightmap input, structure mask, aquifer,
or block-state palette in this decision.

### Surface pass

The second pass scans downward through each column. Two orientations of one
four-level noise bank decide sand and gravel patches, another four-level bank
chooses the surface thickness, and chunk-local random calls make the boundaries
slightly irregular.

- ordinary stone near sea level becomes grass over dirt;
- sand-mask columns become sand over sand;
- gravel-mask columns lose the top block to air/water and use gravel below;
- a missing top block below sea level is repaired to water; and
- Y=0 receives an uneven bedrock layer using a fresh `nextInt(6)` at each Y.

The surface vocabulary is hardcoded and global. The generator obtains variety
from geometry, coastlines, caves, sparse surface masks, and population—not from
regional terrain types.

### Cave pass

`Generator` gives the cave carver a range of eight source chunks in every
direction, so generating one target evaluates a 17 by 17 source-chunk square.
Every source is independently seeded and may send a tunnel into the target;
only the target's block array is mutated, so neighboring chunk contents are not
an input dependency.

For each source chunk the cave count is drawn through three nested random
bounds and then usually zeroed by a 14/15 rejection. Surviving systems may start
with a room and create one or more wandering tunnels. Wide tunnels split once
into two branches. The carver:

- bounds and early-rejects paths that cannot reach the target chunk;
- refuses a candidate volume if it finds flowing or still water;
- replaces only stone, dirt, or grass;
- fills carved space below Y=10 with flowing lava and uses air above; and
- repairs exposed dirt below a removed grass block back to grass.

This is already recognizably classic Minecraft cave generation, but it is
isolated in one small class with no biome carver lists, configuration registry,
carving masks, or aquifer interaction.

### Delayed population

Population reseeds once per chunk and runs a hardcoded list in a fixed order.
The important fixed attempt counts and vertical bounds are:

| Feature | Attempts | Parameters / Y bound |
|---|---:|---|
| Dungeon | 8 | random Y below 128 |
| Clay patch | 10 | size 32, random Y below 128 |
| Dirt vein | 20 | size 32, random Y below 128 |
| Gravel vein | 10 | size 32, random Y below 128 |
| Coal vein | 20 | size 16, random Y below 128 |
| Iron vein | 20 | size 8, random Y below 64 |
| Gold vein | 2 | size 8, random Y below 32 |
| Redstone vein | 8 | size 7, random Y below 16 |
| Diamond vein | 1 | size 7, random Y below 16 |
| Water spring | 50 | nested distribution below 128 |
| Lava spring | 20 | more strongly bottom-weighted nested distribution |

The ore algorithm draws a short random line and expands overlapping ellipsoids
along it, replacing stone only. Tree count comes from `forestNoise` plus
randomness; a 1/10 roll adds an attempt, and another 1/10 roll selects the much
more elaborate large-oak generator for that chunk's tree attempts. The rest is
small direct feature code for flowers, mushrooms, sugar cane, cactus, springs,
and optional winter snow. No general decoration registry or placement-modifier
framework exists.

## What Changes By Alpha v1.2.6

The v1.2.6 comparison was decompiled through the same pinned Feather pipeline
as a client/server merge (`a1.2.6&a0.2.8`). The mapped source grows from 390 to
444 `net/minecraft` Java files, and `world/gen` grows from 16 to 26 files.

| Concern | v1.1.2_01 | v1.2.6 |
|---|---|---|
| Overworld regions | No biomes | 11 named Overworld biomes |
| Climate noise | None | Temperature, downfall, and biome-detail simplex streams |
| Terrain density | Global scale/depth fields | Same min/max/selector skeleton, modulated by climate |
| Surface blocks | Global grass/dirt/sand/gravel rules | Biome-provided top and filler blocks |
| Trees and snow | Global forest noise and whole-world winter bit | Biome-dependent trees and local temperature snow/ice |
| Dimensions | Overworld only | Nether generator, cave carver, and features |
| Added features | Original hardcoded set | Lakes and pumpkins, among other biome-era changes |

`BiomeSource` creates three `PerlinSimplexNoise` streams with seeds derived as
`worldSeed * 9871`, `worldSeed * 39811`, and `worldSeed * 543321`. It selects
Rainforest, Swampland, Seasonal Forest, Forest, Savanna, Shrubland, Taiga,
Desert, Plains, Ice Desert, or Tundra. That climate then affects surfaces,
trees, snow/ice, and terrain scale/depth.

The comparison is valuable precisely because the old density skeleton remains
visible while policy begins accumulating around it. v1.2.6 is the correct
specimen for studying the birth of the biome/Nether architecture; v1.1.2_01 is
the better specimen for cloning the original Alpha terrain core.

## Executable Evidence

The headless probe invokes the actual mapped
`OverworldChunkGenerator.getChunk` rather than reimplementing it. It includes
density terrain, the surface pass, caves, and height/light bookkeeping. It
deliberately stops before delayed population because population mutates a world
and crosses chunk boundaries.

Run:

```bash
pnpm oracle:alpha:terrain -- --seed 12345 --chunk-x 0 --chunk-z 0 --snow false
```

The pinned reference fingerprint is:

```json
{
  "stage": "terrain_surfaces_caves",
  "seed": 12345,
  "chunk_x": 0,
  "chunk_z": 0,
  "snow": false,
  "sha256": "947b3a034360da67c83baef0fc486fd05f8373d57080abc2fe952662db55e833",
  "height_min": 93,
  "height_max": 100,
  "height_mean": 95.496094,
  "blocks": {
    "air_0": 8652,
    "stone_1": 22247,
    "grass_2": 256,
    "dirt_3": 947,
    "bedrock_7": 617,
    "flowing_water_8": 0,
    "water_9": 0,
    "flowing_lava_10": 49,
    "lava_11": 0,
    "sand_12": 0,
    "gravel_13": 0,
    "ice_79": 0
  }
}
```

The probe overrides the unseeded winter choice with an explicit Boolean, uses a
temporary world directory, deletes it after the run, and produces identical
output on repeated runs.

## Implications For A Native Alpha Profile

A useful first port should be narrow and reference-locked to v1.1.2_01 rather
than described generically as "Alpha":

1. add an explicit generator identity such as `alpha-a1.1.2_01` to the shared
   `mclone-worldgen` profile boundary and update the compatibility ledger;
2. port Java `Random`, `ImprovedNoise`, and `PerlinNoise` directly, preserving
   constructor and sample order;
3. implement the 5 by 17 by 5 density lattice and 4 by 8 by 4 interpolation,
   then match terrain-only oracle fingerprints across positive and negative
   chunk coordinates;
4. add the surface pass and cave carver as separately fingerprinted stages;
5. only then add the 2 by 2 population contract and cross-chunk features; and
6. treat global winter and spawn as explicit saved profile state instead of
   allowing hidden wall-clock-seeded randomness.

The best first visual milestone is terrain plus surfaces, without population.
It is small enough to review against one Java class, immediately shows whether
the characteristic overhangs and floating terrain survived the port, and avoids
mixing density errors with cave or feature-order errors.

Known gaps in the present reference work:

- there is no native Alpha generator implementation yet;
- the oracle does not yet fingerprint raw density, surface-only, or
  post-population stages separately;
- the population oracle still needs a controlled multi-chunk world fixture;
- no representative screenshot atlas has been captured across several seeds;
  and
- Infdev 2010-03-27 has not yet been decompiled alongside this source to locate
  the exact code-level inheritance boundary.

Those gaps define follow-up slices; none prevents using v1.1.2_01 as the chosen
Alpha study and clone target.
