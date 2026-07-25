# JJThunder To The Max Reference Study

Topic: `jjthunder-to-the-max-reference`

Status: **JJThunder To The Max v0.8.0 for Minecraft Java 26.2 has been
downloaded, hash-verified, extracted, and statically traced. The pack uses a
2,096-block-tall Overworld, a structured height-field family, finite-difference
pseudo-erosion, terrain-participating rivers, relative-depth cave families, and
a mountain-conditioned Underlands density field. No public source repository
was found; the distributed datapack is itself readable JSON source and is All
Rights Reserved, so no third-party artifact is vendored here.**

## Scope And Short Answer

This is a community-worldgen design study beside the Alpha and Beta source
studies. It is not a parity target, a proposed dependency, or permission to
copy the pack's rule tables.

The immediate height answer is unambiguous:

- Minecraft Java 1.17.1 still used Y `0..=255`, a 256-block Overworld.
- Java 1.18 expanded the Overworld to Y `-64..=319`, 384 blocks. The official
  [Caves & Cliffs Part II summary](https://www.minecraft.net/en-us/article/caves---cliffs-part-ii-the-features)
  describes the new lower bound as Y=-64 and the upper build boundary as
  Y=320.
- The official Java 26.2 server data still says `min_y: -64`, `height: 384`.
  Vanilla has not received another Overworld height increase since 1.18.
- JJThunder v0.8.0 replaces that with `min_y: -64`, `height: 2096`: valid block
  coordinates are Y `-64..=2031`, with 2032 as the exclusive upper boundary.

The pack therefore does **not** fit 1,900-block mountains and the Underlands
into the ordinary modern or 1.17 vertical budget. It expands physical height
to 5.46 times current vanilla and 8.19 times 1.17. It also does not conserve a
fixed amount of stone between surface and underground. The cave density is
subtracted from terrain; high terrain can be both tall and hollow.

## Specimen And Provenance

The primary specimen is the current release as of 2026-07-18:

| Field | Receipt |
|---|---|
| Project | [JJThunder To The Max](https://modrinth.com/datapack/jjthunder-to-the-max) |
| Project ID | `1NnAVIR5` |
| Author | `jjthunder99` |
| Version | [v0.8.0 for Java 26.2](https://modrinth.com/datapack/jjthunder-to-the-max/version/v9oSSj2w) |
| Published | 2026-07-04 |
| Archive | `JJThunder_To_The_Max_26.2_v0.8.0.zip` |
| Size | 181,930 bytes |
| SHA-1 | `267e953bd4a5ff211817297f3dcc9fd8266f7bfe` |
| License | All Rights Reserved |
| Pack format | `107` |

The exact archive can be reproduced outside the repository:

```bash
study_dir=$(mktemp -d /tmp/jjthunder-study.XXXXXX)
curl -fsSL \
  'https://cdn.modrinth.com/data/1NnAVIR5/versions/v9oSSj2w/JJThunder_To_The_Max_26.2_v0.8.0.zip' \
  -o "$study_dir/JJThunder_To_The_Max_26.2_v0.8.0.zip"
shasum -a 1 "$study_dir/JJThunder_To_The_Max_26.2_v0.8.0.zip"
unzip "$study_dir/JJThunder_To_The_Max_26.2_v0.8.0.zip" \
  -d "$study_dir/extracted"
```

The archive contains 279 JSON resources plus `pack.mcmeta`; there is no
compiled code to decompile. Modrinth's
[project API record](https://api.modrinth.com/v2/project/1NnAVIR5) has no
`source_url`, general and GitHub-focused searches found no repository, and the
archive contains no README, license file, author metadata, or external URL.
Static study of the distributed definitions is therefore the reproducible
source path. All analysis below is paraphrase and independently derived
structure; the third-party archive remains under `/tmp` only.

The v0.6.0 Java 1.21.0/1.21.1 archive was also checked because its changelog
names the current cave concept "Underlands 2.0" and describes 500-block-tall
caves under the highest mountains. Its receipt is:

| Field | Receipt |
|---|---|
| Version | [v0.6.0](https://modrinth.com/datapack/jjthunder-to-the-max/version/fQwTBBdJ) |
| Archive | `JJThunder_To_The_Max_1.21.0_1.21.1_v0.6.0.zip` |
| Size | 156,552 bytes |
| SHA-1 | `357da1dc94cc1f03d5ed915085d9433d1c26cb35` |

V0.8 retains the v0.6 Underlands equation but now drives it from a preserved
pre-hollowing height field. Its changelog adds independent river paths,
river-aware slope generation, and reduced volcanic frequency and peak ranges.

## Height Comparison

The top coordinate below is inclusive; the boundary is the first invalid Y.

| Specimen | Minimum Y | Height | Top Y / boundary | 16-block sections |
|---|---:|---:|---:|---:|
| Alpha v1.1.2_01 active output | 0 | 128 | 127 / 128 | 8 |
| Beta 1.7.3 active output | 0 | 128 | 127 / 128 | 8 |
| Java 1.17.1 Overworld | 0 | 256 | 255 / 256 | 16 |
| Java 1.18 through 26.2 Overworld | -64 | 384 | 319 / 320 | 24 |
| JJThunder v0.8 Overworld | -64 | 2,096 | 2,031 / 2,032 | 131 |

The 1.17.1 values are also visible in the local reference's
[`DimensionType.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/dimension/DimensionType.java)
and
[`NoiseGeneratorSettings.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseGeneratorSettings.java).
For the current comparison, the official
[26.2 version metadata](https://piston-meta.mojang.com/v1/packages/9e272020887b72a23b1a525c5d8e74d2d7aa8222/26.2.json)
identifies server SHA-1
`823e2250d24b3ddac457a60c92a6a941943fcd6a`; its bundled vanilla resources
contain:

```text
dimension_type/overworld: min_y=-64, height=384, logical_height=384
noise_settings/overworld: min_y=-64, height=384,
                          size_horizontal=1, size_vertical=2
```

JJThunder changes the same records to:

```text
dimension_type/overworld: min_y=-64, height=2096, logical_height=384
noise_settings/overworld: min_y=-64, height=2096,
                          size_horizontal=1, size_vertical=1
sea_level: 63
```

Two details matter beyond the headline number:

1. Sea level remains vanilla Y=63. Almost all extra space goes above the old
   world, although mountains also contain very deep relative interiors.
2. `logical_height` remains 384. Physical chunks and building extend to 2031,
   but mechanics consulting logical rather than physical height retain a
   vanilla-sized interval. In the 1.17.1 source, portal creation and chorus
   fruit are examples of logical-height consumers. Any mclone height expansion
   would need a mechanic-by-mechanic contract rather than only taller chunks.

## Resource And Runtime Shape

The v0.8 archive replaces entries under the `minecraft` namespace rather than
adding an isolated generator namespace:

| Resource family | Files or entries |
|---|---:|
| density functions | 175 files |
| biome definitions replaced | 42 files |
| multi-noise biome entries | 60 |
| placed-feature overrides | 38 files |
| configured carvers | 3 files |
| structures / structure sets | 5 / 2 files |
| noise definitions | 5 files |

This explains the project's load-order warnings and poor compatibility with
other terrain packs: it replaces the Overworld dimension, dimension type,
noise settings, many vanilla biomes, features, and structures by canonical
ID. It is a complete Overworld policy layer, not a small mountain modifier.

A reference walk from `final_density` and the six custom biome-router fields
reaches 79 of the 175 density-function files. The other 96 are not on the live
v0.8 graph and appear to be retained experiments or previous compositions.
That history is useful archaeology but should not be copied as architecture.

The live conceptual graph is:

```text
coarse landform selector + independent river corridor
  -> blend ocean / plains / hills / extreme hills / extreme mountains
  -> per-family offset + pseudo-eroded factor + conditional jagged detail
  -> raw height field
       |-> remap to biome continentalness / altitude classes
       |-> drive relative depth and mountain-conditioned Underlands
       `-> river processing + v0.8 high-peak hollowing -> surface density

surface-relative depth
  -> small -> medium -> large caves -> Underlands
  -> min(surface density, cave density)
  -> bottom seal + 4-block vertical interpolation
  -> final stone/air field

the same terrain, depth, river, climate, and final-density fields
  -> 3D biome selection -> surface materials -> features -> structures
```

## Most Interesting Surface Ideas

### Route macro intent into specialized terrain families

The pack does not merely multiply one noise by a huge height. A low-frequency
`cave_layer` noise at XZ scale `0.006`, perturbed in its center by a second
field, becomes `determiner/overworld/smart`. Nested splines use it to blend
separate ocean, plains, hills, extreme-hills, and extreme-mountain recipes.
Each recipe owns distinct offset, broad factor, and jagged-detail terms.

This is close to mclone's intended structured-macro approach: first decide
what family a place belongs to, then ask that family's concrete rule for a
height. The benefit is local tuning. Making mountains sharper need not make
plains noisy or oceans corrugated.

### Rivers participate in terrain before decoration

An independent `calcite` noise at XZ scale `0.01` is treated by absolute value
as distance from a river centerline. Close to zero, the height-routing splines
permit only ocean/plain recipes; progressively farther away they admit hills,
extreme hills, and finally extreme mountains. The same field is repurposed in
the biome router to select river or frozen-river biomes.

This produces broad valley corridors rather than painting water onto arbitrary
relief. V0.8's changelog describes the same change from the product side:
rivers have their own paths, avoid ocean-edge generation, and influence slope
generation. It is not a drainage simulation, but the single shared influence
keeps terrain, biome, and water placement coherent.

### Treat normalized height as an explicit coordinate

The base surface density is a linear vertical gradient plus a two-dimensional
height value `h`:

```text
G(y) = 1 - (y + 64) / 1048
D_surface(y) = G(y) + h
unprocessed surface boundary: y = 984 + 1048 h
```

Several apparently peculiar spline constants are exact inverse mappings from
desired block altitude into `h`. The biome `continentalness` field remaps that
raw height coordinate:

| Raw `h` | Implied unprocessed Y | Biome continentalness |
|---:|---:|---:|
| -0.885 | about 57 | -0.19, ocean/shore edge |
| -0.875 | 67 | -0.11, shore/land edge |
| 0.01526717557 | 1,000 | 0.8 |
| 0.2061068702 | 1,200 | 1.0 |
| 0.3969465649 | 1,400 | 1.2 |
| 0.5877862595 | 1,600 | 1.4 |
| 1.0 | 2,032 | 2.0 |

The multi-noise table then places groves, snowy slopes, ordinary peaks, and
three molten-peak variants into successive continentalness bands up to 2.0.
Biome classification is derived from the field that made the terrain, so a
"high peak" biome has a geometric reason to be high.

For mclone, the reusable idea is not to call altitude continentalness. It is
to preserve a named normalized height/relief coordinate, publish its conversion
to block Y, and use the same production value for terrain, ecological
classification, screenshots, and distribution receipts.

### Approximate erosion from a noise gradient

The v0.5+ "fractured" mountain look is a compact pseudo-erosion operator:

1. sample a broad 2D `gravel` noise `n(x,z)`;
2. use `shifted_noise` to sample one-block X and Z offsets;
3. compute finite differences `dx=n(x+1,z)-n(x,z)` and
   `dz=n(x,z+1)-n(x,z)`;
4. approximate `sqrt(dx²+dz²)` with a spline table;
5. approximate reciprocal attenuation, visibly the table `1/(1+x)`, with
   another spline; and
6. multiply the original noise by that attenuation before adding conditional
   high-frequency jaggedness.

The files call the gradient magnitude "divergence", but it is not mathematical
divergence. It is a finite-difference slope magnitude. In several families the
attenuation applies asymmetrically, preserving low portions of the field while
cutting or fracturing positive relief. Jagged-noise amplitude also grows across
the broad factor, giving lower flanks and upper ridges different texture.

This is much cheaper and more controllable than hydraulic erosion. A native
mclone experiment should implement the gradient norm and attenuation directly,
not reproduce JSON sqrt/reciprocal lookup tables.

### Keep raw and processed height fields separate

V0.8 introduces a `hollowed` transform above raw `h=0.8`. The processed surface
adds `h * k(h)`, where `k` transitions from 0 at `h=0.8` to -1.5 at `h=2`.
This suppresses or reverses the most extreme raw peaks and participates in the
volcanic/high-peak update.

Crucially, biome altitude and the Underlands still use preserved base fields.
The pack can transform a peak into a cap, crater, or hollow while retaining
knowledge that the region belongs to the extreme-mountain family. The broader
lesson is valuable: do not overwrite macro intent with final geometry when
later biome, cave, landmark, or review rules still need the pre-transform
signal.

The current
[Warm Area gallery specimen](https://cdn.modrinth.com/data/1NnAVIR5/images/4fa0c433af17791f7b358e4e46cbfba9f284a2ce.jpeg)
visibly supports the broad-family claim: a large traversable plain leads into
a coherent range with long ridges and fractured peaks rather than uniformly
amplified noise. This report did not run a full 26.2 client world, so the image
is visual corroboration rather than an independent runtime oracle.

## Cave Hierarchy And The Underlands

### Density caves replace carvers

All three configured carvers—cave, extra-underground cave, and canyon—have
`probability: 0`. Biomes still name them, but they do no work. The visible
caves come entirely from the final-density graph.

The base cave coordinate combines the same full-height Y gradient with a
low-frequency 2D warp. Three alternating spline bands create nested scales:

| Family | Spline point spacing | Approx. full vertical repeat | Added 3D noise scale XZ / Y |
|---|---:|---:|---:|
| small | 0.025 | 52 blocks | 10 / 4 |
| medium | 0.05 | 105 blocks | 5 / 2 |
| large | 0.1 | 210 blocks | 2.5 / 1 |

The values alternate between solid and void, while 3D `cave_layer` noise and
large-scale jagged noise break the bands into irregular chambers. Separate
barrier noises vary whether a region uses simple isotropic cave noise or the
new warped-band family and interrupt otherwise continuous voids.

### Cave scale is relative to the local surface

The most reusable cave idea is the routing coordinate. The pack uses positive
surface density as an approximate measure of overburden. At the linear
gradient's scale, one density unit is about 1,048 blocks. Spline checkpoints
near `0.1`, `0.35`, `0.7`, and `1.0` therefore move through roughly:

```text
~105 blocks below local surface  -> small caves
~367 blocks below local surface  -> medium caves
~734 blocks below local surface  -> large caves
~1048 blocks below local surface -> Underlands influence
```

Noise and spline blending make these transitions broad rather than exact
roofs. The important contract is relative depth, not the literal numbers. A
cave 300 blocks below a 1,600-block summit can be classified as shallow even
though its absolute Y is far above sea level. This lets cave scale grow with
available mountain mass.

### The Underlands is a mountain-conditioned void sheet

The Underlands is not a second dimension. Its live v0.8 density is:

```text
U(y,h) = gradient(y: -256 -> 1016, value: 1 -> -1) - 1.6 h
```

Within the unclamped interval, its zero crossing is approximately:

```text
y = 380 - 1017.6 h
```

Here `h` is the rivered base height before v0.8 peak hollowing. Raising raw
mountain intent lowers the Underlands density until a large connected void can
win deep inside the terrain. In representative terms:

| Raw `h` | Unprocessed surface Y | Underlands zero Y |
|---:|---:|---:|
| -0.8, plain-like | about 146 | about 1,194, above the terrain: no deep realm |
| 0.2061 | 1,200 | about 170 |
| 0.3969 | 1,400 | about -24 |
| 0.5878 | 1,600 | below the world floor |

These are equation landmarks, not exact cavern bounds; depth routing,
barriers, hollowing, and the final spline blend determine the actual walls.
They do show the intent cleanly. Lowlands do not lose their underground merely
so mountains can be tall. Instead, sufficient local relief unlocks progressively
larger cave families and eventually a continuous interior realm.

The final composition takes the minimum of the surface/cave candidates. In
density-field terms this is a union of empty regions: if either cave system
wants air, it can carve the solid terrain. There is no global volume or height
budget to balance.

## Underground As A Full World Layer

The pack makes the cave geometry, biome system, materials, features, and
structures agree rather than stopping at a giant empty cavity:

- biome `depth` is derived from surface density, so `underground` occupies
  relative depth `0.02..1` and `underground_deep` occupies `1..2`;
- the final-density and river fields are repurposed through the biome
  `ridges` channel to separate surface, river, and underground selections;
- lush, dripstone, sulfur, and deep-dark cave biomes share relative-depth
  ranges with climate/erosion distinctions;
- surface rules replace ordinary underground material with stone, deep zones
  with deepslate, sulfur caves with 3D sulfur/cinnabar bands, and molten peaks
  with deepslate/tuff/magma palettes;
- mineshafts target the generic underground biome, strongholds and trial
  chambers target underground/deep, and ancient cities remain tied to deep
  dark with a much wider 64-chunk spacing; and
- cave vegetation and decoration placement ranges are expanded through the
  2,096-block physical height.

The ore overrides also turn the huge vertical span into progression rather
than empty stone:

| Material | V0.8 placement idea |
|---|---|
| diorite | Y 0..400 |
| andesite | Y 400..800 |
| granite | Y 800..1200 |
| emerald | 100 attempts, Y 1300..2031 |
| iron | 64 attempts across -64..2031, trapezoid |
| copper | 256 large-vein attempts across -64..2031, trapezoid |
| coal | 128 attempts across -64..2031 |

The exact counts are tuned for an enormous Minecraft datapack and should not
be imported. The interesting idea is altitude-stratified geology: the climb
and the descent expose different material languages.

Aquifers remain nominally enabled, but the router's barrier, floodedness,
spread, and lava fields are all constant `-1`. The pack is not a good reference
for rich local water-table composition; its strength is dry density geometry
and biome/material layering.

## Performance And Product Costs

The height increase is a major product choice, not a free parameter:

- a chunk has 131 possible vertical sections versus current vanilla's 24;
- the pack uses `size_vertical: 1`, a 4-block interpolation cell, while current
  vanilla uses `size_vertical: 2`, an 8-block cell;
- that is 524 vertical density cells versus 48, about **10.9 times** as many
  per XZ lattice column before accounting for the more complex function graph;
- lighting, biome samples, features, structures, serialization, networking,
  client section residency, meshing, culling, and future distant-terrain
  presentation all inherit some of the expanded coordinate range; and
- empty-section compression helps storage but does not remove density sampling
  or worst-case tall-mountain costs.

The project's own distribution page warns that it needs a strong computer,
more memory, and optimization mods. Mclone's chunk snapshot format already
records variable `min_y` and `height`, but the live built-in profiles,
interaction cap, spawn search, generation tables, and several gameplay
assumptions still target the 1.17-style 0..255 range. A 2,096-block profile
would be a broad dimension-contract and performance campaign, not merely a new
worldgen formula.

## Lessons For Mclone

### Strong candidates

1. **Specialized landform recipes behind macro fields.** This directly
   reinforces the current mountains/valleys plan: selectors blend concrete
   ocean, lowland, hill, and mountain rules rather than globally amplifying
   relief.
2. **Direct finite-difference pseudo-erosion.** Gradient attenuation is a
   bounded experiment for Tactical 192, provided it is measured against ridge
   maps, slope distributions, and multiple landscape cards.
3. **Raw versus processed terrain intent.** Preserve pre-erosion/pre-hollowing
   relief when biomes, landmarks, caves, and review tooling need semantic
   knowledge that final block height cannot recover.
4. **A river influence consumed by terrain and biome selection.** Keep this
   for the later river slice; it must not be smuggled into the current bounded
   mountain tactical.
5. **Relative overburden as a cave coordinate.** Cave size and biome should be
   able to respond to distance below local terrain, not only absolute Y.
6. **A same-dimension underworld conditioned by macro relief.** A small mclone
   proof could place a broad, connected deep cavern only beneath major ranges
   while retaining ordinary caves elsewhere.
7. **Altitude-stratified geology and landmark rewards.** Tall places and deep
   places need distinct material/content reasons to visit, not only scenery.

### Do not inherit directly

- the 2,096-block height: the cave hierarchy works at smaller scale and does
  not require this performance multiplier;
- the `minecraft:*` registry-replacement/load-order model;
- 96 currently unreachable density definitions and large repeated spline
  tables;
- pseudo-erosion naming that confuses a gradient norm with divergence;
- hardcoded sqrt and reciprocal approximations when native code can express
  and test the intended math directly;
- an expanded physical height with a vanilla logical height left unexplained;
- constant aquifer inputs as a substitute for an eventual water-table design;
  or
- feature attempt counts scaled for a world more than five times vanilla's
  height.

## Position Beside Alpha And Beta

| Question | Alpha v1.1.2_01 | Beta 1.7.3 | JJThunder v0.8 |
|---|---|---|---|
| Primary value | smallest pre-biome core | mature old-Beta climate terrain | modern community composition ideas |
| Active height | 128 | 128 | 2,096 |
| Terrain core | one 5x17x5 density lattice | same skeleton, climate-modulated | routed 2D height families plus density caves |
| Caves | neighbor-seeded tunnel carver | corrected sibling carver | relative-depth density hierarchy |
| Biome relation | none | climate influences geometry and surfaces | terrain height/depth fields directly classify biomes |
| Underworld idea | none in selected Overworld | separate Nether/Sky generators exist | same-Overworld mega-cave under high relief |
| Port posture | close historical translation | close historical translation | extract concepts, never rule-table parity |

Alpha and Beta are compact historical systems that teach exact generator
anatomy. JJThunder is almost the opposite: a large declarative experiment that
shows what can be composed once height, terrain intent, cave depth, and biome
channels are treated as reusable fields. Its best contribution to mclone is
not the headline scale. It is the coordination among independently inspectable
signals.

## Recommended Bounded Experiments

1. During Tactical 192, compare an ordinary ridge/factor field with a direct
   gradient-attenuated variant. Commit field distributions before choosing the
   visual result; do not call it erosion without qualifying it as a heuristic.
2. In the later river slice, make one inspectable river-influence field affect
   valley height and biome/wetland classification before adding river blocks.
3. In the later cave slice, introduce a normalized overburden field and route
   two cave scales from it. A third, rare continuous void under high-relief
   regions can test the Underlands idea inside the existing 256-block world.
4. Consider greater dimension height only after that scaled proof. Audit
   generation, spawn, interaction caps, lighting, persistence, network payload,
   renderer section residency, XR culling, future distant terrain, and
   logical-height gameplay as one explicit platform-neutral campaign.

The research does not change the current tactical boundary: mountains and
valleys remain next; rivers, caves, geology, and structures remain later
families.

## Related

- [`alpha-era-reference.md`](alpha-era-reference.md)
- [`beta-1.7.3-reference.md`](beta-1.7.3-reference.md)
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`../tactical/192-mclone-overworld-mountains-and-valleys.md`](../tactical/192-mclone-overworld-mountains-and-valleys.md)
