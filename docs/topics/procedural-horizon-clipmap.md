# Procedural Horizon Clipmap

Topic: `procedural-horizon-clipmap`

Status: the standalone cross-platform proof, transition hardening, and shared
vegetation service are complete. Terrain Lab runtime-composition adoption and
its hosted human review are also complete. Active coordinating parent Tactical
[`261`](../tactical/261-procedural-horizon-product-integration-roadmap.md)
owns the global path into `mclone-scene`, exact/procedural arbitration,
flat-platform promotion, and XR/multiview acceptance. The first
cross-platform proof was completed and deployed on 2026-07-25 by Tactical
[`249`](../tactical/249-cross-platform-procedural-horizon-proof.md). Shared
toroidal planning, a ten-level fixed-budget renderer, native tree proxies, and
one Rust Explorer session now run through native and browser adapters. This is
not yet an in-game procedural-horizon replacement. Product scope and platform
hosting are independent: the small Explorer and full game may both run in the
browser, while the same terrain system remains usable on desktop, Android,
and XR. Tactical
[`245`](../tactical/245-retire-chunk-far-lod-runtime.md) remains the completed
removal boundary for the rejected chunk-based Far LOD system. Tactical
[`247`](../tactical/247-standalone-world-explorer-foundation.md) supplied the
small native proof host and shared navigation boundary for the first ring; it
deliberately implemented no clipmap residency.
Tactical
[`262`](../tactical/262-world-explorer-exact-procedural-composition.md) has
now reached Human Review 1 with a shared exact-painted snapshot, bounded GPU
mask, caller-owned color/depth target, extracted canonical compiler/codec,
and native `Horizon`, `Exact`, `Composed`, and `Coverage` modes. The 5-by-5
exact footprint stays coherent through delayed movement, negative
coordinates, and teleport. Its explicit 1.5-block procedural collar removes
the earlier full-height footprint wall. Human review accepted that terrain
behavior overall but found a blocking natural-tree ownership defect: an exact
tree can be depth-occluded by the retained collar while fragment masking
leaves the outside part of the same stable record's LOD proxy visible.
Tactical 262 Slice 3A now separates natural-tree admission from terrain and
selects one complete exact-or-proxy representation from stable ID, complete
bounds, exact-safe interior, and drawable readiness. Delayed native movement
and inspected forest captures report zero missing, dual, or unowned records.
Human Review 1A was nevertheless retracted on 2026-07-27 when low pitch made
the whole focus-centered exact patch appear behind procedural terrain.
Deterministic exact/composed/coverage captures and a temporary full-footprint
discard showed that the broad effect was not collar overlap: the orbit eye
was outside the patch, with real procedural foreground between it and the
focus. Slice 3B now uses one viewer-forward anchor for exact terrain,
procedural residency, and vegetation, plus a terrain-safe composed target
height when sea-level targeting would place the low orbit eye in resolved
ground. Four-sided captures and delayed native window/offscreen movement pass
with no missing exact or proxy trees. Slice 4 now runs the same exact renderer
through an isolated browser Worker and accepts deterministic
`composition`/`exactRadius` URLs. Desktop and Pixel 7 semantic receipts match
all 25 painted chunks, coverage generation 27, and the 8 exact/7 proxy split
across 15 whole-tree records. The content-versioned production build now also
passes hosted desktop `Composed`/`Coverage` and Pixel 7 `Composed` gates with
zero missing representations. Human Review 1B rejected those pixels:
procedural terrain uses linear reversed depth while exact chunks use the
engine's nonlinear perspective reversed-Z projection, so the shared depth
target contains incomparable values and procedural surfaces cut nearer exact
trees and hills. Tactical 262 Slice 4B now owns the shared-projection
correction and source-colored silhouette evidence. Its correction landed at
`91fe9302`: terrain and proxy vegetation now consume the same
`ChunkRenderView` matrix as exact chunks, and a magenta exact-source
diagnostic is available natively and through `sourceColors=1`. Matched native
silhouette and hill captures preserve nearer exact geometry and allow nearer
procedural geometry to occlude it. Desktop and phone browser semantic gates
pass, and hosted interactive Human Review 1B accepted the corrected
composition on 2026-07-27. Minor z-fighting limited to the outermost exact
blocks remains a known near-coincident frontier-overlap issue for later
collar/skirt refinement; it does not reopen the shared-depth correction. No
game-scene, Android, or XR adoption has started.
Post-review Explorer evidence then showed that Slice 3B's viewer-forward
anchor was useful for foreground diagnosis but confusing as the product
default: it moves exact residency when yaw changes and can place exact terrain
below the low-angle viewport. Commit `c75b488b` restores the orbit focus as
the native/browser default composition anchor and retains the old placement
through explicit `viewer-forward` options. This changes proof-host placement,
not the accepted shared-depth or whole-record ownership contracts.
Tactical
[`266`](../tactical/266-terrain-lab-runtime-composition-adoption.md) now
extracts the exact renderer, composition session, and browser executors from
World Explorer into `mclone-terrain-view`. Terrain Lab consumes them through
an optional `runtime` pane while retaining its four research panes and prior
defaults. The hosted desktop and Pixel 7 gates reach 25/25 exact chunks,
non-empty whole-tree exact/proxy ownership, and all 160 horizon slots at the
fixed low-angle review site. This is the PH-3 reusable-consumer checkpoint;
hosted Human Review 1 accepted it on 2026-07-27. PH-3 is complete, PH-4
shared terrain-view engine extraction plus full-game scene adoption is active
in Tactical
[`269`](../tactical/269-shared-terrain-engine-scene-adoption.md), and
Android/XR promotion remains later. PH-4 must make World Explorer and
`mclone-scene` peer hosts of one terrain representation/composition owner,
not preserve a proof renderer and a game renderer that merely exchange the
same coverage DTO. Canonical exact generation and live authoritative render
sections remain different truth-source adapters.
Interactive review on 2026-07-26 diagnosed two remaining proof defects and
activated Tactical
[`252`](../tactical/252-procedural-horizon-seams-and-transition-admission.md):
tile-edge normal calculations clamp to local samples and visibly split
lighting, while aligned multi-level movement can expose requested origins
before all entering strips are ready and show coarse fallback for one frame.
Slice 1 now provides fixed requested/staged/committed admission with seven
guard resources per level. Terrain and asynchronous vegetation retain
separate complete presentations and commit atomically without an uncovered
frame. Slice 2 stores a fixed two-sample normal halo while retaining the
`65x65` drawn grid. Same-LOD borders use identical absolute neighbor samples;
a two-cell fine-ring collar converges to the adjacent coarse normal footprint
at their shared edge. A dedicated scalar `69x69` normal-height field keeps the
semantic sample grid at `65x65`; the combined fixed allocation is
`128,837,720` bytes and does not rely on a larger per-frame dispatch budget.
The later shared-projection matrix adds `29,440` bytes across fixed terrain and
tree uniforms; the current World Explorer reports `128,867,704` fixed bytes
including the exact-coverage resources.
Native/offscreen plus headed desktop and Pixel 7 browser closeout passed.
Side-by-side native/browser review also found three proof-host parity gaps.
Completed parent Tactical
[`253`](../tactical/253-world-explorer-cross-host-parity.md) sequences their
independent corrections. Tactical
[`255`](../tactical/255-world-explorer-color-output-parity.md) completed the
shared display-space color contract on 2026-07-26. Terrain, material/river
overrides, tree proxies, and the background now use one generated target
transform, preserving the selected dark appearance on UNORM and sRGB targets.
Tactical
[`256`](../tactical/256-shared-horizon-vegetation-worker-topology.md) replaced
native synchronous/browser-disabled tree compilation with one shared
engine terrain-vegetation coordinator over native-thread and browser-Worker
executors. It completed on 2026-07-26 and explicitly defines World Explorer as
the first proof host, not the final owner. Native uses one named
bounded-channel thread; browser uses an isolated Rust actor, the shared opaque
Worker transport, and a persistent external-SAB result arena. The
coordinator, compiler session, job identity, cache policy, and prepared result
handoff belong in shared crates so a later `mclone-scene` tactical can consume
the same service. A pinned native/offscreen/desktop-browser/phone-browser
receipt agreed exactly on the semantic source, `2,609` records/instances,
family counts, record hash, and `281,772` proxy vertices; failure/restart,
overflow, large coordinates, and shutdown also passed. UI-less host cleanup
completed in Tactical
[`254`](../tactical/254-ui-less-world-explorer-host.md) and remains owned by
the platform-host topic rather than terrain rendering.

Tactical 261 is the macro source of truth for past and future work. Active
child Tactical
[`262`](../tactical/262-world-explorer-exact-procedural-composition.md)
defines one renderer-neutral exact-painted snapshot, caller-owned shared
target, coverage-mask lifecycle, and true World Explorer composition before
the first full-game pixels. Later children own `mclone-scene` adoption,
exact/proxy vegetation XOR, world and device lifecycle, browser/Android
promotion, synthetic stereo, desktop OpenXR, Quest, and full-frame multiview.
Completed proof tacticals remain historical records and are not reopened for
that integration.

## Scope

This topic owns the proposed runtime composition for presenting untouched
natural terrain beyond exact chunks:

- fixed-budget multiscale terrain around a moving observer;
- toroidal geometry-clipmap residency and incremental updates;
- seams between procedural levels;
- exact-chunk replacement of procedural terrain;
- procedural vegetation handoff;
- XR-safe scheduling, coordinate precision, and frame admission; and
- the shared ownership boundary between Terrain Lab and the game.

The terrain field, structured worldgen semantics, and current GPU evidence
remain owned by
[`gpu-procedural-terrain.md`](gpu-procedural-terrain.md). Player-facing map,
orbit, tabletop, touch, and enter-world behavior belong to
[`world-view-navigation.md`](world-view-navigation.md). This topic does not
revive any type or lifecycle from the removed chunk Far LOD.

## Recorded Direction

Use a **toroidal geometry clipmap** as the first in-game procedural-horizon
proof.

Each level has:

- a fixed grid resolution;
- a power-of-two world-space sample spacing;
- a snapped world origin;
- a fixed central hole occupied by the next finer level; and
- fixed GPU storage addressed as a two-dimensional ring buffer.

This gives the runtime a fixed upper bound on resident samples, geometry, draw
count, and per-boundary-crossing updates. Those properties are more important
to the first XR proof than adapting every patch independently to the camera.

A quadtree remains valuable as:

- an adaptive Terrain Lab comparison mode;
- a possible map or tabletop presentation where variable work is acceptable;
- a reference for future terrain-edit summary roll-ups; and
- a later hybrid candidate if measurement identifies waste in fixed rings.

This is a preferred first proof, not a claim that a quadtree can never be used.
The two representations must share terrain evaluation and summary semantics;
they must not become unrelated game and Lab generators.

## Why This Is Not The Removed System

The rejected system retained one identity and lifecycle per 16×16 chunk at
every distance. Reducing vertices inside each chunk did not reduce the number
of covered chunk identities, requests, arbitration decisions, or resident
objects.

A clipmap instead keeps roughly the same number of samples at every level.
Each coarser level increases sample spacing and therefore world-space
footprint:

```text
level 0: N x N samples at spacing S
level 1: N x N samples at spacing 2S
level 2: N x N samples at spacing 4S
...
```

The maximum horizon is consequently bounded by ring count, grid size, and
sample spacing rather than by allocating every chunk beneath the view. A
100–200 km target is a budget and visual-quality question, not a requirement
to create millions of chunk-granular LOD residents.

## Shared Terrain Source

The current reuse is already substantial:

- [`mclone-terrain-view`](../../native/crates/mclone-terrain-view/Cargo.toml)
  depends on `mclone-core`, `mclone-worldgen`, and `wgpu`, and owns terrain
  evaluation, viewport planning, sample buffers, canonical comparison, grid
  rendering, and tree summaries;
- [`mclone-terrain-lab`](../../native/apps/mclone-terrain-lab/Cargo.toml)
  combines that crate with production assets, block catalogue, exact
  generator, mesher, and shared renderer; and
- the Lab's exact pane is therefore a small production render host, not a
  TypeScript imitation of terrain generation.

The Lab is not a complete game engine. It deliberately lacks authoritative
client/server simulation, persistence, propagated world light, collision,
entities, ticks, and protocol lifecycle. React, URLs, Worker orchestration,
diagnostic readbacks, and cheap preview lighting remain host-specific.

The implementation direction is one shared Rust terrain-view service with
multiple consumers:

```text
worldgen semantic source
        |
        v
shared terrain-view / clipmap service
        |
        +-- Terrain Lab and World Explorer
        +-- minimal native renderer diagnostic
        +-- game scene and render session
```

Lab-first means using the Lab as the fastest visual and instrumentation host.
It does not mean landing a second Lab-only clipmap implementation.

Terrain Lab's exact generated-chunk pane has its own
`CanonicalTerrainWorkerCoordinator`. That coordinator is a useful precedent
for isolated Rust actors, cache/session ownership, epochs, bounded admission,
and external-SAB result publication. It is not the procedural LOD service to
move into the game. The shared procedural CPU/GPU products and viewport
contracts are the reusable system.

The full web game's `WebRenderWorkerCoordinator` is likewise a specialized
exact render-section owner, not a universal worker framework. It contributes
generic browser transport and lifecycle evidence, but procedural terrain must
not become an artificial render-section job to reuse it.

Tactical 247 proved the current bounded renderer through both a real native
surface and an offscreen target in the standalone World Explorer. Its
continuous-movement closeout peaked at 171,994,752 resident bytes and 159
tiles, then settled to zero pending work. That is useful portability and
control evidence, but it is not the fixed-memory moving-horizon contract.
The first ring should replace this growing bounded-preview residency in both
proof hosts rather than expand its tile budget.

## Reusable Terrain-LOD Streaming Service

World Explorer proves host portability and remains useful on its own, but it
is not the ownership boundary. Procedural terrain streaming should compose as:

```text
Terrain Lab / World Explorer / Universe preview / game scene
                    |
     shared terrain-LOD coordinator and products
                mclone-terrain-view
                    |
     semantic evaluator and compiler sessions
                 mclone-worldgen
                    |
          native thread | browser Worker
```

The shared service accepts semantic desired tiles and explicit admission
tokens. It owns priority, source identity, budgets, stale rejection, cache
lifecycle, failure state, and prepared products without depending on a WGPU
device, browser API, `winit`, or one app crate. A presentation owner uploads
admitted products afterward.

The crate dependency remains one-way: terrain-view owns
`TerrainViewportTileId`, slot generations, and admission, while worldgen owns
validated compiler requests, semantic source/product revisions, compiler
sessions, and product encoding. Browser correlation fields are fixed-width
opaque scalars at the worldgen codec boundary, not a reason for worldgen to
import terrain-view.

The service API must not bake in World Explorer's current ten-level,
four-by-four default. That configuration is the first measured consumer. The
coordinator derives bounded work from a validated desired set so Terrain Lab,
the Explorer, a later Universe detached-preview presentation, and the game
scene can use different measured ring budgets without different scheduling
semantics.

Platform adapters choose execution mechanics:

- native moves typed jobs and results through bounded channels to one worker
  thread in the first implementation; and
- browser keeps isolated Wasm heaps and publishes encoded results through an
  explicit external `SharedArrayBuffer`.

The same logical service does not require every domain to share one physical
thread or Worker. Exact chunk meshing and procedural vegetation have different
resident state and failure lifecycles. A later measured pool may host multiple
services behind unchanged domain coordinators, but Tactical 256 does not
create a universal job enum or Worker framework.

### One engine with multiple truth sources

The reusable boundary is broader than sharing a clipmap scheduler or an
exact-painted snapshot. Explorer, Terrain Lab runtime composition, the live
game scene, and a future Universe overview should consume one logical terrain
representation engine in `mclone-terrain-view`. That engine owns procedural
residency and admission, exact/procedural coverage and frontier policy,
bounded representation ownership, stale rejection, and immutable prepared
terrain-frame products.

The engine receives exact facts through narrow adapters:

- Explorer and Terrain Lab use a detached canonical source reconstructed from
  a qualified generator recipe and explicitly carry no edit authority.
- `mclone-scene` adapts authoritative client-replica render sections,
  readiness, edits, revisions, and topology while retaining live session and
  frame orchestration.
- A later bounded observer source may consume server-published facts without
  assuming that a seed or generator recipe is available.

The current `TerrainRuntimeExactRenderer` is a transitional proof aggregate.
PH-4 should separate its canonical producer/residency responsibilities from
the generally reusable coordination and draw-preparation path. It must not
become a permanent Explorer-only terrain implementation beside a separate
scene compositor. Equally, the shared-engine requirement does not justify
running `McloneSceneHost`, an integrated server, persistence, or networking in
the lightweight Explorer.

This is logical runtime unification, not a requirement for one physical
thread, Worker, or WGPU allocation. Hosts may choose different view policies,
budgets, and platform executors while consuming the same source-qualified
composition contract. Whether detached and live hosts can safely retain or
transfer GPU resources is a later measured optimization.

The first shared off-thread slice is individual vegetation record planning.
Coarse continuous forest summaries remain part of terrain evaluation and do
not enumerate trees. Terrain evaluation, summary filtering, vegetation
records, and later exact/procedural arbitration remain separable layers behind
one source identity.

## Geometry And Residency

### Fixed nested rings

Every level uses reusable grid topology. The inner level covers the observer;
each outer level draws only the annulus outside the finer level's fixed
footprint. The central holes are part of the mesh/index pattern, not
per-frame Boolean cuts.

Rings should:

- use power-of-two spacing;
- snap origins to their own spacing;
- align coarse and fine sample coordinates where they meet;
- keep a small guard region beyond the visible ring; and
- evaluate terrain from absolute world coordinates, never by stretching
  already-sampled fine data.

The exact grid dimensions and number of levels remain measured settings. The
current Terrain Lab maximum is evidence for broad coverage, not an automatic
100–200 km runtime configuration.

### Two-dimensional toroidal addressing

The GPU allocation stays fixed while its logical world origin advances. A
logical world sample coordinate maps to a physical slot modulo the grid
dimensions:

```text
physical_x = world_sample_x.rem_euclid(grid_width)
physical_z = world_sample_z.rem_euclid(grid_height)
```

The implementation must use Euclidean modulo so negative world coordinates
wrap consistently. A separate snapped logical origin determines which
world-space interval the fixed slots currently represent.

Moving the camera inside a level's sample cell changes no sample residency.
Crossing a snapped boundary exposes only:

- one entering column for movement in X;
- one entering row for movement in Z; or
- one row and one column, with their shared corner generated once, for
  diagonal movement.

The opposite edge is not copied or shifted. Its physical slots are simply
reinterpreted and overwritten as the new entering strip. This is the
two-dimensional extension of an ordinary circular buffer.

### Requested and committed origins

Do not expose a new origin before its entering strips are drawable. Each level
needs explicit state resembling:

```text
resident origin
requested origin
pending strip work
committed origin
source identity and revision
```

Movement requests work against the next snapped origin. Compute and uploads
populate the wrapped entering slots under a bounded budget. Once all required
strips and summaries for that step are ready, the level atomically exposes the
new origin. A guard margin lets ordinary movement remain within already
prepared coverage.

Tactical 252 implements this as 16 logical and seven guard resources
per level. The guard bound is the non-duplicated entering set for a one-tile
diagonal move. Requested origins converge one tile at a time under the
existing dispatch budget, while the committed terrain presentation retains
all 16 old resources until the replacement set is complete. Vegetation owns a
separate committed presentation so Worker latency can retain the prior valid
forest without delaying terrain or exposing a forest-free frame.

A teleport or world switch is different from a one-cell move. Cancel stale
work, establish source identity, refill coarse levels first for immediate
coverage, and progressively admit finer levels. Never let old-source slots
become valid merely because their toroidal indices match.

This still needs a small scheduler or admission queue. It does not need the
old system's area-proportional chunk request graph.

## Level Fidelity

Coarser rings must sample lower-frequency terrain meaningfully. Merely calling
the point evaluator less often can alias coastlines, peaks, water, and
vegetation.

The shared source therefore needs footprint-aware, band-limited summaries
appropriate to each spacing, including eventually:

- representative or conservative terrain height;
- surface material or biome mixture;
- water presence and water surface facts;
- silhouette-preserving extrema where useful; and
- stable vegetation density and instance summaries.

Level selection and ring count should be tied to projected screen-space error,
field bandwidth, and target device budget. They should not be described as
fake chunk render distances.

Large coordinates also require an explicit precision policy. CPU decisions
should retain integer or double-precision world positions. Shaders should use
camera-relative coordinates or a high/low origin decomposition so distant
terrain does not jitter as the observer moves far from world zero.

Tactical
[`250`](../tactical/250-continuous-explorer-presentation-and-cadence.md)
completed the first explicit presentation/residency split on 2026-07-26.
Fractional camera focus and scale update every frame, while toroidal origins
change only at aligned 64-block finest-tile boundaries. Shaders receive a
nearby integer anchor plus a small fractional remainder so camera motion
stays continuous without converting a large absolute `f64` coordinate
directly to `f32`. Terrain and tree vertices share that transform.

Uniform slots grew from 144 to 160 bytes, making the unchanged 160-slot fixed
allocation `86,553,600` bytes. Same-tile two-contact motion changed exact
fractional focus without changing revision 1, 160 initial refills, 10 rebases,
or allocation. Held movement then crossed tile boundaries and produced
bounded entering-strip refills before returning to full readiness. Native,
offscreen, desktop-browser, phone-browser, negative-coordinate,
million-block, and hosted production captures show no unpainted hole,
fine/coarse seam, or terrain/tree separation.

## Seams And Transitions

Skirts are the accepted first seam treatment. Vertical faces are already a
natural part of block terrain, they tolerate imperfect neighboring summaries,
and they avoid making the first proof depend on a complex stitch topology.

The first implementation should combine:

- power-of-two sample alignment;
- a consistent ownership rule at fine/coarse boundaries;
- skirts deep enough to hide expected height disagreement; and
- optional fog or material blending when it improves the horizon.

Crack stitching and geomorphing remain later refinements if measured captures
show skirts are insufficient. Avoid alpha-crossfading two opaque heightfields
at the same depth; it invites overdraw, z-fighting, and double silhouettes.

In a quadtree comparison mode, one active leaf set covers each extent exactly
once. A parent remains visible until all replacing children are ready, then
the parent is removed. The parent is not drawn behind children with several
dynamic holes. Enforce a 2:1 neighboring-level constraint and use the same
skirt policy.

## Exact Terrain Handoff

The game combines two different spatial structures:

- clipmap holes remove portions covered by **finer procedural levels**; and
- one dynamic exact-coverage mask removes portions covered by **real chunks**.

Only the second is scene-dependent.

### One frame snapshot

The exact draw list and procedural coverage mask must be derived from the same
immutable frame snapshot. A chunk becomes `exact-painted` only when all
resources needed to draw it this frame are ready. Generated, loaded, resident,
or compiling is not enough.

The scene publishes a small world-space coverage texture, bitset, or
`R8Uint`-like mask around the exact frontier. Procedural fragments convert
world XZ to the same chunk coordinates and discard when the mask says exact
terrain owns that position. CPU planning may omit whole procedural patches
that are completely covered; the mask handles the ragged boundary.

This is cheap relative to terrain shading because it is a small indexed lookup
and branch, not mesh/mesh intersection or polygon clipping. It must still be
measured on mobile and XR GPUs.

Depth testing alone is not a correct arbitration mechanism. Approximate
terrain can be above the exact surface, can z-fight where close, and can
incorrectly hide caves or edited silhouettes.

### Draw order

The intended opaque composition is:

1. sky and background;
2. masked procedural terrain;
3. exact opaque and cutout terrain using the same depth convention;
4. actors and vegetation; and
5. translucent terrain and water.

Exact terrain needs a narrow frontier skirt or collar so disagreement between
the exact boundary and the procedural sample beneath it cannot expose a crack.

### Vegetation

Far trees and other terrain-native proxies use the same procedural source and
coverage snapshot. A proxy is removed only when all exact chunks intersecting
its footprint are drawable in the replacing exact representation. An
owner-chunk shortcut is incorrect for trees that cross chunk boundaries.

Tree proxy removal and exact tree admission should be atomic from the
observer's perspective. Later density or clustered representations may change
by level, but their placement identity must remain stable enough to avoid
sparkling during movement.

## Natural Terrain Only In The First System

The first procedural horizon intentionally represents untouched natural
terrain only.

When an edited exact chunk is drawable, its real blocks appear. When it leaves
the exact range, the procedural natural surface returns. A distant tower may
therefore pop out or disappear. That limitation is acceptable because the
first product purpose is the wow factor of exploring broad natural terrain.

Do not add speculative edit listeners, dirty ancestor propagation, persistent
LOD databases, distant build silhouettes, or structure-proxy APIs to the first
implementation.

A future edit system can be modeled separately as:

```text
procedural natural base
        +
sparse authoritative edit / structure overlay
```

A quadtree summary roll-up is a plausible future mechanism: mark an edited
leaf dirty, recompute its compact summary, and propagate changed summaries to
parents. Tall buildings may be better represented by explicit sparse
structure silhouettes than by raising a terrain heightfield. Those are future
research questions and must not block natural-terrain proof.

## XR And Frame Predictability

The procedural horizon is view-independent world geometry and is generated
once per frame state, not once per eye. Both XR eyes consume the same committed
ring set with their own view/projection data. Every renderer addition must
support normal mono/per-eye rendering and full-frame multiview, or explicitly
document an unavailable mode.

Residency should be centered on a locomotion/body anchor or stabilized
head-space anchor. Raw per-eye positions and normal head wobble must not
request entering strips.

Frame admission needs explicit budgets for:

- compute evaluation;
- summary and vegetation work;
- GPU upload or buffer writes;
- origin commits; and
- device-loss rebuild.

The system should prefer holding the previous valid coverage over exposing a
partially updated ring. Coarse-first recovery is more useful than isolated fine
patches.

## Shared Ownership

The intended ownership split is:

- `mclone-worldgen`: semantic terrain source, profiles, source identity, and
  footprint-aware evaluators, plus stateful vegetation compiler sessions and
  bounded semantic caches;
- `mclone-terrain-view`: clipmap geometry math, snapped/toroidal addressing,
  sample planning, procedural summaries, WGPU-independent terrain-product
  coordination, stale/admission policy, and a renderer-neutral prepared draw
  service;
- `mclone-render`: reversed-Z-compatible terrain, fog, material, vegetation,
  mono/per-eye, and multiview pipelines;
- `mclone-render-session`: resident GPU lifecycle, pending/committed origins,
  device rebuild, and bounded upload/compute admission;
- `mclone-scene`: exact/procedural arbitration, frame snapshot, world
  switching, locomotion anchor, and cross-feature budgets; and
- app hosts: surface/session cadence, platform events, diagnostics, and
  presentation only.

Exact ownership may move as shared contracts become concrete. The invariant is
that no desktop, browser, Android, or XR app owns terrain semantics or a
private LOD policy.

## Work Streams

The work should proceed as independently reviewable slices:

Tactical
[`247`](../tactical/247-standalone-world-explorer-foundation.md) proved
that `mclone-terrain-view` and platform-neutral navigation compose into a small
native application. Tactical
[`249`](../tactical/249-cross-platform-procedural-horizon-proof.md) owns Steps
2–4 as a cross-platform Explorer proof. It keeps the system in shared Rust and
requires native and browser evidence rather than making either platform the
semantic owner.

1. **Protect the old-system removal boundary.** Finish any branch-local
   cleanup before starting the proof, keep Tactical 245 closed, and do not
   retain compatibility types for an unimplemented replacement.
2. **Prove toroidal addressing — complete in Tactical 249.** Shared tests
   cover negative coordinates, rows, columns, diagonal movement, long wrapping
   walks, retained slots, and teleports.
3. **Draw nested rings in the native and browser Explorer — complete.** Reuse
   the current evaluator and shared renderer while leaving Terrain Lab
   unchanged. The browser adapter must remain suitable for either this small
   product or the full web game.
4. **Promote the renderer contract — first proof complete.** Reversed-Z,
   caller-owned targets, and fixed-capacity diagnostics are proven across
   native and browser. Device rebuild, synthetic stereo, and multiview remain
   hardening work; the Explorer is a cross-platform proof host, not another
   disposable diagnostic.
5. **Harden nested levels.** Fixed aligned holes, coarse-first refill, and the
   first large-coordinate camera-relative precision path are proven. Add
   explicit skirts where independent surfaces require them, retained
   committed origins, footprint summaries, and device-specific budgets.
6. **Build the reusable vegetation streaming service — complete.** Tactical
   256 moved
   semantic tree-record compilation behind one shared coordinator, one native
   thread executor, and one isolated browser Rust actor. World Explorer proves
   identical source identity, records, failure behavior, and presentation
   without becoming the service owner.
7. **Prove exact/procedural composition, then integrate the game scene.**
   Tactical 262 first combines a reusable canonical exact view and the horizon
   on one World Explorer target. Tactical 261 then sequences `mclone-scene`
   adoption, proxy/exact vegetation XOR, edit invalidation, lifecycle
   recovery, and all-target frame admission.
8. **Measure an adaptive comparator only if useful.** A quadtree Lab mode
   should answer a specific waste or quality question, not fork the content
   system.

Shared navigation and the World Explorer product can advance alongside these
slices through [`world-view-navigation.md`](world-view-navigation.md), but
neither should force clipmap ownership into UI code.

## Validation

Each rendered slice requires an inspected capture at its first drawable
milestone. The eventual acceptance set includes:

- stationary, single-axis, diagonal, high-speed, and teleport movement;
- negative coordinates and large distances from world zero;
- no unpainted holes during delayed strip generation;
- source/profile/world switches with stale completion rejection;
- ring seams across coast, mountain, water, and forest cases;
- exact-chunk admission, eviction, edits, and vegetation crossing the mask;
- device loss and surface rebuild;
- desktop and headed-Wayland browser evidence;
- flat Android and Android XR scripted build/validation lanes;
- mono, synthetic stereo, per-eye XR, and full-frame multiview;
- bounded work and stable frame-time evidence under continuous locomotion; and
- a protected feature-off ordinary exact-terrain path.

Queue emptiness, requested origins, or a color-only screenshot are not
sufficient evidence. Coverage, depth, work counts, and committed-source
identity must be observable.

## Flight-Simulator Reference Posture

Microsoft Flight Simulator is a useful experience and progressive-delivery
reference, but not the intended Mclone storage architecture. Microsoft's
official material describes cloud-streamed world data and a rolling cache;
Mclone can reconstruct the untouched natural base from a compact world seed
and generator revision.

Reuse the product lessons—coarse-first coverage, progressive refinement,
predictive movement, and optional caching—without assuming a planet-scale
stored imagery pipeline. See the official
[Microsoft Flight Simulator 2024 FAQ](https://www.flightsimulator.com/microsoft-flight-simulator-2024-faq/)
and
[release/preorder description](https://www.flightsimulator.com/msfs2024-preorder-now-available/).

## Open Questions

- What grid dimension, level count, guard size, and update budget give useful
  horizons on desktop, browser, phone, and Quest?
- Which footprint summaries best preserve coastlines, mountain silhouettes,
  water, and forest character at each level?
- Should samples live as height/material textures, structured buffers, or
  generated vertex data on each backend?
- Does the first ring renderer belong directly in `mclone-render`, or should
  `mclone-terrain-view` initially own a narrowly reusable WGPU presentation
  while the contract settles?
- How should water surfaces transition without slopes or shoreline flicker?
- When are skirts insufficient enough to justify stitching or geomorphing?
- What is the smallest exact-coverage representation that remains cheap and
  correct at the frontier?
- How much compute actually overlaps rendering on each `wgpu` backend?
- Which source descriptor can a remote server expose when its seed or
  generator details are private?

## Related Documents

- [`universe-product-shell.md`](universe-product-shell.md) — detached-preview
  product role, authority transitions, and preview-truth requirements.
- [`gpu-procedural-terrain.md`](gpu-procedural-terrain.md) — terrain
  evaluation, Terrain Lab evidence, exact comparison, and GPU research.
- [`far-lod.md`](far-lod.md) — rejected chunk system and removal boundary.
- [`lod-native-vegetation.md`](lod-native-vegetation.md) — forest intent,
  stable tree records, exact realization, and procedural summaries.
- [`world-view-navigation.md`](world-view-navigation.md) — shared map/orbit
  controls and Explorer-to-play product path.
- [`tabletop-overview-mode.md`](tabletop-overview-mode.md) — active-world scale
  model and scene-owned overview policy.
- [`performance.md`](performance.md) — budgets, profiling, and regression
  evidence.
- [`platform-parity.md`](platform-parity.md) — all-target ownership and
  validation.
- [`../native-engine-architecture.md`](../native-engine-architecture.md) —
  shared renderer, scene, and host boundaries.
- [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md) — frame
  work admission and accounting.
