# LOD-Native Vegetation

Topic: `lod-native-vegetation`

Status: accepted direction 2026-07-24; implementation has not started.

## Scope

This topic owns the immediate migration of natural trees and forest shape in
`mclone-overworld-v1` from vanilla-shaped chunk decoration to a
worldgen-owned semantic plan that can serve:

- exact canonical chunk generation;
- Terrain Lab CPU and GPU LOD panes;
- future in-game Far LOD;
- overview maps, minimaps, and tabletop views; and
- later natural-feature overlays that need stable identity across detail
  levels.

The first implementation is deliberately about trees and forests. Ordinary
grass, flowers, crops, player-planted trees, tree growth, falling-tree
simulation, structures, and general-purpose natural-feature serialization are
separate concerns.

This migration applies only to the original Mclone profile. The
reference-locked Java 1.17.1 `overworld` must retain its exact vanilla feature
placement and random-order semantics. Alpha, Beta, Small Island, and other
profiles do not acquire this system merely because they also place trees.

## Originating Direction

Terrain Lab can now explore essentially unbounded terrain quickly, including
production macro fields, hydrology, surface language, and bounded planned
streams. Its broad LOD panes still omit the most recognizable above-ground
features. Exact Mclone terrain contains oak, pine/spruce, and acacia trees, but
their positions and shapes are decided by the vanilla-shaped placed-feature
and decorator path only after full chunk dependencies exist.

The accepted product direction is to stop extending that path for Mclone
trees. Forests should have a recognizable, cheap shape at kilometer scale,
individual trees should acquire compact stable records when they are visually
relevant, and exact chunks should materialize those same records rather than
running a separate placement lottery.

A motivating tree record is:

```text
base position
tree family and archetype
resolved size
variant seed
conservative bounds
stable identity
```

The record is semantic generator intent, not a cached block mesh and not
gameplay authority by itself.

## Bottom Line

`mclone-overworld-v1` should have one vegetation identity across all
representations:

```text
profile + seed + terrain/structured facts
  -> continuous forest intent
  -> bounded deterministic tree planner
  -> stable tree records
       -> exact block realization
       -> near/mid LOD tree instances
       -> coarse forest/canopy summaries
```

The following decisions are binding:

1. Do not add another Mclone tree through `PlacedFeature`,
   `ConfiguredDecorator`, or a chunk-local random sequence.
2. Do not create independent approximate LOD trees that disagree with exact
   tree identity.
3. Do not enumerate every tree across a multi-kilometer or continent-scale
   viewport.
4. Keep forest summaries and tree proxies logically separate from the terrain
   heightfield.
5. Keep bounded placement search in shared Rust worldgen. WGSL may evaluate
   portable continuous forest fields and render uploaded results, but it must
   not invent a second tree planner.
6. Use the same planned record to derive exact and approximate shape. A detail
   transition may simplify a tree, but it must not move or change its family.
7. Preserve the Java 1.17.1 reference profile unchanged.

## Current State

### Exact generation

Mclone tree selection currently lives in
`mclone-worldgen/src/levelgen/mclone_overworld/decoration.rs`.
`open_lowland_features`, `wooded_upland_features`,
`cool_wet_conifer_features`, and the two warm-dry steppe tables insert tree
features alongside grass and flowers. Counts, square spreading, heightmap
projection, water-depth checks, family selection, and final placement consume
the existing vanilla-shaped feature machinery.

The exact tree builders in `mclone-worldgen/src/feature/tree.rs` support many
vanilla families. That shared code remains required by the reference
Overworld. Mclone must stop using its placement semantics; a first migration
may borrow low-level block-buffer mechanisms only if the Mclone record remains
the sole owner of position, family, resolved dimensions, orientation, and
variation.

`MCLONE_OVERWORLD_DECORATION_REVISION` is currently the cache/diagnostic
revision for the surrounding decoration path. The vegetation migration needs
its own explicit source revision and an intentional decoration revision bump
when exact output changes.

### Terrain Lab

Terrain Lab already has dependency-ordered `Base`, `Hydrology`, `Structured`,
`Surface`, and `Cover` stages. CPU and GPU LOD tiles share seed, aligned
origin, sample spacing, content stage, and a session-local 192-tile LRU. The
planned-stream implementation proves that a bounded CPU-produced record can be
reconstructed only at relevant near levels and consumed by both LOD lanes.

`Cover` does not yet contain production vegetation semantics. The preview
render shader currently infers a constant coverage value from the biome recipe
code and tints the terrain surface. It has no grove identity, tree family mix,
canopy height, stable tree records, or individual proxy geometry. This is a
placeholder to replace, not a contract to preserve.

### Far LOD

The older Far LOD architecture already named `SurfacePlusApproxTrees` as a
future worldgen LOD profile. The current in-game synthetic Far LOD remains a
surface-only presentation cache with strict real/LOD exclusion and settled
coverage requirements.

Terrain Lab is the first implementation and review host for vegetation
semantics. In-game adoption must later reuse the shared vegetation products
and the existing Far LOD control plane; it must not create a second scheduler
or weaken the painted-representation XOR invariant.

### Compatibility

The compatibility safety ledger classifies `mclone-overworld-v1` as
`internal-mutable`. No shipped, external, or named retained world freezes its
current tree placement. The migration may intentionally change its exact
output in place, provided revisions, fingerprints, deterministic fixtures,
maps, captures, documentation, and disposable internal worlds are updated.

The stored `overworld` profile is reference-locked for Minecraft Java 1.17.1
parity. A shared refactor that touches its tree implementation must prove
oracle-identical output; avoiding such a refactor is preferred when a
Mclone-specific realization path is sufficient.

## Vocabulary

### Forest Intent

Forest intent is the continuous, cheap vegetation-scale description of a
location before individual trees are enumerated. Its conceptual facts are:

```rust
struct McloneForestIntentSample {
    coverage: f32,
    density: f32,
    dominant_family: McloneTreeFamily,
    secondary_family: Option<McloneTreeFamily>,
    family_mix: f32,
    mean_canopy_height: f32,
    canopy_height_variation: f32,
    grove_or_opening_influence: f32,
}
```

This is not a locked ABI. A landed slice should include only facts consumed by
exact planning, LOD rendering, diagnostics, or tests.

Forest intent must be a pure function of:

- generation profile and vegetation revision;
- signed seed and dedicated vegetation seed domains;
- dimension and supported sampling topology;
- absolute canonical coordinates;
- production terrain, climate, landform, hydrology, and surface semantics;
  and
- any explicit structure/reservation facts ordered before vegetation.

It must not depend on chunk request order, cache residency, unrelated feature
random draws, or whether the caller is exact generation, a browser Worker, or
Terrain Lab.

A forest is not a biome constant. Existing biome recipes and climate provide
important inputs, but density and openings should vary continuously within a
region so a wooded biome does not become one uniform carpet and a meadow does
not become a hard zero/one boundary.

### Tree Family And Archetype

`McloneTreeFamily` is ecological/material identity, such as:

- temperate broadleaf;
- cool-wet conifer;
- warm-dry acacia; and
- future birch, dark/ancient forest, jungle, swamp, or authored original
  families.

`McloneTreeArchetype` is the resolved silhouette grammar used by exact and LOD
realizers, such as:

- rounded or irregular broadleaf;
- layered conifer;
- forked or flat-crowned dryland tree; and
- exceptional/landmark variants.

Family and archetype should not be inferred again in the renderer from biome
IDs. Material selection belongs to exact worldgen/asset semantics; proxy
rendering receives the packed family/archetype facts it needs.

### Tree Record

The initial semantic record should be close to:

```rust
struct McloneTreeRecord {
    id: McloneTreeId,
    canonical_base: BlockPos,
    family: McloneTreeFamily,
    archetype: McloneTreeArchetype,
    trunk_height: u16,
    crown_radius: u16,
    crown_depth: u16,
    orientation: u8,
    landmark_rank: u8,
    variant_seed: u64,
    bounds: StructureBoundingBox,
}
```

This is conceptual, not a locked layout. The implementation may use a
tree-specific compact bounds type rather than the existing procedural
structure type. The semantic requirements are:

- a stable ID independent of query order and lifted working coordinates;
- a canonical base including resolved ground height;
- family and archetype;
- explicit silhouette dimensions needed by every LOD;
- a seed for residual branch, crown, and voxel variation;
- conservative bounds known before exact block writes; and
- optional significance sufficient to preserve exceptional trees farther
  away.

Do not make the renderer replay a random sequence to rediscover height or
crown radius. Those dimensions are record facts. `variant_seed` controls
variation remaining after the stable silhouette has been resolved.

### Vegetation Source Identity

Cache and stale-result rejection need a source identity containing at least:

```text
dimension/profile identity
world seed
terrain field revision
structured terrain or reservation revision
vegetation-plan revision
sampling topology
preview content stage where applicable
```

Camera, diagnostic layer, pane visibility, and presentation style are not
generation identity. Proxy-mesh style may have its own render-resource
revision without invalidating the semantic records.

### Forest Summary

A coarse forest summary represents a footprint without enumerating all of its
trees. Candidate facts include:

```rust
struct McloneForestSummary {
    coverage: u8,
    dominant_family: McloneTreeFamily,
    secondary_family: Option<McloneTreeFamily>,
    family_mix: u8,
    mean_canopy_y: i16,
    min_canopy_y: i16,
    max_canopy_y: i16,
    canopy_roughness: u8,
    exceptional_tree_count: u8,
}
```

As with the other conceptual types, add only measured consumers. The summary
must be directly synthesizable at coarse scale. It must never wait for exact
chunks or millions of child tree records.

## Deterministic Spatial Planner

### Planning cells

Individual records should be generated from aligned world-space planning
cells rather than chunk-local decorator streams. The first tactical must
measure and choose the cell size and fixed candidate count; neither is locked
here. A 32- or 64-block cell is a plausible starting range.

Each cell has a small fixed number of candidate slots. Dedicated seed domains
hash:

```text
world seed
canonical planning-cell coordinate
candidate slot
vegetation-plan revision
```

to produce a stable candidate position and ID. Independent domains derive
density acceptance, conflict priority, family/archetype choice, and the
record's variant seed so future changes to one decision do not silently shift
every later choice.

### Candidate evaluation

A candidate pipeline should:

1. jitter a candidate inside its canonical planning cell;
2. sample production structured terrain at the candidate;
3. reject water, unsupported substrate, excessive slope/exposure, or an
   explicit earlier reservation;
4. sample forest intent and deterministically accept against its density;
5. choose family and archetype from semantic inputs plus an independent hash;
6. resolve height, crown dimensions, orientation, and landmark rank;
7. resolve minimum spacing or clustering against the bounded neighboring-cell
   candidate set; and
8. emit a conservatively bounded record.

The exact acceptance rules belong to the original profile and should be
visually reviewed. They are not translations of vanilla decorators.

### Local-priority spacing

Collision and minimum-spacing decisions use deterministic local-priority
inhibition. After terrain and forest-intent evaluation, every preliminarily
eligible candidate receives an independent `u64` conflict priority. A
candidate survives if and only if no preliminarily eligible candidate with
which it conflicts has a better lexicographic `(priority, McloneTreeId)`.

Survival never depends on whether the competing candidate itself survives.
This may leave an occasional additional opening, but it prevents a greedy
accepted-neighbor chain from extending beyond the query halo. One large query
and any partition of that query therefore evaluate the same finite candidate
facts.

The conflict predicate must be symmetric. It uses canonical integer block
positions, topology-aware horizontal displacement, and integer squared-distance
comparisons against family/archetype spacing resolved before inhibition. The
maximum conflict distance determines a fixed neighboring-cell halo. Clustering
comes from the continuous grove/opening influence and preliminary density, not
from recursively attaching candidates to already accepted trees.

Candidate position, density acceptance, conflict priority, family/archetype,
resolved silhouette, and residual variation use independent deterministic hash
domains. Cache contents, enumeration order, floating-point sort behavior, and
the survival of another candidate are not inputs to the decision.

### Query contract

Worldgen should expose a bounded query resembling:

```rust
fn tree_records_intersecting(
    source: McloneVegetationSource,
    bounds: McloneVegetationBounds,
) -> Result<Vec<McloneTreeRecord>, McloneVegetationError>;
```

The planner expands the query by the maximum possible crown/trunk overhang,
visits every intersecting planning cell plus the required neighbor halo,
deduplicates by canonical ID, returns records whose conservative bounds
intersect the original query, and sorts by stable ID.

The same query must produce identical records for:

- one large request or many partitioned requests;
- forward, reverse, or random chunk order;
- cold or warm caches;
- native threads and browser Workers; and
- ordinary plane topology and the supported 384-chunk X-periodic cylinder.

### Periodic topology

For the periodic cylinder, record identity uses canonical X while a query
receives the lifted working copy nearest its requested coordinates. Candidate
fields, spacing checks, bounds, and exact writes must agree across the seam.
Any new grove/clustering field must satisfy the same 6,144-block period as the
live Mclone terrain or explicitly reject the topology.

## Pipeline Ordering

Vegetation depends on the ground it occupies, so the intended original-profile
order is:

```text
macro terrain and climate
  -> natural hydrology
  -> bounded planned streams and terrain-affecting structures
  -> final surface/biome semantics
  -> reservations and exclusion facts
  -> forest intent and tree records
  -> exact tree realization
  -> ordinary low vegetation and later decoration
```

The planner resolves `canonical_base.y` from the structured final surface
before trees are written. Exact realization must not project the base through
a second heightmap decision.

Trees must not mutate already-final neighboring chunks through late jobs.
Generation asks for every record whose bounds intersect the target dependency
footprint, applies records in stable ID order, and clips writes through the
ordinary owned chunk buffers.

Conflicts with earlier structures or water are planner decisions. Exact
realization should not silently abandon a record because another tree happened
to be written first. If later natural feature families can overlap, define an
explicit inter-family layer and priority rather than relying on mutation order.

## Exact Tree Realization

The exact Mclone realizer consumes a fully resolved `McloneTreeRecord` and
writes canonical blocks. It does not:

- select a new family;
- choose a new base;
- reroll height or crown dimensions;
- run `ConfiguredDecorator`;
- consume a shared decoration random sequence; or
- use request-local chunk position as identity.

The first realizer needs original, bounded grammars for the three live
silhouette families: temperate broadleaf, cool-wet conifer, and warm-dry
acacia. It may initially use the same log/leaf block materials as today, but
the spatial and shape grammar is Mclone-owned.

The preferred implementation is a small Mclone-specific realization module,
not an adapter that hides the old placed-feature pipeline behind a record.
Neutral helpers for clipped block writes or foliage voxel emission may be
shared when doing so does not alter reference output. If the generic vanilla
tree builder is refactored, reference oracle and random-order output must
remain byte-identical.

Exact shape and LOD shape need not contain the same geometry, but the record's
silhouette facts must be honest. A recorded 12-block conifer cannot become a
6-block exact tree, and a forked acacia cannot become a round oak proxy.

## LOD Representation Hierarchy

One representation is not appropriate at every scale.

### Exact

Canonical generated chunks contain ordinary log and leaf blocks produced from
the record. These blocks remain the authority for collision, interaction,
lighting, persistence, decay, edits, and later gameplay.

### Near and middle LOD

Where individual trees are large enough on screen and the instance budget
admits them, render compact instances derived from `McloneTreeRecord`.

The first proxy family should use a shared archetype mesh and a small instance
record. One GPU mesh per archetype/LOD is cached; a unique mesh per tree is
forbidden. Candidate proxy shapes include:

- a trunk plus one or more blocky crown volumes;
- low-segment conifer layers;
- a forked trunk plus a flat/irregular crown; and
- map-mode crown disks or symbols derived from the same instance.

Proxy geometry must support normal mono, per-eye, and multiview rendering when
it reaches the game. Tree placement and GPU buffers are view-independent.

### Coarse LOD

Once individual records would exceed their value or budget, synthesize forest
summaries directly from forest intent. A coarse request may retain only
exceptional ranked trees. Ordinary trees become:

- forest coverage and family color in map mode;
- bounded canopy masses or a separate canopy summary layer in 3D;
- canopy-height/roughness contributions to screen-space error; and
- stable grove/opening silhouettes.

The summary is a separate semantic/render layer. Do not add tree height to
`surface_y`, turn forests into terrain cliffs, or let terrain skirts create
vertical walls around canopy.

### Selection policy

Hard-coded sample-spacing thresholds are not locked here. The implementation
should select between exact instances, representative instances, cluster
summaries, and pure forest summaries using:

- projected tree/canopy size;
- requested sample spacing and footprint;
- family/archetype significance;
- stable per-tile instance and memory budgets;
- device/capability policy; and
- whether a complete parent representation is already drawable.

If too many ordinary tree records qualify, selection is deterministic by
projected importance, landmark rank, and stable ID. The omitted population
must still contribute to the forest summary.

## Terrain Lab Contract

Terrain Lab is the first product host.

`Cover` becomes the vegetation checkpoint:

- `Base` through `Surface` remain unchanged by tree presentation;
- `Cover` adds production forest intent and scale-aware forest summaries;
- admitted near detail also adds stable tree-record overlays; and
- exact `Final features` shows blocks realized from those same records.

The CPU reference lane owns exact forest intent and record planning. Portable
continuous forest equations used by summaries should enter the shared
CPU/WGSL field specification so the GPU lane can evaluate them. Bounded
candidate enumeration, neighbor collision, family/archetype resolution, and
record identity remain Rust-owned. The records are produced once and uploaded
for either LOD lane rather than independently reinvented in WGSL.

The Lab must distinguish:

```text
forest summary available
individual tree records available
individual records intentionally aggregated at this scale
record overlay pending
```

It must not label an inferred biome tint as complete vegetation coverage.

Useful diagnostics include:

- forest coverage/density and dominant/secondary family;
- source and vegetation-plan revisions;
- queried planning-cell and candidate counts;
- accepted, rejected, representative, aggregated, and exceptional tree
  counts;
- record-cache hits and misses;
- instance/summary GPU byte counts;
- time to first coarse forest pixel and first target tree overlay;
- a selected tree's ID, base, family, archetype, dimensions, seed, and bounds;
  and
- exact/LOD agreement for admitted record bases and families.

Changing diagnostic layer, map/3D view, camera, pane visibility, or exact
vegetation visibility must not regenerate tree records.

## Cache And Residency

### Semantic record cache

Add a bounded `McloneOverworldVegetationPlanCache` only when the first concrete
caller needs it. Its natural key is vegetation source identity plus canonical
planning-cell coordinate. It should retain resolved cell candidates or
records, not generated chunks and not meshes.

The cache must:

- be optional for correctness;
- produce byte-identical cold and warm results;
- have bounded session/worker ownership;
- reset on seed, profile, topology, or relevant revision change;
- expose hit/miss/retained counts; and
- avoid a second worker round trip inside one worldgen request.

### Terrain Lab tile cache

The existing viewport LRU should retain the vegetation summary and packed
instance product associated with a tile/content-stage identity. A `Cover` tile
compiled with one vegetation revision cannot be reused under another.

Panning overlap should reuse planning cells and resident archetype meshes.
`Cache off` must remain a real cold path; it may retain only resources required
by the active request and immutable shared archetype pipelines.

### Render resource cache

Cache one mesh/material binding per archetype and render LOD, then instance it.
Semantic size, orientation, family tint/material selection, and seed-derived
small variation belong in the instance buffer. Do not cache a custom GPU mesh
for every tree.

### Persistence

Do not require a persistent vegetation-record database for the first slices.
Natural records are cheap, deterministic generator products. Canonical chunks
remain persisted through the existing world store after realization.

Persistent record or summary storage is justified only by measured planning
cost, expensive later landmark families, server-streamed LOD, or authoritative
edit overlays. A cache format is not a world compatibility promise.

## Exact/LOD Handoff

Terrain Lab initially compares separate exact and LOD panes, so it can prove
identity before solving in-game compositing.

In-game integration must preserve:

```text
painted exact vegetation XOR visible vegetation proxy
```

and the broader:

```text
painted real terrain XOR visible procedural terrain
```

Tree bounds frequently cross chunk edges. Suppressing an entire proxy merely
because its base chunk loaded can leave missing crowns; retaining it merely
because one touched chunk is missing can overlap exact leaves. The in-game
tactical must choose and prove one explicit mechanism, such as:

- clipping proxy fragments/instances against the same painted-real chunk
  coverage used by terrain arbitration; or
- separating exact vegetation draw admission sufficiently to replace one
  bounded record atomically.

Loaded, generated, traversal-ready, or resident are not substitutes for
painted-capable readiness. No owner-chunk shortcut may silently violate the
XOR contract.

Transitions may cross-fade, dither, or morph only after overlap accounting is
defined. Position, family, dimensions, and world-anchored variation remain
stable throughout the transition.

## Authority, Mutation, And Multiplayer

The tree planner is canonical generation policy for untouched natural
`mclone-overworld-v1` terrain. The resulting exact blocks become authoritative
world state. Terrain Lab and Far LOD copies remain removable presentation
data and cannot satisfy collision, raycasts, harvesting, decay, lighting,
ticks, persistence, or AI.

For remote worlds:

- a server that exposes the profile/seed/revisions may allow local natural
  presentation synthesis;
- a server may stream compact summaries or records without exposing the seed;
- a concealed or non-Mclone source disables local record synthesis; and
- server-supplied edits always override natural presentation.

Player-planted, grown, cut, burned, or otherwise edited trees do not reuse the
natural planner as current truth. Later sparse overlays need record-ID or
bounded-region invalidation so a felled tree is not resurrected by distant
LOD. That edit problem does not block the first Terrain Lab and exact-natural
generation slices, but the source identity must leave room for it.

## Shared Ownership

- `mclone-worldgen` owns forest intent, seed domains, topology, tree IDs,
  bounded record planning, exact realization, source revisions, summaries,
  reference comparison, and deterministic tests.
- `mclone-terrain-view` owns vegetation tile products, progressive summary/
  instance readiness, LRU integration, GPU instance buffers, proxy and map
  rendering, picking, and comparison aggregation.
- `mclone-terrain-lab` owns narrow Wasm serialization and browser-facing
  methods.
- `tools/terrain-lab` owns controls, URL projection, labels, inspector
  presentation, and browser assertions. It must not classify forests or
  generate records.
- `mclone-app-runtime` owns eventual in-game vegetation coverage/refinement
  requests and capability policy through the existing Far LOD control plane.
- `mclone-render-session` owns eventual resident proxy lifecycle and
  replacement readiness.
- `mclone-render` owns eventual shared proxy draw pipelines and
  mono/per-eye/multiview resources.
- `mclone-scene` owns exact/proxy arbitration, frame budgets, and final
  representation diagnostics.

Do not put this policy in `mclone-native-client`, browser TypeScript, an app
crate, or a renderer-only WGSL implementation.

## Migration Plan

### Slice 1: Record-first exact Mclone vegetation

Create a tactical whose first result is a deliberate exact-generation
migration, not an LOD-only duplicate:

1. add `mclone_overworld/vegetation.rs` with source identity, forest intent,
   tree family/archetype, tree ID, record, bounded planner, and exact realizer;
2. define dedicated, periodic-compatible vegetation seed domains;
3. implement fixed-work planning-cell queries with stable neighbor resolution;
4. implement original broadleaf, conifer, and acacia record grammars and exact
   block realization;
5. remove Mclone tree entries from its placed-feature tables while retaining
   ordinary grass/flower/fern/berry decoration temporarily;
6. integrate record planning after structured terrain/surface facts and before
   low vegetation;
7. bump vegetation/decoration revisions and intentionally update Mclone
   fingerprints and disposable worlds;
8. prove partition, order, cache, Worker, and periodic seam determinism; and
9. capture and inspect representative exact meadow, woodland, conifer, steppe,
   transition, stream-edge, and cylinder-seam pixels.

The slice is incomplete if exact generation still chooses tree positions
through `ConfiguredDecorator`.

### Slice 2: Terrain Lab forest summaries and record overlays

1. replace shader-side biome constants with production forest intent;
2. extend `Cover` preview/cache identity and comparison facts;
3. synthesize scale-aware forest summaries without enumerating trees;
4. query individual records only for admitted near/mid detail;
5. upload one shared record overlay for CPU and GPU LOD presentation;
6. add instanced broadleaf/conifer/acacia proxies in map and 3D;
7. add inspector provenance and availability/performance diagnostics;
8. compare exact final tree bases/families with the LOD records; and
9. inspect desktop and phone pixels while zooming from exact scale to at least
   the existing 65.5-km proof.

The GPU-only base/hydrology path must remain independent. `Cover` may wait for
the CPU-owned record overlay only where individual records are requested.

### Slice 3: Hierarchy, budgets, and cache proof

1. tune filtered forest summaries and grove/opening stability across sample
   levels;
2. add deterministic representative-instance and exceptional-tree admission;
3. prove bounded work and memory during pan, zoom, cache-on, cache-off, and the
   fixed cold race;
4. prove no individual-tree enumeration in continent-scale summary requests;
5. retain complete parent vegetation until a child representation is
   drawable; and
6. select explicit summary/instance transitions from inspected pixels and
   timings.

### Slice 4: In-game Far LOD adoption

Only after Terrain Lab proves the semantic products:

1. feed vegetation summaries/records into the shared Far LOD request and
   resident lifecycle;
2. add mono/per-eye/multiview proxy rendering;
3. implement exact/proxy clipping or atomic bounded replacement;
4. extend settle ledgers to vegetation representation;
5. validate stationary, moving, teleport, world-switch, device-rebuild, and
   source-revision invalidation;
6. measure desktop, web, Android, Quest, and XR budgets selected by the
   affected contract; and
7. retain a clean Far-LOD-off control.

### Later generalization

Do not begin with a universal `NaturalFeatureRecord` enum. After trees, planned
streams, and at least one materially different caller such as boulders,
outcrops, or structures exist, extract only the common identity, bounds,
source-revision, query, and LOD-significance mechanisms they actually share.

Likely representation families are:

- point instances: trees and boulders;
- routes: streams, roads, walls, and trails;
- bounded volumes/footprints: arches, outcrops, buildings, and settlements;
- regional fields: forests, wetlands, grasslands, and geology; and
- authoritative sparse overlays: player edits and mutable authored content.

Their payloads and planners should remain family-specific.

## Acceptance Contract

### Semantic and exact generation

- Mclone tree position, family, dimensions, orientation, and variation are
  owned by stable tree records.
- Mclone tree placement no longer calls the vanilla-shaped decorator path.
- Exact blocks at a record agree with its base, bounds, family, and silhouette
  facts.
- Grass and other retained placed features cannot change tree identity through
  random-consumption order.
- Reference `overworld` output and oracle fixtures remain unchanged.
- Intentional `mclone-overworld-v1` output changes are revisioned and recorded
  under the compatibility ledger.

### Determinism and topology

- Single, batched, partitioned, reversed, and randomized queries produce the
  same sorted records and exact block output.
- Cold and warm plan caches agree byte-for-byte.
- Native and browser Worker results agree.
- The supported 384-chunk cylinder has canonical record identity, exact
  cross-seam spacing, intersecting bounds, and block output.
- Negative coordinates and planning-cell boundaries have direct fixtures.

### LOD coherence

- The same record base/family appears in CPU LOD, GPU LOD, and exact final
  terrain where individual records are admitted.
- Zooming or panning does not relocate, reroll, or change the family of a
  retained tree.
- Coarse forest edges, clearings, dominant families, and canopy silhouette
  remain recognizable without individual enumeration.
- Exact and approximate vegetation never co-render in the same admitted
  representation footprint once integrated in-game.
- Parent summaries remain until the replacing child instances are drawable.

### Performance

- Coarse work is proportional to the requested summary lattice, not the number
  of possible trees or ordinary chunks in its footprint.
- Near record work is proportional to bounded planning cells with a fixed
  candidate ceiling.
- Panning overlap records real planning-cell/tile cache hits.
- Archetype geometry is shared and instanced; unique per-tree mesh allocation
  is zero.
- CPU planning, packing, GPU upload, resident bytes, and draw cost are reported
  separately.
- The existing Terrain Lab cold race remains honest about CPU publication,
  GPU submission/readback, and record-overlay readiness.

### Rendered output

- Inspect exact and LOD meadow, temperate woodland, conifer, and dry-steppe
  transitions.
- Inspect a forest edge and internal clearing while zooming through every
  representation level.
- Inspect tree silhouettes against mountain and horizon backgrounds.
- Inspect stream banks and water exclusions.
- Inspect map and 3D views on desktop and phone.
- Inspect periodic seam pixels.
- When integrated in-game, inspect mono, stereo/per-eye, and full-frame
  multiview, plus exact/LOD handoff during movement.

## Stop Conditions And Guardrails

Stop and correct the architecture if:

- a coarse request enumerates individual trees across its full footprint;
- exact and LOD callers derive family, dimensions, or position independently;
- the browser or WGSL owns placement search or tree-family policy;
- a tree query requires materializing complete `GeneratedChunk` values;
- cache presence changes records;
- the periodic seam uses duplicate identities or loses neighbor spacing;
- proxy geometry is baked into the terrain heightfield;
- in-game suppression uses loaded/resident state without painted-capable
  evidence;
- a universal feature abstraction grows before a second and third concrete
  family prove its shape; or
- a shared refactor changes reference Overworld output.

Do not add persistent record storage, GPU-side placement search, editable
Terrain Lab tree authoring, wind, bushy-leaf detail, falling-tree behavior, or
player-edit overlays to the first exact migration merely because those
features mention trees.

## First Tactical Questions To Lock

The implementing tactical should resolve these with measurements and reviewed
pixels:

1. What planning-cell size and fixed candidate count give sufficient density
   without visible grids or excessive neighbor work?
2. Which existing fields plus one bounded grove/opening domain produce
   recognizable forest shapes and exact 6,144-block periodicity?
3. What minimum semantic parameters let broadleaf, conifer, and acacia exact
   and proxy silhouettes agree?
4. Which record packing keeps the near overlay compact without making the ABI
   the semantic owner?
5. At what projected sizes and budgets do individual, representative,
   clustered, and summary representations switch?
6. Should the first 3D far forest use canopy clusters, a separate canopy
   surface, or both at different levels?
7. Which exact/proxy clipping mechanism can later preserve the painted XOR
   across cross-chunk crowns?
8. Which source revisions must invalidate exact generation, preview summaries,
   record caches, and render resources independently?

## Code And Documentation Map

- Mclone decoration tables:
  `native/crates/mclone-worldgen/src/levelgen/mclone_overworld/decoration.rs`
- Mclone feature dependency cache:
  `native/crates/mclone-worldgen/src/levelgen/mclone_overworld/feature_batch.rs`
- current vanilla-shaped tree realization:
  `native/crates/mclone-worldgen/src/feature/tree.rs`
- planned-stream record/cache precedent:
  `native/crates/mclone-worldgen/src/levelgen/mclone_overworld/streams.rs`
- generic procedural bounds/start precedent:
  `native/crates/mclone-worldgen/src/procedural_structure.rs`
- Terrain Lab preview stages and structured overlay:
  `native/crates/mclone-worldgen/src/terrain_preview.rs`
- shared viewport scheduler/cache/renderer:
  `native/crates/mclone-terrain-view/src/viewport.rs` and
  `viewport_renderer.rs`
- current placeholder cover rendering:
  `native/crates/mclone-terrain-view/src/shaders/terrain_preview_render.wgsl`
- Terrain Lab product contract: `tools/terrain-lab/README.md`
- durable procedural terrain direction:
  [`gpu-procedural-terrain.md`](gpu-procedural-terrain.md)
- original profile terrain and structured sampling:
  [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- original profile breadth ledger:
  [`mclone-overworld-breadth.md`](mclone-overworld-breadth.md)
- compatibility safety ledger:
  [`world-generation-profiles.md`](world-generation-profiles.md)
- current synthetic Far LOD:
  [`far-lod.md`](far-lod.md) and
  [`../lod-architecture.md`](../lod-architecture.md)
- leaf-block presentation, deliberately separate:
  [`bushy-leaf-rendering.md`](bushy-leaf-rendering.md)
- tree mutation/physics, deliberately separate:
  [`falling-tree-physics.md`](falling-tree-physics.md)

## Recommended Next Work

Open one implementation tactical for Slice 1: record-first exact Mclone
vegetation. Do not start with a Terrain Lab-only visual approximation. The
first commit should establish the source identity, conceptual record contract,
and deterministic planning fixtures; the first drawable milestone should
replace one Mclone tree family end to end and be captured before broadening to
all three live families.

Use `Topic: lod-native-vegetation` on the implementing commit series and append
that exact slug to `topics.md` when the first commit is created.
