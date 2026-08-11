# Mclone Overworld Breadth

Topic: `mclone-overworld-breadth`

Status: active 2026-07-27. This is the original Mclone Overworld breadth
ledger: vanilla Minecraft 1.17.1 supplies a measured reference vocabulary,
while Mclone owns its regional recipes, distribution, terrain geometry, and
visual identity. Tactical
[`223`](../tactical/223-mclone-climate-and-bookend-biomes.md) has completed
the first climate-driven implementation. Human Review 1 accepted its
vocabulary but requested a broader steppe; the core/shoulder correction is
complete and awaiting Human Review 2. Tactical 259 has since completed the
cross-era coast survey and selected the first original-profile coast
vocabulary. Tactical 260 completed that candidate and corrected the first
review's hard material boundaries, sand-only snow response, and repeated
water-edge collars. Human Review 2 found it substantially improved but
deferred final coast acceptance until stronger inland terrain can reach the
water. Tactical 263 now owns the cross-era inland diagnosis and next landform
plan. Tactical 264 has implemented the first ordinary inland fabric and
passed its objective gates. Human Review A found it improved but still
structurally scalar and disconnected from water. Tactical 265 selects a
bounded hybrid landform planner, and Tactical 267 now implements the fixed
research-only comparison. It is paused at Human Review B before new breadth
families or production integration. Tactical 277 separately makes the current
cow/chicken natural-spawn habitat consume Mclone's generated biome payload;
the richer original ecology vocabulary remains open.

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
copy all vanilla biome IDs. Cross-system sequencing and the composition of
terrain, water, geology, ecology, routes, landmarks, structures, and negative
space live in
[`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md).

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

Field revision 21 emits eight vanilla-compatible biome IDs:

| ID | Current Mclone meaning | Decoration |
|---:|---|---|
| `0` | ocean | none |
| `16` | beach / low shore | none |
| `1` | open lowland, mountain valley/shoulder, and wetland land | oak, grass, dandelion, poppy |
| `4` | sheltered wooded upland | denser oak, grass, dandelion, poppy |
| `7` | major river and planned stream water | none |
| `5` | cool-wet conifer country | spruce/pine, ferns, berries |
| `13` | cold alpine highland | snow/rock, treeless initially |
| `35` | warm-dry steppe core and regional shoulder | core acacia/tall grass, sparser shoulder |

The terrain has more mechanisms than the biome list: continents, shelves and
deep basins, coast families, lowlands, wooded uplands, a detailed mountain
family, valleys, exposed stone, major rivers, wetlands, and the reviewed
spring-fed creek all exist. Tactical 263 corrected the prior breadth
assessment: those names did not imply a well-distributed inland vocabulary.
Tactical 264 now makes continuous quiet, rolling, ridge/valley, broad-basin,
and mountain intent live without adding another noise field. Across the same
nine ordinary-land windows, median vertical span is now 25 blocks and
radius-8 detrended roughness is 1.139 blocks. Objective validation passes.
Human Review A accepts the increase as an improvement but finds that the
distant terrain still reads as similarly scaled scalar hills, with repeated
closed contour rings and water features that lack shared basin/outlet
authority. Tactical 265 therefore treats revision 21 as the production
control for a research-only hybrid planner, not as final ordinary-land
fabric. Tactical 267 now supplies that comparison: its fixed corpus has
explicit drainage, divergent divides, protected basins/spills, quiet
counterform, coast arrivals, and indexed reconstruction, while production
biomes and terrain remain revision 21. Internal inspection sees a material
structural improvement but retains raster-linearity, smooth-profile, limited
grammar, and local-realization concerns for Human Review B.

Twelve surface recipes are live: ocean floor, sandy coast, gravel coast,
rocky coast, snow cover, river bed, wetland bed, river bank, grass/soil,
eroded slope, alpine snow, and exposed stone. Four land decoration tables are
live: temperate meadow, temperate woodland, cool-wet conifer, and warm-dry
steppe. Alpine is deliberately undecorated in its first pass.

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
| rolling upland, ridge/valley, and broad basin fabric | Alpha/Beta continuous terrain, 1.17 hills and plateaus, modern terrain slices | live continuous landform intent from existing periodic fields; selected future ridge/divide, drainage, and basin plan | existing regions interpret revision 21; no new ecology was added | `live` control, Human Review A requests structural successor, Tacticals 264-265 |
| ocean, shelf, and deep basin | ocean and deep-ocean families | bathymetry and coast | gravel/sand, no aquatic vegetation | `reviewed`, one climate |
| beach and shore | beach, stone shore, snowy beach | shared coast intent plus bounded depositional/rocky geometry | locally mixed sand, gravel, ordinary soil, grass-topped rock, cross-substrate snow, and varied banks | `live`, substantially improved; final balance deferred until inland landforms reach water |
| major river and wetland | rivers, swamps, lakes | flat Y63 corridor, banks, pools | gravel, sand/grass banks, clay pools | `reviewed`, no climate variants |
| peaceful spring-fed creek | springs and bounded water landmarks | 91-96-block planned valley stream | grassed shoulders, fixed-point drops | `reviewed` |
| cool-wet conifer upland | taiga, taiga hills | temperature + moisture + landform | spruce/pine, ferns, berries | `live`, vocabulary accepted in Human Review 1 |
| snowy alpine | snowy mountains, tundra | altitude-adjusted cold rugged terrain | snow, exposed rock, no trees initially | `live`, vocabulary accepted in Human Review 1 |
| warm-dry steppe | savanna and plateau | warm/dry suitability core plus shoulder | golden grass, core acacia/tall grass, sparser shoulder | `live`, scale correction awaiting Human Review 2 |
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
| terrain | continents, coasts, broad lowlands, continuous quiet/rolling/ridge-valley/basin intent, a detailed mountain family, connected lows, and local valleys | accepted balance for the new ordinary fabric, plus plateaus, dunes, mesas, escarpments, volcanic terrain, high basins, and planned passes/corridors |
| surfaces | grass, dirt, sand, gravel, clay, stone | snow/ice, podzol/coarse dirt, terracotta/red sand, fungal and richer rocky palettes |
| vegetation | oak, grass, dandelion, poppy, and sparse wetland lily-pad/reed cover | every other tree family, richer undergrowth and aquatic plants, desert flora, fungi |
| water | ocean depth, Y63 rivers, wetlands, one creek family | lakes, climate variants, dramatic cascades/gorges/falls, reefs, frozen water |
| geology | stone mass and exposed faces | rock types, strata, ores, boulders, scree, volumetric outcrops, arches, caves |
| landmarks | bounded stream start/pieces | natural rock landmarks, ruins, bridges, towers, monuments, settlements |
| ecology | authored residents; durable cow/chicken biome spawning; Mclone-only block-derived 2-4 mallard flocks, shore preference, and collectible wetland eggs | broader habitat fitness, cohesion/swimming, nests/breeding, ambient life, predators, and aquatic life |
| ambience | common sky/fog/audio | climate weather, regional fog/sky/water color, particles, biome sound |

## First-Party Idea Garden

This is a deliberately low-commitment inbox for original Mclone world ideas.
An entry records a feeling or possibility worth preserving; it is not an
accepted design, promised feature, priority change, or implementation
tactical. Promote an idea into the regional ledger, generation topic,
Structure Lab, or its own focused topic only after it gains a concrete owner
and review direction.

### Coastal traces and small finds

- Seashells could gather in sparse clusters along sandy beaches, especially
  near strand lines, sheltered coves, and storm-tossed patches. Their first
  purpose is to make the boundary between sea and land feel inhabited;
  collectibility, crafting, and respawn behavior can remain separate ideas.

### Paths, trails, and traces

- Human paths could include old dirt roads, desire lines, switchbacks,
  bridges, milestones, and routes connecting settlements, resources, and
  ruins.
- Animal paths could include narrow deer trails between cover, water, grazing
  areas, salt or mineral sites, and favored ridge crossings. They should be
  subtler, more meandering, and less completely connected than human roads.
- Different histories should leave different marks: hoof-worn earth, a path
  widened by carts, a road fading into grass, or a surviving bridge whose
  destination has disappeared.

### Ruined fortifications

- Broken-down old castles could appear as incomplete curtain walls, isolated
  gatehouses, collapsed towers, overgrown keeps, half-buried foundations, and
  fragments incorporated into later paths or settlements.
- A ruin should not need to materialize as one pristine template with a
  random damage pass. Site plan, surviving pieces, collapse, erosion,
  vegetation, and later reuse can be separate deterministic layers.

### Layered landscape history

The motivating visual is compelling because it shows more than a castle
placed on terrain. A circulation network follows the slopes, crosses gates,
and connects partial walls and enclosures, making the whole landscape imply a
past use.

Preserve that as a first-party design principle: generated landmarks should
invite movement and suggest what may have happened there. Paths and ruins are
therefore especially interesting as related regional systems rather than
isolated decorations. A future recipe might layer natural terrain, animal
movement traces, human routes, built landmarks, collapse, vegetation, and
reuse while keeping each result traversable and legible at block scale.

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

The shared workload model, recorded preview baselines, selective-density
direction, 500 km-class projection, distant-summary rules, and benchmark
vocabulary live in
[`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md#volumetric-terrain-and-scale-aware-sampling).
For geology specifically, retain these obligations:

- run a cheap two-dimensional macro selector first;
- bound every landmark or regional influence in X/Y/Z;
- sample coarse 3D lattices and interpolate rather than evaluating many
  octaves independently for every block;
- skip unaffected sections and preserve exact target partition/order output;
- expose silhouette, opening, material, and coverage summaries to the shared
  distant terrain hierarchy;
- benchmark no-formation controls and dense formation hotspots separately;
- retain exact plane and 384-chunk-X periodic behavior; and
- stop for pixel review at boulder, outcrop, arch, and regional-density
  milestones rather than tuning all shapes in one pass.

## Breadth Expansion Sequence

Cross-system order now lives in
[`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md).
That direction now places ordinary inland landform fabric between the first
coast implementation and major-water/geology campaigns. Tactical 263 grounds
the correction; Tactical 264 implements the first scalar candidate; Tactical
265 selects the bounded hybrid successor without requiring a generic
composite review product. Within the breadth ledger, retain this content
order:

1. Complete Human Review 2 for Tactical 223's bounded steppe-scale
   correction.
2. Preserve the completed first topology-aware coast intent:
   - Tactical 260 proves sandy depositional, gravel transitional,
     rocky/exposed, cold response, and ordinary direct-water outcomes;
   - geometry remains separate from surface material and rivers retain
     authority;
   - Human Review 2 found the correction substantially improved but not final;
     and
   - defer further coast-only tuning until inland ridges, valleys, hills, and
     later escarpments can reach the water.
3. Establish ordinary inland landform fabric:
   - Tactical 264 makes continuous quiet, rolling, ridge/valley, basin, and
     existing mountain intent live from the current periodic fields;
   - connected 384/128/48-block form and selective 32/8-block detail now feed
     ordinary land and reach the coast;
   - quiet coverage, retained water authority, exact topology, persistence,
     streaming completeness, and cheap previews pass objective validation;
   - Human Review A finds it improved but rejects repeated scalar hills and
     disconnected terrain/water planning as final; and
   - Tactical 265 selects the hybrid plan grammar and Human Review B gate.
4. Prototype regional envelopes, ridge/divide and drainage skeletons, basins,
   and quiet counterform outside production:
   - Tactical 267 compares revision 21, graph reconstruction, and
     reconstruction with subordinate local detail on the same plane/cylinder
     seeds;
   - cold plan, five-iteration cached query, memory, far-summary, topology,
     sink, spill, cycle, contour, coast-arrival, and journey evidence is
     complete; and
   - the work is stopped at Human Review B before production integration.
5. Add the first 3D-geology tactical before broad caves:
   - baseline and map the current heightfield limitation;
   - derive a small semantic formation-intent selector from existing macro,
     climate, ruggedness, ridge, and water facts;
   - prove one placed boulder/talus family;
   - prove one bounded signature outcrop such as a tor or arch;
   - compare a small regional density modifier for cliff shelves/overhangs;
   - select the reusable mechanism only after all three are rendered and
     measured.
6. Expand hot/dry and wet/humid regional corners as coordinated surface,
   vegetation, clutter, and landmark bundles with explicit transition
   shoulders.
7. Add ocean-climate families and climate-aware water treatment.
8. Add independent caves, strata, ores, and underground landmarks.
9. Grow authored and procedural structures on the resulting regional
   vocabulary.

## Evidence Required Per Family

Every live family should eventually record:

- selector coverage and boundary ratios on several seeds;
- deterministic field, biome, surface, and feature maps;
- exact output fingerprints and partition/order/periodic seams;
- representative surface, vegetation, water, and landmark block counts;
- agreement between exact output and any future distant representation for
  terrain-scale facts;
- cold/warm generation cost and a movement soak when work is substantial;
- fully warmed high-view-distance pixels; and
- the human description worth preserving, including both peaceful and
  deliberately dramatic visual families.

## Related

- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md)
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`still-life-and-tectonic-reference.md`](still-life-and-tectonic-reference.md)
- [`modern-minecraft-reference.md`](modern-minecraft-reference.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md)
- [`../worldgen-status.md`](../worldgen-status.md)
- [`../structures.md`](../structures.md)
- [`../tactical/135-overworld-biome-palette-matrix.md`](../tactical/135-overworld-biome-palette-matrix.md)
- [`../tactical/146-overworld-macro-terrain-geometry-parity.md`](../tactical/146-overworld-macro-terrain-geometry-parity.md)
- [`../tactical/192-mclone-overworld-mountains-and-valleys.md`](../tactical/192-mclone-overworld-mountains-and-valleys.md)
- [`../tactical/222-bounded-valley-stream-structures.md`](../tactical/222-bounded-valley-stream-structures.md)
- [`../tactical/259-modern-and-historical-coast-reference-survey.md`](../tactical/259-modern-and-historical-coast-reference-survey.md)
- [`../tactical/263-cross-era-inland-landform-survey.md`](../tactical/263-cross-era-inland-landform-survey.md)
- [`../tactical/264-mclone-ordinary-inland-landform-fabric.md`](../tactical/264-mclone-ordinary-inland-landform-fabric.md)
- [`../tactical/265-macro-landform-grammar-research.md`](../tactical/265-macro-landform-grammar-research.md)
