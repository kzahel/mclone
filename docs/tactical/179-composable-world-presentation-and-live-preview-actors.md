# 179: Composable World Presentation And Live Preview Actors

Status: proposed 2026-07-14. No implementation slice has started.

Topic: `embedded-worlds`

Workstream: shared native Rust and native web/WASM render contracts, scene
composition, actor presentation ownership, and portable authored scenario
content. Desktop offscreen is the fastest pixel-development lane, production
browser WebGPU is a per-slice acceptance lane, and synthetic stereo plus the
shared multiview implementation protect XR. Platform apps remain target,
storage, Worker/thread, and event-loop adapters.

## Decision

Extend the landed live-diorama terrain proof into the first renderer-neutral
world-presentation composition milestone:

1. normalize the existing placed-terrain path around one small shared
   composition context;
2. prove that the context can express both the current scaled/bounded diorama
   and complementary half-space terrain coverage without taxing ordinary
   single-world rendering;
3. separate actor renderer resources into shared immutable topology and
   per-world mutable presentation/cache state;
4. make actors consume the same placement, source-selection, and clip
   semantics as terrain; and
5. publish live destination entities and players through the existing
   mono/per-eye/multiview scene phases.

This is not a diorama-only actor renderer. The same actor submission must work
for a miniature destination and for an eventual unscaled `x=0` split. The
half-space proof in this tactical is renderer-only and uses a deliberately
open/authored seam; boundary-face resolution, collision, and authority handoff
remain separate later milestones.

The existing direct terrain and actor shaders remain the feature-off path.
Composition-aware placement or clipping is selected only when a second world
presentation is actually submitted.

## Milestone

The tactical is complete when all of the following are true:

- `WorldPlacement` remains the one scale/translation contract used by terrain
  and actors. Neither actor code nor a platform adapter invents a second model
  transform.
- Source interest/residency, coarse source selection, exact composition
  clipping, camera-frustum culling, and depth occlusion are represented as
  distinct concepts.
- The existing live lobby diorama still renders destination opaque, cutout,
  and translucent terrain through shared depth on native and production web.
- A renderer fixture submits two terrain sources with complementary
  composition-space half-spaces in mono, synthetic stereo, WebGPU, and the
  full-frame multiview implementation. The fixture does not claim to solve
  clipped voxel boundary faces.
- Compatible world slots share actor textures, figures, shader/pipeline
  topology, and layouts. Each slot retains only the mutable actor instances,
  CPU/GPU mesh cache, interpolation/presentation state actually required for
  isolation.
- The ordinary one-world frame still takes the existing direct actor path and
  does not construct a composition context, clipped pipeline, second actor
  cache, or world-qualified actor collection.
- Destination cows, chickens, item actors, remote players, and the
  destination slot's own joined-player representation use the same
  composition-aware actor submission.
- Destination actors are selected only from the preview source region, sample
  lighting from their source world, scale with the miniature, and share depth
  correctly with both worlds' terrain.
- A real authoritative entity in the destination visibly changes position and
  animation as ordinary server updates reach the retained runtime. Renderer
  motion is not fabricated in an app or diagnostic overlay.
- A second ordinary joined player is published through the existing remote
  player protocol, moves under ordinary server authority, and visibly moves in
  the composed destination. A player-shaped NPC or renderer-only test actor is
  not an acceptable substitute for this end-to-end receipt.
- The destination slot's own joined player appears as a full body while the
  slot is viewed from another world. It is not expected to consume active-world
  input or continue walking while that slot lacks physical authority.
- Whole-slot A-to-B-to-A activation carries actor presentation/cache ownership
  with the world identity: no actor appears twice, disappears because ids
  collide, or forces an atlas/pipeline rebuild at the switch.
- Native and browser feature-off performance remains within the contract below,
  while actor-on CPU rebuild/upload cost, GPU bytes, and browser frame impact
  are reported separately.

## Experience Contract

The product lobby remains the first user-facing sample:

```text
active lobby A
  ordinary full-scale terrain and actors
  one physical camera/input/UI authority

destination island B
  ordinary retained authoritative runtime
  bounded terrain and actor source region
  miniature WorldPlacement on the four-block table
  no physical movement or interaction authority while retained
```

When B contains authoritative entities, they appear on the table and follow
their normal server snapshots. Remote players already tracked by B's client
replica also appear. The source connection's own player is normally omitted
from `ClientRuntime::actor_presentations()` because first-person clients do not
receive themselves as a remote player; scene composition therefore derives
one full-body source-local player presentation from B's accepted player/camera
state. This is presentation only and does not change authority.

After activation, B becomes the ordinary active world. Its local player again
follows the existing first-/third-person visibility rules, while A becomes the
preview and may show A's source-local full body on the return table. Remote
players and entities remain world-local across the exchange.

The fixture should contain at least one persisted ordinary passive entity in
the island source region so an interactive launch always demonstrates the
feature. Use the existing authoritative entity store and supported species;
do not add a decorative actor recipe. Version managed content
non-destructively so an existing v1 lobby/island database is never silently
rewritten as v2.

## Explicit Non-Goals

- No player, inventory, health, velocity, or entity authority transfer between
  worlds.
- No physical collision or block/entity interaction against preview content.
- No complete walkable split-world product, portal aperture, or stencil
  renderer.
- No arbitrary-plane voxel cutting, cap generation, boundary-face resolver, or
  combined cross-world voxel mesher. The half-space proof uses an authored open
  seam.
- No second sky, clouds, Far LOD, world UI, selection outline, particles,
  audio, or cross-world dynamic lighting/shadows for the preview.
- No general N-world registry. The scene retains one active and one optional
  preview slot.
- No recursive preview composition.
- No GPU skinning, general instancing rewrite, or animation-system campaign.
  The first composed actors preserve the current figure/mesh semantics and
  measure their existing whole-cache rebuild behavior.
- No browser-only or native-only actor/composition policy.
- No browser WebXR product adoption. Browser flat WebGPU is required; native
  per-eye and full-frame multiview parity remains required by the shared
  renderer contract.
- No remote observer protocol or completion of Tactical 175's remote-preview
  source. The moving-player proof may use an ordinary joined multi-client
  server fixture through the existing protocol.

## Current Ground Truth

### Already reusable

- `mclone-render::placement::WorldPlacement` validates a finite positive
  uniform scale and finite `f64` source/composition anchors, derives the
  inverse source-local camera, and preserves source rebase ordering in the
  placed shaders.
- `EmbeddedChunkRegion` bounds the retained terrain records and fixed source
  interest used by the live diorama.
- `TexturedSectionDrawResources` is per world. Compatible slots already share
  `Arc<TexturedSectionSharedResources>` while retaining independent section
  buffers, record caches, cull scratch, traversal, and uploads.
- Placed terrain already has opt-in solid, cutout, and translucent mono/per-eye
  and full-frame multiview variants. The no-preview branch does not construct
  them.
- Scene composition already orders both worlds' opaque/cutout terrain, active
  actors, and globally sorted A/B translucent terrain in one physical color
  and depth target.
- The production browser drives the same `McloneSceneHost`, two retained
  runtimes, shared terrain renderer, placement, readiness, activation, and
  complete-slot exchange as native.
- `ClientRuntime::actor_presentations()` already exposes authoritative
  entities and remote players. `mclone-render-session` maps both to
  `ActorInstance`s with source-world packed light.
- The server already publishes remote-player add/update/remove records between
  dedicated joined players and publishes entity snapshot/update/remove records
  according to tracked chunk interest. Its current remote-player tracker omits
  the local integrated `CommandTarget` as both observer and subject; the
  managed island therefore needs that shared server gap closed before a real
  auxiliary joined-player witness can appear to its local source client.
- `ActorDrawResources` already supports direct mono/per-eye and full-frame
  multiview drawing, asset-lab player/chicken figures, cows, debug cubes, and
  items.
- `ActorInterpolationState` exists as a client-side presentation primitive,
  though the production scene currently gathers raw current presentations.
  This tactical must characterize that fact before choosing whether to adopt
  interpolation; it must not silently turn every moving actor into a
  per-render-frame full mesh upload.

### Gaps to remove

- `McloneSceneHost` owns one global `actors: ActorDrawResources` instead of
  actor draw/cache state moving with each `DrawableWorldSlot`.
- `current_actor_instances()` reads only `active_world.runtime` and applies
  local-player visibility only to the active view.
- `ActorDrawResources` combines immutable renderer pipelines, atlas, texture
  layout, and figures with one mutable `ActorMeshCache`. Rendering A then B
  through that cache would rebuild and upload both whole lists every frame.
- Actor vertices are CPU-built in absolute source-world coordinates. Actor
  shaders multiply them directly by the physical view-projection and have no
  `WorldPlacement`, composition position, source bounds, or clip plane.
- Actor collection allocates a fresh `Vec` and uses unqualified presentation
  ids. Per-slot ownership avoids collisions locally, but aggregated scene
  diagnostics/submissions must qualify identities with `WorldInstanceId`.
- The existing terrain composition APIs carry placement and bounded records as
  separate terrain-specific arguments. There is no shared vocabulary an actor
  renderer can consume without reproducing those concepts.
- The live island authored fixture currently contains chunk records only.
  Managed web storage can validate entity-chunk records, but the portable
  scenario payload does not yet author or provision them.
- `RemotePlayerTracking` currently treats only dedicated players as visible
  subjects/observers. The local integrated player has a stable server player id
  and tracked chunk view but does not participate in those pairings, so a
  multi-client managed-island proof cannot be faked solely at the renderer.
- No production browser smoke currently requires actor pixels inside the
  miniature or proves a player figure through the placed path.

## Architecture Contracts

### Composition context

Keep world identity and runtime policy in `mclone-scene`, but add a small
renderer-neutral context in `mclone-render`. Provisional names:

```rust
struct WorldCompositionContext {
    placement: WorldPlacement,
    source_bounds: Option<WorldSourceBounds>,
    clip: CompositionClip,
}

struct WorldSourceBounds {
    min: Vec3d,
    max: Vec3d,
}

enum CompositionClip {
    Unbounded,
    HalfSpace(CompositionHalfSpace),
}

struct CompositionHalfSpace {
    // Retain points where dot(normal, composition_position) + offset >= 0.
    normal: Vec3,
    offset: f32,
}
```

Exact Rust names may change, but these separations may not:

- **runtime interest/residency** decides which source chunks and actor updates
  a client asks its server to retain;
- **source bounds** coarsely select source objects before GPU submission;
- **frustum culling** rejects selected objects outside the physical camera;
- **composition clip** exactly restricts submitted geometry in physical
  composition space; and
- **depth testing** resolves occlusion among the geometry that survives.

`EmbeddedChunkRegion` remains the terrain/runtime request shape. It may derive
a section-aligned `WorldSourceBounds`, but it must not become a universal actor
or particle identity. Actor membership follows the source entity's feet/owning
section, matching the existing tracking namespace; a diorama boundary does not
slice an animal merely because its model AABB protrudes past the final source
block.

`WorldPlacement` remains scale plus translation only. Do not add rotation or
non-uniform scale merely to make the new context look general.

### Terrain adoption and clipped variants

Refactor only the opt-in placed/composed terrain entry points to accept
`WorldCompositionContext`. Keep the ordinary direct terrain uniform bytes,
shaders, functions, culling scratch, and no-preview calls unchanged.

Use specialized pipeline variants:

- placed/unclipped: the current live diorama path;
- placed/half-space-clipped: the split-world proof; and
- direct/unplaced: the unchanged ordinary path.

Do not add a dynamic clip branch or extra uniform to every direct-world
fragment. WebGPU portability is the baseline: export composition position and
use a fragment plane test plus `discard` in the clipped variant rather than a
native-only clip-distance extension. Pair exact fragment clipping with CPU
section-bound rejection so fully excluded sections do not consume fragment
work.

The half-space fixture must use complementary planes over two independent
terrain stores and one shared depth target. Its seam is authored as air, a
frame, or a deliberate membrane so omitted neighbor faces cannot be mistaken
for solved boundary geometry.

### Boundary resolver handoff

Coverage answers “which source may contribute here.” It does not answer “what
surface exists where two voxel worlds meet.” Record the next boundary contract
without implementing it:

```text
BoundaryResolver
  AuthoredOpenSeam
  RetainBoundaryFaces(axis-aligned block/chunk plane)
  GeneratedMembrane
  future CombinedVoxelBoundary
  future ArbitraryCutAndCaps
```

The first real split-world tactical should begin with an axis-aligned
block/chunk plane and either retained boundary faces or a generated membrane.
It must not infer that a successful fragment-discard image solved meshing,
collision, or traversal.

### Actor resource ownership

Split the current aggregate along the same boundary already proven for
terrain. Provisional shape:

```rust
struct ActorSharedResources {
    atlas: GpuActorTextureAtlas,
    texture_layout: ActorTextureLayout,
    actor_figures: ActorFigureSet,
    pipeline_topology: ActorPipelineTopology,
}

struct ActorWorldDrawState {
    shared: Arc<ActorSharedResources>,
    mesh_cache: ActorMeshCache,
    presentation_scratch: Vec<ActorInstance>,
}

struct ComposedActorRenderer {
    placed_unclipped: optional pipeline/uniform state,
    placed_clipped: optional pipeline/uniform state,
    multiview counterparts,
}
```

Pipelines/layouts may live in a slightly different shared shell if WebGPU
binding ownership requires it. The non-negotiable result is one compatible
atlas/figure/pipeline owner and independent mutable caches. Per-world actor
vertex/index buffers are expected mutable state and must be measured. They are
created lazily for a retained actor-capable preview; feature-off construction
still allocates exactly one actor cache.

Actor cache, source-local player presentation state, and any interpolation
state move with `DrawableWorldSlot` during complete-slot exchange. Asset epoch
or device generation replacement invalidates the optional composed actor
topology before stale actors can draw. It must not duplicate immutable actor
assets for the standby.

### Actor placement, bounds, lighting, and clipping

Keep `ActorInstance` facts in source-world units. Current meshes are built
from absolute source positions; composed actor shaders apply
`WorldPlacement` to every generated vertex, which naturally scales position,
body dimensions, figure geometry, item geometry, and animation displacement
without rebuilding a second scaled CPU mesh.

The composed vertex shader exports composition position for physical fog and
optional half-space clipping. Packed light is sampled from the source slot's
`ClientRuntime` before placement, preserving B-local block/sky lighting.

Before mesh preparation:

- qualify the source with `WorldInstanceId` at the scene submission boundary;
- retain only actors whose source feet/owning section belongs to the bounded
  preview;
- reject actors whose transformed bounds are wholly outside the frustum or
  clip half-space when that can be decided cheaply; and
- preserve deterministic ordering so unchanged lists do not cause cache
  churn.

For a later split, an actor intersecting the plane may be geometrically clipped
by the renderer even though its authority remains wholly in one world. Entity
transfer and collision at the seam are deliberately not inferred from pixels.

### Player presentation policy

There are two player sources and they must remain distinct:

1. **Remote players** are ordinary `ActorPresentationKind::RemotePlayer`
   records from the source client replica. Their add/update/remove lifecycle,
   appearance, walk distance, and authoritative movement require no new
   protocol.
2. **The source slot's own joined player** is absent from remote-player records.
   While that slot is a preview, build one full-body actor from its accepted
   slot-local player/camera state and selected figure. Do not use the physical
   active camera or first-person hide rule for this source-local preview body.

The source-local preview body is not a remote observer, ghost authority, or
new server entity. It shows the pose of the player connection already retained
by that slot. While preview-only, that player continues to receive no physical
movement or interaction commands, so it may remain stationary. A moving-player
acceptance receipt therefore uses a second ordinary joined player whose real
remote updates reach the source replica.

After activation, the newly active local player resumes current first-/third-
person body rules. Ensure the source-local preview body and remote-player list
cannot contain the same server identity.

### Frame composition

`WorldSubmission` is scene vocabulary, not a literal call that renders an
entire world in isolation. The compositor must preserve global phases:

```text
active environment/sky and optional active Far LOD
all visible worlds' opaque/cutout terrain
all visible worlds' opaque/cutout actors
globally ordered visible-world translucent terrain
active-world selection, UI, HUD, screen effects, and audio presentation
```

The first implementation remains exactly A plus optional B. Do not introduce a
world registry or virtual dispatch in the ordinary path. The no-preview branch
continues to call the current direct full-frame functions.

Actor rendering currently uses alpha testing and depth writing rather than a
general translucent-material stack. If a future actor material needs blending,
it must join composition-level translucent ordering rather than being appended
per world.

### Motion and interpolation

“Live and moving” means the displayed pose changes because ordinary
authoritative entity or remote-player updates changed the source replica.
Record actor ids, source positions, update sequence, composed bounds, mesh
rebuild/upload counts, and pixel deltas across the movement witness.

Do not enable per-render-frame actor interpolation blindly. The current
`ActorMeshCache` CPU-builds and uploads one combined mesh whenever any
`ActorInstance` changes. Stepping every actor every display frame may therefore
turn interpolation into a full actor-buffer upload every frame. Slice 0
characterizes current active behavior; later slices may either:

- preserve authoritative update cadence for this milestone; or
- adopt the existing interpolation state for both active and preview slots
  only after measured rebuild/upload cost clears native, browser, and mobile
  gates.

Do not give preview B a hidden native-only 20 Hz simulation exception merely
to make movement look smoother. Standby cadence remains explicit and shared.

## Cross-Platform Contract

This tactical has no web-parity follow-up. Shared code lands on every supported
client lane at each slice.

- `mclone-render` owns placement, bounds, clip math, actor/terrain pipeline
  semantics, and mono/per-eye/multiview implementations.
- `mclone-scene` owns per-slot actor state, source selection, phase ordering,
  readiness, lifecycle, and activation/swap behavior.
- `mclone-render-session` maps source client presentations and source lighting
  to renderer instances.
- `mclone-server` owns any authored entity records and authoritative movement.
- `mclone-app-runtime` owns one portable versioned scenario payload containing
  shared chunk and entity-chunk codec bytes.
- Native adapters write SQLite records and run native threads; browser adapters
  write IndexedDB records and run Workers. Neither authors an entity, chooses
  a clip plane, or decides actor visibility.

Half-space shaders must validate through production browser WebGPU. Do not use
native-only clip-distance, geometry-shader, or stencil assumptions. Browser
flat is required. Browser WebXR remains an existing unsupported product lane,
while native per-eye and full-frame multiview code still lands with the same
slice.

If adding entity records changes the managed lobby recipe, increment the shared
fixture/content versions and use new stable managed keys. Keep old records
untouched. Browser provisioning publishes metadata, block chunks, and entity
chunks atomically through the existing tokened operation and must not perform
fixture generation or large serialization synchronously in rAF.

## Performance Contract

### Feature-off invariant

Before a preview exists, every platform retains:

- one runtime, one actor world cache, and the current direct terrain/actor
  frame path;
- no `WorldCompositionContext` construction in the hot draw loop;
- no clipped terrain or actor pipeline/materialization;
- no second actor vertex/index allocation or presentation collection;
- no world-qualified actor aggregation, bounds filter, clip-plane branch, or
  second actor render pass; and
- no new Worker/thread, fixture operation, or actor-specific platform service.

Source/ownership tests lock these absences. Timing alone is insufficient.

### Measurement protocol

- Build before measuring and verify no compiler, bundler, emulator, deploy
  hook, or unrelated high-CPU process overlaps the batch.
- Run at least five native and five production-browser feature-off samples.
  Retain every sample; report median, range, P95, and the idle-state preflight.
- Do not silently discard an outlier. Retain rejected batches with the measured
  external cause and rerun the complete batch.
- Prefer interleaved already-built candidate/control samples if machine drift
  exceeds the within-batch range.
- A median average or P95 slowdown above 3% triggers investigation. Above 5%
  blocks the slice unless an untouched interleaved control attributes it and
  the candidate is not worse. Above 10% requires explicit user direction.
- Preserve deterministic work counts, zero frame-budget accounting violations,
  and the direct-path screenshot/hash witnesses where movement is frozen.

Tactical 178's final native direct-path medians, 2.470 ms average and 4.262 ms
P95 versus its 2.492/4.338 ms control, are historical context rather than a
portable budget. Slice 0 records fresh controls on the current machine and
browser.

### Feature-on receipts

Record separately:

- active and preview actor counts by entity/remote/source-local-player kind;
- source-bounds and frustum/clip rejection counts;
- actor CPU mesh rebuild count/time and uploaded bytes per slot;
- retained CPU vectors and GPU vertex/index capacity per slot;
- shared actor atlas/pipeline owner count and bytes;
- mono, per-eye, and multiview actor draw/index counts;
- scenario startup-to-first-actor latency;
- authoritative update-to-visible-preview latency;
- browser rAF contribution, longest attributable frame gap, and mobile
  CPU-throttled behavior; and
- simulation cadence and Worker/thread counts, proving actors do not silently
  restore full standby cadence or allocate another compiler/server worker.

## Unattended Execution Protocol

One explicit instruction to implement this tactical authorizes sequential work
through Slices 0–7, with one verified commit per green endpoint and
`Topic: embedded-worlds`. It does not waive stop rules or platform evidence.

For each slice:

1. update this document with status and a concrete completion record;
2. implement only the bounded slice, splitting it further if its evidence is
   not reviewable as one change;
3. run focused crate, ownership, native, browser, and performance gates named
   by the slice;
4. capture every first drawable native and browser result under `/tmp` and
   inspect it before proceeding;
5. retain reports and rejected performance samples under `/tmp`;
6. commit the green endpoint with the shared topic trailer; and
7. continue unless a stop rule fires or the image needs subjective human
   judgment.

Slice 5 is the first useful interactive checkpoint: the product lobby should
show source actors and the full-body source-local player on the table. An
unambiguous captured result may continue unattended; pause if scale, clipping,
animation, occlusion, or player identity is visually uncertain.

An unavailable real multiview device is recorded honestly and does not block
host-neutral work when shared full-frame code, compilation, synthetic stereo,
and available-device validation pass. It does prevent claiming a new real
headset receipt.

## Implementation Slices

### Slice 0: Characterize Ownership, Pixels, And Cost

No intended product behavior change.

Deliverables:

- Lock the existing terrain placement, bounded-record, scene phase, actor
  collection, actor resource/cache, source-local body, remote-player, asset
  replacement, and complete-slot exchange call sites with executable ownership
  tests.
- Record the exact `ActorDrawResources` immutable/mutable allocation ledger,
  minimum GPU buffer capacities, shader/pipeline topology, multiview
  materialization behavior, and whole-list rebuild/upload triggers.
- Characterize whether production active actors currently consume raw or
  interpolated presentations and record update/rebuild frequency for a moving
  cow and player.
- Lock that the destination runtime already contains entity/remote-player
  presentations even though scene composition omits them.
- Capture native and browser lobby screenshots plus actor counts proving the
  current terrain-only preview, while separately proving active-world actors
  still render.
- Run fresh five-sample native and production-browser feature-off controls
  after the idle-system preflight.
- Add a duplication ledger classifying atlas, figures, pipelines, uniforms,
  mesh caches, CPU lists, GPU buffers, and multiview topology as shared,
  inherently per world, or pending refactor. Leave no unclassified duplicate.

Exit criteria: the refactor starts from measured actor behavior and exact
ownership facts rather than an assumed renderer architecture.

### Slice 1: Shared Composition Context And Terrain Normalization

No intended product pixel change.

Deliverables:

- Add validated `WorldSourceBounds`, `CompositionHalfSpace`,
  `CompositionClip`, and `WorldCompositionContext` contracts beside
  `WorldPlacement` in shared `mclone-render` code.
- Test finite values, normalized/nonzero plane normals, inclusive/exclusive
  boundary convention, source/composition conversions, transformed AABBs,
  complementary half-spaces, large source anchors, and mono/stereo camera
  facts.
- Derive source bounds from `EmbeddedChunkRegion` without moving runtime chunk
  interest into renderer vocabulary.
- Refactor only the opt-in placed terrain APIs to consume the context. Preserve
  direct terrain function calls and uniform/shader bytes.
- Keep diorama source filtering, placed culling, fog, light, translucent
  ordering, and mono/per-eye/multiview behavior pixel-equivalent.
- Prove no-preview construction does not allocate the context or any new
  renderer resource.
- Run and inspect native offscreen, synthetic-stereo, production browser lobby,
  and CPU-throttled mobile browser captures.

Exit criteria: the current diorama is expressed through one renderer-neutral
context without changing its pixels or direct-path cost.

### Slice 2: Portable Half-Space Terrain Proof

Prove future split-world coverage without claiming a walkable seam.

Deliverables:

- Add opt-in half-space-clipped placed terrain pipelines for solid, cutout, and
  translucent phases in mono/per-eye and full-frame multiview together.
- Implement composition-position fragment clipping through portable WGSL
  `discard`; do not request a native-only GPU feature.
- Add CPU transformed-section-bound rejection for sections wholly outside the
  retained half-space.
- Add a static renderer fixture with two independent terrain stores,
  identity-scale placements, complementary planes, one shared depth target,
  and an authored open/framed seam.
- Prove no overlap/gap away from the deliberately authored seam, correct depth
  on both sides, complementary stereo pixels, and identical source selection
  between per-eye and multiview preparation.
- Render the same proof through browser WebGPU and inspect all captures.
- Lock that the ordinary and placed-unclipped pipelines contain no clip branch
  or extra uniform.

Exit criteria: terrain validates the context's scale/bounds/clip separation for
both diorama and split-world directions. Boundary-face resolution remains a
named later dependency.

### Slice 3: Per-World Actor State And Shared Immutable Resources

Refactor ownership while preserving active-only actor pixels.

Deliverables:

- Split immutable actor atlas, figures, texture layout, shader/pipeline layouts,
  and compatible pipeline topology from `ActorMeshCache` and mutable uniform
  state.
- Give `DrawableWorldSlot` the actor presentation/draw state that must move
  with its runtime and world id. Keep the second cache lazy until a retained
  slot is actor-capable.
- Keep compatible immutable actor resources shared across active and standby
  slots. Record strong-owner counts and retained bytes.
- Route the ordinary active-world actor draw through the active slot without a
  composed submission or world-id lookup in the hot loop.
- Make complete-slot exchange carry actor cache/presentation state and prove
  A-to-B-to-A does not rebuild immutable actor resources or cross-contaminate
  equal actor ids from different worlds.
- Update asset-epoch/device-resource replacement and cancellation to drop or
  rebuild composed actor topology before stale drawing.
- Capture exact/frozen active actor pixels before and after the extraction.
- Run the five-sample native/browser feature-off performance gate before
  proceeding.

Exit criteria: actor mutable state is genuinely per world, immutable resources
are singular, and ordinary rendering remains behaviorally and measurably
unchanged.

### Slice 4: Composition-Aware Actor Renderer

Prove placed and clipped actors without scene product integration.

Deliverables:

- Add opt-in placed-unclipped and placed-half-space actor shader/pipeline paths
  using `WorldCompositionContext`, including mono/per-eye and full-frame
  multiview.
- Keep direct actor uniforms and shaders unchanged.
- Apply placement in the vertex shader to source-world actor mesh vertices and
  export composition position for fog and optional clip.
- Add source-bounds membership and transformed actor-bound frustum/half-space
  rejection before mesh preparation.
- Preserve B-local packed light and deterministic actor ordering.
- Add renderer fixtures containing a cow, chicken figure, item, remote-player
  figure, and source-local player figure. Prove scale, feet anchoring,
  orientation, walk/flap pose, lighting, table/terrain occlusion, complementary
  clipping, and distinct eye parallax.
- Verify per-world caches do not alternate/reupload A and B lists merely
  because both render in one frame.
- Run and inspect native mono, synthetic stereo, and browser WebGPU captures;
  exercise multiview on a capable adapter when available.

Exit criteria: actors are a second renderer leaf consuming the same world
composition semantics as terrain, with no scene policy or platform fork.

### Slice 5: Scene-Composed Diorama Actors And Player Bodies

Make the product scene submit destination actors.

Deliverables:

- Build qualified active/preview actor submissions from each slot's runtime,
  world id, source client lighting, actor cache, and composition context.
- Keep active local-player first-/third-person behavior unchanged. Add the
  preview-only full-body source-local player policy and prevent self/remote
  duplication.
- Include source runtime entities and remote players only when their source
  membership lies in the preview region.
- Compose phases as all opaque/cutout terrain, then active and preview actors,
  then globally ordered terrain translucency and active-only later visuals.
- Add mono, per-eye, and full-frame multiview scene branches together. Preserve
  the direct no-preview wrappers.
- Eagerly materialize required composed actor topology before publishing actor
  capability. Terrain preview visibility must not wait for a nonexistent actor,
  and actor topology failure must not install stale resources.
- Extend preview diagnostics with actor kind/count, source rejection, mesh
  rebuild/upload, GPU capacity, and placed draw facts.
- Prove whole-slot activation and return show each world's actors exactly once
  with the correct local-body visibility on both sides.
- Capture and inspect product native flat, synthetic stereo, production browser
  desktop, and CPU-throttled mobile lobby images.

Exit criteria: the user can enter the lobby on native or web and see every
currently present destination actor, including the destination connection's
full-body player, standing in correct miniature geometry.

This is the first optional human visual checkpoint.

### Slice 6: Authored Live Entity And Movement Witness

Make the sample visibly alive through ordinary authority and persistence.

Deliverables:

- Extend the shared authored fixture payload to carry versioned
  `EntityChunkRecord` codec bytes alongside block chunks. Reuse one Rust author
  and existing SQLite/IndexedDB entity stores.
- Add at least one supported persisted passive entity to a versioned island
  fixture inside the preview bounds. Prefer a cow plus chicken when both can
  move through existing authoritative behavior without fixture-only AI.
- Bump fixture/scenario content ids and managed keys non-destructively; retain
  old v1 storage and never reinterpret it as the new recipe.
- Provision block and entity records atomically in native and browser managed
  content operations without main-rAF generation or serialization.
- Prove server load, entity-ticking eligibility, ordinary AI/movement,
  snapshot/update publication, client presentation, source lighting, placed
  actor rebuild/upload, and changed table pixels in one bounded receipt.
- Record at least two authoritative source poses and visible composed poses for
  the same stable entity id. Keep a frozen initial-frame witness for
  deterministic pixel comparison and a separate time-bounded motion witness.
- Prove entity persistence/reload and A-to-B-to-A activation do not duplicate
  or reset the entity unexpectedly.
- Run desktop and mobile browser scenario captures and record update-to-visible
  latency plus feature-on actor costs.

Exit criteria: the checked-in lobby scenario reliably contains a real
destination creature that the user can watch move on the table on native and
web.

### Slice 7: Live Joined-Player Proof And Closeout

Close the milestone with ordinary multiplayer presentation and final cost
evidence.

Deliverables:

- Build a deterministic multi-client fixture using an ordinary authoritative
  server, one source-world observing client, and a second joined player. Drive
  the second player through normal position admission and `MovePlayer`
  commands; do not inject renderer-only poses.
- Generalize shared server remote-player pairing so the local integrated
  `CommandTarget` can observe dedicated peers and peers can observe it under
  the same tracked-chunk rules. Keep this below native/browser runner adapters
  and lock add/update/remove symmetry for local/dedicated pairs.
- Route `RemotePlayerAdd`, successive `RemotePlayerUpdate`s, appearance/walk
  facts, and removal through the source `ClientRuntime` and composed scene.
- Capture source-player movement over the diorama with stable world-qualified
  identity, changed composed pixels, and unchanged active-world actors.
- Exercise the same shared actor submission through production browser WebGPU.
  A validation-only browser control may transport a shared Rust-authored
  deterministic auxiliary-player script to the destination server Worker, but
  that script must drive ordinary server admission/movement and resulting
  protocol updates. TypeScript must not author player positions, inject
  `ServerUpdate`s, or implement a second visibility policy.
- Prove source-local player and remote player figures can coexist without id
  collision and that activation/return changes local-body policy without
  duplication.
- Re-run native flat, synthetic stereo, browser desktop/mobile, lifecycle,
  asset replacement, device/resource rebuild, Android package/AVD, desktop XR
  compile, Android XR package, and available multiview gates.
- Run final five-sample native and browser feature-off comparisons and publish
  feature-on actor CPU/GPU/memory/frame receipts.
- Update this tactical, `docs/topics/embedded-worlds.md`, `docs/platforms.md`,
  `docs/native-web.md`, and tactical index status with landed evidence and the
  next boundary-resolver direction.

Exit criteria: live authoritative creatures and joined players visibly move in
the destination diorama through one shared implementation on native and web,
without changing preview authority or regressing the single-world path.

## Validation Matrix

Every slice runs its focused tests plus, for shared scene/render changes:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-client
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:thin-adapters:purity
pnpm native:scene-host:purity
pnpm native:web:build
pnpm native:web:typecheck
git diff --check
```

Existing rendered and lifecycle gates remain green:

```bash
pnpm native:lobby-scenario:smoke
pnpm native:lobby-scenario:stereo-smoke
pnpm native:live-diorama:activation-smoke
pnpm native:desktop-offscreen:smoke
pnpm native:xr-emulation:smoke
pnpm native:web:lobby-scenario-smoke
pnpm native:web:lobby-scenario-mobile-smoke
pnpm native:web:lobby-scenario-lifecycle-smoke
```

Add stable checked-in commands rather than one-off invocations for:

```text
native composed half-space terrain proof
native composed actor proof
native live-preview actor movement smoke
native live-preview remote-player movement smoke
browser composed half-space WebGPU proof
browser live-preview actor/player WebGPU proof
```

Browser ABI/runtime changes additionally run the current Worker and app matrix:

```bash
pnpm native:web:thread-smoke
pnpm native:web:app-smoke
pnpm native:web:catalog-smoke
pnpm native:web:mobile-smoke
pnpm native:web:asset-pack-smoke
pnpm native:web:block-edit-probe
pnpm native:web:movement-perf
pnpm native:web:remote-smoke
```

Final platform validation follows `docs/platforms.md` and the provided Android
scripts:

```bash
pnpm native:android:apk
pnpm native:android:avd-smoke -- --skip-build
pnpm native:android:avd-session-smoke -- --skip-build
pnpm native:android-xr:apk
pnpm native:xr:check
```

Use a real XR device when available. Save every screenshot/report under
`/tmp`, never in the repository.

## Risk Register

1. **False generalization.** A giant `render_world()` abstraction could hide
   global phase ordering and tax the direct path. Keep a small data context and
   explicit scene phases.
2. **Terminology collapse.** Treating chunk interest, source bounds, frustum,
   clip, and depth as one “visibility” flag will produce incorrect actor and
   split-world behavior. Lock the distinct types and tests in Slice 1.
3. **Actor cache thrash.** One cache used for A then B would rebuild/upload
   twice each frame. Per-world mutable caches are mandatory before composition.
4. **Accidental immutable duplication.** A second actor atlas/figure/pipeline
   shell wastes browser/XR memory. Stop if sharing is not possible; measure and
   report the exact incompatible resource rather than silently duplicating it.
5. **Interpolation upload storm.** CPU-baked moving figure meshes can upload on
   every changed pose. Characterize and measure before enabling per-frame
   interpolation.
6. **WebGPU clipping behavior.** Fragment discard may reduce early-depth
   efficiency. Keep it in an opt-in pipeline and reject whole sections/actors
   on CPU first.
7. **Boundary illusion.** A clean authored half-space screenshot can conceal
   neighbor-culled holes in arbitrary terrain. Keep the boundary resolver an
   explicit next milestone.
8. **Player identity ambiguity.** The retained connection's own body and remote
   players come from different sources. Keep the policy explicit and qualify
   aggregate ids by world.
9. **Fixture migration.** Adding real entities changes managed content. Use new
   versioned keys and atomic entity-record provisioning; never overwrite a
   user's existing managed v1 records in place.
10. **Standby simulation cost.** Moving actors may tempt an unconditional
    full-cadence destination. Preserve explicit cadence, measure scenario-on
    cost, and do not hide a platform-specific override.
11. **Scene lifecycle races.** Asset replacement, device rebuild, Back/Quit,
    and late browser Worker completion can leave actor topology paired with the
    wrong slot. Reuse stable world id, token, epoch, and generation checks.
12. **Native-first validation drift.** A green WASM compile is insufficient.
    Every pixel slice requires a production WebGPU capture before it closes.

## Checkpoints And Stop Rules

- Stop in Slice 1 if the context requires changing ordinary direct terrain
  uniforms/shaders or inserting per-frame virtual dispatch into the one-world
  path.
- Stop in Slice 2 rather than use a GPU capability unavailable to browser
  WebGPU or rather than call an open seam a solved voxel boundary.
- Stop in Slice 3 if compatible slots cannot share actor atlas/figures/pipeline
  topology, or if active-only pixels/performance cannot be restored before
  placed actors begin.
- Stop in Slice 4 if composed A/B drawing alternates one mutable cache or
  rebuilds unchanged actor lists each frame.
- Stop in Slice 5 if actor topology/readiness would block the terrain preview
  forever when the source contains zero actors, or if player identity could be
  duplicated across source-local and remote lists.
- Stop before a destructive managed-content migration or a TypeScript fixture,
  actor, placement, or visibility implementation.
- Stop if making actors visibly move requires granting standby B physical
  movement/interaction authority or forwarding A's input into both worlds.
- Stop if a browser-only cache, shader policy, or scene branch is proposed.
- At any slice, a feature-off regression beyond the performance contract
  blocks progression until attributed and resolved or explicitly accepted.
- Pause after Slice 5 only when captures are visually ambiguous; otherwise the
  unattended protocol may continue to the authoritative movement proofs.

## Planning Estimate

Plan around **8–13 focused engineering days**, with **2–3 weeks** as the risk
range if actor immutable-resource extraction, browser entity-record
provisioning, per-frame mesh uploads, or the multi-client rendered harness
exposes deeper work. The context/terrain normalization should be small; actor
resource ownership and live player evidence are the schedule risks.

The estimate does not include a production walkable split-world seam,
boundary-aware remeshing, authority transfer, remote observer protocol,
particles, or environment composition.

## Expected End State

The lobby diorama is no longer terrain-only. It shows the real retained
world's bounded terrain, water, authoritative entities, remote players, and
source-local joined-player body as live geometry in the same physical mono/XR
coordinate system. Creatures and other joined players visibly move because
their ordinary server updates reach the retained client replica. The same
implementation runs on native and production web, shares immutable resources,
and preserves the direct single-world fast path.

The renderer also has a proven shared composition vocabulary and a portable
complementary half-space terrain/actor path. A later split-world tactical can
therefore focus on boundary faces, environment policy, collision, and authority
handoff instead of replacing a diorama-specific terrain or actor renderer.
