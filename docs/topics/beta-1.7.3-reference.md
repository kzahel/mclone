# Beta 1.7.3 Reference Study

Topic: `beta-1.7.3-reference`

Status: **Minecraft Java Beta 1.7.3 is the selected Beta specimen. Its mapped
client/server merge is reproducibly decompiled and its world-generation source
has been traced and measured against Alpha v1.1.2_01. This is research only:
there is no native Beta profile, parity oracle, compatibility identity, or
implementation tactical.**

## Scope And Selection

Beta 1.7.3 is the mature endpoint of the old Beta terrain family immediately
before Beta 1.8's Adventure Update generation changes. It is the useful Beta
bookend for the existing Alpha study because it answers two different
questions at once:

- how much architecture accumulated while the original Alpha density skeleton
  remained recognizable; and
- how large a close, playable old-Beta Overworld port might be relative to the
  implemented `alpha-v1` profile.

This report is intentionally not an implementation proposal. It does not
allocate a `beta-v1` identity, select exact versus flavor-close population,
decide whether the Nether belongs to a future profile, or authorize native
Rust changes. Those choices should be made only after reviewing this source
map and, if desired, building a staged Java oracle.

The earlier Alpha ladder and historical selection rationale live in
[`alpha-era-reference.md`](alpha-era-reference.md). The deliberately
close-but-not-perfect Alpha implementation contract lives separately in
[`alpha-world-generation.md`](alpha-world-generation.md).

## Reproducible Decompilation

Run:

```bash
pnpm reference:beta
```

The entry point `scripts/decompile-beta-mc.sh` pins `b1.7.3` and delegates to
the shared legacy Feather pipeline. The pipeline:

1. resolves `b1.7.3` through Mojang's official version manifest;
2. downloads and SHA-1-verifies the official client;
3. checks out Ornithe Feather at a pinned revision;
4. obtains Feather's matching preserved server input and CC0 mappings;
5. maps and merges the client/server bytecode and decompiles it with
   Vineflower; and
6. records the source, named jar, Tiny mapping, and provenance under the
   gitignored `reference/minecraft-b1.7.3/` tree.

Observed provenance:

| Field | Value |
|---|---|
| Minecraft version | `b1.7.3` |
| Mojang type | `old_beta` |
| Official client SHA-1 | `43db9b498cb67058d2e12d394e6507722e71bb45` |
| Official client size | 1,465,375 bytes |
| Feather revision | `f9c6723b76d00cfffd48f10de317a4646919bbfc` |
| Feather merged build | `b1.7.3` |
| Mapping license | CC0-1.0 |

The generated source is a reconstruction, not an original source release.
Control flow, constants, and bytecode-visible data come from the program;
class/member names come from community mappings; local-variable structure and
some expression shape come from Vineflower.

## Start Here In The Decompiled Tree

Paths below are relative to
`reference/minecraft-b1.7.3/src/net/minecraft/`.

- `world/gen/chunk/OverworldChunkGenerator.java`: density terrain, surface
  replacement, caves, and the complete hardcoded Overworld population order;
- `world/biome/source/BiomeSource.java`: temperature, downfall, and climate
  detail noise;
- `world/biome/Biome.java`: the 64 by 64 climate lookup, top/filler blocks,
  precipitation flags, spawn lists, and default tree selection;
- `world/biome/{Forest,Rainforest,Taiga}Biome.java`: tree-family overrides;
- `world/gen/noise/{ImprovedNoise,PerlinNoise,SimplexNoise,
  PerlinSimplexNoise}.java`: Beta's two related noise families;
- `world/gen/Generator.java` and
  `world/gen/carver/CaveWorldCarver.java`: neighbor-seeded cave traversal;
- `world/gen/feature/`: 21 small hardcoded feature classes;
- `world/gen/chunk/NetherChunkGenerator.java` and
  `world/gen/carver/NetherCaveCarver.java`: Nether generation;
- `world/gen/chunk/SkyChunkGenerator.java`: the present but ordinarily
  inaccessible Skylands prototype;
- `world/dimension/`: dimension-to-biome-source/generator selection;
- `world/chunk/{ChunkCache,WorldChunk}.java`: generation admission, raw chunk
  storage, height/light state, and delayed population; and
- `world/chunk/storage/{RegionChunkStorage,RegionFile,RegionIo}.java`: McRegion
  persistence and conversion from Alpha chunk files.

There is no structure package and no ravine, mineshaft, stronghold, or village
generator in this specimen. Those are not hidden behind configuration; the
corresponding classes and dispatch paths are absent.

## Measured Size Relative To Alpha

The counts below measure mapped `net/minecraft` Java source, excluding bundled
libraries. Blank lines, imports, and decompiler formatting are included, so
the numbers are comparative evidence rather than an engineering estimate by
themselves.

| Surface | Alpha v1.1.2_01 | Beta 1.7.3 | Ratio |
|---|---:|---:|---:|
| Official client jar | 897,164 bytes | 1,465,375 bytes | 1.63x |
| Minecraft Java files | 390 | 588 | 1.51x |
| Minecraft Java lines | 45,306 | 74,133 | 1.64x |
| `world/gen` files | 16 | 32 | 2.00x |
| `world/gen` lines | 1,804 | 4,012 | 2.22x |
| Overworld generator lines | 495 | 648 | 1.31x |
| cave-carver lines | 206 | 207 | 1.00x |
| noise files / lines | 3 / 225 | 5 / 444 | 1.67x / 1.97x |
| feature files / lines | 10 / 851 | 21 / 1,559 | 2.10x / 1.83x |
| biome files / lines | 0 / 0 | 10 / 469 | new |

The raw `world/gen` 2.22x ratio overstates an Overworld-only port because Beta
adds a 391-line Nether generator, a 542-line Skylands generator, a Nether cave
carver, and Nether-only features. A selected direct-source slice for the Beta
Overworld—its generator, generic generator, cave carver, five noise files,
seven required biome/source files, and all reachable feature classes—is about
3,024 Java lines versus Alpha's 1,804, or **1.68x**.

The whole-game growth is also broader than world generation. Relative to the
Alpha tree, the mapped Beta tree adds classes most heavily under the client
(43 new paths), networking (28), blocks (26), world generation (16), items
(15), entities (13), statistics (11), inventory (11), biomes (10), and world
storage (9). Copying Beta terrain does not imply copying all of Beta.

## Overworld Pipeline

```text
World
  -> Dimension
     -> BiomeSource
        temperature: 4-octave simplex-perlin field
        downfall:    4-octave simplex-perlin field
        detail:      2-octave simplex-perlin field
     -> OverworldChunkGenerator
        buildTerrain: 5 x 17 x 5 density -> 16 x 128 x 16 blocks
        buildSurfaces: biome top/filler + sand/gravel/sandstone/bedrock
        CaveWorldCarver: range-eight neighbor-seeded caves
        WorldChunk height/light initialization

2 x 2 neighboring chunks present
  -> populateChunk
     lakes -> dungeons -> clay/ores -> biome vegetation
     -> springs -> temperature-derived snow
  -> terrainPopulated
```

This remains a compact, direct system. `OverworldChunkGenerator` still owns
all population policy in one 648-line class. `Biome` owns a few values and a
tree selector, but there is no decoration registry, configured-feature graph,
placement-modifier framework, structure manager, carver list, or data-driven
worldgen layer.

### Random ownership

The core seeding shape remains recognizably Alpha:

- the same eight Overworld Perlin banks are constructed in the same order from
  one `Random(worldSeed)`;
- terrain and surfaces reseed with
  `chunkX * 341873128712 + chunkZ * 132897987541`;
- caves and population derive two odd multipliers from the world seed and seed
  source/target chunks with `(x * multiplierX + z * multiplierZ) ^ worldSeed`;
  and
- delayed population consumes one fixed, order-sensitive random stream.

Beta adds three independently seeded climate streams:

| Field | Random seed | Octaves | Sampling scale |
|---|---:|---:|---:|
| temperature | `worldSeed * 9871` | 4 | 0.025 |
| downfall | `worldSeed * 39811` | 4 | 0.05 |
| climate detail | `worldSeed * 543321` | 2 | 0.25 |

Unlike pre-biome Alpha, winter is no longer an unseeded whole-world Boolean.
Ice, snow, weather, surface material, and vegetation derive from the seeded
local climate. Spawn selection still uses the World's separately constructed
`Random` and wanders until `Dimension.isValidSpawnPoint` accepts a sand column;
that historical creation behavior should not be confused with generator seed
parity.

## Terrain: Same Skeleton, Different Field

Beta keeps all of the easily recognizable Alpha machinery:

- 16 by 128 by 16 chunks with one-byte block IDs;
- sea level 64;
- the eight min/max/selector/surface/scale/depth/forest noise banks;
- a 5 by 17 by 5 coarse density lattice;
- 4 by 8 by 4 trilinear expansion into blocks; and
- the top-three-lattice-layer fade toward density -10.

It is not byte-equivalent Alpha terrain with biomes painted afterward. Two
changes alter the field itself.

First, `ImprovedNoise.add` has a dedicated `sizeY == 1` branch with a 2D
gradient function. Alpha routes those requests through the generic 3D path.
The Beta branch affects the horizontal scale/depth and surface/forest fields,
so a future port cannot blindly reuse Alpha's bulk-noise output.

Second, `generateHeightMap` reads the 16 by 16 temperature and downfall arrays.
It multiplies downfall by temperature, applies a strong fourth-power shaping
curve, and uses the result to modulate broad terrain scale before applying the
depth field. Biome class identity does not directly select mountains or
valleys; the continuous climate fields influence geometry, while the discrete
biome chooses surfaces and population.

At each final block, positive density is stone. Non-positive density is air
above Y=64 and water below it. Water at Y=63 becomes ice when local
temperature is below 0.5.

### Biome selection

`BiomeSource` shapes temperature and downfall into `[0, 1]`, then looks up a
64 by 64 precomputed climate table. The effective Overworld results are:

- Rainforest, Swampland, Seasonal Forest, Forest;
- Savanna, Shrubland, Taiga, Desert, Plains; and
- Tundra.

`ICE_DESERT` is declared and assigned sand/snow policy but is not returned by
the recovered `computeBiome` decision tree. `HELL` and `SKY` are fixed
dimension biomes rather than Overworld climate results.

Biome data is not stored in each chunk. The chunk NBT contains blocks,
metadata, sky/block light, height map, entities, block entities, and the
`TerrainPopulated` bit; biome and temperature are recomputed from world seed.

### Surface pass

The surface pass still scans downward and uses the four-octave sand/gravel
bank plus the four-octave depth bank. Beta changes the material policy:

- the default top/filler comes from `Biome.surfaceBlock` and
  `Biome.subsurfaceBlock`;
- Desert uses sand over sand while most biomes use grass over dirt;
- the old global sand and gravel masks can override biome material near sea
  level;
- sand transitions to sandstone after its random-depth layer is exhausted;
- missing top material below sea level is repaired to water; and
- the uneven Y=0 bedrock pass remains.

The result is still far smaller than modern surface builders: two biome block
fields plus hardcoded overrides, not a rule graph.

## Caves

The Beta cave carver is 207 lines versus Alpha's 206. Its range, source seeding,
room/tunnel branching, water rejection, low lava, replaceable blocks, and
target-only mutation are otherwise the same recognizable algorithm.

One small source change matters for exact parity. Beta adds a horizontal
ellipse guard before walking the raw Y cursor. Alpha decrements that cursor
even for columns horizontally outside the tunnel; Beta does not. The native
Alpha port deliberately preserves the older cursor quirk, so a Beta carver
would need its own corrected loop even if most code were shared.

There is no ravine/canyon carver and no carving-mask or aquifer system.

## Delayed Population And Features

Population still begins only after the cache sees a 2 by 2 neighborhood. It
mutates the world at `+8`-shifted positions and can cross chunk borders, so
historical output remains sensitive to population state and chunk loading.

The fixed core order and counts are:

| Feature | Attempts / gate | Y distribution or note |
|---|---:|---|
| water lake | 1 in 4 chunks | uniform below 128 after ground search |
| lava lake | 1 in 8, then height gate | nested below 120; usually below sea level |
| dungeon | 8 | uniform below 128 |
| clay | 10 | size 32, uniform start below 128, water-only |
| dirt | 20 | size 32, uniform below 128 |
| gravel | 10 | size 32, uniform below 128 |
| coal | 20 | size 16, uniform below 128 |
| iron | 20 | size 8, uniform below 64 |
| gold | 2 | size 8, uniform below 32 |
| redstone | 8 | size 7, uniform below 16 |
| diamond | 1 | size 7, uniform below 16 |
| lapis | 1 | size 6, sum of two `nextInt(16)` draws |
| water spring | 50 | nested below 128 |
| lava spring | 20 | triple-nested, strongly bottom-weighted |

One biome sampled at `(chunkOrigin + 16, chunkOrigin + 16)` controls the
chunk's population counts and tree selector. Forest/Rainforest/Taiga are
tree-heavy; Seasonal Forest is moderately wooded; Desert, Tundra, and Plains
subtract enough attempts to be effectively treeless. Forest can choose oak,
large oak, or birch; Taiga chooses pine or spruce; Rainforest favors large oak.

Other visible population additions beyond the primary Alpha specimen are:

- biome-weighted yellow flowers and one-block tall grass/fern variants;
- desert dead bushes and cactus;
- rare red flowers and mushrooms;
- sugar cane and rare pumpkin patches;
- sandstone beneath sand;
- lapis ore; and
- local temperature-derived surface snow.

The feature architecture is still deliberately small: 21 subclasses of an
11-line `Feature` base. Existing Alpha features mostly retain their logic with
API renames or correctness fixes. Notable byte-visible changes include true
flooring at negative coordinates for veins, clay, and large-oak branches, and
immediate liquid ticking after a spring is placed.

## Other Generator Families In The Tree

Beta 1.7.3's full world-generation package is larger than its Overworld:

- `NetherChunkGenerator` is a separate 391-line density/surface/population
  pipeline with ceiling and floor bedrock, lava sea, soul sand, gravel,
  Nether caves, fire, glowstone, lava falls, and mushrooms.
- `SkyChunkGenerator` is a 542-line floating-island prototype attached to
  `SkyDimension`/`Biome.SKY`. The code is present under dimension ID 1, but it
  is not part of ordinary Beta 1.7.3 survival progression and should not be
  silently included in an Overworld clone scope.

The Nether already existed by late Alpha and is historically part of Beta,
but it is a distinct implementation and product decision. An eventual
`beta-b1.7.3` Overworld profile does not automatically imply that both extra
generator families must ship with it.

## Chunk Cache And Persistence Evolution

The generated block representation remains the old compact form:

- 32,768 block bytes in X/Z/Y order;
- separate metadata, sky-light, and block-light nibble arrays;
- a 256-byte height map; and
- a `TerrainPopulated` Boolean.

The storage and runtime shell are more mature than the Alpha specimen:

- `ChunkCache` uses loaded-chunk maps/lists and an unload set rather than
  Alpha's single fixed 32 by 32 ring;
- the delayed 2 by 2 population gate remains explicit;
- `.mcr` region files group 32 by 32 chunks with 4 KiB sectors, offset and
  timestamp tables, and zlib/GZip payload support; and
- migration code converts Alpha's per-chunk directory layout into regions.

These systems explain part of the whole-source growth but do not need to be
ported to reproduce terrain inside mclone's existing persistence/scheduler
architecture.

## What A Future Native Port Would Actually Add

No implementation is authorized yet, but the source gives a useful boundary
for the later discussion.

Reusable from the completed Alpha work:

- Java `Random` behavior and seed-domain helpers;
- the 5 by 17 by 5 density/interpolation shape;
- 128-block active-height handling;
- most cave traversal concepts and deterministic feature-region scheduling;
- ore ellipsoids, simple plants, trees, springs, and population cache shape;
  and
- the engine's existing sandstone, lapis, birch/spruce, dead-bush, pumpkin,
  grass/fern, snow, and ice content.

New or Beta-specific work:

- the dedicated Beta 2D `ImprovedNoise` branch;
- simplex/perlin-simplex climate streams and biome lookup;
- climate-modulated density and local ice/snow;
- biome surface/filler and sandstone transition;
- corrected Beta cave cursor behavior;
- biome-dependent population counts and tree families;
- lakes, lapis, grass/fern, dead bushes, and pumpkins in the exact Beta random
  order; and
- new Beta semantic mapping and staged oracle fixtures.

Beta's one-block tall-grass metadata should map to mclone's short `GRASS` and
`FERN`, not the modern two-block `TALL_GRASS_LOWER/UPPER` pair.

## Relative Implementation Assessment

The conclusion is encouraging but not “Alpha again with a biome enum.”

- **Terrain/surface/caves only:** still compact. Much of the Alpha shape is
  reusable, while climate and the changed 2D noise path are the main new
  parity work.
- **Flavor-close playable Overworld:** plausibly around 1.5–2x the conceptual
  worldgen surface of Alpha, consistent with the 1.68x selected Java slice.
  Existing mclone content and feature primitives should reduce the amount of
  wholly new Rust.
- **Exact post-population parity:** materially harder because Beta retains
  cross-chunk, load-order-sensitive population and adds more random consumers.
- **Full Beta including Nether/Skylands:** a separate scope; the raw 2.22x
  `world/gen` ratio becomes more representative.

The Alpha implementation arrived quickly because its visible core was one
global terrain language and because flavor-close deterministic population was
accepted. The same policy could keep a Beta Overworld tractable, but exact
climate, terrain, surface, and cave stages should still be proved independently
before any feature approximation.

## Evidence And Validation

Completed in this research slice:

- official client SHA-1 verification;
- pinned Feather mapping/decompile completion;
- idempotent `pnpm reference:beta` rerun;
- direct trace of Overworld, biome, noise, cave, feature, dimension, chunk, and
  region-storage source;
- file/line measurements against the pinned Alpha source; and
- explicit absence search for ravines and structure generators.

Not yet built, by design:

- a Beta Java terrain/biome/surface/cave oracle;
- committed Beta output receipts;
- a native profile or binary/persistence identity;
- a parity policy for population;
- Beta screenshots; or
- Nether/Skylands scope and product decisions.

## Recommended Next Discussion

Before writing Rust, choose these boundaries explicitly:

1. Is the target only the Beta 1.7.3 Overworld, or also the Nether?
2. Should terrain, climate, surfaces, and caves be exact at pinned oracle
   stages while population remains deterministic/flavor-close, as with Alpha?
3. Should the profile reproduce Beta's biome palette exactly, including
   unreachable declarations only as reference data, or expose only effective
   generated biomes?
4. Is spawn still a deterministic mclone adaptation rather than Beta's
   unseeded sand-search walk?
5. Should shared legacy noise/cave primitives be extracted from `alpha.rs`, or
   should Beta remain a sibling direct port until both parity suites are stable?

If those answers favor a close Overworld-only profile, the next bounded action
should be a Java oracle and implementation tactical—not generator code in the
same research commit.
