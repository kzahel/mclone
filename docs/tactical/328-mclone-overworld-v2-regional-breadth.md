# Tactical 328: Mclone Overworld V2 Regional Breadth

Status: **authorized 2026-08-21. Human Review D selected revise while retaining
the continental catchment mechanism. Implementation is proceeding through
regional continuity, LOD presentation correction, a selectable persisted
`mclone-overworld-v2` profile, mesa-desert and humid-jungle vertical slices,
and live-game review. `mclone-overworld-v1` remains selectable and the default.**

Topic: `continental-ecoregion-planning`
Topic: `world-generation-profiles`
Topic: `procedural-horizon-clipmap`

## Instruction Synthesis

The continental catchment has started to cohere. Its tributary confluence and
river entrance into the lake are promising and should be retained. Human
review nevertheless finds visible joins between regional domains, a regular
crisscross or checkerboard pattern at broad LOD scales, and too much terrain
that reads as flat planes with similar trees. One temperate drainage proof is
not the desired breadth: Mclone needs strongly characterized province-scale
places such as a mesa desert and a dense humid jungle.

Do not wholesale replace the existing Mclone Overworld while the new
generator's visual and performance character is unresolved. Preserve
`mclone-overworld-v1` as a selectable profile and keep it as the new-world
default. Promote the detached continental candidate into a separate
experimental `mclone-overworld-v2` identity, exercise it through the real
shared game/server/persistence paths, compare its costs with V1, and stop at a
human review before changing either default or compatibility posture.

## Product Question

Can the continental system become a real selectable game world with smooth
regional joins, non-gridlike distant presentation, and at least three
recognizably different province-scale stories, while preserving V1 and
remaining bounded and responsive enough for streamed play?

## Review D Decision

Human Review D is **revise**.

Retain:

- bounded generative catchments and stable directed drainage;
- the tributary-confluence, trunk, lake-inlet, spill, and outlet vocabulary;
- one directly queryable source shared by exact chunks and procedural LOD;
- continental, province, ecoregion, and landscape-mosaic scales; and
- deterministic review-site selection rather than coordinate-specific
  generation branches.

Revise before production use:

- discrete owner joins that remain visible in shape, tint, material, cover,
  or subordinate feature layout;
- regular far-view grid, triangle, tile, or level-transition patterns;
- the narrow mostly temperate terrain and vegetation vocabulary;
- synchronous exact/clipmap settling costs; and
- the detached Explorer-only product boundary.

## Profile Decision

Allocate `mclone-overworld-v2` as a persisted, selectable sibling of
`mclone-overworld-v1`.

- V1 remains the default and retains its present generator path and visual
  regression evidence.
- V2 initially carries an experimental display label and may change under the
  internal-unshipped compatibility ledger.
- Each stored dimension owns one immutable profile identity during a session;
  persistence hits continue to win over generation.
- V2 uses the ordinary shared generator plan, scheduler, worker, biome,
  feature, lighting, persistence, terrain-view, scene, and platform contracts.
  No Explorer-only or app-local generator fork is permitted.
- Remote clients continue to consume authoritative chunks. They need no V2
  generator implementation unless they also host an integrated authority.
- Making V2 the default, retiring V1, or promising cross-build V2 world
  compatibility requires a later explicit decision.

## Three Initial Characteristic Domains

The first breadth proof contains three coherent province-scale stories. Their
initial spans are approximately 8-30 km, with internal ecoregions and
landscape mosaics rather than one homogeneous biome patch.

### Temperate Mountain Catchment

Retain and refine the Tactical 326 range, branching headwaters, tributary
confluence, trunk/floodplain, lake, varied shore, spill, outlet, forest,
meadow, wetland, and quiet-lowland vocabulary.

### Mesa Desert

Realize a warm dry leeward province as a causal bundle:

- broad caprock tables, asymmetric escarpments, mesas, buttes, and stepped
  benches rather than ordinary hills recolored with sand;
- dry washes descending toward alluvial fans, gravel plains, dune or sandy
  pockets, and occasional salt or closed-basin flats;
- exposed layered substrates and slope-dependent caprock/talus response;
- sparse clustered cover, open migration ground, shade/refuge opportunities,
  and ephemeral drainage semantics; and
- explicit ecotones to neighboring upland, savanna-like shoulder, riparian,
  and quiet regions.

### Humid Jungle

Realize a warm wet windward or basin province as another causal bundle:

- forested massifs, steep wet shoulders, sheltered lowlands, river galleries,
  and locally bounded wet clearings rather than flat land with more trees;
- dense multi-scale canopy with emergent crowns, understory/edge response,
  and intentionally limited sightlines in cores;
- high drainage permanence, wet soils, floodplain or marsh opportunities,
  and clear river dependence;
- coherent canopy gaps, disturbance history, refuge, and crossing semantics;
  and
- explicit ecotones to mixed forest, exposed upland, wetland, and quiet
  regions.

These are authored regional archetypes, not three new nearest-parameter biome
IDs. Their landform, water, climate, substrate, vegetation structure, and
ecology facts must agree. The architecture must permit later growth toward
roughly 12-18 strong archetypes without changing query or persistence shape.

## Ownership

- `mclone-worldgen` owns V2 regional plans, adjacency/ecotone facts, direct
  surfaces, exact lowering, biome/material/vegetation semantics, deterministic
  witnesses, and profile-owned generation work.
- `mclone-server` owns the stored V2 profile identity, authoritative
  generation dispatch, spawn/admission, and persistence behavior.
- `mclone-terrain-view` owns the shared V2 procedural source, mesh/normal/LOD
  presentation correction, exact composition, and bounded performance facts.
- `mclone-scene`, `mclone-app-runtime`, and `mclone-ui` own shared live-game
  orchestration and profile selection. Platform apps remain adapters.
- World Explorer and Terrain Lab own review selection and diagnostics only.

## Phases And Commit Gates

### Phase 0: Tactical And Baselines

- Record Human Review D as revise in the tactical and living topics.
- Allocate V2 as `planned-unallocated` in the compatibility safety ledger.
- Preserve V1 as the default and define side-by-side review requirements.
- Capture deterministic content-seam and far-LOD-pattern baselines with the
  narrowest existing native and browser harnesses.
- Commit the tactical before implementation.

### Phase 1: Regional Continuity And LOD Presentation

- Separate content seams from representation artifacts using exact-only,
  horizon-only, composed, owner/transition, normal, and coverage evidence.
- Make unlike neighboring regional owners publish one symmetric adjacency
  fact and one pair-owned ecotone. The same coordinate must not depend on
  which endpoint owner was selected first.
- Remove discontinuous subordinate feature salts, biome/tint steps, material
  steps, or cover layout at unintended owner joins. Preserve intentionally
  sharp geological boundaries only when typed and visibly authored.
- Prove bounded height and first-derivative behavior across sampled owner
  boundaries and point/window/cache-reset equivalence.
- Correct the regular far-view crisscross at its actual owner: sampling,
  tessellation, normals, tile joins, or clipmap-level transition. Do not hide
  it with added worldgen noise or weaken exact/LOD identity.
- Inspect pixels after the first content correction and first LOD correction.
- Commit each independently useful correction.

Implemented on 2026-08-21. The plan and surface broad fields now use periodic
gradient noise rather than rectangular value interpolation; the realized
surface also applies deterministic coordinate warps. Continental, province,
and ecoregion scalar facts fade symmetrically through neutral ownership bands,
while clearing candidates and local vegetation fingerprints remain stable in
absolute space instead of being re-seeded by the winning region ID.

The continental preview now has an explicit spacing-aware query. It preserves
spacing-one exact facts and progressively removes shore, local, walking, and
micro frequencies that cannot be represented by coarse clipmap lattices. A
65,536-block inspected map capture changed from regular diagonal grain to
smooth broad form without changing the clipmap's tile topology. The plan
witness is `525e7ccc5c375473c16dcca466fe3ca0437793dd3db0e2f3366a016fd43628e6`;
the exact surface witness is
`4948cda1470da098c05911ea0b5885e65788fcc53fe013db36e37d9683cf7309`.
The full 474-test `mclone-worldgen` suite exposed one pre-existing continental
proxy acacia footprint under the newly selected content; its bound now matches
the ordinary realizer's three-block branch reach, and the focused exact suite
passes.

### Phase 2: Selectable Persisted V2

- Add `mclone-overworld-v2` to the shared stored profile enum, stable binary
  tag, JSON label, local catalog, UI selection, startup descriptors,
  dedicated-server CLI, Web startup transport, and reopen validation.
- Route true missing chunks through the continental exact generator and its
  typed dependency cache. Preserve persisted chunks and session-immutable
  profile identity.
- Add V2 spawn selection and compatible biome/top-material lookup without
  borrowing V1 terrain facts.
- Expose the same V2 identity to Distant Terrain and exact composition in the
  live scene. V1 remains the default and unchanged.
- Prove native/Wasm descriptor round trips, worker dispatch, target partition,
  scheduler admission, SQLite/IndexedDB reopen, and exact/LOD source identity.
- Reach a real locally playable V2 world before adding the two new domains.
- Commit.

Implemented on 2026-08-21. The shared profile enum allocates JSON label
`mclone-overworld-v2`, stable binary tag 9, the ordinary Mclone season policy,
and target-only authoritative plans. Missing chunks run through a bounded
continental dependency cache while worker results publish only requested
chunks. The catalog displays V2 as experimental after V1 and leaves V1 as the
default. Spawn selection, biome lookup, SQLite reopen, native and Web startup
codecs, exact terrain compilation, proxy vegetation, scene admission, and the
procedural horizon all retain the V2 identity.

The first live capture exposed and then closed a source-classification defect:
vegetation workers treated every non-candidate profile as V1 and attempted to
unwrap a V1 forest cache for V2. Continental review and V2 identities now
share the stateless proxy source explicitly; the wire round trip and live
transport report zero failures. An inspected 1,280-by-720 native frame at seed
`12345` shows a playable V2 spawn with exact terrain and two composed horizon
levels. Focused profile, plan, spawn, worker, persistence, catalog, Web codec,
preview, terrain-view, and scene tests pass, as does the workspace check.

### Phase 3: Mesa Desert Vertical Slice

- Add stable regional archetype and formation identities, bounded influence,
  direct point/window queries, and one deterministic review journey.
- Realize caprock tables, escarpments, mesas/buttes, benches, washes, fans,
  basin flats, substrate layers, and sparse clustered vegetation through
  exact chunks and every LOD level.
- Derive arid habitat, shade/refuge, crossing, water-permanence, and open-range
  facts without implementing animal migration in this tactical.
- Prove exact/LOD agreement, owner-boundary continuity, negative coordinates,
  supported cylinder repetition, and live-game traversal.
- Capture and inspect exact, composed, broad horizon, and live-game pixels.
- Commit.

Implemented on 2026-08-21 through the shared continental surface, exact
generator, terrain preview, and deterministic journey contracts. Stable
8,192-block formation cells now realize bounded asymmetric tables and buttes
with caprock, upper and lower escarpments, benches, aprons, dry washes,
alluvial fans, basin flats, and dune pockets. One regional archetype and one
formation identity carry the same causal facts into material strata,
open-range, shade-refuge, crossing, and ephemeral-drainage semantics. Exact
columns lower red sand, red sandstone, and layered terracotta from the same
surface classification used by every procedural LOD level.

The first broad capture exposed a second representation defect: categorically
dithering red sand and coarse soil with a continuous 4,096-block field caused
different clipmap lattices to display large material bands. Mesa ground now
uses an authored transition-soil to red-sand sequence, reserving distinct
materials for typed landforms. A permanent 16,384-block cross-LOD scan proves
formation, archetype, and landform identity at 16, 64, 256, and 1,024-block
spacing. Inspected 16,384-block map, 8,192-block oblique, and 512-block
composed captures show one coherent red-sand province, raised mesa geometry,
and exact layered strata without the rejected categorical bands. The updated
surface-suite witness is
`46e0388f147e4efbbd8a190c1b621f97f374bcaf621e1a862f5626ceaeea448f`.
The shared V2 live path was already proven in Phase 2; the final packaged live
journey is part of Phase 5.

### Phase 4: Humid Jungle Vertical Slice

- Add stable humid-jungle regional and canopy identities with one
  deterministic review journey.
- Realize forested massifs/lowlands, wet drainage, river galleries, canopy
  cores, emergents, understory/edges, and bounded gaps through exact chunks
  and every LOD level.
- Derive wet refuge, crossing, canopy-cover, clearing, and drainage-permanence
  facts for later ecology consumers.
- Prove exact/LOD agreement, whole-tree ownership, owner-boundary continuity,
  negative coordinates, supported cylinder repetition, and live-game travel.
- Capture and inspect exact, composed, broad horizon, and live-game pixels.
- Commit.

Implemented on 2026-08-21 through the same shared continental surface,
exact-column, semantic-tree, canonical-mesh, and terrain-view paths as the
other V2 domains. Four continuous periodic-ready fields now shape an
8,192-block forested massif, wet shoulders and sheltered lowlands, a
2,048-block canopy, bounded 768-block gaps, and 192-block canopy clusters.
The surface publishes stable 4,096-block canopy identities plus river-gallery,
emergent, understory, wet-refuge, crossing, and permanent-drainage facts.
Humid relief quiets the temperate catchment's radial range signal instead of
stacking an unrelated mountain spine beneath the forest.

The first exact/LOD lowering looked like an ordinary brown forest plantation.
It was rejected: blanket podzol was removed, the ordinary tree proxy was not
reused, and a categorical jungle boundary no longer gates the denser canopy
lattice. A fourth `HumidJungleBroadleaf` family and `LayeredJungle` archetype
now share stable records across exact jungle logs/leaves and the mono,
per-eye, and multiview proxy shader. Normal crowns, tall emergents, low
understory, two bounded candidates per 24-block cell, cluster-modulated
density, and canopy-gap rejection produce one coherent but internally varied
forest. The semantic compiler is v3, `MCHV` transport is v2, canonical terrain
batches are v4, and family receipts cover all four families.

The deterministic seven-journey catalog now includes a 16,384-block humid
jungle traverse at seed `12345`, centered on `(10240, -54784)`. Direct tests
cover the causal surface bundle, cross-LOD identity through 1,024-block
spacing, negative coordinates, cylinder repetition, partition-independent
whole-tree records, dense canopy, emergents, understory, and exact jungle
voxels. Inspected 128-block exact/composed and 512-block composed captures
show the layered tree family and clustered canopy; a 16,384-block map and
oblique capture establish the province and drainage extent but intentionally
do not count as individual-crown evidence. The updated surface-suite witness
is
`36a9e648fa15002f2428136258e665e1f414eb80b8dbcaa846f40aa2dc6b0cf5`.
The final packaged live journey remains part of Phase 5.

### Phase 5: Performance, Cross-Platform, And Human Review E

- Compare V1 and V2 cold exact generation, dependency work, meshing,
  vegetation, clipmap refill, retained movement, memory, and presented-frame
  costs on the same host and view descriptors.
- Correct obvious repeated or synchronous work without making caches semantic.
  Do not claim parity where the evidence shows a material V2 cost.
- Run affected shared suites and native/Wasm boundaries; build flat Android
  and XR boundaries affected by the profile enum and shared source.
- Create/reopen V1 and V2 through the real catalog. Require V1 to remain the
  default and both profiles to remain selectable.
- Package sequential native, headed-browser, and live-game review artifacts
  outside the repository. Push and deploy the exact revision, verify public
  pixels, then stop at Human Review E.
- Commit.

Performance checkpoint on 2026-08-21: the new
`mclone-overworld-profile-performance-v1` release harness compares matched V1
and V2 surface, cold exact, warm exact, dependency, retained-memory, and block
work. It exposed an avoidable V2 adapter cost: a compact 5-by-5 target batch
was replaying nine cloned dependencies and one tree query separately for every
target. V2 now materializes the shared 7-by-7 region and vegetation query once
for a bounded compact batch. Single-target, batched, and reversed output are
byte-identical; sparse or excessively broad batches retain the bounded old
path.

At seed `12345`, radius two, and three release iterations, the temperate
25-chunk window measures V1/V2 cold exact at `134.01/132.57 ms`; V2 warm exact
remains materially slower at `34.63 ms` versus `14.73 ms`. At the dense jungle
window, V1/V2 cold exact is `114.12/151.31 ms` and warm exact is
`15.30/38.84 ms`; V2 produces roughly twice as many non-air blocks there.
This is bounded and playable, but is not performance parity.

A matched live startup run isolates the larger remaining gap to the
procedural horizon. With Distant Terrain Off, V1/V2 both reach the full exact
view in about `1.385 s`, average about `2.4 ms`, and remain near `4 ms` p99.
With High enabled, V1/V2 full-view readiness is `1.393/1.968 s` and initial
render completion is `1.474/3.142 s`; V2 has 17 large terrain-product
admission/render frames and `298.50 ms` p99 versus V1's `10.36 ms`. V2 still
uses the CPU-authored continental source where V1 has a GPU-native evaluator.

World Explorer's earlier all-tree policy also enumerated full dense forests
through spacing 16. Stable tree identity is now complete through spacing 4,
half-sampled by stable rank at spacing 8, and one-eighth sampled at spacing
16 before coarse summaries take over. The reviewed jungle frame drops from
`42,637` to `10,889` instances, from `4,093,152` to `1,045,344` vegetation
bytes, and from `195.21` to `42.11 ms` vegetation compilation while retaining
a continuous visible forest. Cold debug target readiness remains about seven
seconds because CPU continental terrain compilation, not vegetation, now
dominates. Retained movement and platform/package evidence follow in this
phase.

## Automated Acceptance

- V1 profile label, tag, generator, default status, fixtures, and ordinary
  missing-chunk output remain unchanged.
- V2 has one stable stored identity across shared Rust, native, Web,
  dedicated, Android, and XR boundaries.
- Exact V2 chunks and procedural V2 samples agree on quantized height, water,
  biome, top material, stable whole vegetation, and regional identities.
- Unlike owner joins have symmetric adjacency identities and bounded ecotone
  widths; reordered queries and either-side approaches agree.
- Sampled unintended joins satisfy bounded height and slope continuity.
- Exact-only output contains no candidate-domain step; horizon-only output
  contains no regular checkerboard, diagonal, tile, or ring pattern accepted
  merely as coarse terrain.
- Mesa and jungle review domains each cover province-scale extents, have
  multiple internal landscape stories, and remain distinguishable without a
  diagnostic overlay.
- Cold teleport and retained movement perform bounded work independent of
  travel distance. Caches change cost only.
- V2 worlds create, play, save, close, and reopen through ordinary authority
  paths while V1 remains available and default.

## Human Review E

Review V1 beside the three V2 characteristic domains in exact, composed, and
live-game views. The decision is:

1. **accept** V2 as a continuing selectable experimental world and authorize
   the next regional-archetype campaign;
2. **revise** named seam, LOD, terrain, water, vegetation, performance, or
   live-game behavior; or
3. **reject** V2 promotion while retaining independently useful catchment,
   adjacency, profile, or representation mechanisms.

Acceptance does not make V2 the default or retire V1.

## Non-Goals

This tactical does not:

- change the new-world default from V1;
- retire, alias, or rewrite V1 worlds;
- promise release-frozen V2 output or migrate external saves;
- implement every intended regional archetype;
- implement unloaded migration, population simulation, seasons, structures,
  caves, full geology, sediment erosion, or dynamic floods;
- solve arbitrary volumetric distant terrain; or
- accept regular visual artifacts because metrics or exact/LOD hashes pass.

## Related

- [`326-continental-catchment-and-landform-realization.md`](326-continental-catchment-and-landform-realization.md)
- [`../topics/continental-ecoregion-planning.md`](../topics/continental-ecoregion-planning.md)
- [`../topics/continental-hydrography.md`](../topics/continental-hydrography.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/lod.md`](../topics/lod.md)
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
