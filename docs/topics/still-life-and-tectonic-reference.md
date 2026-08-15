# Still Life And Tectonic Terrain Reference Study

Topic: `still-life-and-tectonic-reference`

Status: **Still Life 0.2, its contemporaneous Lithosphere 1.7 terrain
dependency, and Tectonic v3 have been inspected as design references for the
custom `mclone-overworld-v1` profile. Still Life is primarily an ecology,
biome, surface, and feature system; Lithosphere owns nearly all of the terrain
below it. Tectonic is primarily a terrain-density system with named regional
landform families and relatively light biome replacement. Their strongest
combined lesson is to coordinate terrain and ecology through small, semantic
regional recipes, not to import either pack's large Minecraft JSON graph.**

## Scope And Short Answer

The request called the second project "techtonic." This study interprets that
as [Tectonic](https://modrinth.com/mod/tectonic), the actively maintained
Minecraft terrain project by Apollo.

This is design research, not a parity target or a dependency proposal. The
custom mclone profile remains original work. In particular:

- Still Life's released archive and Lithosphere are All Rights Reserved. They
  were inspected and measured, but no third-party artifact or rule table is
  copied into the repository.
- Tectonic is MIT-licensed and has public source, but its implementation is
  deeply shaped by Minecraft's density-function and multi-noise formats.
  Reusing its concepts is more valuable than translating its JSON literally.
- The legacy Java-1.17-shaped `overworld` profile is separate but no longer a
  correctness target. These recommendations apply only to the product-default
  `mclone-overworld-v1`.

The compact answer is:

| Project | What actually owns its identity | Most useful mclone lesson |
|---|---|---|
| Still Life | A large biome routing table, biome-specific surfaces, and correlated vegetation/detail features | Give ecology explicit regional recipes and transitional shoulders, driven by the same facts as terrain |
| Lithosphere, required by Still Life | Continental terrain, coasts, mountains, rivers, lakes, swamps, and caves | Compose named terrain mechanisms in a deliberate order and gate them regionally |
| Tectonic | A mostly data-defined density graph with continent/island separation and several named regional landform grammars | Route bounded landform recipes from shared macro/climate fields, and make those semantic fields inspectable |

Still Life is therefore not a second independent answer to "how should a
height generator work?" It is a useful answer to "how should a terrain
generator become a legible world of neighboring ecological regions?"
Tectonic is much closer to an answer to the first question.

## Specimens And Provenance

The study was performed on 2026-07-24.

| Specimen | Receipt |
|---|---|
| Still Life | [Project page](https://modrinth.com/datapack/still-life), release 0.2, project ID `fK6aflho` |
| Still Life archive | `still-life-0.2.zip`, 830,577 bytes, SHA-1 `98c871919adc57e5439d7961b37ba6b96b0cd7de` |
| Lithosphere | [Project page](https://modrinth.com/datapack/lithosphere), inspected release [1.7](https://modrinth.com/datapack/lithosphere/version/1.7) |
| Lithosphere archive | `lithosphere-1.7.zip`, 146,986 bytes, SHA-1 `8078db17b619d1328c4b3043aeb168fe19cb1ccf` |
| Tectonic | [Project page](https://modrinth.com/mod/tectonic), [public repository](https://github.com/Apollounknowndev/tectonic), MIT |
| Tectonic source | Commit [`34241bd`](https://github.com/Apollounknowndev/tectonic/tree/34241bdb35acda67b5367d49f354c66c05e098e2), declared by its build as 3.0.25 |

Still Life 0.2 was built against the Lithosphere generation family current at
the same time. Lithosphere 1.7 is the closest reproducible specimen and owns
all of the density-function IDs referenced by Still Life. The current
Lithosphere page describes subsequent additions such as karst, cave entrances,
and alternative continent shapes; those newer claims are not silently read
back into the 1.7 equations below.

Modrinth records no public source repository for Still Life or Lithosphere.
Both distributed datapacks are readable JSON, so this study traces the exact
release artifacts but cannot inspect their authoring history. Tectonic has the
opposite posture: its repository and MIT license permit a direct source trace.

Tectonic's current public releases are slightly newer than the inspected
repository commit. The 3.0.26 and 3.0.27 release notes describe mountain
jaggedness and crash fixes, not an architectural replacement. Claims about
current user-visible features come from the project page; exact equations and
defaults in this study come from the pinned 3.0.25 source.

The archives can be reproduced outside the repository:

```bash
study_dir=$(mktemp -d /tmp/mclone-terrain-study.XXXXXX)

curl -fsSL \
  'https://cdn.modrinth.com/data/fK6aflho/versions/z55fBbb2/.still_life%200.2%2016.08.2025.zip' \
  -o "$study_dir/still-life-0.2.zip"
shasum -a 1 "$study_dir/still-life-0.2.zip"

curl -fsSL \
  'https://cdn.modrinth.com/data/iv9jp2k9/versions/jBiNUKhM/lithosphere%201.7%2015.08.2025.zip' \
  -o "$study_dir/lithosphere-1.7.zip"
shasum -a 1 "$study_dir/lithosphere-1.7.zip"

git clone https://github.com/Apollounknowndev/tectonic.git \
  "$study_dir/tectonic"
git -C "$study_dir/tectonic" checkout \
  34241bdb35acda67b5367d49f354c66c05e098e2
```

The linked release pages and verified hashes are the durable receipts if
Modrinth changes its CDN paths.

## Architectural Comparison

All three packs use Minecraft's modern worldgen vocabulary, but their
ownership differs:

```text
Still Life
  climate remap -> 4,093 biome parameter boxes -> biome identity
                                               -> surface recipe
                                               -> vegetation/detail bundle
  Lithosphere  -> terrain height and density -> caves and water context

Tectonic
  continent/island facts -> terrain family router -> surface height/density
              climate facts ------------^       -> vanilla/modded biome fit
  cave, underground-river, and lava fields ------> final 3D density

mclone today
  shared macro/climate/water facts -> regional height recipe -> biome recipe
                                  -> surface/decorations -> LOD and debug facts
```

Still Life is the most ecology-heavy. Tectonic is the most geometry-heavy.
Mclone already has the cleaner ownership boundary: its fields, terrain,
biomes, surfaces, streams, decoration, and synthetic LOD live as explicit
shared Rust stages rather than global namespace replacement. The opportunity
is to enrich the facts passed between those stages without losing that
boundary.

## Still Life

### What It Is

Still Life describes itself as a full Overworld overhaul with naturalistic
counterparts for vanilla surface biomes, more shrubs, flowers, and trees,
transitional biomes, and several fantasy cave biomes, all using vanilla
blocks. It requires Lithosphere to be loaded below it.

That dependency statement is architecturally exact. The released pack has:

- 109 custom biome definitions;
- 4,093 multi-noise biome-routing entries selecting 110 unique biome IDs;
- 276 configured features and 304 placed features;
- a large surface-rule graph;
- only four Still Life density functions, all concerned with temperature or
  vegetation; and
- no independent terrain-density graph.

The Overworld noise settings explicitly use `lithosphere:depth` and
`lithosphere:density/final_density`. Still Life owns the ecological reading of
the world, while Lithosphere owns the land under it.

### Biome Routing

Minecraft's multi-noise biome source selects a biome from intervals on six
parameters. Still Life uses all six:

| Parameter | Distinct intervals in the released table | Role |
|---|---:|---|
| temperature | 131 | broad thermal ecological fit |
| humidity | 45 | forest, grassland, scrub, wetland, and desert fit |
| continentalness | 20 | ocean, coast, inland, and deep-interior context |
| erosion | 16 | relief and exposure context inherited from terrain |
| weirdness | 53 | ridge/valley and special topographic context |
| depth | 10 | separates surface ecology from cave families |

These counts do not form a Cartesian grid. The 4,093 records are hand-shaped,
overlapping hyperrectangles. Of those, 3,890 have the surface depth interval
`[0, 0]`; the rest route cave contexts over positive depth bands.

The vocabulary is the important result. Instead of a handful of generic
forest/plains/desert outputs, Still Life distinguishes arctic, tundra, taiga,
steppe, temperate, Mediterranean, savanna, tropical, highland, beach, river,
wetland, and ocean variants, with explicit intermediates. Common transitional
outputs have many entries: mixed forest-steppe, dry and cold steppe, temperate
forest, semiarid steppe, xeric shrubland, and wooded savanna act as shoulders
between ecological poles.

The benefit is not merely "more biomes." A player crossing the parameter
space usually encounters a plausible intermediate identity rather than an
abrupt palette flip.

The cost is maintainability. A 4,093-record nearest-parameter table is hard to
audit for holes, overlaps, accidental rarity, and boundary churn. Two defined
Still Life biomes are not referenced by the live table. Mclone should preserve
the concept of intermediate ecological recipes without adopting this data
shape.

### Surface And Detail

Still Life's surface graph contains at least 83 biome conditions, 220 block
rules, 123 noise thresholds, 44 vertical tests, 40 stone-depth tests, and four
steepness tests. Its palette is deliberately vanilla: grass, dirt, coarse
dirt, podzol, mud, sand, gravel, stone, calcite, tuff, terracotta, snow, ice,
and related blocks.

The feature graph is even more characteristic:

| Configured-feature family | Count |
|---|---:|
| random patches | 133 |
| random selectors | 57 |
| trees | 25 |
| vegetation patches | 19 |
| disks | 16 |
| fallen trees | 14 |

Noise-based counts, rarity filters, block predicates, heightmaps, and
water-depth tests make vegetation arrive as correlated patches rather than
uniform independent rolls. Individual biomes reference roughly 35 to 67
feature placements across Minecraft's generation stages.

Repeated terrain details include cliffs, monoliths, small monoliths, boulders,
fallen trees, low bushes, leaf litter, flower groups, and locally dense tree
families. These make a biome recognizable at walking distance after the
large-scale color and silhouette have already established its identity.

This is Still Life's strongest contribution to mclone: an ecological recipe
should be a coordinated bundle, not just a top block and a tree probability.

### Climate Fields

Still Life replaces temperature and vegetation with shifted-noise functions
and splines. In release 0.2, its normal and "large" variants are effectively
the same definitions. This gives the biome table broad, smooth environmental
coordinates while letting the many interval records describe transitions.

There is no evidence that Still Life computes ecological succession,
rain-shadow physics, soil transport, or drainage. Its naturalism comes from a
careful classifier and correlated decoration, not a physical ecosystem
simulation.

### Cave Biomes

The positive depth bands select both retained vanilla cave biomes and custom
families such as barren, frozen, glowing, infested, mushroom, pale, scorched,
and haunted caves. Temperature, humidity, continentalness, erosion, and
weirdness continue to participate below the surface.

This creates deliberate contrast: the surface pursues recognizable natural
ecology while the underground can be overtly fantastical. The useful design
lesson is that underground identity can consume surface-scale regional facts
without being a vertical copy of the surface biome.

### What Is Unique

Still Life's distinctive qualities are:

1. **Ecological continuity.** It spends a great deal of data on intermediate
   climates and covers rather than only on spectacular endpoint biomes.
2. **Biome-as-bundle design.** Surface materials, low vegetation, trees,
   clutter, landmarks, spawns, and color are coordinated.
3. **Strong local texture with a restrained block palette.** Variety comes
   from distribution and composition rather than new content.
4. **Independent underground tone.** Cave identities share regional inputs
   but are allowed a different fantasy register.
5. **A clean conceptual terrain/ecology split.** The implementation is coupled
   by Minecraft IDs and load order, but the authored responsibility is clear.

Its main liabilities are the enormous routing table, duplicated definitions,
pack-order coupling, reported generation cost, and an All Rights Reserved
license that rules out copying its authored tables.

## Lithosphere: The Terrain Under Still Life

Still Life cannot be explained honestly without tracing the terrain it
consumes. In Lithosphere 1.7, the broad composition is:

```text
continent/coast base
  + mountain and volcanic contribution
  + coastal cliffs and mesas
  -> river and lake carving
  -> low, humid swamp flattening
  -> final 2D topography
  + vertical gradient
  -> terrain density
  min cave families
  -> final 3D density
```

### Continents, Mountains, And Coasts

Lithosphere combines several low-frequency noises and spline remaps for
irregular continents, coast trenches, and orogeny. Orogeny gates mountain
contributions, while separate base, shape, variation, valley, and weathering
fields control their expression. Coastal cliffs are selected near the
continent boundary using their own placement, height, and roughness facts.

The important pattern is staged composition. Mountains are not simply a
high-frequency term added everywhere. A broad tectonic-like permission field
decides where the mountain family is eligible, then smaller fields shape it.

### Rivers, Lakes, And Swamps

The river mask is built from the absolute value of a low-frequency river
noise, creating corridors around its zero contours. Three valley profiles are
selected or blended according to erosion. River roughness is reduced near the
center. Lakes are macro depressions with separate placement and depth. Swamps
use humidity, continental context, and low relief to flatten suitable land
toward sea level.

These are terrain-participating waterforms: they modify topography before
surface rules run. They are not a directed drainage graph, do not prove
downstream monotonicity, and do not provide mclone's fixed-point water closure.

### Caves And Underground Rivers

Lithosphere combines cavern, connecting, maze, claustrophobic, tunnel, and
underground-river fields by density union. Its underground rivers occupy a
bounded vertical band and use folded terrain parameters to form long flooded
corridors. Aquifer floodedness and barriers are changed where the corridor is
active.

This is a visually effective density-and-aquifer construction, not explicit
hydrology. That distinction also matters for Tectonic.

### What Mclone Should Retain

Lithosphere provides a useful order of operations:

- establish broad land permission;
- select mountain/coast/lowland mechanisms;
- incorporate waterforms into topography;
- give wetlands a terrain response rather than only a green palette;
- derive the final surface-relative density; and
- add bounded underground families.

Mclone already follows much of this ordering with stronger deterministic,
periodic, request-partition, fluid, and LOD contracts. The reference supports
continuing that architecture, not replacing it.

## Tectonic

### What It Is

Tectonic advertises continent-scale landmasses, very deep oceans, mountain
ranges extending for tens of thousands of blocks, overhangs and valleys,
underground rivers, lava tunnels, jungle pillars, badlands canyons, dunes,
tiered plateaus, valleys, and wetlands.

The implementation is primarily a resource-pack density graph. Java supplies
four small configurable density-function types, configuration loading,
world-height adaptation, and runtime fixes for snow, lava level, ocean
monuments, and chunk blending. Terrain policy remains legible in JSON.

The four custom density primitives are:

- a configuration-backed constant;
- a configuration-backed noise with scale, multiplier, and offset;
- a configuration-backed clamp; and
- reciprocal/inversion.

This is a useful separation: the terrain graph is data, while code provides a
small vocabulary and platform integration. It is not, however, a reason for
mclone to convert its typed Rust stages into a generic density DSL.

### Macro Continents And Separate Islands

Tectonic computes a raw continent field from low-frequency configured noise,
an absolute-value/remap construction, and an ocean offset. Deep ocean
interiors activate a separate island selector. Island terrain combines two
independent island noises and uses a more vanilla-like shaping spline.

```text
raw continent field
  |-> ocean/coast/mainland terrain
  `-> deep-ocean mask -> independent island candidates -> island terrain
                     \___________________________________/
                                      blend
```

This is more deliberate than expecting one fractal field to produce both
convincing continents and convincing small islands. It also lets islands keep
a smaller, more familiar relief grammar while continental mountains become
extreme.

Mclone's existing continentalness and bathymetry are already separate facts.
If island variety becomes a concrete visual gap, a bounded island-family mask
is a better experiment than globally retuning continental frequency.

### Regional Terrain Router

Tectonic's most original and reusable mechanism is its regional terrain
router. An independent region selector, the sign of a remapped erosion field,
climate bands, ridge position, and inland eligibility choose among four
internally named families:

| Internal family | Characteristic landforms |
|---|---|
| Heart | rolling hills and climate-gated jungle pillars |
| Club | plateaus, single or double valleys, badlands ridges, and climate-selected valley forms |
| Spade | lower and upper tier plateaus or badlands canyons |
| Diamond | wetlands or desert dunes |

The names are arbitrary. The architecture is not. Each region gets a coherent
formation grammar instead of receiving every interesting noise term at low
strength. The router only operates in suitable inland continentalness and
erosion bands, and a regional height multiplier varies its strength.

Climate participates in geometric selection. Hot/dry contexts can receive
dunes or badlands forms; humid/hot contexts can receive pillars; other
contexts receive hills, valleys, wetlands, or plateaus. Tectonic also remaps
the biome router's erosion and ridge values from the same terrain facts, so
vanilla or compatible modded biomes tend to agree with the landform beneath
them.

This is a better reference for mclone than any one Tectonic landform. It shows
how a small semantic selector can create regional memorability while keeping
the underlying fields continuous.

### Mountain Ridges

Broad mountain permission comes from continental and erosion-scale fields.
Within eligible mountains, Tectonic adds a distinctive ridge-detail
construction:

1. sample a broad ridge base;
2. estimate X and Z slope signals from differences against X- and Z-shifted
   samples, using a constant shift of ten in the density definition;
3. take the absolute slope components;
4. use their amplified values to shift the coordinates of a detailed noise;
5. combine that shifted detail with a separate weathering mask; and
6. gate the result by mountain jaggedness.

The result is detail oriented by the changing broad ridge field rather than
unrelated isotropic grit. The exact amplification is highly pack-specific,
but the concept is useful: derivatives of a semantic macro field can orient
local geology.

Mclone already uses hierarchical relief, ridge, ruggedness, and mountain
detail fields plus restrained domain warping. A bounded A/B test of
gradient-oriented detail in mountain-only regions could be informative. It
should not become a global field revision until maps, periodic seams, LOD,
generation cost, and production pixels all show an improvement.

### Height, Density, And Caves

Tectonic's surface is still predominantly a 2D offset/height family combined
with a vertical gradient. `sloped_cheese` adds continent/island scaling,
mountain ridge and weathering detail, regional dunes, jaggedness, and ocean
roughness. Three-dimensional cave families are then combined into final
density.

The default source keeps modern vanilla-like bounds of Y -64 through 319 and
uses a vertical scale of 1.125. Configuration and presets can raise the
ceiling; an "overkill" preset reaches a 768 top boundary and a much larger
vertical scale. Extreme height is optional configuration, not the essential
mechanism.

Cheese, noodle, spaghetti, and vanilla carver families can be enabled
independently. A surface-relative cave cutoff reduces accidental cave
openings. This is another useful pattern for mclone's planned 3D geology:
volumetric mechanisms should know their relationship to the regional surface,
not operate as an unbounded world-wide noise soup.

### Underground Rivers

Tectonic's underground river is selected by a product of gates:

- a narrow Y band, approximately 32 through 72 in the default world;
- suitable continentalness;
- moderate folded erosion;
- a near-zero folded ridge corridor; and
- local 3D shape variation.

The density field forms a roofed corridor where a surface-scale river-like
ridge field intersects tall terrain. Pillars are unioned into the space.
Aquifer floodedness and barrier fields are adjusted inside it, and placed
features add lichen, optional lanterns, and optional ice.

This explains the public claim that underground rivers connect to ordinary
rivers at mountains. The continuity is geometric because both are tied to
the same long ridge corridor. There is no searched graph of sources,
confluences, outlets, or downstream heights.

For mclone this is inspiration for a future **subterranean reach**, not a
replacement for the current stream plan. A mclone version should consume a
named planned surface-water fact, carve only a bounded 3D connector under an
eligible formation, preserve monotonic route and fixed-point source-plane
contracts, and expose the whole reach to authoritative fluid wake.

### Lava Tunnels

Lava tunnels use a warped, folded ridge field in a thin band near the world
bottom. The density subtraction and configured global lava level make the
tunnel fill coherently. Like the underground river, this is a narrow semantic
mechanism with explicit vertical eligibility, not generic cave noise.

### Inspectability And Compatibility

Tectonic's command support can locate mountain ranges, underground rivers,
jungle pillars, rolling hills, badlands canyons and plateaus, dunes, and
valleys by testing their density predicates. That is a surprisingly important
quality: named landforms remain queryable after procedural composition.

The project also carries overlays for biome compatibility, alternate
carvers, ore behavior, smoother terrain, and Terratonic. The mod uses
Lithostitched modifiers for compatibility rather than placing all policy in
app code.

Mclone should preserve the queryability lesson. Its debug maps and generation
facts are already a stronger foundation than an opaque Minecraft density
graph; new formation recipes should make that foundation more semantic.

### What Is Unique

Tectonic's distinctive qualities are:

1. **Regional formation grammars.** A region gets dunes, wetlands, plateaus,
   valleys, hills, or pillars according to a coherent recipe.
2. **Terrain-biome coordination.** Climate helps choose geometry and terrain
   facts are remapped back into biome selection.
3. **Separate continent and island families.** Scale classes do not have to
   share one terrain response.
4. **Gradient-oriented ridge detail.** Macro-field slope helps orient local
   mountain texture.
5. **Semantic density corridors.** Underground rivers and lava tunnels are
   bounded, named 3D mechanisms.
6. **Inspectable landforms.** Feature-locator predicates survive the
   procedural graph.

Its liabilities for direct mclone adoption are Minecraft-specific spline
graphs, large JSON indirection, configurable world-height complexity,
aquifer-dependent water semantics, and some spectacular families that would
conflict with mclone's peaceful visual target if applied too frequently.

## Direct Comparison

| Question | Still Life | Tectonic | Mclone implication |
|---|---|---|---|
| Primary layer | ecology and biome presentation | macro terrain and 3D density | keep shared stages separate, strengthen their contract |
| Terrain source | Lithosphere dependency | own density graph | do not confuse a biome pack with a terrain algorithm |
| Regional identity | many ecological transition cells | a few strong landform grammars | prefer a small recipe router plus explicit shoulders |
| Biome coordination | terrain parameters select 109 custom biomes | terrain and climate remaps preserve vanilla/mod compatibility | let terrain and ecology consume the same facts |
| Water | inherited contour valleys, lakes, and aquifer caves | contour valleys and aquifer-filled underground corridors | retain mclone's planned streams and fixed-point closure |
| Local detail | rich correlated vegetation and surface features | ridge weathering and signature terrain forms | coordinate feature bundles with formation recipes |
| 3D geology | mostly inherited cave families | caves, river corridors, lava tunnels, overhangs | add bounded recipe-gated density modifiers |
| Maintainability risk | 4,093 biome boxes and duplicated per-biome data | deep JSON/spline graph and config matrix | typed recipes and named intermediate facts |
| Best quality to borrow | transitional ecological coherence | semantic regional terrain routing | combine them at a shared regional-intent boundary |

### Visual Read Of The Official Galleries

The official images are promotional selections, not controlled frequency or
performance evidence, but they reinforce the ownership found in the data.
Still Life's
[warm-temperate mountain](https://cdn.modrinth.com/data/fK6aflho/images/29cbd2c692d509c0bbe3489e95c9e3f4acb899a8.png)
and
[cold-steppe](https://cdn.modrinth.com/data/fK6aflho/images/7f1049f5f66f6e6ed9ee600bf36ce8e9119d08f0.png)
scenes are distinguished at walking scale by canopy density, sparse versus
wooded cover, low shrubs, flower patches, and surface exposure. The dramatic
mountain silhouette in the former belongs to the Lithosphere terrain layer
underneath.

Tectonic's
[mountain-range](https://cdn.modrinth.com/data/lWDHr9jE/images/e312ff19967d89e3fd79f2d5b78270a44fb9f216.png)
and
[badlands-plateau](https://cdn.modrinth.com/data/lWDHr9jE/images/0e2d3bd5969fb17a3ec459e7717a860fce16fb37.png)
scenes reverse that emphasis. Region-scale silhouette, relief, ridge
orientation, and a singular formation family dominate; ordinary biome
decoration supports the geometry. The useful mclone synthesis is to make both
scales intentional without forcing one subsystem to own both.

## Application To `mclone-overworld-v1`

### What Mclone Already Does Better

The current custom generator already has:

- hierarchical continentalness, relief, ruggedness, ridge, mountain-detail,
  climate, bathymetry, water, and morphology facts;
- climate-aware regional terrain and biome recipes;
- bounded planned streams with monotonic route and fixed-point water
  contracts;
- exact request-partition and optional 6,144-block periodic behavior;
- raw field maps, terrain cards, receipts, fingerprints, and measured
  generation/LOD cost;
- shared ownership across desktop, Android, XR, server, and browser targets;
  and
- a current roadmap that deliberately prefers regional 3D geology over
  globally switching to a density-noise terrain soup.

Neither reference justifies weakening those contracts. Tectonic's contour
rivers and aquifer corridors are less hydrologically explicit. Still Life's
large routing table is less inspectable. Both references nevertheless expose
gaps in regional vocabulary and coordination.

### Recommended Shared Contract

The next useful addition is a compact **regional formation intent** derived
once from existing raw fields and consumed by terrain, geology, ecology, LOD,
and debug views.

Conceptually:

```text
macro fields + climate + water proximity
  -> RegionalFormationIntent {
       family,
       strength,
       transition_weight,
       wetness_response,
       geology_permissions,
       ecology_permissions
     }
  |-> 2D terrain response
  |-> bounded 3D formation recipe
  |-> biome/ecology recipe
  |-> surface and feature bundle
  |-> synthetic LOD and debug predicate
```

This should be a typed Rust result or a small decision table, not a generic
JSON spline language and not thousands of parameter boxes. Raw selector
values should remain available beside the classified family so boundaries can
be tuned and audited.

The current `McloneOverworldLandformSample`, biome-decision, surface-recipe,
and debug layers are the natural integration points. A new shared formation
type may be warranted once the first 3D recipe is concrete. App crates should
not know these families.

### Recommended Sequence

#### 1. Use Tectonic's router lesson in the planned 3D-geology campaign

Keep the current next-work direction: overhangs, outcrops, tors, arches, and
regional 3D formation recipes. Select a small number of mutually legible
geology families from existing climate, ruggedness, ridge, continentalness,
and water facts.

A first campaign should still use the breadth topic's bounded mechanisms:

- one exposed rock/boulder or talus family;
- one signature authored formation; and
- one regional density modifier.

The new lesson is that these should be a coherent regional recipe. A tor
region should not also receive every arch, pillar, and overhang term merely
because all their noise thresholds happen to pass.

#### 2. Add semantic predicates and coverage evidence

Every formation family should expose:

- raw eligibility and strength;
- selected family;
- transition weight;
- eligible and realized coverage;
- boundary ratios;
- representative coordinates for lab inspection; and
- LOD agreement.

This is the mclone-native equivalent of Tectonic's landform locator, but it
should integrate with field maps, cards, receipts, and existing lab tooling.

#### 3. Follow geology with an ecological-breadth campaign

Still Life supports the existing breadth direction: move beyond the current
small set of surface recipes by adding a few high-contrast ecological
families and explicit intermediates. Do not start by adding 109 biome IDs.

A useful first slice would add approximately three endpoint recipes and their
shared transition shoulders, for example:

- dry grassland / shrub-steppe;
- cool mixed forest / forest-steppe; and
- wet lowland / sparse marsh woodland.

Each should own a coordinated bundle:

- top and subsurface palette;
- tree and low-shrub group;
- ground litter or rock group;
- density and spacing response;
- one walking-scale landmark family; and
- spawn/color/audio hooks when those shared systems are ready.

The important test is whether a region is recognizable in silhouette and
walking-scale composition, not how many biome IDs exist.

#### 4. Run a mountain-only derivative-detail experiment

Tectonic's finite-difference ridge warp deserves one isolated A/B experiment
against mclone's current mountain detail. It should:

- be gated to one mountain formation family;
- be derived from periodic-safe samples of an existing macro field;
- preserve exact cylinder seams;
- have a synthetic-LOD equivalent;
- report its incremental generation cost; and
- be rejected if it reads as noisy grit rather than oriented geology.

This experiment should follow, not block, the first formation-recipe slice.

#### 5. Defer subterranean rivers until surface-water facts can own them

A future cave-water tactical may use Tectonic's roofed corridor as a visual
reference. It should not use a folded noise contour as an independent
hydrology system. Instead, a planned stream reach or water landmark should
authorize a bounded subterranean connector with explicit inlet, outlet,
monotonic floor, source planes, and authoritative wake.

### Ideas To Avoid Or Defer

Do not adopt:

- Still Life's literal 4,093-entry biome table or authored biome definitions;
- Lithosphere's or Tectonic's JSON spline graph as a new mclone runtime DSL;
- a global 3D density soup for all terrain;
- aquifer heuristics as proof of stable or connected water;
- a large world-height expansion merely because Tectonic exposes one;
- every spectacular Tectonic landform in every region;
- production settings with dozens of fine-grained generator toggles before
  stable formation families exist; or
- third-party names, tables, thresholds, palettes, or feature placements from
  All Rights Reserved packs.

The references are strongest as evidence for composition and ownership:
regional permissions, named mechanisms, shared facts, coherent bundles, and
legible transitions.

## Validation Consequences

Any implementation influenced by this study should retain the existing
mclone evidence bar:

- exact deterministic fingerprints and request/order/partition invariance;
- exact plane and periodic-cylinder seams where applicable;
- maps for raw selectors, classified recipes, strengths, and transitions;
- family coverage and boundary statistics over several seeds;
- water masks and authoritative wake for any water-participating family;
- warmed generation and synthetic-LOD timings with incremental cost;
- mono, stereo/per-eye, and multiview-safe rendering for any new debug visual;
- first-drawable and production screenshots inspected from `/tmp`; and
- human review of both region-scale silhouette and walking-scale ecological
  composition.

For ecology, add adjacency evidence: measure how often endpoint families touch
directly versus through an intended shoulder. For geology, add overlap
evidence: measure how often mutually exclusive signature families are
realized in the same local region.

## Conclusions

Still Life and Tectonic solve complementary parts of overworld authorship.

Still Life shows that naturalistic breadth comes from intermediate ecological
identities and coordinated detail bundles. Its surface can feel rich without
new blocks because material, vegetation, clutter, and local landmarks respond
together. Its dependence on Lithosphere also demonstrates that ecology can be
authored separately from terrain.

Tectonic shows that memorable terrain does not require every region to run
the same noise equation. A small regional router can select coherent formation
grammars while climate and terrain facts keep biomes in agreement. Its
mountain-detail warp and bounded underground corridors are worthwhile
experiments, but not contracts to copy.

For mclone, the synthesis is a shared regional-formation intent that feeds the
already separate terrain, 3D geology, ecology, surface, feature, LOD, and debug
stages. The next concrete use should be the planned bounded 3D-geology
campaign. An ecological-breadth campaign should follow with a small number of
coherent endpoint and transition recipes. This advances the custom overworld
without sacrificing its deterministic, periodic, hydrological, performance,
and cross-platform advantages.

## Primary Sources

- [Still Life project page](https://modrinth.com/datapack/still-life)
- [Still Life 0.2 release](https://modrinth.com/datapack/still-life/version/z55fBbb2)
- [Lithosphere project page](https://modrinth.com/datapack/lithosphere)
- [Lithosphere 1.7 release notes](https://modrinth.com/datapack/lithosphere/version/1.7)
- [Tectonic project page](https://modrinth.com/mod/tectonic)
- [Tectonic source repository](https://github.com/Apollounknowndev/tectonic)
- [Pinned Tectonic 3.0.25 source](https://github.com/Apollounknowndev/tectonic/tree/34241bdb35acda67b5367d49f354c66c05e098e2)
- [Tectonic configuration source](https://github.com/Apollounknowndev/tectonic/blob/34241bdb35acda67b5367d49f354c66c05e098e2/src/common/main/java/dev/worldgen/tectonic/config/ConfigState.java)
- [Tectonic continent field](https://github.com/Apollounknowndev/tectonic/blob/34241bdb35acda67b5367d49f354c66c05e098e2/src/common/main/resources/resourcepacks/tectonic/data/tectonic/worldgen/density_function/noise/raw_continents.json)
- [Tectonic regional terrain router](https://github.com/Apollounknowndev/tectonic/blob/34241bdb35acda67b5367d49f354c66c05e098e2/src/common/main/resources/resourcepacks/tectonic/data/tectonic/worldgen/density_function/terrain_spline/offset/regions.json)
- [Tectonic mountain ridge detail](https://github.com/Apollounknowndev/tectonic/tree/34241bdb35acda67b5367d49f354c66c05e098e2/src/common/main/resources/resourcepacks/tectonic/data/tectonic/worldgen/density_function/mountain_ridges)
- [Tectonic underground river field](https://github.com/Apollounknowndev/tectonic/tree/34241bdb35acda67b5367d49f354c66c05e098e2/src/common/main/resources/resourcepacks/tectonic/data/tectonic/worldgen/density_function/underground_river)
- [Tectonic wiki](https://github.com/Apollounknowndev/tectonic/wiki)

## Related

- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`mclone-overworld-breadth.md`](mclone-overworld-breadth.md)
- [`jjthunder-to-the-max-reference.md`](jjthunder-to-the-max-reference.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`../reference-minecraft.md`](../reference-minecraft.md)
- [`../tactical/196-periodic-mclone-terrain-fields.md`](../tactical/196-periodic-mclone-terrain-fields.md)
- [`../tactical/222-bounded-valley-stream-structures.md`](../tactical/222-bounded-valley-stream-structures.md)
