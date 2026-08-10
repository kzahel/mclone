# Starter Farmstead Settlement

Topic: `starter-farmstead-settlement`

Status: **vision and staged direction accepted 2026-07-21. Tacticals 208–210
prove the standalone building loop, human-directed charm iteration, and first
bounded building families: named cottage depth/entry plans and barn
length/lean-to compositions over shared templates, themes, transforms, a small
full-cube plus glass/stair/slab palette, normal persisted chunks, SQLite reopen,
and inspected production renders. Tactical 214 externalizes those families
through the completed Structure Lab and adds a Lab-native coop. On 2026-08-10,
the selected project focus shifted from open-ended terrain/LOD research to the
first playable intro homestead. Active Tactical
[`274`](../tactical/274-playable-intro-homestead.md) owns the deterministic
random-seed near-spawn scout, true structure lifecycle, persisted overlay and
plan identity, terrain-adaptive first composition, safe arrival, residents,
and shareable desktop/web acceptance. The existing original terrain and
procedural horizon remain foundations; further speculative expansion is
paused unless the playable opening exposes a concrete blocker.**

Last reconciled: **2026-08-10**.

## Scope

This topic owns the product vision and continuing decisions for a welcoming
demo starting world centered on a small farmstead settlement. It covers:

- the maximal composition and the first upgradeable subset;
- site and seed selection;
- reusable structure-authoring vocabulary;
- terrain grading, hydrology, and decoration reservation;
- initial resident and player-spawn intent;
- generated source-of-truth versus materialized persistence;
- the relationship to normal terrain and future distant presentation; and
- the staged path from one art-directed settlement to placement on arbitrary
  suitable seeds.

Generic vanilla-shaped structure statuses, starts, references, pieces, and
persistence remain owned by [`../structures.md`](../structures.md). Original
terrain fields and their sequencing remain owned by
[`mclone-overworld-generation.md`](mclone-overworld-generation.md). Creature
and persistence behavior remain owned by their existing subsystem documents.
A future multiscale terrain architecture will own distant presentation. This
topic records how the farmstead should consume those systems; it does not
silently redefine them.

## Accepted Product Direction

Use the terrain-adaptive blueprint approach:

1. author a reusable farmstead recipe rather than treating saved chunks as the
   only source;
2. select a particularly attractive seed and site for the first polished demo;
3. survey and adapt that recipe to the local terrain through bounded grading,
   foundations, paths, and water shaping;
4. materialize ordinary persistent chunks, structure metadata, and resident
   entities from the recipe; and
5. retain the same blueprint as original terrain generation changes, re-scouting
   and rebuilding new demo worlds when necessary.

The first chosen seed is an art-direction input, not the structure format. A
prebuilt starting-region cache may eventually make startup cheap, but it is a
derived artifact keyed by generator and blueprint versions. It must not become
the only recoverable copy of the settlement.

“Any seed” means deterministic search for a suitable nearby site plus bounded
adaptation. It does not mean flattening every mountain until the same plan fits.
The polished demo may use a known-good seed indefinitely while the generic site
selector improves.

## Maximal Farmstead Vision

The destination is a compact, affectionate settlement that feels inhabited
rather than a checklist laid on a grid. Its composition should be legible on
foot, attractive from the arrival direction, and rich enough to reward looking
around:

- an arrival path or footbridge leading toward a central green;
- a farmhouse with a kitchen garden and intimate yard;
- a substantial barn and working barnyard;
- a small church or chapel on a modest rise with a churchyard;
- sheds, workshops, storage buildings, a stable, and other outbuildings;
- fenced or hedged paddocks for cows, sheep, pigs, and horses;
- chicken and future duck areas that read differently from large-livestock
  enclosures;
- crop plots, vegetable beds, flowers, an orchard, hay, and small work details;
- a pond connected to a pleasant descending stream or runnel;
- a small watermill or wheelhouse where the authored or natural water grade
  makes it compositionally credible;
- bridges, stepping stones, banks, reeds, and wet-margin planting;
- a few deliberately authored trees, especially one memorable old oak, instead
  of uncontrolled ordinary tree decoration inside the settlement;
- cows, chickens, sheep, pigs, horses, ducks, and other appropriate farm life as
  their shared entity implementations become ready; and
- irregular paths, slight changes of angle, terraces, retaining edges, and
  planted boundaries that make the place belong to its terrain.

The maximal composition is a north star, not the first acceptance gate. Deferred
buildings and creatures should remain named blueprint roles or planned parcels
so later additions enrich the same place rather than force a replacement
design.

## First Upgradeable Composition

The first drawable farmstead should prove composition and regeneration with a
small set:

- a clear arrival/spawn point and short path;
- one farmhouse or cottage;
- one barn or combined barn/shed;
- one garden plot and one animal enclosure;
- a small pond with a short authored inlet or outlet;
- one focal authored oak and restrained planned planting;
- a cow group and chicken group using the currently proven persistent entity
  path; and
- enough site grading and foundation work to demonstrate that the settlement
  belongs to rolling terrain rather than a flat test world.

Church, stable, extensive outbuildings, multiple paddocks, real crop breadth,
horses, ducks, richer props, and more elaborate water can land later. The first
templates should use semantic material roles such as `foundation`, `wall`,
`roof`, `path`, `fence`, and `crop` even if an early compiler maps some roles to
temporary available blocks. That keeps content upgrades from requiring the
composition to be authored again.

## Current Readiness And Deferrals

The direction is accepted before all of its consumers are ready:

- `mclone-overworld-v1` has a live lowland/upland and vegetation language, but
  mountains, valleys, periodic fields, rivers, and broader climate/biome work
  are still staged in
  [`mclone-overworld-generation.md`](mclone-overworld-generation.md). A final
  demo seed should not be treated as stable while the relevant macro fields are
  still moving.
- The live Rust engine now has a small pure template/transform/material-role
  kernel, persisted standalone Structure Lab, and bounded cottage/barn families
  from Tacticals 208–210. Tactical 214 has since externalized and promoted all
  bounded members through canonical TypeScript-authored records, and added the
  first Lab-native chicken coop, but no true structure-start/reference/piece runtime.
  [`../structures.md`](../structures.md) explicitly requires that foundation
  before large cross-chunk structures.
- The generated block-state lane now includes oak/spruce planks, cobblestone,
  stone bricks, vertical hay, glass, and straight spruce stairs/slabs. The
  extracted and repo-owned asset paths, transforms, light facts, and
  multi-box collision path cover that first detail family. A convincing farm
  kit still needs doors, fences and gates, farmland, crops, props, and their
  collision/render/gameplay facts.
- The live shared protocol and persisted authored fixture currently prove cow
  and chicken residents. Additional animals must be admitted only after their
  actual shared simulation, persistence, protocol, asset, and render paths are
  ready. Ducks are original mclone content rather than a Java 1.17.1 parity
  port.
- The retired synthetic Far LOD omitted exact features, structures, and
  player/block edits. The settlement, its pond/grade changes, and its authored
  trees therefore have no honest distant representation yet.

None of these gaps should shrink the maximal vision. They control staging and
acceptance claims.

## Prerequisite And Capability Tiers

Do not turn the maximal vision into one all-or-nothing dependency gate. The
blueprint should declare hard requirements, optional capabilities, and clean
fallbacks so a useful settlement can land before every terrain and gameplay
system is complete.

| Capability | First composition | Maximal or later use |
|---|---|---|
| structure starts, references, pieces, clipping, and persistence | hard requirement | same lifecycle scales to the full site |
| templates, markers, semantic material roles, and ordered processors | hard requirement, even if the first vocabulary is small | alternate themes, richer variation, and weathering |
| site survey, grading budget, reservation, and safe arrival | hard requirement | broader candidate search and more complex terraces |
| planks, roof, foundation, fence/gate, door/window, path, garden, and crop content | enough of a coherent visual kit is required | full content and gameplay behavior replace temporary role mappings |
| original mountains and valleys | not a placement requirement; important before choosing a durable showcase seed | supplies the intended scenic setting and meaningful water grades |
| original rivers, streams, and wetlands | not required; use an authored pond and short runnel | unlocks natural-watercourse attachment and more convincing mill sites |
| generated waterfall reaches | not required | unlocks terrain-selected falls, cascades, and steeper mill races |
| watermill animation or mechanics | not required | a visual wheel may precede actual power, input, and output simulation |
| cows and chickens | enough to prove persistent residents | sheep, pigs, horses, ducks, and richer habitat behavior join when ready |
| fence collision, gates, and entity navigation | required before enclosures are claimed to contain active residents | supports larger paddocks and routine animal movement |
| crop hydration, growth, and harvesting | not required for a visual first garden | required before the farm claims complete crop gameplay |
| distant structure/edit representation | not required at the ordinary-terrain spawn experience | needed for honest distant landmark continuity |

Mountains/valleys and hydrology are therefore upstream quality investments,
not excuses to postpone the reusable structure machinery. Natural-stream,
waterfall, and watermill variants may require them; the basic farmstead does
not.

## Cross-Profile Composition Contract

The farmstead should be a starter-content overlay consumed by a generation
profile, not a new terrain profile and not an unconditional mutation of every
world. The persisted identity will eventually need to distinguish, in concept:

```text
base generation profile + seed + topology
starter-content/structure set + version
realized settlement instance plan
```

The current `WorldGenerationDescriptor` contains only profile, seed, and
topology, so this overlay identity is a future contract rather than a live
field. It must remain separate from the mutable/protected-lobby behavior
profile, which answers a different question.

This separation is especially important for the reference `overworld` profile.
Pure `overworld` must remain Minecraft Java 1.17.1 seed-parity output.
`overworld` plus an explicitly selected `intro-homestead-v1` overlay is a
different world identity whose terrain delta is intentional and persisted.
Flat Grass, Small Island, Mclone Overworld, Alpha, and Beta can consume the
same overlay engine without pretending that their terrain generators are the
same.

The shared site survey should expose neutral facts instead of profile names:

- surface elevation, slope, surface material/replaceability, and support;
- biome, climate, vegetation, and fluid tags when available;
- bounds, topology canonicalization, protection, and existing-structure facts;
- water surface, bank, wetness, flow direction, and grade when the profile has
  hydrology; and
- a deterministic way to obtain the required samples, either from pure
  generator fields or from bounded chunk-status inputs.

Each profile supplies those facts through an adapter. The blueprint declares
requirements such as buildable core, grade budget, dry arrival, minimum land
area, and optional natural-water access. It may select a composition tier,
omit optional parcels, substitute authored water, or reject the candidate.
Using one machinery therefore does not promise that the same maximal layout
fits every world:

| Base profile/site | Expected adaptation |
|---|---|
| Flat Grass | nearly no grading; authored pond/channel; useful deterministic placement canary |
| Small Island | fit a compact variant, prune named optional parcels, use a coastal outlet, or reject when the usable land cannot honestly hold the plan |
| Mclone Overworld | use bounded grading now; prefer a natural valley/tributary when original hydrology exists |
| reference Overworld | survey vanilla terrain but apply only through the explicit overlay identity, leaving pure parity output unchanged |
| Alpha/Beta | admit only after their survey adapter and structure compatibility are proven |

Topology remains orthogonal. A finite world must fit the complete plan and its
local processing halo within bounds or choose a smaller variant. A periodic
world must canonicalize survey and placement coordinates, preserve seam-
continuous terrain/water facts, and reject a footprint that wraps into or
overlaps itself. Profiles must opt into each topology; the farmstead must not
bypass those admissions merely because it is authored content.

## Vanilla 1.17.1 Reference Findings

Minecraft Java 1.17.1 supplies useful nouns and pipeline lessons, but mclone
will author original first-party content rather than copy vanilla structure
templates or village assets.

Primary sources reviewed for this topic:

- [`StructureTemplate.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/templatesystem/StructureTemplate.java)
  stores template size, alternate palettes, block records, block-entity NBT,
  and non-player entity records; it transforms, processes, clips, places, fixes
  edge shapes, preserves liquids, and optionally finalizes placed mobs.
- [`StructurePlaceSettings.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/templatesystem/StructurePlaceSettings.java)
  carries mirror, rotation, pivot, clipping box, entity/liquid policy, random
  selection, processor order, known-shape behavior, and entity finalization.
- [`StructureProcessor.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/templatesystem/StructureProcessor.java),
  [`RuleProcessor.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/templatesystem/RuleProcessor.java), and
  [`ProcessorRule.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/templatesystem/ProcessorRule.java)
  establish an ordered transform/filter chain whose rules may inspect template
  state, existing world state, local position, placed position, and structure
  origin.
- The concrete processors include ignore, integrity/rot, gravity, protected
  blocks, jigsaw replacement, liquid handling, block aging, and rule-based
  substitution. [`ProcessorLists.java`](../../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/ProcessorLists.java)
  uses the same mechanism for village crop variation, mossiness, and streets
  meeting water.
- [`StructureTemplatePool.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/structures/StructureTemplatePool.java)
  supplies weighted elements, fallback pools, and `RIGID` versus
  `TERRAIN_MATCHING` projection. Terrain matching is implemented as a gravity
  processor against `WORLD_SURFACE_WG`; rigid elements retain their shape.
- [`StructurePoolElement.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/structures/StructurePoolElement.java)
  has single-template, legacy-single, list, configured-feature, and empty
  element forms. Empty elements and fallback/terminator pools allow deliberate
  branch endings rather than treating every failed attachment as an error.
- [`JigsawBlock.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/block/JigsawBlock.java),
  [`JigsawBlockEntity.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/block/entity/JigsawBlockEntity.java), and
  [`JigsawPlacement.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/structures/JigsawPlacement.java)
  define connector orientation, `name`, `target`, `pool`, `joint`, and
  `final_state`; shuffle candidate elements and rotations; reject bounding-box
  collisions; attach compatible connectors; queue children to a maximum depth;
  and retain junction metadata.
- [`PoolElementStructurePiece.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/PoolElementStructurePiece.java)
  persists element, position, ground-level delta, rotation, bounding box, and
  jigsaw junctions, then places the clipped piece through the ordinary
  per-chunk structure path.
- [`TemplateStructurePiece.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/TemplateStructurePiece.java)
  exposes structure-block data markers through `handleDataMarker` and replaces
  consumed jigsaw markers with their final states.
- [`StructurePiece.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/StructurePiece.java)
  supplies useful placement verbs including `placeBlock`, `generateAirBox`,
  `generateBox`, `generateMaybeBox`, `maybeGenerateBlock`, and
  `fillColumnDown`, all clipped by the target bounding box.
- [`ChunkStatus.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)
  gives `STRUCTURE_REFERENCES` dependency range `8`.
  [`ChunkGenerator.createReferences`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java)
  interprets that as a fixed `[-8,+8]` scan around the target chunk and records
  only starts whose bounding boxes intersect the target chunk column.
- [`PlainVillagePools.java`](../../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/PlainVillagePools.java)
  separates town centers, terrain-matching streets, rigid houses, farms,
  animal pens, stables, temples, accessories, terminators, trees, decor,
  villagers, and animal pools. Decor pools can select configured features such
  as an oak, flowers, or hay alongside template elements.
- [`JigsawFeature.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/JigsawFeature.java)
  can project the start to a heightmap and uses noise-affecting structure
  metadata. [`Beardifier.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/Beardifier.java)
  blends rigid pieces and jigsaw junctions into terrain density before final
  block placement.

### Vocabulary To Preserve

Use vanilla terms with their vanilla meaning when the concept matches:

| Term | Meaning retained for mclone |
|---|---|
| structure start | persisted root metadata and ownership for one structure instance |
| structure reference | a touched chunk's link back to the start that owns the metadata |
| structure piece | bounded, transformable placement unit clipped into each touched chunk |
| bounding box | conservative piece/start bounds used for references, clipping, and collision |
| structure template | authored local blocks, block data, markers, optional entities, and size |
| palette | one aligned alternate block-state list inside a template |
| place settings | transform, pivot, bounds, randomness, liquid/entity flags, and processors |
| processor / processor list | ordered filtering or transformation of proposed template blocks |
| processor rule | predicate over input, existing location, and position yielding replacement state/data |
| template pool | weighted pool elements plus a fallback pool |
| pool element | single template, legacy template, list/composite, configured feature, or empty ending |
| projection | `rigid` or `terrain_matching` relationship between a pool element and ground |
| jigsaw connector | oriented `name`/`target`/`pool`/`joint`/`final_state` attachment marker |
| junction | persisted connection and vertical relationship between joined pieces |
| ground-level delta | element offset between its local content and the intended ground line |
| data marker | authored marker consumed by structure-specific placement logic |
| terminator | fallback element that closes a path or branch cleanly |

Vanilla `palette` must not become a catch-all name for theme selection. A farm
style such as “warm timber with fieldstone foundations” is better called a
**material theme** or expressed through a substitution processor. It may compile
to alternate template palettes later, but the authoring concepts are distinct.

### Processing Lessons To Adopt

- Keep capture/authoring, placement settings, and processors separate.
- Apply processors in an explicit deterministic order; a processor may filter a
  block by returning no placement.
- Let rules inspect both intended template state and existing terrain state.
- Clip every touched chunk's piece locally instead of allowing distant
  structure jobs to mutate already-final chunks.
- Preserve explicit liquid behavior, block-entity data, edge-shape updates, and
  entity-finalization policy rather than treating a template as only a vector
  of raw block writes.
- Keep weighted choices, empty choices, fallbacks, and terminators available
  even when the first farmstead uses a fixed layout.
- Keep rigid buildings distinct from terrain-matched paths and planting.
- Preserve foundation-fill and protected-block concepts.
- Keep authored data markers distinct from jigsaw connectors.
- Allow a pool element to invoke a configured feature. This is a good eventual
  shape for a deliberately selected special tree, flower patch, or hay pile.

### Deliberate Mclone Extensions

Vanilla's piece projection is not sufficient for the intended farmstead. A
gravity processor can follow a heightmap and Beardifier can blend nearby density,
but neither is a coherent landscape architect for terraces, a pond, a descending
stream, several building pads, paddock drainage, and an arrival composition.

Mclone therefore needs a settlement-level **site plan** before piece placement.
The site plan may use vanilla-shaped pieces and processors afterward, but owns:

- candidate scoring and chosen anchor/rotation;
- parcel and circulation layout;
- bounded cut/fill and terrace elevations;
- foundation and retaining-wall intent;
- pond, stream, bank, bridge, and wet-margin intent;
- decoration reservation and authored-feature masks;
- player arrival/spawn intent; and
- resident spawn markers and enclosure associations.

This extension should make future terrain parity easier, not fork the shared
structure lifecycle. The realized farmstead remains a true cross-chunk
structure with starts, references, pieces, clipping, and persistence.

## Farmstead Authoring Vocabulary

The first content recipe should be format-neutral but speak in these layers:

| Mclone term | Responsibility |
|---|---|
| farmstead blueprint | versioned source recipe for the whole intended composition |
| site survey | pure terrain/biome/water measurements for one candidate anchor and rotation |
| site score | inspectable fit, grading cost, arrival, water, and scenic-ring result |
| site plan | deterministic realized anchor, parcels, grading, hydrology, reservations, and pieces |
| parcel / plot | house yard, garden, paddock, churchyard, orchard, barnyard, or reserved expansion area |
| circulation | path, lane, bridge, gate approach, and connector graph |
| landscape intent | cut, fill, terrace, retaining edge, pond, channel, bank, or preserve-existing order |
| template | one authored building, prop cluster, tree, or bounded landscape element |
| piece instance | transformed placement of a template with theme and processors |
| material role | semantic `foundation`, `wall`, `roof`, `trim`, `path`, `fence`, or `crop` slot |
| material theme | role-to-content selection for one coherent visual language |
| socket | future-compatible doorway, path, fence, water, or attachment endpoint |
| reservation mask | area where ordinary terrain decoration is suppressed or constrained |
| special feature | explicitly authored tree, flower bed, hay pile, reed patch, or similar intentional accent |
| resident marker | one-time persistent entity placement with kind, local pose, group, and enclosure intent |
| arrival marker | preferred player spawn pose and facing after terrain clearance validation |
| realized settlement | persisted structure instance and normal chunks/entities produced from the plan |

The blueprint may begin as one fixed list of piece instances. Sockets and
parcels still belong in that first data so later weighted pools can replace
fixed choices without discarding the authored composition.

## Site Selection And Seed Scouting

Do not pick the final seed by wandering manually alone. Once the relevant
original terrain fields are accepted, use the same production samplers to rank
many seeds and candidate sites. Keep the metrics and the final visual decision
separate.

The scout should report at least:

- buildable-core area and elevation range;
- slope distribution and estimated cut/fill volume;
- flood/water intersection and a plausible descending watercourse;
- dry access from the proposed arrival direction;
- space for the basic plan and named maximal expansion parcels;
- biome/surface compatibility;
- scenic relief, woodland, meadow, coast, and water in a wider ring;
- expected ordinary-tree clearing inside the reservation; and
- candidate spawn safety and arrival view.

Generate full chunks and rendered review cards only for the best metric
candidates. Inspect an aerial/site-plan view, the player arrival, the central
green, and exterior views toward the future church/barn silhouettes. A seed is
provisional until both the terrain distribution and the composed pixels are
accepted.

Real original rivers are not a hard prerequisite for the first farmstead. A
short pond-fed or spring-fed authored stream can prove hydrology. When the
profile's river field lands, re-evaluate whether the maximal settlement should
select a natural tributary and adapt it instead.

## Hydrology, Waterfalls, And The Mill

Original Mclone terrain should gain valleys and coherent water systems for its
own landscape quality, not solely to decorate this settlement. The existing
long-term order in
[`mclone-overworld-generation.md`](mclone-overworld-generation.md) remains
sound: establish mountains and valleys, prove periodic field behavior, then add
river and wetland influence before final surface recipes. The first river slice
may be an inspectable deterministic field; it does not need to wait for a full
rainfall and flow-accumulation simulation.

The useful terrain-owned hydrology contract is richer than a boolean `river`:

- continuous corridor or centerline influence;
- water-surface elevation, width, depth, and bank/floodplain extent;
- downstream direction and local grade;
- confluence and continuity identity across chunk/order boundaries;
- wetland, spring/headwater, and coastal-outlet classification; and
- optional reach classification for calm water, riffle, cascade, fall, or
  mill-compatible flow.

The first farmstead may supply authored pond, channel, source, and outlet facts
inside its site plan. Once original hydrology exists, the site survey can expose
the same neutral facts and the blueprint can attach to, preserve, divert a
short mill race from, or cleanly decline a natural reach. This keeps water
placement policy out of profile-name branches.

Waterfalls should be consequences of a continuous watercourse crossing a
terrain drop, not isolated cliff decorations. Their generated block states
must form a stable source/falling-water arrangement with a valid downstream
destination. Mclone already has authoritative stored water states and local
scheduled-flow behavior; broad oceans and rivers can remain mostly stable
source blocks rather than permanently ticking every cell. Visible falling
water, bank meshing, sound, mist, and splash effects are separate presentation
capabilities and can arrive incrementally.

A watermill has three separable acceptance levels:

1. a mill building beside authored water, with a static wheel;
2. an animated wheel on a surveyed reach with sufficient bank clearance,
   foundation support, flow direction, grade, and downstream space; and
3. functional milling with gameplay-owned power, inputs, outputs, controls,
   persistence, sound, and failure behavior.

Only the first level belongs in a basic visual settlement. A natural or
terrain-selected mill site depends on the richer hydrology survey. Functional
machinery is not a world-generation prerequisite and should not be smuggled
into the structure system.

## Terrain And Placement Pipeline

The intended logical order is:

```text
profile terrain fields and structure candidate
  -> deterministic site survey and site plan
  -> persisted start, piece, reference, and reservation metadata
  -> base terrain plus settlement terrain influence
  -> surface and bounded cut/fill, terraces, pond, and stream
  -> per-chunk clipped foundations, paths, and rigid building pieces
  -> ordered template processors and data markers
  -> authored special trees, planting, crops, and props
  -> one-time persistent resident realization and validated player arrival
  -> ordinary decoration only where the reservation permits it
  -> shared lighting, publication, and persistence
```

The exact status split belongs to the future structure tactical. The important
invariants are already binding:

- the site plan is deterministic from profile, seed, blueprint version, and
  candidate identity;
- all touched chunks derive the same plan without request-order dependence;
- a large settlement is not implemented as an ordinary `PlacedFeature` doing
  arbitrary far writes;
- each chunk applies only its clipped slice;
- terrain influence is available before final surfaces when the chosen grading
  method needs it; and
- ordinary decoration cannot later grow a generic tree through a roof, path,
  paddock, or designed view.

### Dependency Radius And Large-Plan Scheduling

Vanilla's structure dependency range `8` is often misread. It means that
`STRUCTURE_REFERENCES` examines a square from eight chunks west through eight
chunks east and eight north through eight south: `17 x 17`, or 289 candidate
start chunks. It is a metadata/status discovery shell, not an `8 x 8` mutable
construction region. The start chunk owns the piece metadata, intersecting
chunks record references, and each target chunk later places only its clipped
slice.

A centered 17-chunk window spans 272 blocks, which is already generous for the
intended buildings, paddocks, pond, and mill. The first maximal blueprint
should provisionally keep its materialized settlement footprint within eight
chunks of a central start. Its seed-scouting and scenic-analysis ring may be
wider because reading terrain for composition does not make that terrain part
of the structure.

Current mclone ordinary feature planning is deliberately different and much
smaller. One target expands to `3 x 3` backend feature centers and `5 x 5`
Surface prerequisites. That machinery is appropriate for bounded trees and
patches; it must not be enlarged until it can stamp the whole maximal
farmstead. Making radius `8` mean full mutable chunks would demand 289 chunks
for one target before any additional prerequisite halo, and the square cost
keeps growing with settlement size.

The farmstead instead needs metadata-first scheduling:

1. discover and survey a candidate using pure/coarse profile samples or one
   explicitly bounded planning batch;
2. compute the deterministic site plan once and persist its root identity,
   exact piece bounds, landscape bounds, reservations, and touched chunks;
3. make touched chunks refer to the owning start through an exact instance
   manifest or structure spatial index; and
4. let each chunk materialize only its clipped blocks plus the small declared
   local halo required by grading, bank continuity, lighting, or similar
   processors.

The total settlement footprint and the local processing halo are separate
numbers. If a later composition covers more chunks than vanilla's fixed
`+/-8` lookup can discover from one central start, that still does not require
all of those chunks to be live together. Original structures should declare
their reach/touched-chunk set or use an exact spatial index rather than silently
raising one global fixed radius. Vanilla parity structures must retain
vanilla's exact range and semantics.

The future tactical should still impose an explicit maximum farmstead extent,
piece count, grade volume, and planning-sample budget. Those are content and
admission limits, not scheduler-wide mutable radii.

## Trees And Decoration Reservation

Every tree inside the settlement's designed envelope should be intentional.
The reservation suppresses ordinary profile tree placement there. The blueprint
may then place:

- one focal old oak;
- a small deliberately spaced orchard;
- a churchyard or stream-bank tree where composition calls for it; and
- explicitly authored hedges, flower beds, reeds, and wild margins.

These may be templates, special tree configurations, or later configured-
feature pool elements. They must use settlement-owned seed domains and markers,
not depend on whatever ordinary decoration happened to run first.

Outside the designed envelope, normal original-biome decoration continues. A
soft transition band may permit constrained grass and flowers while still
rejecting trees and large conflicting features.

## Residents And Spawn Semantics

Settlement residents are authoritative entities, not render decorations or
ordinary natural-spawn outcomes. A resident marker should eventually declare:

- stable marker identity within the blueprint;
- entity kind and local position/facing;
- group and enclosure identity;
- one-time creation policy; and
- optional later habitat or role metadata.

Materialization creates normal persistent entity records once. Save/reopen and
chunk cache invalidation must not duplicate killed or moved animals. By default,
a killed resident stays gone; an explicit demo reset can rebuild the source
world. A replenishing paddock would be a later gameplay system, not hidden
natural spawning.

The player arrival marker similarly declares intent, not an unsafe absolute
answer. Final admission validates support, headroom, fluids, and the realized
path before fixing the spawn pose and facing.

## Persistence And Upgrade Policy

The source identity for a fresh demo world is conceptually:

```text
world-generation profile and revision
world seed
farmstead blueprint id and version
site candidate/anchor and rotation
settlement seed-domain revision
```

From that identity, a new world can regenerate the site plan, structure
metadata, chunks, and initial residents. Materialized chunks then become
ordinary durable world state. Player edits and resident history win over later
generator output.

During the current internal-unshipped period, the canonical demo world may be
discarded and rebuilt when original terrain or the blueprint changes. After a
release freeze, upgrades require an explicit migration or a new demo identity;
they must not silently restamp a played farm over user edits.

The existing authored-only fixture path in
[`../../native/crates/mclone-server/src/authored_fixture.rs`](../../native/crates/mclone-server/src/authored_fixture.rs)
is useful evidence that authored chunks and cow/chicken entity records can flow
through shared persistence. It is not the target farmstead authoring format.

## Distant Presentation Contract

Tactical [`245`](../tactical/245-retire-chunk-far-lod-runtime.md) removed the
rejected chunk-based Far LOD. Its historical profiles omitted exact features,
structures, and edits. Therefore:

- the farmstead landscape, buildings, special trees, and residents are absent
  from that historical runtime;
- avoiding ordinary trees inside the site makes near composition deterministic
  but does not by itself solve far representation;
- the first farmstead may accept appearance at the normal-terrain handoff, and
  the spawn experience should begin inside ordinary rendered terrain; and
- documentation and captures must not imply that the settlement already has
  distant continuity.

A later **settlement LOD proxy** may derive a coarse graded surface/water patch
and landmark silhouettes for the barn, chapel, and focal oak from the same site
plan. That proxy remains presentation-only and must use the spatial identity,
coverage ledger, and exact/procedural handoff of a future multiscale terrain
architecture. It must not preserve the current `ChunkPos + level` coordinator.
It is explicitly deferred; do not add a farmstead-specific second authority
world or ad hoc app-rendered model.

See [`far-lod.md`](far-lod.md) and
[`gpu-procedural-terrain.md`](gpu-procedural-terrain.md).

## Implementation Tracking Model

Track this concern at four levels, each with a different owner:

1. **This topic is the dashboard and durable contract.** It owns the accepted
   farmstead vision, dependency ledger, integration state, evidence links, and
   next recommended farmstead slice.
2. **Subsystem topics own upstream truth.** Original terrain and hydrology stay
   in [`mclone-overworld-generation.md`](mclone-overworld-generation.md), true
   structure lifecycle stays in [`../structures.md`](../structures.md), and
   liquids, entities, and persistence stay in their own documents; a future
   multiscale terrain architecture owns distant presentation. This dashboard
   records the capability revision or evidence the settlement consumes; it
   does not copy their detailed implementation checklist.
3. **Numbered tactical docs own bounded execution.** Open one zero-padded
   tactical only after its inputs and acceptance contract are known. A
   farmstead tactical should produce one reviewable layer, not attempt the
   maximal settlement in one document.
4. **Tests, receipts, persistence fixtures, and inspected captures are proof.**
   Code presence alone does not move a row to `proven`.

Use these states consistently in the ledger:

| State | Meaning |
|---|---|
| `accepted` | product or architecture contract is decided; implementation may still be absent |
| `waiting` | a named upstream capability or decision is missing |
| `ready` | prerequisites are available and a bounded tactical may be opened |
| `active` | one linked tactical currently owns the implementation slice |
| `proven` | implementation and every declared acceptance gate have evidence |
| `deferred` | intentionally later and not a blocker for the current stage |
| `superseded` | replaced by a named newer contract or tactical |

At most one farmstead-specific tactical should be `active` at a time. Independent
upstream work such as mountains/valleys, structure foundations, or block
content may advance under its own topic and tactical. This keeps the content
integration coherent without serializing the whole engine behind the farm.

### Upstream Capability Watchlist

These capabilities improve later site selection or unlock optional variants,
but they are not farmstead implementation sequence numbers and do not block
standalone building iteration:

| Upstream ID | Capability | State | Farmstead effect |
|---|---|---|---|
| `UP-WG-192` | original mountains and valleys | `proven` | supplies the current macro relief and traversable valley language consumed by the intro scout |
| `UP-WG-196` | periodic original terrain fields | `proven` | gives accepted terrain fields explicit plane/cylinder behavior; additional topology breadth is not an intro blocker |
| `UP-HYDROLOGY` | original rivers, wetlands, and bounded streams | `proven for the first intro` | existing rivers, wetlands, and bounded valley streams supply optional survey facts; Revision 1 may still use authored pond/runnel water |
| `UP-WATER-REACHES` | richer cascades, waterfalls, and mill-compatible reaches | `deferred` | improves later scenery and mill selection but does not block the first playable composition |

### Progress Ledger

Stable `FS-*` identifiers refer to integration workstreams, not tactical
numbers. Allocate the next available tactical number only when a slice is
approved; do not reserve a block of numbers in advance.

| ID | Workstream | State | Current evidence or dependency | Next transition |
|---|---|---|---|---|
| `FS-00` | vision, vocabulary, staging, and cross-profile contract | `accepted` | this topic and reviewed Java 1.17.1 template/jigsaw/structure sources | keep reconciled as implementation changes facts |
| `FS-01` | standalone structure-authoring lab | `proven` | Tactical 214: strict TypeScript source graph, generated JSON drift gate, Rust loader/compiler, twelve promoted family members, Lab-native coop, and inspected read-only catalogue | add buildings through finite source-first recipes; do not broaden into arbitrary dimensions or browser editing |
| `FS-02` | coherent farm block/material/collision kit | `ready` | fourteen original farmstead materials now cover cottage, barn, and coop presentation; glass and transform/collision-aware stairs/slabs are proven; doors, fences/gates, farmland, crops, and props remain | resume with the next composition-driven family after the terrain campaign or when an active terrain review needs it |
| `FS-03` | template records, semantic roles, transforms, markers, bounds, and touched chunks | `proven` | Tactical 208 pure kernel tests and persisted cross-chunk lab receipts | extend only when a caller needs processors, codecs, alternate palettes, entities, or block data |
| `FS-04` | true structure statuses, starts, references, pieces, clipping, and persistence | `active` | Tactical 274 Slice 1 first proves a tiny original cross-chunk canary before the farmstead becomes the larger caller | exact clipped placement, metadata persistence, Worker parity, and no-far-write gates pass |
| `FS-05` | starter-content overlay and realized-instance identity | `waiting on FS-04 in Tactical 274` | conceptual identity accepted; Tactical 274 Slice 0 adds the codec seam and Slice 3 persists the selected plan | persisted overlay/blueprint/instance identity lands without changing pure Overworld output |
| `FS-06` | site survey, scoring, grading, water fallback, reservation, and safe arrival | `waiting on FS-04 in Tactical 274` | Revision 1 search bounds, score facts, full/compact tiers, fallback, and review gates are specified in Tactical 274 | arbitrary-seed corpus and Human Review R1 accept one showcase site before materialization |
| `FS-07` | first fixed farmhouse/barn/garden/pond/oak composition | `waiting on FS-04 through FS-06 in Tactical 274` | promoted cottage/barn families and a Lab-native coop are reusable; Tactical 274 bounds the first full and compact compositions | selected-site chunks render, persist, reopen, and retain player edits |
| `FS-08` | resident and player marker realization | `waiting on FS-07 in Tactical 274` | cow/chicken persistence and player respawn are proven separately; Tactical 274 owns settlement marker idempotence and arrival | no duplicate residents and safe first arrival/respawn across reopen and partial materialization |
| `FS-09` | cross-profile and topology adaptation | `deferred` | same engine contract accepted; first composition must land before breadth | Flat Grass, Small Island, Mclone, and explicit Overworld-overlay cases prove fit/fallback/rejection; admitted topology cases pass |
| `FS-10` | maximal parcels, church, outbuildings, richer animals, and visual mill | `deferred` | named destination and optional parcels are preserved | additions pass composition, persistence, and performance reviews without replacing the first hierarchy |
| `FS-11` | functional farm simulation and machinery | `deferred` | deliberately outside first visual/worldgen acceptance | shared gameplay contracts own crops, roles, power, inputs, and outputs |
| `FS-12` | settlement distant-presentation proxy | `deferred` | the removed chunk Far LOD omitted structures and edits | a future multiscale terrain architecture exists, then a shared presentation-only proxy passes its spatial handoff and coverage gates |

The `ready` rows are not an instruction to start all of them. They identify
work that can be scheduled without inventing a missing predecessor. The topic's
**Next Work** section below chooses the recommended farmstead-specific slice;
project priority still determines when upstream tacticals run.

### Tactical Completion Receipt

Every farmstead tactical should state which of these gates apply before work
begins and link the resulting evidence when it closes:

- **contract:** shared owner, persisted identity/version, input/output, bounds,
  seed domains, topology behavior, and explicit non-goals;
- **data correctness:** deterministic unit/fixture receipts, request-order and
  partition equivalence, exact touched chunks, and no far writes;
- **durability:** save/reopen, partial regeneration, player-edit precedence,
  and resident idempotence where relevant;
- **host parity:** affected native, dedicated, browser Worker, Android, and XR
  boundaries compile or run through the shared owner as required;
- **visual acceptance:** production-backed maps or rendered captures are
  inspected at the first drawable milestone and after meaningful expansion;
- **performance:** planning sample budget, chunk/status dependency footprint,
  piece count, grade volume, and startup/runtime cost are reported; and
- **documentation:** update this ledger, the owning subsystem topic, the
  tactical result, tactical index status, compatibility ledger when generator
  output changes, and the recommended next slice.

Use `Topic: starter-farmstead-settlement` on implementation commits for this
series. If a commit also implements an upstream concern, add that concern's
exact existing `Topic:` trailer rather than treating this topic as its owner.
Append the topic string to root `topics.md` when the first implementation commit
series actually begins, not for this documentation-only planning phase.

## Next Work

Active Tactical
[`274`](../tactical/274-playable-intro-homestead.md) is the selected next
product slice. The terrain campaign has reached a sufficient stopping point
for a first intro: current Mclone terrain, climates, coasts, rivers, wetlands,
and bounded streams can supply neutral survey facts, while authored pond or
runnel water remains an accepted fallback. Richer waterfalls, drainage, and
mill-compatible reaches are later quality work rather than prerequisites.

Tactical 274 deliberately starts with `FS-04`, the true cross-chunk structure
lifecycle. A tiny original cross-chunk canary must prove status metadata,
references, clipped per-chunk placement, persistence, Worker parity, and no far
writes before the farmstead becomes the larger caller. It then sequences:

1. separate persisted `intro-homestead-v1` overlay and realized-plan identity;
2. deterministic random-seed site scouting near provisional spawn, with full
   and compact tiers plus typed rejection;
3. Human Review R1 of candidate maps, grading, arrival, and a showcase seed
   selected by the same arbitrary-seed algorithm;
4. bounded terrain adaptation, reservation, buildings, authored water,
   planting, and one-time residents; and
5. safe arrival, save/reopen, player-edit precedence, and shareable
   desktop/browser product-flow acceptance.

Do not resume open-ended terrain representation, worlds-within-worlds, or
procedural-horizon expansion during this tactical unless the playable intro
exposes a specific correctness or presentation blocker. Do not make the
maximal settlement the structure-foundation canary.

## Staged Direction

### Stage 0 — research and readiness

- Tactical 274 owns the current readiness audit and baseline locks.
- Treat current original terrain and procedural-horizon behavior as sufficient
  foundations unless the intro exposes a concrete blocker.
- Repair the forced-Original capture path before relying on visual acceptance.
- Prove the true structure-lifecycle canary before composing the farmstead.
- Keep authored-water fallback and optional natural-hydrology facts explicit.
- Select a showcase seed through the accepted scout; do not hard-code
  seed-specific behavior or imply a release compatibility freeze.

### Stage 1 — basic fixed blueprint on a selected site

- Execute Tactical 274's structure canary, then consume that same shared
  start/reference/piece and template foundation.
- Build the production-backed random-seed site scout near provisional spawn,
  retain metric receipts, and select one showcase site through the same path.
- Author fixed `intro-homestead-full-v1` and compact fallback compositions
  with semantic roles, reservations, a focal tree, basic water, spawn, cows,
  and chickens.
- Keep starter-content identity separate from the base terrain profile and use
  Flat Grass as a simple cross-profile placement canary without changing pure
  reference Overworld output.
- Materialize and reopen it through shared persistence on every affected host.
- Iterate through rendered captures at each drawable milestone.

This stage can remain tied to one selected seed while proving an adaptable site
plan. It does not need recursive jigsaw assembly.

### Stage 2 — maximal composition growth

- Add the church, stable, outbuildings, multiple parcels, richer water and
  planting, real farm blocks/crops, and additional ready animals.
- Add a visual watermill when the scene has a credible authored or natural
  reach; do not gate it on functional machinery simulation.
- Add material themes, alternate template palettes where useful, ordered
  processors, marker breadth, and deliberate weathering/variation.
- Preserve named empty parcels so additions do not erase the original spatial
  hierarchy.

### Stage 3 — reusable settlement grammar

- Turn selected fixed piece roles into weighted pools.
- Use sockets, connector compatibility, fallbacks, and terminators.
- Add a small number of coherent layout variants without sacrificing arrival,
  hydrology, parcel, or grading constraints.
- Prove deterministic candidate search, fallback/rejection, and bounded
  adaptation on varied seeds and admitted Flat Grass, Small Island, Mclone,
  and explicit reference-Overworld-overlay worlds.
- Prove finite and periodic placement only as the selected base profile admits
  those topologies, including no wraparound self-overlap.

### Stage 4 — simulation and distant representation

- Add richer habitat/role behavior only through shared entity systems.
- Add horses and original ducks when their complete contracts are ready.
- Evaluate a settlement LOD proxy only after a shared multiscale terrain
  producer and renderer exist.
- Consider upgrade provenance only when a real shipped-world migration requires
  it.

## Remaining Validation Contract

Tacticals 208–210 and 214 prove only the standalone template, first detail
family, bounded building-family selection, source-first authoring/preview
pipeline, and persisted-gallery loop.
Settlement tacticals should additionally plan for:

- deterministic receipts for candidate scores, chosen anchor/rotation, grade
  volume, plan bounds, touched-chunk manifest, pieces, reservations, special
  features, hydrology capability/fallback, and markers;
- request-order, partition, native-thread, and browser-Worker equivalence;
- exact cross-chunk clipping and no far writes;
- cross-profile survey equivalence at the neutral contract, plus clean
  rejection when a composition cannot fit;
- finite-bound containment and periodic canonicalization/no-self-overlap when
  those topologies are admitted;
- structure start/reference and save/reopen fixtures;
- no duplicate residents after reopen, cache reset, or partial regeneration;
- spawn support/headroom/fluid validation;
- no generic-tree or large-feature intrusion into the reservation;
- water continuity and descending-stream checks;
- foundation, terrace, retaining-edge, and maximum-grade-budget checks;
- rendered aerial, arrival, ground-circulation, water, barn, church, and
  enclosure reviews as those pieces land; and
- explicit distant-presentation captures only when a real proxy exists.

Rendered-output review is part of authoring, not a final cosmetic check. Inspect
the first graded pad, then the first building, then circulation/water, and then
each composition expansion before moving on.

## Binding Decisions

- Use a terrain-adaptive blueprint, first polished against a selected seed.
- Preserve the maximal farmstead vision while allowing a small first
  composition.
- Keep the blueprint and site plan as source; treat prebuilt chunks as derived
  cache or ordinary played-world state.
- Use the true structure lifecycle for cross-chunk settlement content.
- Borrow vanilla structure nouns and processing lessons without importing its
  registry/codec framework or copying its village templates.
- Keep vanilla `palette` terminology narrow; use material theme and processors
  for broader style selection.
- Add a coherent settlement-level site plan beyond vanilla piece gravity.
- Suppress ordinary trees inside the designed envelope; place only intentional
  settlement trees and planting there.
- Represent residents as one-time persistent authoritative entities.
- Permit an authored first stream/pond before original macro rivers are ready.
- Continue original valleys and hydrology as terrain-owned systems, while
  exposing optional neutral water facts to site selection.
- Treat the farmstead as an explicit starter-content overlay orthogonal to the
  base profile and topology; do not modify pure reference Overworld parity.
- Reuse one survey/plan/template engine across profiles, with profile adapters,
  composition tiers, fallbacks, and honest rejection rather than a promise
  that the maximal plan fits everywhere.
- Keep the total settlement footprint separate from a small local processing
  halo; use exact touched-chunk metadata or a spatial index instead of growing
  the ordinary feature radius or globally extending vanilla's `+/-8` lookup.
- Provisionally keep the first maximal blueprint within eight chunks of its
  central start; allow a wider read-only scenic survey ring.
- Allow a visual watermill before animation or functional machinery, and keep
  gameplay mechanics outside world-generation ownership.
- Record the retired Far LOD omission honestly and defer a shared settlement
  proxy until a new multiscale terrain architecture exists.
- Start building art as complete authored stamps with controlled transforms,
  semantic themes, markers, and bounded optional modules; do not require final
  terrain or the full structure lifecycle for that standalone iteration.
- Require a dedicated tactical and fresh readiness audit before each larger
  settlement integration slice.

## Open Questions

- Which original terrain milestone is stable enough for the first meaningful
  seed scout and selected demo seed?
- Which semantic farm material roles should be content prerequisites versus
  temporary mappings?
- Does the first watercourse terminate in the pond, leave it, or adapt a later
  natural river/wetland field?
- What is the minimum hydrology sample required to classify a mill-compatible
  reach, and which waterfall visuals belong in the first natural-water slice?
- How large may cut/fill and the site reservation become before a candidate is
  rejected?
- What maximum piece count and touched-chunk footprint should the accepted full
  and compact intro compositions declare after the Tactical 274 survey?
- Should starter-content identity live directly in persisted world metadata or
  in a more general ordered structure/content-overlay list?
- Which compact composition tier is the intended Small Island fallback?
- Which residents belong in Stage 1 beyond cows and chickens if their shared
  implementations advance first?
- When distant-landmark work begins, should the proxy include only silhouettes
  or also the settlement's terrain/water delta?

## Related

- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`far-lod.md`](far-lod.md)
- [`../liquids.md`](../liquids.md)
- [`../structures.md`](../structures.md)
- [`../persistence-architecture.md`](../persistence-architecture.md)
- [`../entity-architecture.md`](../entity-architecture.md)
- [`../creatures.md`](../creatures.md)
- [`../lod-architecture.md`](../lod-architecture.md)
- [`../tactical/208-standalone-building-lab.md`](../tactical/208-standalone-building-lab.md)
- [`../tactical/209-building-charm-pass.md`](../tactical/209-building-charm-pass.md)
