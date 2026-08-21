# LOD-Native Vegetation

Topic: `lod-native-vegetation`

Status: exact records, the Terrain Lab hierarchy, reusable native/browser
vegetation execution, detached whole-record ownership, and the first live-game
consumer are landed. The rejected in-game chunk-based adapter was removed by
Tactical 245. PH-4 live-game composition was provisionally accepted on
2026-07-27 with the exact/proxy overlap described below retained as a known
limitation.

Tactical
[`328`](../tactical/328-mclone-overworld-v2-regional-breadth.md) extends the
same semantic record and renderer boundary for experimental
`mclone-overworld-v2`: `HumidJungleBroadleaf` and `LayeredJungle` add dense
clustered crowns, emergents, and understory without changing V1 records. The
continental planner uses two bounded candidates per existing 24-block proxy
cell only where continuous jungle-canopy facts permit them. Exact jungle
logs/leaves and all procedural render paths consume those stable records;
coarse levels remain summaries. Compiler source v3, `MCHV` wire v2, and
canonical batch v4 carry four-family receipts.

Native World Explorer composition review on 2026-07-26 proved that the first
chunk/fragment mask is not a valid exact/LOD tree handoff. One stable natural
tree can appear as exact block geometry partly hidden by retained procedural
collar terrain plus the outside fragments of its own LOD proxy. Tactical
[`262`](../tactical/262-world-explorer-exact-procedural-composition.md)
Slice 3A now implements the focused correction through a small neutral
bounded-representation ownership envelope and stable-tree adapter. Exact
natural-tree admission is separate, and one complete stable-ID
representation is selected from the record's full bounds and the actual
exact-safe terrain interior. Delayed native window/offscreen movement has
zero missing, dual, or unowned records at forest checkpoints. That identity
proof remains valid, but Human Review 1A was retracted on 2026-07-27 because
the focus-centered exact patch sat behind procedural foreground at low orbit
pitch. Tactical 262 Slice 3B now gives exact terrain, procedural residency,
and vegetation one viewer-forward composition anchor. Its delayed native
window/offscreen smoke again reports zero missing exact or proxy records;
Slice 4 now carries separated exact-tree records through the portable
canonical batch codec and isolated browser Worker. Desktop and Pixel 7
composition receipts reproduce all 15 records as 8 exact-owned and 7
proxy-owned, with zero missing representations. The versioned production
build passes hosted desktop composed/coverage and Pixel 7 composed gates with
the same split and zero missing representations. Human Review 1B nevertheless
rejected the pixels because procedural and exact renderers write incompatible
depth encodings; exact-owned trees and exact hills can both be cut by the
procedural surface. The Slice 4B correction at `91fe9302` replaces both
procedural terrain and proxy-tree linear depth with the exact chunk
view-projection matrix. Magenta exact-source native captures show a complete
exact canopy surviving over farther LOD while nearer green procedural
geometry can still win. Browser semantic parity passes, and hosted
interactive Human Review 1B accepted the corrected composition on
2026-07-27. Minor z-fighting at the outermost exact terrain blocks is recorded
as a terrain-frontier collar/skirt follow-up, not a vegetation-ownership
failure.

World Explorer review on 2026-07-26 found that native enabled synchronous
near-level vegetation compilation during frame encoding while browser disabled
vegetation entirely. Tactical
[`256`](../tactical/256-shared-horizon-vegetation-worker-topology.md) records
the completed replacement: one `mclone-terrain-view` coordinator owns job
identity, budgets, cache lifecycle, stale rejection, and admission, while
native threads and an isolated browser Rust actor provide equivalent execution
through platform-appropriate mailboxes. Neither presentation thread remains a
normal compiler path. Its parity closeout completed on 2026-07-26:
World Explorer is the first parity host for a reusable engine terrain-LOD
service, not the owner or final destination. Native, offscreen, desktop
browser, and phone browser agreed on the complete pinned semantic receipt,
including `2,609` records/instances and `281,772` proxy vertices. A later
`mclone-scene` tactical will consume the same coordinator/compiler boundary
and separately solve exact/proxy arbitration, edits, multiworld lifecycle,
and all-target presentation.

Tactical
[`252`](../tactical/252-procedural-horizon-seams-and-transition-admission.md)
Slice 1 closes the presentation-side transition gap exposed after that Worker
cutover. The shared terrain-view admission owner retains a complete
same-source vegetation level while replacement products are in flight, then
commits the whole replacement level. Terrain has an independent complete
presentation, so neither host exposes a forest-free transition frame and
neither waits for platform-specific executor latency. Source changes still
invalidate old vegetation immediately.

Live-game review after the shared terrain quality correction found a narrower
ownership gap that detached Explorer and Terrain Lab composition do not have.
Their canonical exact renderer separates natural-tree meshes from base
terrain, so the selected stable-record owner controls both sides of the
handoff. The live game deliberately retains its ordinary authoritative chunk
meshes; natural tree blocks in those meshes are not a separately suppressible
draw. The shared owner can remove an exact-owned record's proxy, but a
proxy-owned frontier record may still overlap the exact tree already present
in a live chunk. The reviewed example visibly combines textured exact leaves
with the solid proxy crown and produces localized z-fighting.

This is accepted for the current PH-4 checkpoint. It does not redefine
complete-record XOR as achieved in the live scene. The procedural vegetation
represents the original generated natural world and is already allowed to be
stale with respect to cutting, planting, or other persisted edits outside
exact range. A later revisit may add record- or region-level invalidation or
storage. A smaller proxy crown in X/Z is also a plausible rough visual
mitigation, but it would reduce overlap rather than establish ownership and is
not implemented.

## Scope

This topic owns the immediate migration of natural trees and forest shape in
`mclone-overworld-v1` from vanilla-shaped chunk decoration to a
worldgen-owned semantic plan that can serve:

- exact canonical chunk generation;
- Terrain Lab CPU and GPU LOD panes;
- a future multiscale in-game terrain presentation;
- overview maps, minimaps, and tabletop views; and
- later natural-feature overlays that need stable identity across detail
  levels.

The first implementation is deliberately about trees and forests. Ordinary
grass, flowers, crops, player-planted trees, tree growth, falling-tree
simulation, structures, and general-purpose natural-feature serialization are
separate concerns.

This migration applies only to the original Mclone profile. The legacy
Java-1.17-shaped `overworld`, Alpha, Beta, Small Island, and other profiles do
not acquire this system merely because they also place trees. If a shared
refactor touches those profiles, use their own scoped regression evidence; no
Java parity lock applies.

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

`mclone-worldgen/src/levelgen/mclone_overworld/vegetation.rs` now owns
production forest intent, 32-block planning cells with 32 fixed candidate
slots, independent hash domains, deterministic local-priority inhibition,
stable IDs and records, a bounded optional cell cache, topology-aware lifted
occurrences, and exact broadleaf, conifer, and acacia realization.

`MCLONE_OVERWORLD_VEGETATION_REVISION` is
`mclone-overworld-v1-vegetation-2`; the surrounding exact output is pinned by
`mclone-overworld-v1-decoration-14`. Mclone decoration tables contain only
retained low vegetation. Every natural Mclone tree is realized once in stable
record-ID order through clipped mutable feature buffers before those remaining
placed features run.

The vanilla-shaped tree machinery remains intact for the reference Overworld
and other profiles. The complete reference regression suite is unchanged.
Partitioned/reversed/cold/warm record and exact-output fixtures, native
job-frame and resident-delta fixtures, browser/WASM builds, negative
coordinates, persistence, and the 6,144-block periodic seam all pass. Tactical
243 records the output fingerprints, inspected pixels, and final performance
receipts.

### Terrain Lab

`Cover` carries production forest coverage, density, family mix, canopy
height/variation, and grove/opening influence in
`mclone-terrain-preview-reference-grid-v8`. Spacings `1/2/4` retain exact
point intent with fixed-radius local slope. Coarser samples use a four-tap
footprint quadrature at one-quarter spacing, including coverage-weighted
family and canopy statistics. Both CPU cases remain fixed at five terrain
evaluations per output point.

GPU evaluator `mclone-overworld-v1-gpu-preview-a7` applies the same coarse
filter to portable macro fields. Coarse GPU-only `Cover` is now an independent
presentation path with zero CPU sample, byte, or tile dependency. Near GPU
views still request shared Rust products for exact records and planned streams;
WGSL does not plan semantic trees.

Near requests query stable records only at spacings `1`, `2`, and `4`.
Admission is globally deterministic by landmark rank: all records, ranks
`2/3`, then rank `3`. Coarser requests are summary-only and prove zero record
queries. One 48-byte instance per admitted tree drives one shared procedural
trunk/crown archetype pipeline for broadleaf, conifer, and acacia in both map
and 3D views. The viewport LRU retains the summary/record product and reports
source revision, packing, upload, residency, proxy, and planning-cache facts.

All 21 terrain-view tests and the targeted desktop/Pixel 7 headed-WebGPU
contracts pass. The 65.5 km `Cover` smoke now records cold, warm, cache-off,
and GPU-only CPU/GPU work and byte receipts. At cache-off spacing `512`, each
lane evaluates 109,850 lattice points, 549,250 terrain points, 439,400 forest
intents, and 109,850 footprint summaries with zero record/cache traffic.
GPU-only auto detail publishes spacing `256` with every CPU counter at zero.

### Removed in-game compatibility adapter

The retired synthetic Far LOD evaluated production forest intent at each
Mclone lattice sample and appended stable record-derived tree volumes to its
chunk-tile payload. Auto level one admitted all records, level two admitted
landmark ranks `2/3`, and level three was summary-only. Native compile workers
and persistent browser compiler sessions reused one bounded vegetation plan
cache beside their surface cache.

Every trunk/crown volume was clipped to its 16-by-16 tile. Cross-chunk crowns
were queried and clipped in every intersecting tile, so its painted-capable
whole-tile arbitration replaced the exact footprint without an owner-chunk
shortcut or vegetation scheduler. The same payload followed the mono, per-eye,
and full-frame multiview renderer.

Commit `b1db968c` was a disposable compatibility adapter rather than the future
vegetation architecture. Tactical
[`245-retire-chunk-far-lod-runtime.md`](../tactical/245-retire-chunk-far-lod-runtime.md)
removed it with the rest of that runtime.

### Compatibility

The compatibility safety ledger classifies `mclone-overworld-v1` as
`internal-mutable`. No shipped, external, or named retained world freezes its
current tree placement. The migration may intentionally change its exact
output in place, provided revisions, fingerprints, deterministic fixtures,
maps, captures, documentation, and disposable internal worlds are updated.

The stored `overworld` profile is an internal-mutable legacy reference surface.
A shared refactor that touches its tree implementation should update affected
regression fixtures; exact oracle comparison is needed only when the change
continues to make a specific Minecraft-behavior claim.

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

Off-thread LOD compilation wraps this cache in one shared worldgen compiler
session. The executor thread or Worker actor owns that session; the
presentation host never mirrors its planning cells. Native moves typed
products out of the session, while browser encodes the same semantic product
through a versioned external-SAB result ABI. Serialization is a transport
concern and must not become a second record definition.

The semantic compiler/source revision, semantic occurrence-product revision,
and browser `MCHV` wire version are separate. Only the first two participate
in record/cache identity. The wire version validates isolated-Wasm transport
compatibility and cannot make native and browser semantic sources differ.

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
tactical must use the record's full conservative bounds and drawable
readiness. Interactive World Explorer review rejected clipping proxy
fragments against painted chunks: retained procedural collar terrain can
occlude the wholesale exact tree while fragments of the same proxy survive
outside the chunk mask.

The selected first mechanism is:

- keep exact natural-tree draw admission separate from exact terrain
  admission;
- define an exact-safe terrain interior that excludes unpainted terrain and
  every retained procedural collar fragment;
- make a record exact-owned only after its complete working bounds lie inside
  that interior and all exact draw resources are ready;
- otherwise keep the complete record proxy-owned; and
- switch the whole record atomically by stable ID and coverage generation.

Loaded, generated, traversal-ready, or resident are not substitutes for
painted-capable readiness. No owner-chunk shortcut may silently violate the
XOR contract. Per-fragment discard is not tree ownership.

Transitions may cross-fade, dither, or morph only after overlap accounting is
defined. Position, family, dimensions, and world-anchored variation remain
stable throughout the transition.

### Accepted live-scene exception

The mechanism above is implemented end to end for detached canonical exact
composition. The first live-game adapter currently implements only the proxy
side of that decision because its exact tree blocks remain part of ordinary
authoritative chunk meshes. It can therefore draw both representations for a
stable record near the frontier.

The 2026-07-27 hosted review accepted that overlap provisionally because:

- terrain composition, depth, seam closure, lighting, and horizon reach are
  otherwise believable;
- the defect is localized to trees near the exact frontier;
- the natural LOD is already an intentionally edit-unaware reconstruction of
  the generated baseline; and
- correcting persisted live geometry deserves a focused ownership design
  rather than an unsafe block-material or bounding-box deletion rule.

Future work should compare at least these paths:

1. inset proxy crown width/depth slightly as a cheap cosmetic mitigation;
2. partition precisely reconstructed generated-feature voxels from the live
   exact mesh by stable record ID, leaving differing authoritative edits in
   the ordinary mesh; and
3. add sparse record/region invalidation or stored LOD overrides so distant
   proxies can eventually reflect authoritative mutation.

The first option is explicitly not a correctness fix. The second must account
for generator revision and persisted chunks. The third is the durable route
for a forest that has been cut down but remains visible in procedural LOD.

## Authority, Mutation, And Multiplayer

The tree planner is canonical generation policy for untouched natural
`mclone-overworld-v1` terrain. The resulting exact blocks become authoritative
world state. Terrain Lab copies remain removable presentation data and cannot
satisfy collision, raycasts, harvesting, decay, lighting, ticks, persistence,
or AI.

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
  stateful LOD compiler sessions, product codecs, reference comparison, and
  deterministic tests.
- `mclone-terrain-view` owns vegetation tile products, progressive summary/
  instance readiness, the WGPU-independent desired-work coordinator, source
  epochs, slot-generation stale rejection, budgets, LRU integration, GPU
  instance buffers, proxy and map rendering, picking, and comparison
  aggregation.
- `mclone-terrain-lab` owns narrow Wasm serialization and browser-facing
  methods.
- `tools/terrain-lab` owns controls, URL projection, labels, inspector
  presentation, and browser assertions. It must not classify forests or
  generate records.
- World Explorer and later game app crates own only platform executor
  construction, surface/session cadence, raw events, and presentation
  mechanics.
- A future `mclone-scene` tactical composes the proven shared service and owns
  world lifecycle, multiscale coverage/refinement, resident proxy lifecycle,
  exact-painted snapshots, and exact/proxy arbitration. The rejected chunk
  Far LOD control plane does not own that future contract.

Do not put this policy in `mclone-native-client`, browser TypeScript, an app
crate, or a renderer-only WGSL implementation.

The compiler codec consumes worldgen-owned requests and opaque fixed-width
correlation scalars. It does not depend on terrain-view tile or slot types;
`mclone-terrain-view` translates between those compiler facts and its semantic
tile/admission identity.

## Migration Plan

### Slice 1: Record-first exact Mclone vegetation — complete

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

Exact generation no longer chooses Mclone tree positions through
`ConfiguredDecorator`. Tactical
[`243-lod-native-vegetation-exact.md`](../tactical/243-lod-native-vegetation-exact.md)
is the completed execution record.

### Slice 2: Terrain Lab forest samples and record overlays — initial path landed

Tactical
[`244-lod-native-vegetation-presentation.md`](../tactical/244-lod-native-vegetation-presentation.md)
records the bounded Terrain Lab presentation implementation and the now-removed
compatibility adapter. Its durable result is globally consistent landmark-rank
admission and CPU-owned vegetation products for both displayed Lab lanes.

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

Tactical 244 landed the local Cover product, bounded landmark-rank admission,
one procedural instance pipeline, responsive desktop/phone pixels, and
planning/packing/upload/residency metrics. Footprint-filtered summaries and
continent-scale `Cover` evidence remain open under the next slice.

### Slice 3: Hierarchy, budgets, and cache proof — active

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

Near admission is explicit and bounded: Terrain Lab uses spacing/rank
admission, coarse levels issue zero record queries, one viewport LRU owns the
semantic product, and panning records planning-cell reuse. The active work is
to prove `Cover` at 65.5 km, add direct work/cost evidence, filter summaries by
sample footprint where point sampling aliases, and inspect fixed anchors
through the spacing hierarchy.

### Slice 4: rejected in-game compatibility adapter — removed

The adapter proved that stable records could cross a game renderer boundary,
not that chunk tiles were the right future hierarchy. Tactical 245 removed it.
A later architecture may consume the semantic vegetation products through a
different spatial hierarchy.

### Slice 5: reusable off-thread vegetation service — complete

Tactical
[`256`](../tactical/256-shared-horizon-vegetation-worker-topology.md)
owns this portability slice.

1. wrap the existing preview product compiler/cache in a stateful shared
   worldgen job session;
2. add one WGPU-independent terrain-view coordinator over semantic desired
   tiles and explicit physical-slot generations;
3. preserve deterministic coarse-to-fine priority, one in-flight job, and one
   admission per pump across executors;
4. move native execution to one bounded worker thread with typed moved
   products;
5. move browser execution to one isolated Rust/Wasm actor with domain-blind
   transport and a persistent external-SAB result arena;
6. prove source, record hash, family counts, instances, cache behavior,
   failure/restart, overflow, and shutdown parity; and
7. leave a documented service-construction seam for later `mclone-scene`
   adoption.

The default Explorer currently contributes `48` record-capable tiles: sixteen
each at sample spacings `1`, `2`, and `4`. Coarser clipmap levels retain
continuous forest summaries and issue no individual-tree jobs. The
coordinator accepts a validated desired set rather than baking that Explorer
configuration into its ABI.

This slice does not integrate the game scene. That later work adds the
exact-painted coverage snapshot, cross-chunk crown XOR, authoritative edit
invalidation, multiworld budgets, device rebuild, and mono/stereo/multiview
presentation around the already proven service.

### Slice 6: whole-record frontier arbitration — Human Review 1A candidate

Tactical
[`262`](../tactical/262-world-explorer-exact-procedural-composition.md)
Slice 3A owns the reusable untouched-natural-tree proof before browser or
game-scene promotion.

1. publish a neutral source-, generation-, stable-unit-, bounds-, readiness-,
   and owner-aware bounded-representation snapshot;
2. adapt stable tree occurrences to that envelope without creating a
   universal natural-feature payload enum;
3. separate exact natural-tree draws from exact terrain draws;
4. classify against the actual exact-safe interior, including the procedural
   collar;
5. admit or suppress complete exact/proxy records rather than fragments;
6. prove exactly one visible representation during delayed admission,
   eviction, movement, negative coordinates, teleport, and source reset; and
7. repeat the reviewed forest boundary in native window/offscreen captures
   before Human Review 1A.

Authoritative edits, production-scene invalidation, multiworld lifecycle, and
all-target promotion remain later parent phases.

The neutral envelope is deliberately smaller than a `NaturalFeatureRecord`
system. Trees retain `McloneTreeRecord`; future boulders, structure pieces,
and route segments may reuse only identity, bounds, readiness, and atomic
owner selection after their concrete needs exist. Forest summaries, grass,
dynamic entities, and arbitrary player builds remain separate representation
and authority families.

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
- Detached canonical composition never co-renders exact and approximate
  vegetation in one admitted footprint. The current live-game exception is
  recorded above and remains future correction work.
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
- exact/proxy arbitration clips one logical tree by base chunk or fragment
  position instead of selecting its complete stable record;
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
- rejected synthetic Far LOD and its removal plan:
  [`far-lod.md`](far-lod.md) and
  [`../tactical/245-retire-chunk-far-lod-runtime.md`](../tactical/245-retire-chunk-far-lod-runtime.md)
- leaf-block presentation, deliberately separate:
  [`bushy-leaf-rendering.md`](bushy-leaf-rendering.md)
- tree mutation/physics, deliberately separate:
  [`falling-tree-physics.md`](falling-tree-physics.md)

## Recommended Next Work

Coordinating parent Tactical
[`261`](../tactical/261-procedural-horizon-product-integration-roadmap.md)
sequences the remaining integration campaign. PH-4 shared-engine and
live-scene adoption is accepted with the live exact/proxy overlap above.
Complete live-tree XOR and edit-aware proxy invalidation remain a deliberate
PH-5-or-later revisit rather than an immediate blocker. Lifecycle recovery,
flat-platform promotion, and mono/stereo/multiview admission remain separate
parent phases.

The active Terrain Lab hierarchy work remains an independent presentation
quality track: retain its 65.5 km work receipts, footprint filtering, and
fixed-anchor inspection. Do not reopen Tactical 256 to change forest
algorithms or representation thresholds. The service has moved compilation
off both presentation threads; the remaining runtime work is scene
composition and exact/proxy ownership.
