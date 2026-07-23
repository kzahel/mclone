# Mclone Overworld Breadth

Topic: `mclone-overworld-breadth`

Status: active 2026-07-23. This is the original Mclone Overworld breadth
ledger: vanilla Minecraft 1.17.1 supplies a measured reference vocabulary,
while Mclone owns its regional recipes, distribution, terrain geometry, and
visual identity. Tactical
[`223`](../tactical/223-mclone-climate-and-bookend-biomes.md) has completed
the first climate-driven implementation and is awaiting Human Review 1.

## Scope

This topic answers three recurring questions:

1. What visual and gameplay breadth does vanilla demonstrate?
2. Which of those mechanisms already exist in the shared engine, and which are
   actually used by `mclone-overworld-v1`?
3. Which original Mclone region, geology, water, ecology, and landmark
   families are live, active, or still missing?

The durable terrain architecture remains in
[`mclone-overworld-generation.md`](mclone-overworld-generation.md). Exact Java
1.17.1 biome parity remains in
[`../tactical/135-overworld-biome-palette-matrix.md`](../tactical/135-overworld-biome-palette-matrix.md).
This ledger is deliberately neither a parity checklist nor a requirement to
copy all vanilla biome IDs.

## Accounting Rule

Every row distinguishes four states:

- `reference`: vanilla or another studied source demonstrates the idea;
- `mechanism`: shared Rust can express the blocks, feature, renderer, or
  structure shape;
- `live`: `mclone-overworld-v1` selects and generates the family in production;
- `reviewed`: deterministic evidence and human visual review accepted it.

A shared vanilla feature implementation does not make the corresponding
Mclone family live. This distinction matters today: the engine can generate
many tree, plant, ice, ocean, ore, and surface families that the original
profile never selects.

## Measured Reference Baseline

The Java 1.17.1 palette matrix contains 68 registered Overworld rows and 66
rows emitted by the default layered biome source. Many are hills, shores,
plateaus, or modified variants of a smaller visual family. Mclone should use
that breadth as a prompt, not chase 66 one-to-one copies.

Useful grouped reference families are:

- plains, sunflower plains, flower forest, birch forest, and dark forest;
- taiga, snowy taiga, giant-tree taiga, and giant-spruce taiga;
- savanna, plateau, shattered savanna, desert, and desert lakes;
- badlands, wooded badlands, plateau, modified, and eroded variants;
- jungle, jungle edge, modified jungle, and bamboo jungle;
- swamp, swamp hills, mushroom fields, and mushroom shore;
- tundra, snowy mountains, ice spikes, frozen rivers, and frozen oceans;
- ordinary, warm, lukewarm, cold, frozen, and deep oceans;
- beaches, stone shores, rivers, lakes, springs, caves, ravines, and the
  structure families associated with those regions.

The Mclone target can obtain comparable or greater perceived breadth from
roughly 12-18 strong regional recipes plus meaningful landform, water,
geology, vegetation, and landmark variation inside them.

## Current Original-Profile Baseline

Field revision 13 emits eight vanilla-compatible biome IDs:

| ID | Current Mclone meaning | Decoration |
|---:|---|---|
| `0` | ocean | none |
| `16` | beach / low shore | none |
| `1` | open lowland, mountain valley/shoulder, and wetland land | oak, grass, dandelion, poppy |
| `4` | sheltered wooded upland | denser oak, grass, dandelion, poppy |
| `7` | major river and planned stream water | none |
| `5` | cool-wet conifer country | spruce/pine, ferns, berries |
| `13` | cold alpine highland | snow/rock, treeless initially |
| `35` | warm-dry steppe | sparse acacia, tall grass, restrained flowers |

The terrain is already substantially broader than that biome list: continents,
shelves and deep basins, beaches, open lowlands, wooded uplands, rugged
mountains, valleys, exposed stone, major rivers, wetlands, and the reviewed
spring-fed creek all exist. The narrowness is regional climate, surface
palette, vegetation, ecology, geology, and generated landmark breadth.

Eight surface recipes are live: ocean floor, beach, river bed, wetland bed,
river bank, grass/soil, exposed stone, and alpine snow. Four land decoration
tables are live: temperate meadow, temperate woodland, cool-wet conifer, and
warm-dry steppe. Alpine is deliberately undecorated in its first pass.

Temperature and moisture are now independent broad periodic fields with
smaller cross-warp detail. Altitude-adjusted temperature is derived during
regional classification. These climate facts alter biome, surface, and
decoration language but do not change terrain geometry.

## Regional Recipe Ledger

| Mclone family | Vanilla breadth prompts | Terrain / climate | Surface and vegetation | Status |
|---|---|---|---|---|
| temperate meadow and pastoral valley | plains, flower forest | existing lowland/valley geometry | grass, oak accents, grass, two flowers | `reviewed`, narrow palette |
| temperate wooded upland | forest, wooded hills | sheltered moderate upland | oak woodland | `reviewed`, narrow palette |
| rugged mountain and open shoulder | mountains, gravelly mountains, shattered savanna | ridges, ruggedness, slope, exposure | grass/stone response | `reviewed`, no alpine climate |
| ocean, shelf, and deep basin | ocean and deep-ocean families | bathymetry and coast | gravel/sand, no aquatic vegetation | `reviewed`, one climate |
| beach and shore | beach, stone shore, snowy beach | sea-level band | sand only | `live`, one shore family |
| major river and wetland | rivers, swamps, lakes | flat Y63 corridor, banks, pools | gravel, sand/grass banks, clay pools | `reviewed`, no climate variants |
| peaceful spring-fed creek | springs and bounded water landmarks | 91-96-block planned valley stream | grassed shoulders, fixed-point drops | `reviewed` |
| cool-wet conifer upland | taiga, taiga hills | temperature + moisture + landform | spruce/pine, ferns, berries | `live`, Human Review 1 pending |
| snowy alpine | snowy mountains, tundra | altitude-adjusted cold rugged terrain | snow, exposed rock, no trees initially | `live`, Human Review 1 pending |
| warm-dry steppe | savanna and plateau | warm + dry open terrain | golden grass, tall grass, sparse acacia | `live`, Human Review 1 pending |
| flower meadow and birch grove | sunflower plains, flower/birch forest | temperate local selectors | broader flowers, birch stands | `planned` |
| deep/dark or ancient forest | dark forest, giant taiga | humid sheltered terrain | dark oak or giant conifer, fungi, boulders | `planned` |
| arid desert and dune field | desert and desert lakes | hot + very dry low relief | sand/sandstone, cactus, dead bush, oasis | `planned` |
| badlands and hoodoo country | badlands family | hot + dry eroded relief | layered terracotta, red sand, hoodoos | `planned` |
| humid jungle and bamboo country | jungle family | hot + wet relief | jungle trees, bamboo, vines, understory | `planned` |
| swamp and marsh forest | swamp family | warm + wet lowland/water | dark water, orchids, lilies, wet trees | `planned` |
| rare fungal or uncanny region | mushroom fields | rare isolated selector | mycelium, mushrooms, unusual ambience | `planned` |
| warm reef ocean | warm/lukewarm oceans | warm shallow shelf | coral, seagrass, pickles, bright water | `planned` |
| cold and frozen ocean | cold/frozen oceans | cold shelf and basin | kelp, ice, icebergs, blue ice | `planned` |

This table tracks families, not a promise that each row maps to exactly one
persisted biome ID. Climate, landform, surface recipe, ecology, and landmark
selectors may combine without multiplying nominal biome keys unnecessarily.
Biome identity remains explicit wherever tint, ambience, spawning, protocol,
or persistence needs it.

## Breadth Beyond Biome Names

| Dimension | Current Mclone | Important missing families |
|---|---|---|
| climate | periodic temperature/moisture, altitude snowline, temperate/conifer/alpine/steppe response | regional water climate, more hot-wet and hot-dry extremes |
| terrain | continents, coasts, lowlands, mountains, valleys | plateaus, dunes, mesas, escarpments, volcanic terrain, high basins |
| surfaces | grass, dirt, sand, gravel, clay, stone | snow/ice, podzol/coarse dirt, terracotta/red sand, fungal and richer rocky palettes |
| vegetation | oak, grass, dandelion, poppy | every other tree family, undergrowth, aquatic plants, desert flora, fungi |
| water | ocean depth, Y63 rivers, wetlands, one creek family | lakes, climate variants, dramatic cascades/gorges/falls, reefs, frozen water |
| geology | stone mass and exposed faces | rock types, strata, ores, boulders, scree, volumetric outcrops, arches, caves |
| landmarks | bounded stream start/pieces | natural rock landmarks, ruins, bridges, towers, monuments, settlements |
| ecology | debug/passive demonstrations | biome-owned natural spawn tables, ambient life, predators, aquatic life |
| ambience | common sky/fog/audio | climate weather, regional fog/sky/water color, particles, biome sound |

## Three-Dimensional Geology And Rock Formations

### Current truth

Mclone terrain is currently a two-dimensional surface-height field filled
solid beneath `surface_y`. It can produce steep faces but cannot produce a
true overhang, arch, suspended shelf, natural window, sea cave, or undercut
outcrop. The creek's water stencils and authored structures place 3D blocks,
but they do not make the regional terrain density volumetric.

Small surface boulders are a different mechanism. The shared feature system
already contains a Java-shaped block-blob implementation, but Mclone does not
select it. A few blobs would add dressing, not solve volumetric geology.

### Target formation vocabulary

Rock work should receive sustained visual iteration across several scales:

- individual boulders, split rocks, mossy rocks, and clustered erratics;
- scree fields, talus fans, exposed strata, and broken cliff toes;
- tors, fins, needles, hoodoos, balanced rocks, and sea stacks;
- cliff shelves, undercuts, shallow rock shelters, and recessed alcoves;
- natural arches, windows, bridges, and perforated ridges;
- dramatic canyon walls and waterfall-cut notches;
- region-specific materials such as granite-like massifs, layered badlands,
  pale coastal stacks, dark volcanic columns, and alpine fractured stone.

### Mechanism classes

Do not force every formation through one global 3D-noise soup.

1. **Placed rock features**
   - bounded blobs, boulder clusters, scree, and small exposed formations;
   - cheap site predicates and ordinary feature clipping;
   - useful dressing, but not a substitute for terrain silhouette work.
2. **Structure-shaped volumetric landmarks**
   - deterministic starts, bounded pieces, signed-distance or constructive
     solid/void shapes, and optional 3D noise modulation;
   - appropriate for arches, tors, hoodoos, sea stacks, and signature outcrops;
   - the Tactical 222 start/reference kernel is the natural metadata base.
3. **Regional density modifiers**
   - pure `(x,y,z)` density added to or subtracted from selected landform
     regions before surface treatment;
   - appropriate for repeated cliff shelves, overhang bands, undercut ridges,
     and more pervasive volumetric mountain character.
4. **Subsurface subtractive systems**
   - caves, ravines, tubes, chambers, and later geology;
   - related mathematically, but independently seeded and reviewed so cave
     prevalence does not dictate surface-rock character.

### Vanilla lessons and deliberate divergence

Java 1.17.1's `BlendedNoise` proves the established coarse-lattice,
interpolated three-dimensional density model. `BlockBlobFeature`, ice spikes,
basalt columns/pillars, fossils, and geodes demonstrate bounded feature
vocabularies. Mclone should reuse those lessons and existing shared
mechanisms, while owning the distribution and formation recipes.

This does not authorize porting the disabled Caves & Cliffs Part 1
`Aquifer`, `Cavifier`, `NoodleCavifier`, or ore-vein paths into the
reference-locked Overworld. A later Mclone-only density tactical should be an
explicit original-profile system.

### Performance and LOD contract

Three-dimensional geology must not make every ordinary chunk pay for dense
voxel noise:

- run a cheap two-dimensional macro selector first;
- bound every landmark or regional influence in X/Y/Z;
- sample coarse 3D lattices and interpolate rather than evaluating many
  octaves independently for every block;
- skip unaffected sections and preserve exact target partition/order output;
- expose silhouette/material facts to synthetic far LOD;
- benchmark no-formation controls and dense formation hotspots separately;
- retain exact plane and 384-chunk-X periodic behavior; and
- stop for pixel review at boulder, outcrop, arch, and regional-density
  milestones rather than tuning all shapes in one pass.

## Recommended Sequence

1. Complete Human Review 1 for Tactical 223's climate fields and first three
   regional recipes; apply only bounded art-direction corrections.
2. Add the first 3D-geology tactical before broad caves:
   - baseline and map the current heightfield limitation;
   - prove one placed boulder/talus family;
   - prove one bounded signature outcrop such as a tor or arch;
   - compare a small regional density modifier for cliff shelves/overhangs;
   - select the reusable mechanism only after all three are rendered and
     measured.
3. Expand hot/dry and wet/humid regional corners.
4. Add ocean-climate families and climate-aware water treatment.
5. Add independent caves, strata, ores, and underground landmarks.
6. Grow authored and procedural structures on the resulting regional
   vocabulary.

## Evidence Required Per Family

Every live family should eventually record:

- selector coverage and boundary ratios on several seeds;
- deterministic field, biome, surface, and feature maps;
- exact output fingerprints and partition/order/periodic seams;
- representative surface, vegetation, water, and landmark block counts;
- far-LOD agreement for terrain-scale facts;
- cold/warm generation cost and a movement soak when work is substantial;
- fully warmed high-view-distance pixels; and
- the human description worth preserving, including both peaceful and
  deliberately dramatic visual families.

## Related

- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md)
- [`../worldgen-status.md`](../worldgen-status.md)
- [`../structures.md`](../structures.md)
- [`../tactical/135-overworld-biome-palette-matrix.md`](../tactical/135-overworld-biome-palette-matrix.md)
- [`../tactical/146-overworld-macro-terrain-geometry-parity.md`](../tactical/146-overworld-macro-terrain-geometry-parity.md)
- [`../tactical/192-mclone-overworld-mountains-and-valleys.md`](../tactical/192-mclone-overworld-mountains-and-valleys.md)
- [`../tactical/222-bounded-valley-stream-structures.md`](../tactical/222-bounded-valley-stream-structures.md)
