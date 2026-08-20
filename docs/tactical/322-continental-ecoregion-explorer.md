# Tactical 322: Continental Ecoregion Explorer

Status: **implementation active 2026-08-20. Phases 1-3 are implemented: the
disconnected shared Rust plan, bounded direct queries, typed temperate
grammar, native/Wasm canonical corpus, fixed-cost Terrain Lab atlas, initial
distribution and journey metrics, and desktop/phone browser evidence now
pass. Pixel review rejected the first cellular composition and produced the
larger-scale Revision 2 candidate. Same-coordinate 65 km and 131 km atlases
now include a direct current-production field-revision-21 control. Sampled
transition, clearing, recurrence, and habitat-connectivity distributions are
also complete. Decomposed land/ocean, climate, biome, forest-openness, height,
and water controls now pass and have been pixel-reviewed. Human Review A
first selected “revise” on 2026-08-20: retain the continental, province,
ecoregion, and clearing candidate. Both named corrections are implemented:
typed, adjacency-aware shoulders replace the generic transition band, and
stable typed spine-and-branch routes replace the single continental corridor
stripe.
The corrected atlas returned to Human Review A and was accepted on 2026-08-20.
The reviewer found the wooded route geometry visibly inorganic and broad-map
panning too slow. Those are required presentation/realization follow-ups, not
a rejection of the macro grammar. The current production generator remains a
visible control, not a protected output target.**

Topic: `continental-ecoregion-planning`

## Instruction Synthesis

Mclone Overworld is still too samey. It can look plausible from kilometre-high
LOD views while failing to contain memorable large places: continents that
organize a journey, broad clearings and open country, desert provinces,
distinct forest interiors and edges, connected wetlands, and enough habitat
extent for land-animal populations and migration.

Take a genuinely top-down and bottom-up approach. Establish a coarse authored
regional plan before local noise realizes it, keep broad previews fast without
generating exact chunks, and support an unbounded plane plus optional small or
eventually continental-scale cylinders. Start in the terrain exploration
tools, but do not preserve Mclone Overworld field revision 21 merely because it
exists. The previous multiscale experiments were technically useful and
visually slightly disappointing; reuse their invariants where helpful rather
than promoting their terrain shapes or making LOD the design objective.

## Product Question

Can a small authored regional grammar create recognizable, causally related
places over 65-131 km domains while remaining deterministic, directly
queryable, cheap enough for an interactive atlas, and suitable for later exact
terrain and ecology?

The first candidate succeeds only if a reviewer can describe and distinguish
whole places. More colors, more biome names, or smoother noise are not enough.

## Objective

Build one Rust-owned `ContinentalEcoregionPlan` candidate and expose it in:

1. Terrain Lab as a fast, pannable plan atlas with a current-production
   control, independent semantic layers, distributions, and journey evidence;
2. World Explorer as an explicit candidate terrain source only after Human
   Review A accepts the atlas grammar; and
3. a small exact-site comparison only after the broad three-dimensional
   candidate survives Human Review B.

The tactical ends with an evidence-backed promote, revise, or reject decision.
Production integration may be substantial, but it requires a focused follow-up
slice after this candidate has earned it.

## Non-Goals

This tactical does not:

- implement animal migration or unloaded aggregate populations;
- replace existing durable visible-animal semantics;
- design every climate zone or ship a final biome catalog;
- generate every exact chunk in a broad atlas window;
- persist a complete infinite-world plan or introduce a mandatory plan cache;
- make camera distance, LOD residency, request order, or exploration history
  generation inputs;
- promote Tactical 272/273 range and basin geometry;
- add universal three-dimensional density, caves, structures, or settlements;
- select a new default cylinder period; or
- mutate production terrain before the plan and broad realization are
  reviewable.

## Shared Ownership

- `mclone-worldgen` owns descriptors, canonical identities, authored regional
  grammar, direct point/window queries, topology handling, summaries, metrics,
  exact invariants, and candidate surface realization.
- `mclone-terrain-lab` owns only Wasm serialization and tool-session adapters.
- Terrain Lab TypeScript owns Worker transport, canvas presentation, controls,
  and labels. It may not choose geography or reconstruct semantic facts.
- `mclone-terrain-view` consumes accepted candidate surface facts for broad 3D
  review. It does not own continents, ecoregions, clearings, or habitat.
- World Explorer apps select a shared source and host the view. They do not
  lower the plan independently by platform.
- Production exact chunks remain disconnected until the review gates below
  authorize a focused integration slice.

Native and `wasm32-unknown-unknown` must execute the same worldgen code and
produce the same canonical semantic receipt. A browser fallback may run
inline behind the same adapter while the existing Worker lifecycle remains the
target execution model.

## Starting Control

Preserve `mclone-overworld-v1` field revision 21 as the same-seed visual and
metric control. This is an A/B reference, not a compatibility promise. Record:

- land/ocean, climate, biome, openness, height, and water maps at 65,536 and
  131,072-block extents;
- connected-component size for current biome and open-cover classifications;
- 10, 25, and 50 km journey sequences;
- repeated-scene and short-dwell alarms; and
- cold and warm preview cost with exact-chunk work counted separately.

The control must continue to render through its existing implementation. Do
not duplicate production worldgen inside the candidate.

## Implementation Evidence

### Phases 1-2: shared plan and exact harness

Landed on 2026-08-20 in shared `mclone-worldgen` ownership:

- `continental_ecoregion` owns the revisioned descriptor, topology
  compatibility, typed continent/province/ecoregion/mosaic facts, authored
  temperate grammar, explicit clearing plans, direct point queries, bounded
  windows, construction counts, and semantic checksums;
- the production generator imports none of the candidate facts and retains
  field revision 21 unchanged;
- the 6,144-block proof cylinder fails explicitly as too small, while the
  corpus proves exact lifts on an aligned 196,608-block cylinder;
- all point/site coordinate construction remains safe at the complete signed
  `i32` world-coordinate boundary; and
- `mclone_continental_ecoregion` writes a machine-readable native receipt to
  `/tmp/mclone-continental-ecoregion/receipt.json` by default.

The pinned native/Wasm witness is
`cd1a8638630df83d4fc5b9f642da9e9dbbbdd06fd1169558a98e9621cf2ef925`.
It covers three seeds on plane and cylinder through 11,301 exact comparisons:
whole versus split windows, randomized point traversal, direct coarse
projection, owner/work caps, zero exact-chunk work, periodic lifts, and four
independent native threads. The dedicated Wasm test produces the same
witness through `wasm-bindgen-test-runner`.

The complete Review A macOS `release` receipt at
`/tmp/mclone-continental-ecoregion/receipt-v8.json` measured the corrected
plan:

| Query | Observed time |
|---|---:|
| continental point | 296 ns/sample |
| province point | 430 ns/sample |
| ecoregion point | 524 ns/sample |
| mosaic point | 581 ns/sample |
| 65,536-block, 256x256 candidate plan | 68.1 ms total / 1,039 ns per sample |
| 131,072-block, 256x256 candidate plan | 56.8 ms total / 867 ns per sample |
| 65,536-block candidate + complete control, cold / warm | 192.5 / 192.8 ms |
| 131,072-block candidate + complete control, cold / warm | 189.0 / 188.3 ms |

These timings are descriptive, not yet a budget, and exclude canvas drawing.
The equal 256x256 cost at the two extents demonstrates direct coarse sampling:
the 131 km atlas changes sample spacing rather than generating a larger hidden
fine plan. Both the candidate and control report zero exact-chunk work. Cold
and warm receipts pin identical candidate/control checksums and work; there is
no cache whose warmth can change geography. The
same candidate windows contain 58,159 land / 7,377 ocean and 42,736 land /
22,800 ocean samples respectively, so this receipt exercises actual
land-ocean organization rather than an all-land regional palette.

### Phase 3: direct Terrain Lab atlas

The shared `continental_ecoregion_atlas` compiler now publishes one flat,
typed, fixed-resolution atlas for both native and Wasm consumers. Terrain Lab
runs it in a dedicated Worker and can paint land/ocean, province, ecoregion,
transition, openness, clearings, water, habitat, and composed layers without
generating exact chunks. The browser exposes stable IDs under the pointer and
reports component distributions, typed counts, ecoregion adjacencies, quiet
space, six journey receipts, work counts, and semantic checksums. Layer
changes repaint the same arrays and preserve the checksum.

The first drawable candidate failed visual review: 4,096-block ecoregion
owners, 2,048-block clearing owners, and canopy suppression at every owner
edge produced a field of similarly sized cells and scattered dots. That is
the same failure class this tactical exists to prevent. Revision 2 therefore:

- uses 8,192-block ecoregion owners and 24-32 km climate context;
- selects compatible ecoregion roles from province plus regional climate;
- uses 4,096-block clearing owners with sparse 0.9-3.8 km elliptical reach;
- preserves forest structure through an ecotone instead of erasing canopy at
  every owner boundary; and
- advances the plan schema to `mclone-continental-ecoregion-plan-v2`.

The inspected Revision 2 captures are:

- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-65km.png`;
- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-provinces.png`;
- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-clearings.png`; and
- `/tmp/mclone-terrain-lab-phone-chrome-ecoregion-ui.png`.

Focused Playwright acceptance passes on desktop Chrome and Pixel 7 emulation.
It verifies the pinned witness, zero exact chunks, production-disconnected
status, typed inspection, layer-only redraws, deterministic reload, and a
bounded 256-sample horizontal resolution. Rust ownership tests forbid browser
code from gaining continental, ecoregion, or clearing-owner policy.

### Paired field-revision-21 control

The atlas now calls the existing `McloneOverworldSampler` at exactly the same
seed, coordinates, and sample spacing as the candidate. Shared Rust publishes
signed continentalness, surface height, temperature, moisture, relief,
ruggedness, water, and the ordinary production biome recipe. It also publishes
a separate checksum, land/ocean and component distributions, journey runs,
and work counts. A focused equivalence test locks those arrays to the existing
production terrain-preview reference grid; this is a consumer of production
worldgen, not a browser reconstruction or a second generator.

The control remains `mclone-overworld-v1-fields-21` on the unbounded plane
when the candidate switches topology. Both sides compile 65,536 direct samples
and zero exact chunks. At 131,072 blocks, the candidate has 101 connected
ecoregion-kind components while current production has 15,963 connected
biome-kind components. At 65,536 blocks the corresponding counts are 51 and
9,417. The counts are not a quality score, but they quantify the visible
failure motivating this work: production is dominated by small recurring
patches rather than region-scale ecological identities.

The inspected matched 131 km captures are:

- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-131km.png`; and
- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-control-131km.png`.

The control UI capture is
`/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-control-131km-ui.png`.
Focused browser acceptance proves that selecting the control does not rebuild
or mutate the candidate receipt, labels its independent topology and revision,
keeps exact-chunk work at zero, and exercises both 65 km and 131 km review
extents.

### Review distributions and habitat graph

Atlas schema `mclone-continental-ecoregion-atlas-v3` closes the remaining
initial distribution gaps without adding browser-owned geography:

- transition width is the smaller horizontal/vertical sampled span through
  every transition-band sample;
- clearing area, boundary length, and center-to-center isolation aggregate by
  stable planned clearing identity rather than by canvas color;
- regional recurrence measures the nearest distinct ecoregion-instance center
  with the same continent-story, province-kind, and ecoregion-kind signature;
  and
- habitat patches are typed open, forest, or wetland components, while
  connected corridor components supply explicit graph edges between patches.

Every quantity is descriptive at the receipt's declared sample spacing. It is
not exact polygon geometry, an animal migration simulation, or an automated
quality score. The square 131 km native receipt reports a 4,096-block median
and 19,968-block p90 transition span; 140 clearings with 2.36 km² median area,
4,864-block median isolation, and 7,168-block median edge length; and a
4,220-block median / 7,210-block p90 regional recurrence distance. Its habitat
graph has 374 typed patches, 787 links, a largest connected network of 58
patches, and only 27.0% of patches linked to another patch.

The initial desktop-aspect 131 km browser evidence independently reported 113
planned clearings, 4.1 km median transition width, 4.2 km median recurrence,
and 24.4% connected habitat patches. That low connected fraction selected the
route-network correction; it was not a passing mark hidden by the broad
union-mask component count.

### Decomposed production control and Review A handoff

Production forest openness now comes from the same shared production-preview
forest helper used by Terrain Lab's ordinary cover view. The equivalence test
compares the atlas against that existing reference grid, including forest
coverage. At each of the fixed 65,536 samples the complete control performs
five field samples, four coarse forest-intent samples, one footprint summary,
and zero exact chunks. Layer changes only redraw the transferred arrays and do
not compile the candidate or control again.

The inspected 131 km production maps are:

- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-land-131km.png`;
- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-climate-131km.png`;
- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-biome-131km.png`;
- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-openness-131km.png`;
- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-height-131km.png`;
  and
- `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-water-131km.png`.

The composed 65 km control is
`/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-control-65km.png`.
The 131 km browser evidence compiled candidate plus complete production control
in 215 ms and redrew the selected layer in 10 ms. Current production contains
1,050 and 2,647 connected open-cover components in the square 65 km and 131 km
receipts respectively. Its decomposed maps show that the same fine recurring
fabric is present in land/ocean, climate, biome, cover, height, and water; the
control's sameyness is not merely a composite-palette artifact.

Human Review A should now choose one of the tactical's three outcomes. The
review should consider both the candidate's legible continents, provinces,
large clearings, forest/open structure, and quiet areas and its current weak
habitat connectivity and broad p90 transition spans. Do not add an arid
province or begin World Explorer realization until that decision is recorded.

### Human Review A decision: revise

The reviewer retained the general Revision 2 composition and selected the
recommended named revision rather than accepting or rejecting the grammar:

1. replace the generic nearest-owner transition shoulder with typed,
   adjacency-aware ecotone widths and continuous cover/climate blending; and
2. replace the one warped continent-axis corridor with stable, typed route
   identities forming a bounded spine-and-branch habitat network.

Return to Review A with the same 65 km and 131 km maps, transition
distributions, habitat graph, production control, fixed-cost receipt, and
browser evidence. Do not tune an acceptance threshold into the generator:
the corrected maps and graph remain evidence for a human decision.

Plan schema `mclone-continental-ecoregion-plan-v3` implements the first
correction. Every ecoregion sample names its unlike transition peer and the
adjacency rule's complete ecotone width. Same-kind owner boundaries no longer
paint a false transition. Unlike neighbors receive an authored 0.8-2.6 km
width, and cover plus climate interpolate continuously to their shared
boundary. Atlas schema `mclone-continental-ecoregion-atlas-v4` transfers those
facts directly instead of estimating width from horizontal and vertical
raster runs.

The inspected desktop-aspect 131 km evidence now reports a 1.8 km median and
2.2 km p90 authored transition width. The transition map is
`/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-transitions-131km.png`; the
matched composed map and evidence card retain their existing `/tmp` paths.
The fixed atlas still requests 50,944 direct samples at 512-block spacing and
constructs zero exact chunks. Habitat connectivity remains intentionally
unchanged in this first correction.

Plan schema `mclone-continental-ecoregion-plan-v4` completes the second
correction. Every continental story now owns a typed riparian spine, wetland
chain, woodland pass, or open-range link plus three bounded cross-links. Route
IDs derive from the stable continent identity and slot, and the scalar
corridor influence is now a realization of those facts rather than their only
identity. Atlas schema `mclone-continental-ecoregion-atlas-v5` transfers route
kind and ID alongside its influence.

In the inspected desktop-aspect 131 km atlas, the unchanged habitat graph rule
now observes 217 typed habitat patches and 2,791 route-mediated links. The
largest network contains 97 patches, and 44.7% of patches touch at least one
other patch, up from roughly one quarter under the single stripe. This still
does not prove animal migration or set an acceptance threshold. The current
evidence card is
`/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-metrics-131km.png`, and the
typed route map is
`/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-habitat-routes-131km.png`.

### Human Review A decision: accept macro grammar

The reviewer accepted the corrected continental, province, ecoregion,
clearing, and transition composition. This opens arid-contrast and World
Explorer work, but records two corrections before the plan should become an
ecology or production authority:

1. keep panning responsive by moving the retained map immediately, coalescing
   full rebuilds, and adding bounded overlap reuse where measurement justifies
   it; and
2. preserve typed route identities while replacing the visible regular
   spine-and-cross-link ladder with anchor-responsive, variably wide,
   interrupted route geometry.

The first is a representation/cache concern: it must not change geography.
The second is a realization correction: route identity and connectivity stay
canonical while forest cores, clearings, passes, wetland opportunity, and
continental story shape their visible course. Neither correction authorizes a
migration simulator or a production-profile switch.

The interaction correction is implemented without a semantic cache. During
pan and zoom, Terrain Lab immediately translates/scales the last accepted
raster in world coordinates and keeps its exact receipt visible. Candidate
plus production-control reconstruction waits for a 100 ms interaction-settle
window; intermediate view changes replace one pending request rather than
forming a queue. Raster pixels are rebuilt only when the accepted semantic
checksum or selected layer changes.

Focused browser acceptance held an eight-step 131 km pan open for 200 ms. The
canvas changed on the retained frame while the exact checksum stayed fixed,
then one settled Worker result replaced it and cleared the retained-frame
offset. The inspected in-motion capture is
`/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-retained-pan.png`. A full
atlas still recompiles after settling; bounded tile/overlap reuse remains an
optional measured optimization rather than a correctness dependency.

## Candidate Plan Revision 2

Revision 2 is deliberately a concrete authored grammar, not a generic planning
framework. It uses a finite hierarchy and bounded coordinate-pure queries:

| Level | Initial span | Revision 2 responsibility |
|---|---:|---|
| continental district | 32-96 km | land/ocean body, broad relief and climate exposure |
| physiographic province | 8-24 km | upland, basin, lowland, rain-shadow, and major water relation |
| ecoregion instance | 4-12 km | dominant ecological identity and compatible signatures |
| landscape mosaic | 512 m-4 km | clearing, forest core/edge, wetland, disturbance, and corridor patches |
| local realization | 8-256 m | continuous relief and cover irregularity without changing plan identity |

These are review hypotheses, not persisted public constants. Changing scale,
owner layout, feature reach, grammar tables, identity inputs, or topology
rules changes the candidate revision and its pinned receipts.

### Authored grammar

The first complete grammar is one temperate forest/open-land province with at
least these compatible ecoregion roles:

- old forest core;
- broad meadow or lightly wooded open country;
- riparian woodland and connected wetland;
- rolling mixed woodland/agricultural-looking grassland without actual farms;
- exposed upland or rocky ridge; and
- quiet transition country.

Each province instance receives one deterministic regional fingerprint that
chooses a dominant story, orientation, openness balance, wetness relation,
signature permissions, and repetition budget from a curated envelope. It may
not freely combine every parameter. Direct adjacency rules prevent implausible
endpoint contact and assign explicit transition shoulders.

The mosaic layer plans large clearings positively. Each clearing records a
stable ID, cause family, bounds, core, shoulder, edge relation, succession
state, and links to nearby cover or water. Forest cores, wetlands, and habitat
corridors use the same bounded-query infrastructure but retain distinct typed
facts and realization rules.

After Human Review A accepts the temperate grammar, add one arid
rain-shadow/desert province as the first strong contrast. It must alter relief,
drainage permanence, surface opportunity, and vegetation structure together;
it may not be a sand-color threshold over temperate hills.

### Query shape

A point query returns one typed `LandscapePlanSample` containing stable IDs
and continuous weights for:

- continental district and land/ocean intent;
- physiographic province and relief family;
- ecoregion core and transition shoulder;
- openness and canopy structure;
- clearing, forest-core, wetland, and corridor membership;
- climate normals and exposure relation;
- major-water opportunity; and
- a bounded local-realization seed/fingerprint.

A window query batches the same point contract and may return typed feature
outlines and summaries. It must enumerate only the owners intersecting the
window plus declared influence halos. A continental summary query must stop
without constructing mosaic or local children. Missing bounds or exceeded
caps fail explicitly rather than broadening work silently.

### Identity and topology

Every semantic identity derives only from:

```text
stored profile and candidate revision
world seed
dimension
topology and world-scale descriptor
planning level and canonical owner
typed feature family and stable child slot
```

The request window, viewport, camera, LOD level, thread, Worker, cache,
completion order, and exploration path are excluded.

Revision 2 supports the unbounded plane and an aligned 196,608-block
periodic-X candidate. It handles the existing 6,144-block periodic-X proof
explicitly by reporting that it is too small for this grammar rather than
silently compressing or self-overlapping a continent. No semantic primitive
may accidentally self-overlap across a supported periodic seam.

## Exact Validation

Before pixels, prove on native and Wasm:

- repeated point and window queries produce one pinned canonical receipt;
- point facts equal the corresponding window samples;
- forward, reverse, randomized, tiled, and differently partitioned queries
  agree exactly;
- cold, warm, bounded-eviction, and no-cache execution agree exactly;
- serial and parallel native execution agree;
- equivalent periodic lifts agree where the descriptor declares support;
- every typed feature has one resolvable owner and stable identity;
- direct continental and province queries report zero mosaic/local work;
- owner, influence, output, and retained-byte caps are never exceeded; and
- existing field-revision-21 fingerprints remain unchanged by the disconnected
  candidate.

The canonical receipt excludes timings but includes descriptor bytes, owner
counts, fact counts, semantic checksums, cap usage, and construction counts by
level.

## Terrain Lab Atlas

Add one synchronized `Continental ecoregions` view to the existing Terrain Lab
navigation and Worker topology. It must support 65 km and 131 km extents from
the first reviewable build and expose these independently addressable layers:

- land/ocean and inland distance;
- physiographic province and dominant relief family;
- ecoregion identity, core, and transition shoulder;
- vegetation openness and forest core/edge;
- clearing plans with cause and succession family;
- major water, wetland, and riparian relation;
- habitat patches and corridor connectivity; and
- composed plan.

Include current-production, plan-only, and candidate-realization modes. The
atlas must label semantic identities under the cursor and report:

- component-size distributions by typed layer;
- adjacency pairs and transition-width distributions;
- clearing size, isolation, and edge-length distributions;
- quiet-space fraction and regional-signature recurrence distance;
- habitat-patch and corridor graph connectivity;
- 10-50 km journey runs with dwell lengths and repeated-scene alarms;
- owner/fact construction counts;
- cold, warm, and batched query time; and
- exact-chunk, fine-child, and cache work performed.

Changing overlays, zoom, or presentation must not change semantic checksums.
Save screenshots and machine-readable review receipts under `/tmp`, never in
the repository.

## Human Review A: Plan Grammar

Pause candidate expansion after the atlas and exact evidence are complete.
Review same-seed 65 km and 131 km maps plus at least six 10-50 km journeys.

Accept only if the reviewer can identify multiple coherent places, broad
clearings/open country, forest interiors and edges, connected water/wetland
country, quiet space, and transitions that look authored rather than randomly
thresholded. The maps must also leave obvious room for large-animal ranges and
future routes.

The decision is one of:

1. accept the temperate grammar and add the arid contrast;
2. revise named grammar, scale, adjacency, or mosaic rules; or
3. reject the candidate while preserving its exact-query evidence.

Do not infer acceptance from automated metrics or attractive colors.

## World Explorer Realization

After Human Review A, expose the accepted plan as a named candidate terrain
source through the shared terrain representation path. Realization must:

- derive broad height envelopes, water opportunity, substrate family, and
  vegetation structure from plan facts;
- use local fields only to realize or irregularize accepted semantics;
- keep continent, province, ecoregion, clearing, forest, water, and corridor
  identity stable across broad and close views;
- provide a direct coarse surface query without exact chunks;
- preserve one source identity across native/browser, mono/stereo/multiview,
  terrain, water, and vegetation consumers; and
- keep current production selectable beside the candidate.

Capture matched overview, oblique, horizon, and fly-through frames at ordinary
and signature regions. Do not tune only a showcase coordinate. Record broad
query, terrain compilation, retained-memory, upload, and completed-frame cost
separately; LOD remains a downstream budget rather than an author of terrain.

## Human Review B: Broad Three-Dimensional Geography

Review at least:

- one broad clearing between forest cores;
- one long forest-edge journey;
- one connected riparian/wetland sequence;
- one quiet ordinary region;
- one strong upland or basin transition; and
- after Review A permits it, one arid rain-shadow/desert province.

Accept only if the candidate remains recognizable from map to oblique view,
does not collapse into one noisy heightfield when approached, and offers
meaningfully different travel stories. Reject a result whose variety is mainly
palette, whose terrain ignores its plan, or whose broad preview requires exact
chunk generation.

## Exact-Site Probe And Human Review C

After Human Review B, route a small bounded set of candidate sites through the
ordinary exact surface/decorated generation boundary without making it the
new-world default. Compare map, broad surface, and exact blocks for semantic
agreement at ordinary as well as signature locations. Include vegetation,
water, habitat-query, and clearing-edge facts, but continue to defer migration,
structures, and universal density.

Human Review C decides whether to open a production-integration tactical. That
follow-up may intentionally revise `mclone-overworld-v1`, fixtures, disposable
worlds, spawn, ecology inputs, and downstream summaries together.

## Performance Evidence

Report descriptive evidence before choosing budgets:

1. direct point samples for each planning level;
2. 65,536 and 131,072-block atlas compilation at representative resolutions;
3. later 524,288-block compilation only after direct coarse work is proven;
4. cold, warm, batched, and bounded-eviction plan queries;
5. native and browser/Wasm query and presentation time;
6. broad World Explorer terrain generation and completed-frame time;
7. retained canonical-plan and optional-cache bytes; and
8. exact chunks, local facts, and feature owners constructed per operation.

Fast pixels are not sufficient if a broad query secretly constructs fine
children. A rich plan is not sufficient if it cannot be reviewed
interactively.

## Commit And Documentation Sequence

Land coherent, reviewable milestones rather than one large speculative change:

1. shared descriptors, typed plan facts, direct queries, and invariant tests;
2. native/Wasm canonical receipt and performance harness;
3. Terrain Lab atlas, control, metrics, and inspected browser pixels;
4. Review A corrections and optional arid contrast;
5. shared World Explorer candidate realization and native/browser pixels;
6. Review B corrections and bounded exact-site probe; and
7. tactical/topic status, evidence, and promotion decision.

Each implementation commit reuses `Topic: continental-ecoregion-planning`.

## Acceptance

- one Rust implementation owns every geographic decision;
- native/Wasm and all traversal/cache/partition variants agree exactly;
- plane and declared periodic topology behavior is explicit;
- direct coarse queries perform no hidden fine or exact work;
- 65 km and 131 km atlases remain interactive and inspectable;
- the same-seed production control stays visible;
- broad clearings, forest cores/edges, wetlands, corridors, transitions, and
  quiet spaces are typed plan facts rather than incidental thresholds;
- component, adjacency, journey, recurrence, habitat-connectivity, cost, and
  cap receipts accompany visual review;
- the accepted plan remains recognizable in World Explorer without making LOD
  canonical geography;
- current production remains unchanged until its own gated integration; and
- Human Reviews A-C record promote, revise, or reject decisions rather than
  being inferred from automated success.

## Related

- [`../topics/continental-ecoregion-planning.md`](../topics/continental-ecoregion-planning.md)
- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/habitat-driven-creature-ecology.md`](../topics/habitat-driven-creature-ecology.md)
- [`../topics/deterministic-streamed-landscape-planning.md`](../topics/deterministic-streamed-landscape-planning.md)
- [`../topics/multiscale-terrain-representation.md`](../topics/multiscale-terrain-representation.md)
- [`272-multiscale-semantic-refinement-witness.md`](272-multiscale-semantic-refinement-witness.md)
- [`273-semantic-terrain-reconstruction-sandbox.md`](273-semantic-terrain-reconstruction-sandbox.md)
