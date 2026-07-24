# GPU Procedural Terrain

Topic: `gpu-procedural-terrain`

Status: textured projection and navigation follow-up completed with local and
hosted desktop/mobile validation 2026-07-24 under
Tactical
[`233-terrain-lab-projection-materials-and-navigation.md`](../tactical/233-terrain-lab-projection-materials-and-navigation.md).
The canonical multi-pane Terrain Lab workspace completed local and hosted
desktop/mobile headed-WebGPU validation 2026-07-24 under Tactical
[`232-terrain-lab-canonical-workspace.md`](../tactical/232-terrain-lab-canonical-workspace.md).
Independent CPU/GPU Terrain Lab publication, cache controls, and cold
benchmark implementation previously completed local and hosted desktop/mobile
headed-WebGPU validation 2026-07-24. Tactical
[`227-web-terrain-lab-vertical-slice.md`](../tactical/227-web-terrain-lab-vertical-slice.md)
landed the deployable, web-first Terrain Lab, shared production-reference grid,
and one shared Rust/WGPU resident tile. Tactical
[`228-production-large-fields-in-terrain-lab.md`](../tactical/228-production-large-fields-in-terrain-lab.md)
then replaced its unrelated approximation with the production 64-bit field
hash, value/gradient noises, field warps, base surface, climate, and
bathymetry. Fixed 2 km and 65.5 km BrowserWebGPU receipts now measure zero
base-height mean/P95 error, 100% ocean agreement, and about `2e-8` mean
continentalness error. Tactical
[`229-viewport-driven-terrain-lab.md`](../tactical/229-viewport-driven-terrain-lab.md)
then added continuous viewport zoom, aligned resident tiles, Auto/manual
detail, coverage-first progressive publication, epoch cancellation, bounded
residency, full-target comparison, and first coarse/target pixel timing. Final
river/wetland/planned-stream disagreement remains explicit. Tactical
[`230-terrain-lab-independent-race-and-benchmarks.md`](../tactical/230-terrain-lab-independent-race-and-benchmarks.md)
then separated CPU/GPU scheduling and publication, exposed cache bypass and
invalidation, added a repeatable cold race, and fixed trackpad/page-scroll and
vertical map-grab behavior. Tactical 232 makes the Lab a configurable
canonical/CPU-LOD/GPU-LOD workspace. It adds bounded exact final chunk
generation, shared textured block rendering, cheap preview lighting,
progressive worker publication, and presentation-only feature visibility.
Scale-aware coarse summarization and band limiting remain the next far-field
correctness work. A thin native profiling host and eventual in-game LOD/map
consumers should reuse the shared engine rather than becoming separate
implementations. Optional GPU-backed authoritative chunk generation and
volumetric terrain remain separate later experiments.

## Scope

This topic owns the continuing direction for:

- reconstructing first-party procedural terrain directly on the GPU;
- showing coarse terrain before canonical chunks are generated, lit, meshed,
  and published;
- progressively refining one visible terrain representation through nested
  sample spacings from continent-scale footprints down through 16, 8, 4, and 2
  blocks;
- scale-aware summaries that let one far sample represent hundreds or
  thousands of chunks without first generating those chunks;
- retaining procedural terrain on the GPU without CPU-built per-tile meshes;
- a standalone Terrain Lab for fast macro-to-local generator iteration, its
  shared engine boundary, and its relationship to native diagnostics and
  in-game consumers;
- optionally using a GPU as an asynchronous authoritative-worldgen worker;
- presentation lighting appropriate to coarse GPU terrain;
- later sparse volumetric density, occupancy, meshing, or ray-casting
  experiments; and
- capability- and budget-aware product policy across desktop, Steam Deck,
  browser, Android, and XR.

It does not own:

- creative terrain, biome, hydrology, geology, or structure rules, which remain
  with [`mclone-overworld-generation.md`](mclone-overworld-generation.md);
- the current synthetic Far LOD lifecycle, coverage, suppression, and settle
  defects, which remain with [`far-lod.md`](far-lod.md);
- exact authoritative light propagation, which remains with
  [`lighting.md`](lighting.md);
- general point-light and shadow policy, which remains with
  [`dynamic-point-lights.md`](dynamic-point-lights.md);
- normal chunk authority, persistence, protocol, or client-replica semantics;
  or
- a blanket replacement of the portable CPU generator.

The first experiment may reuse the current Far LOD lifecycle, but this topic is
broader than Far LOD. It distinguishes visual readiness, collision readiness,
and canonical simulation readiness instead of requiring one fully generated,
lit, meshed chunk to satisfy all three.

## Motivation

The original reason not to put Minecraft-parity terrain generation on the GPU
was sound but narrower than the current opportunity. Java-shaped generation
contains serial status dependencies, structures, decoration, ticks, full block
state, persistence, and authoritative behavior. Moving that entire pipeline to
a renderer-owned device would entangle world authority with presentation and
would still require CPU-visible results.

Mclone's first-party generator changes the premise:

- Mclone owns the profile, seed domains, fields, and compatibility policy.
- The current macro terrain is already a pure absolute-coordinate sampler.
- Untouched terrain can be reconstructed from a compact profile identity,
  seed, coordinate, and sparse planned-feature metadata.
- Coarse terrain does not need to become a `GeneratedChunk`, client replica
  snapshot, collision view, or save record merely to be visible.
- The existing Far LOD producer already proves bounded residency, detail
  levels, movement guards, native/browser workers, real-versus-LOD exclusion,
  and mono/per-eye/multiview rendering.

That creates a product opportunity unavailable to a generic Minecraft LOD
consumer: a new seed can show recognizable geography to the horizon
immediately, without first pregenerating or importing every canonical chunk.
Desktop may extend that horizon for kilometers. Steam Deck, browser, and
Quest-class devices may use shorter ranges, coarser outer levels, smaller
budgets, or no GPU procedural path while retaining the same world.

The central reframing is:

```text
do not make GPU memory the world authority

make untouched procedural terrain a GPU-reconstructible representation
```

## Current Implementation Facts

The live Mclone sampler already exposes a promising semantic seam:

- `McloneOverworldSampler` is constructed from a signed seed and sampling
  topology;
- `McloneOverworldTerrainSample` exposes terrain, climate, bathymetry,
  watercourse, and surface-height facts;
- point and bounded-region requests call the same sampler; and
- chunk materialization applies bounded planned-stream records before writing
  surface columns.

See
[`mclone-worldgen/src/levelgen/mclone_overworld/fields.rs`](../../native/crates/mclone-worldgen/src/levelgen/mclone_overworld/fields.rs)
and
[`terrain.rs`](../../native/crates/mclone-worldgen/src/levelgen/mclone_overworld/terrain.rs).

The current synthetic Far LOD compiler is still CPU materialization:

1. select a generation profile and seed;
2. generate and retain complete surface-stage `GeneratedChunk` values;
3. extract surface columns at the requested spacing;
4. build CPU `Vec<f32>` vertices and `Vec<u32>` indices; and
5. return a packed mesh for GPU upload.

The relevant entry points are
[`compile_far_terrain_lod_worker_input`](../../native/crates/mclone-app-runtime/src/far_lod.rs)
and `FarTerrainLodWorkerCache` in the same file. Even 4-, 8-, or 16-block
surface sampling currently pays for a full surface chunk before discarding
most of its facts.

The terrain compute pipeline now exists in
[`mclone-terrain-view`](../../native/crates/mclone-terrain-view/). It:

- accepts aligned 64-cell / 65-sample production-reference tiles;
- evaluates the separately revisioned `mclone-overworld-v1-gpu-preview-a2`
  `mclone-overworld-v1-gpu-preview-a3` production graph through
  `base_surface_y` and macro surface material into GPU-resident storage;
- emulates the production unsigned 64-bit lattice hash with pairs of portable
  WGSL `u32` values and generates domain/scale constants from a shared Rust
  field specification;
- retains separate base-surface/ocean and final-surface/watercourse comparison
  facts so an omitted field family cannot masquerade as arithmetic error;
- draws 24,576 generated vertices per resident tile and panel without CPU
  vertex or index arrays;
- supports terrain, height, continentalness, climate, and error layers in map
  or oblique views; and
- progressively publishes complete nested levels while retaining a complete
  coarser parent;
- retains up to 192 stable tile identities, with center-first visible
  admission and a target-level preload margin;
- rejects obsolete queued work when the viewport epoch changes; and
- performs bounded asynchronous readbacks aggregated over the complete target
  viewport for lab comparison.

[`mclone-terrain-lab`](../../native/apps/mclone-terrain-lab/) owns the narrow
browser surface/device facade and exact typed Worker payload. It now exposes a
second canonical surface that builds the normal block catalogue and texture
atlas from the first-party authored and generated-fallback packs, incrementally
updates shared textured section resources, and renders with preview lighting.
The shared `CanonicalTerrainCompiler` in `mclone-terrain-view` calls the
production surface or final generator and supplies a Rust-owned center-first
chunk order.

[`tools/terrain-lab`](../../tools/terrain-lab/) owns URL state, replaceable
Worker epochs, responsive presentation, and cache controls. The workspace
shows any unique subset of canonical, CPU LOD, and GPU LOD panes. Every pane
shares seed, center, viewport, camera, and navigation; the exact radius remains
separately bounded from the broad visual footprint. Water and vegetation
switches remesh retained exact blocks without changing generation.

Three-dimensional left drag orbits with conventional pitch direction without
changing URL-addressed geography; right drag, Shift+left, or middle drag pan,
focused arrow keys pan in map and 3D, map left drag pans, wheel and pinch
change a separate continuous viewport, and map zoom is cursor anchored. Auto
targets approximately two CSS pixels per sample cell;
an explicit manual request remains visible when the interactive tile budget
raises effective spacing. CPU and GPU LOD publish independently at exact
matching coordinates. Wide procedural canvases place them side by side;
portrait canvases stack full-width panels.

The deployed `/terrain/` product remains independent of the game client,
server, collision, persistence, and authoritative propagated lighting. It now
does load the production first-party packs and bounded exact chunks. Generated
fallback tiles remain visible for materials without curated first-party
textures, and final rivers/wetlands/planned streams remain absent from the GPU
LOD path rather than being hidden by the exact pane.

Normal terrain and the current Far LOD path still arrive at `mclone-render` as
CPU-constructed mesh products. The viewport planner and resident renderer are
a reusable experimental service, not yet an in-game replacement.

Existing performance records motivate measurement without proving a GPU win:

- one RD10 split measured raw feature generation at `551.5` chunks/s cold
  while server-only loading with authoritative lighting reached `40.6`
  chunks/s;
- the same recorded footprint measured `11.037s` of CPU meshing for 8,464
  sections and `0.133s` to upload the resulting prebuilt 360 MB mesh
  footprint; and
- Quest RD10 stress evidence already attributes about `10.7ms` to real GPU
  execution, so moving CPU work to the GPU may harm standalone XR even when it
  helps desktop.

These values live in [`performance.md`](performance.md) and
[`../performance-records.md`](../performance-records.md). They are pipeline
evidence, not a benchmark of the proposed kernel or a promise that every
profile has the same cost shape.

## Accepted Direction

### Separate Readiness States

The target pipeline has independent milestones:

```text
seed + profile descriptor
  |
  +-> GPU coarse visual coverage
  |     -> finer GPU visual refinement
  |     -> approximate materials, water, and lighting
  |
  +-> authoritative host generation
        -> collision-ready canonical terrain
        -> features, ticks, fluids, and exact lighting
        -> client snapshot and exact textured mesh
        -> replace the procedural presentation
```

The first visible terrain need not be authoritative. The first authoritative
terrain need not yet have every visual refinement. Product and diagnostics
should use explicit names such as:

- `VisualReady`: a presentation-only procedural representation can paint the
  expected terrain;
- `CollisionReady`: the host and prediction boundary possess the canonical
  facts required for local movement and interaction; and
- `CanonicalReady`: the required generation statuses, mutation facts,
  lighting, publication, and persistence obligations are satisfied.

Those names are conceptual until a tactical proves their exact integration.
They must not silently weaken existing chunk-status or publication contracts.

### Coverage Before Detail

The first product behavior should prioritize complete coarse coverage over
isolated fine tiles:

1. cover every expected visible location at the coarsest admitted level;
2. refine the camera neighborhood and important visible regions;
3. preserve the complete parent until all required child data is drawable;
4. replace a parent atomically or with an explicit morph/dither transition;
5. let exact real terrain replace procedural terrain only when it can paint;
   and
6. never trade a missing representation for real/LOD co-rendering.

This extends the existing Far LOD settle invariant:

```text
painted real terrain XOR visible procedural terrain
```

The GPU path must reuse or deliberately extend the same representation ledger.
It must not create a second independent coverage scheduler with different
handoff truth.

### Nested Refinement

Candidate sample spacings are 16, 8, 4, 2, and optionally 1 block. They are not
promised user settings. Their important property is nesting:

```text
16-block samples are a subset of 8-block samples
 8-block samples are a subset of 4-block samples
 4-block samples are a subset of 2-block samples
 2-block samples are a subset of 1-block samples
```

Shared anchors reduce geometry movement during refinement. Skirts may hide
temporary cracks, but they are not a substitute for aligned coordinates,
neighbor-level metadata, and complete parent-to-child ownership.

A camera-centered geometry clipmap is a strong candidate for kilometer-scale
surface terrain because it bounds GPU memory and work while each outer ring
represents exponentially more area. The current chunk-granular resident-tile
system remains a valid first implementation substrate. A tactical should
measure tile, clipmap, and hybrid shapes before choosing a durable cache
identity.

### Extreme-Distance Scale Hierarchy

At multi- and tens-of-kilometers range, chunks should stop being the unit of
generation. The renderer should evaluate a bounded grid of scale-aware terrain
summaries directly. Increasing the sample spacing lets the same small GPU tile
cover exponentially more world area:

| Sample spacing | Area covered by 64 x 64 cells | Equivalent chunk width |
| ---: | ---: | ---: |
| 2 blocks | 128 x 128 blocks | 8 chunks |
| 16 blocks | 1,024 x 1,024 blocks | 64 chunks |
| 64 blocks | 4,096 x 4,096 blocks | 256 chunks |
| 256 blocks | 16,384 x 16,384 blocks | 1,024 chunks |
| 1,024 blocks | 65,536 x 65,536 blocks | 4,096 chunks |

A 64-cell tile may require a 65th shared-edge sample or neighbor lookup; the
table describes its world footprint, not a locked buffer layout. At
256-block spacing, one such tile covers the area of 1,048,576 ordinary chunks.
That is the intended meaning of grouping chunks: the system does not enumerate
or aggregate a million `GeneratedChunk` objects merely to produce the tile.

The distance bands above are illustrative. Runtime level selection should use
projected screen-space error, terrain roughness, viewport, and device budget.
A flat ocean may remain at a very coarse level much closer to the camera than a
mountain skyline. Stable powers-of-two identities are still valuable for
cache reuse and parent/child transitions.

One center-point height is insufficient at coarse scale. A logical far summary
may include:

```rust
struct FarTerrainSummary {
    representative_y: i16,
    min_y: i16,
    max_y: i16,
    water_y: i16,
    roughness: u16,
    dominant_material: u16,
    water_coverage: u8,
    feature_mask: u8,
}
```

This is conceptual rather than a locked ABI. The important information is:

- a representative surface for drawing;
- minimum, maximum, variance, or another conservative relief measure for
  screen-space error and skyline preservation;
- water presence and fractional coverage for coastlines and large lakes;
- dominant or blended surface language rather than one arbitrary center
  material; and
- compact flags for major features that must survive at the current scale.

The hierarchy needs two production modes:

1. **Direct coarse synthesis:** when no finer data exists, evaluate filtered
   generator fields over the footprint and produce the summary immediately.
2. **Truthful roll-up:** when child summaries or canonical terrain are already
   resident, reduce them into the same parent identity so edited or refined
   facts can improve the coarse representation.

Direct synthesis must never wait for children; otherwise the design recreates
the Voxy pregeneration problem. Roll-up is a later accuracy and edit-overlay
path, not a prerequisite for first sight.

Scale-aware evaluation must also be band-limited. Sampling all fine noise at
the center of a 512-block footprint produces temporal shimmer, aliasing, and
false geography. Each level should omit or analytically filter field bands
smaller than its footprint and retain the large-scale continent, climate,
watershed, and relief bands. Nonlinear composition may require explicit
per-level field definitions or conservative multi-sampling rather than simply
dropping the last noise octaves.

Major features need scale-specific proxies. A distant river can be a water
coverage or centerline fact before it becomes block-accurate banks; a
settlement can be a footprint or landmark flag before its buildings are
meshed. Features should appear when their projected importance warrants them,
not only when the renderer reaches one-block samples.

Every dispatch should use a tile-local coordinate frame plus a stable
high/low world-origin decomposition. Tens-of-kilometers coverage is easy to
address but will expose `f32` camera-relative jitter if large absolute
coordinates leak directly into vertex positions.

## Workstream A: GPU Visual-First Terrain

This is the recommended first experiment.

The GPU receives compact immutable inputs:

```text
world and dimension identity
generation profile and field revision
seed split into portable words
sampling topology
tile or clipmap origin
sample spacing and neighbor levels
material palette parameters
bounded planned-feature or structure records
```

A compute pass writes a logical surface representation such as:

```rust
struct ProceduralSurfaceSample {
    surface_y: i16,
    water_y: i16,
    material: u16,
    biome_tint: u16,
    normal_or_slope: u16,
    ambient: u8,
    flags: u8,
}
```

This is conceptual, not a locked ABI. A storage texture or packed storage
buffer may be a better physical format. The logical facts should remain
separate from one GPU layout so format changes do not redefine terrain
semantics. At extreme distance, `FarTerrainSummary` is the corresponding
footprint-aware value; it is not assumed to be the same physical record as a
near-field surface sample.

Rendering should begin with a reusable indexed grid or vertex-ID-generated
grid that reads the resident sample data. The first proof should avoid:

- CPU-built per-tile vertex and index arrays;
- GPU-to-CPU readback;
- per-frame regeneration of settled rings;
- caves, arbitrary overhangs, exact vegetation, or player edits;
- a whole-world dispatch; and
- direct dependence on desktop-only shader features.

The initial field subset should establish continents, oceans, relief,
mountains, climate/material language, and water surfaces. The current bounded
stream system cannot be reproduced by independent point sampling alone. Small
CPU-planned stream/landmark records may be uploaded as sparse modifiers after
the base kernel works. Omitting them is acceptable only in an explicitly
approximate first visual probe whose handoff error is measured.

GPU visual terrain remains a derived cache:

- it may be discarded on device loss or world close;
- it carries a source identity including profile/field revision;
- it never enters `ClientWorld`, collision, persistence, or protocol authority;
- a stale result is rejected by source and request revision; and
- CPU generation may duplicate its field work near the player without waiting
  for or reading back the GPU result.

That duplication is intentional latency hiding, not automatically wasted work.

## Workstream B: Optional GPU Canonical Generation

A GPU may later act as an asynchronous worldgen worker without owning world
authority.

The server-facing concept remains:

```rust
trait GenerationBackend {
    async fn generate(request: GenerationRequest) -> GeneratedChunk;
}
```

The concrete trait, mailbox, and crate boundary are deferred. The invariant is
that `mclone-server` and generator planning do not depend directly on `wgpu`.
An integrated desktop host may supply a GPU-backed worker service, while
dedicated servers, unsupported adapters, browser configurations, and
GPU-budgeted devices retain the CPU backend.

Candidate readback levels are:

1. semantic columns: surface height, water height, biome/material, and compact
   terrain intent, expanded into canonical blocks on the CPU;
2. packed occupancy/material sections for a later volumetric profile; or
3. a complete canonical generated-chunk payload.

The first is the strongest candidate for current 2.5D terrain because it
minimizes transfer and keeps ticks, mutable feature execution, and canonical
buffer construction in shared Rust.

Requirements:

- dispatch and readback are asynchronous and several requests ahead of demand;
- no current-frame or scheduler hot-poll path waits synchronously for a map;
- results cross the same request/revision/stale-result boundary as CPU workers;
- the CPU backend remains a complete fallback;
- persistent output is ordinary canonical chunk data, not a GPU-only save
  format;
- exact output equivalence is proven before a GPU result may satisfy authority;
  and
- renderer load and generation load share an explicit GPU budget.

A fast non-equivalent GPU result may still serve Workstream A. It may not be
quietly promoted to Workstream B.

## Workstream C: Volumetric Residency And Rendering

The later research direction replaces a surface-only sample with sparse 3D
density, occupancy, or material bricks. This is needed for first-class:

- caves visible from outside or through openings;
- overhangs, arches, cliffs, and floating terrain;
- deep or vertically extensive worlds;
- GPU voxel DDA, cone, or short visibility rays;
- compute-generated surface geometry; and
- a Voxy-like hierarchical resident representation.

Possible renderers include:

- GPU-generated blocky or greedy meshes;
- surface-nets or dual-contouring variants for a future smooth-density profile;
- direct sparse voxel DDA/ray casting;
- hybrid raster surfaces plus ray-traced shadows; and
- GPU-driven culling and indirect draws over compact resident bricks.

No technique is selected. Per-pixel voxel ray casting scales with resolution,
steps, and eye count, so it is not assumed to beat a reusable surface mesh on
Steam Deck, browser, or XR. The surface-heightfield proof should land before a
volumetric renderer changes the problem.

This workstream complements, but does not replace, the separate cubic-residency
lab described in
[`world-height-and-volumetric-streaming.md`](world-height-and-volumetric-streaming.md).
A renderer acceleration structure is not automatically a suitable simulation,
lighting, protocol, or persistence store.

## Lighting Direction

A ray cast answers visibility along a segment; it is not by itself a complete
lighting model.

The first GPU surface terrain should use inexpensive presentation facts:

- normals derived from neighboring height samples or field derivatives;
- directional sunlight;
- several horizon samples or bounded heightfield rays for broad terrain
  shadows;
- local slope/curvature ambient occlusion;
- biome/material tint;
- atmospheric haze and fog; and
- later selected coarse emissive landmarks or admitted point lights.

Exact server sky/block lighting remains authoritative near the player. When a
real chunk arrives, its geometry, material, ambient occlusion, and packed light
replace the approximate presentation together. Transition policy must prevent
a visually severe light pop from being hidden behind geometry-only readiness.

Later volumetric candidates include sparse-brick DDA shadows, bounded
wavefront propagation, distance-field ambient visibility, or a compact
radiance hierarchy. None should be called Java light parity. Mob spawning,
light-sensitive gameplay, block updates, and save data continue to use the
authoritative solver unless a separate explicit design changes that contract.

## Arithmetic And Determinism

The current first-party sampler is not a literal portable WGSL program:

- the profile seed and seed domains use 64-bit values;
- terrain fields and composition use extensive `f64`;
- planned streams include bounded searches and structured metadata; and
- floating-point agreement at integer height/material thresholds matters.

Portable WGSL currently exposes runtime `i32`, `u32`, `f32`, and optional
`f16`, not `i64` or `f64`; see the
[WGSL scalar type specification](https://gpuweb.github.io/gpuweb/wgsl/#scalar-types).

Two honest contracts are available:

### Approximate Visual Evaluator

- implement a GPU-friendly `f32`/`u32` evaluator;
- identify it as presentation-only;
- compare it to the CPU source at fixed seeds and reviewed sites;
- measure height, water, material, and topology errors;
- retain the CPU source as handoff truth; and
- conceal bounded differences with aligned samples and an explicit transition.

This is sufficient for Workstream A.

### Exact Dual-Backend Evaluator

- define generator arithmetic in 32-bit integer, fixed-point, or otherwise
  locked portable operations;
- provide CPU and GPU evaluators of that same profile revision;
- fingerprint point, region, chunk, seam, and request-partition output;
- test across native GPU vendors and browser WebGPU;
- reject devices/backends that cannot satisfy the required contract; and
- keep a CPU implementation for dedicated and fallback use.

This is required before Workstream B may produce authoritative results. It may
justify a later GPU-compatible profile or explicit field revision rather than
silently changing the already-reviewed current terrain.

## Voxy Lessons And Deliberate Difference

Voxy demonstrates the value of compact multi-level voxel storage,
GPU-resident geometry, hierarchical traversal, compute-built visibility and
draw commands, and enormous distant views.

The source inspected for this topic is upstream commit
[`b164a6d98c378ebb6bbb3e538770d76d527c001f`](https://github.com/MCRcortex/voxy/tree/b164a6d98c378ebb6bbb3e538770d76d527c001f).
At that revision:

- [`WorldConversionFactory`](https://github.com/MCRcortex/voxy/blob/b164a6d98c378ebb6bbb3e538770d76d527c001f/src/main/java/me/cortex/voxy/common/voxelization/WorldConversionFactory.java)
  converts already-existing Minecraft sections into Voxy data on the CPU;
- [`RenderDataFactory`](https://github.com/MCRcortex/voxy/blob/b164a6d98c378ebb6bbb3e538770d76d527c001f/src/main/java/me/cortex/voxy/client/core/rendering/building/RenderDataFactory.java)
  builds section geometry on the CPU; and
- [`MDICSectionRenderer`](https://github.com/MCRcortex/voxy/blob/b164a6d98c378ebb6bbb3e538770d76d527c001f/src/main/java/me/cortex/voxy/client/core/rendering/section/backend/mdic/MDICSectionRenderer.java)
  uses compute and indirect operations for visibility and draw-command work.

Voxy therefore supports the resident-hierarchy and GPU-driven-rendering
direction, but it is not evidence that canonical Minecraft terrain generation
already lives entirely on the GPU. Its generic Minecraft role also explains
why a world normally has to exist before Voxy can represent it.

Mclone may deliberately differ because it owns a known first-party generator.
Untouched distant terrain can be synthesized from the seed rather than
ingested from pregenerated chunks. Arbitrary imported, edited, remote, or
authored worlds still require streamed or persisted facts.

Voxy's
[`LICENSE.md`](https://github.com/MCRcortex/voxy/blob/b164a6d98c378ebb6bbb3e538770d76d527c001f/LICENSE.md)
reserves all rights. It remains conceptual background only; do not copy source
or depend on its implementation.

## Multiplayer, Persistence, And Edits

Presentation-only GPU terrain follows the existing client-replica rule:

- no client fallback may synthesize a canonical chunk from the seed;
- a GPU procedural tile is a loading/LOD product, not a snapshot;
- collision, reach, interaction, AI, fluid simulation, and block queries
  report missing authority until canonical data arrives;
- normal chunks always replace procedural presentation when drawable; and
- the host remains the only source of canonical edits.

For local first-party worlds, the client and integrated host may share the seed
and profile descriptor. For remote worlds:

- a server may deliberately disclose the descriptor and allow local visual
  synthesis;
- a server may stream compact non-authoritative LOD facts or regional
  modifiers;
- a server may conceal the seed and disable procedural synthesis; or
- authored/imported terrain may use only server-supplied LOD.

These are protocol/product policies, not assumptions for the first local proof.

The long-term edit model is:

```text
visible distant terrain
  = procedural base
  + sparse authoritative edit/structure overlay
```

The first implementation may ignore edits outside normal chunks, matching the
current Far LOD contract. Later overlays need their own revisions,
invalidation, bounds, cache identity, and server privacy policy. They must
remain rebuildable presentation data unless explicitly promoted elsewhere.

## Platform Posture

GPU procedural terrain is one shared feature with capability- and
budget-selected implementations, not a desktop-owned gameplay fork.

- **Desktop:** primary kilometer-scale experiment; the best candidate for
  aggressive range, fine refinement, GPU timestamps, and optional asynchronous
  canonical generation.
- **Steam Deck:** first physical handheld target for a smaller/coarser
  GPU-resident horizon under real memory, thermal, and frame-budget evidence.
- **Browser/WebGPU:** primary Terrain Lab product and a required game-client
  reuse lane. Retain the same visual contract where compute, storage, and
  device limits admit it. Worker/device ownership must follow the browser host
  boundary rather than leaking terrain policy into TypeScript.
- **Flat Android:** use measured mobile Vulkan/WebGPU capability and memory
  policy; no assumption that desktop defaults transfer.
- **Android XR / Quest:** may disable the path, reduce range, rebuild only in
  large movement increments, or use a minimal coarse shell. Existing GPU stress
  evidence makes this a falsification lane, not an automatic beneficiary.
- **Dedicated server:** CPU generation remains complete. A future server GPU
  backend is optional and headless; it cannot become a deployment requirement.

Every renderer-visible path must support mono, per-eye, and multiview. Terrain
compute is view-independent and should be shared by both eyes; presentation
still uses each view's own transforms and projections.

## Terrain Lab Product Direction

The recommended answer is not to choose permanently between a desktop binary,
a website, and an in-game feature. Build one shared terrain-view engine and
give it three deliberately different hosts:

```text
shared generator semantics + terrain summary/evaluation contracts
  |
  +-> web Terrain Lab: primary exploration and iteration product
  +-> native diagnostic host: precise profiling, capture, and adapter testing
  `-> game consumers: Far LOD, overview map, minimap, or tabletop view
```

The working product name is **Terrain Lab**; **Terrain Generation Lab** is a
clear UI title if the shorter name is too ambiguous. `/terrain/` is the natural
candidate deployment route alongside `/animals/` and `/structures/`, but a
tactical should lock the final route and package name.

### Primary Web Product

Terrain Lab should be a standalone, deployable web application and the fastest
normal way to inspect a generator change. It should be usable from a desktop
browser and a phone without starting a server, joining a world, waiting for
canonical chunks, or navigating the game camera.

Its central interaction is one continuous scale:

- begin with complete continent- or region-scale coverage;
- pan and zoom through the hierarchy without changing tools;
- switch between a topographic/map view and a 3D orbit or flyover view;
- refine the selected area toward block-scale surface samples;
- expose which scale, field bands, tiles, and representation are currently
  visible; and
- preserve seed, position, zoom, profile revision, view mode, and selected
  diagnostic layers in a shareable URL.

Terrain Lab is also the canonical generator review surface. Its workspace may
show any unique subset of:

- exact production terrain after final feature generation;
- the CPU LOD-style surface evaluator; and
- the GPU-resident LOD-style surface evaluator.

These are synchronized logical panes, not three unrelated viewers. They share
seed, coordinates, viewport, map/3D camera, and navigation. The exact pane has
its own bounded chunk radius because canonical coverage and visual horizon
coverage are intentionally different readiness states. At continent scale,
the exact footprint must remain honest rather than silently expanding into
millions of generated chunks.

The canonical pane uses ordinary generated block/biome facts, the first-party
asset packs, the shared textured section compiler, and the shared textured
renderer. It may bypass propagated light in favor of explicitly labeled cheap
preview lighting. Generation-stage selection and presentation visibility are
different controls: hiding water or vegetation after generation must not alter
feature execution or deterministic random consumption.

The first useful controls are seed, profile/field revision, position, scale,
presentation mode, and diagnostic layer. Later authoring controls can expose
typed profile parameters. The browser must not accept arbitrary shader text or
reimplement generator composition in TypeScript merely to make a slider easy.
A parameter edit creates a new source revision, invalidates affected resident
tiles, restores complete coarse coverage first, and then refines.

High-value diagnostic views include:

- rendered terrain, height, slope/roughness, water coverage, climate, material,
  field-band contribution, and feature masks;
- split or swipe A/B comparison between revisions or parameter sets;
- CPU reference versus GPU result with numeric and visual error;
- tile boundaries, selected levels, parent/child handoff, and cache residency;
- dispatch, first-coverage, refinement, draw, and resident-memory timing; and
- pinned sites or camera bookmarks that make generator review reproducible.

Phone support is part of the product shape, not a promise of desktop range.
The UI should be responsive and touch-native, while the engine selects smaller
tile budgets, fewer layers, and coarser refinement when necessary. A device
without an admitted WebGPU compute path may offer a clearly labeled,
lower-resolution CPU/WASM reference mode or an honest unsupported message. It
must not report CPU fallback timing as GPU evidence.

The desired development loop is one command that watches Rust/WASM, WGSL, and
the web shell; rebuilds the affected layer; restores URL-addressed view state;
and redraws coarse coverage immediately. Shader-only hot reload may be a
development convenience if deployed shaders still come from the same
content-hashed source. Rebuild and first-redraw latency should be measured as a
product metric rather than assumed to be fast.

### Relationship To Existing Labs

Asset Lab and Structure Lab prove the Vite/React/Zustand catalogue shell,
responsive controls, shareable URL state, Playwright capture, aggregate
deployment, and subpath hosting. They mostly ship build-time semantic JSON and
baked display artifacts. Texture Lab is a local mutable authoring service
because its curation workflow writes files and invokes local pipelines.

Terrain Lab is a fourth shape:

- it can reuse the established web-product shell and aggregate deployment;
- it needs a live Rust/WASM and WebGPU evaluator rather than only baked assets;
- it should remain read-only with respect to the repository and saved worlds
  in its first slice;
- its shareable state is a compact generator/view recipe, not a generated
  multi-kilometer mesh; and
- it must exercise the same evaluator, summary, scheduling, and rendering
  contracts intended for the game.

React or another small web shell may own DOM controls, responsive layout, URL
encoding, and browser capability presentation. It must not own terrain
semantics, level selection policy, tile validity, or CPU/GPU comparison truth.
Those remain in shared Rust and renderer contracts. Unlike Structure Lab,
Terrain Lab should not bake every possible view ahead of time.

The existing aggregate native-web deployment can eventually stage the lab
under its own route. Terrain Lab should have a smaller dedicated WASM entry
surface or split payload rather than forcing a phone visitor to boot a complete
game session. It may share compiled crates and shader sources with
`mclone-web-client`; sharing does not require sharing the full application
binary.

### Native Diagnostic Host

A minimal native host remains useful, but it is supporting infrastructure, not
the product boundary. Native is the strongest lane for:

- GPU timestamps and vendor/backend comparisons;
- RenderDoc or platform graphics-debugger capture;
- deterministic offscreen screenshots and large stress sweeps;
- testing ranges that exceed mobile browser budgets; and
- quickly separating WebGPU/browser-host cost from kernel cost.

This could be a small dedicated app binary, an offscreen test harness, or a
diagnostic mode over an existing thin host. The first tactical should choose
the least code that can instantiate the shared service. It must not acquire
its own terrain evaluator, level policy, or UI model.

### In-Game Reuse

The game should consume the shared terrain-view service, not embed the complete
Terrain Lab UI. Candidate consumers include:

- the current Far LOD replacement;
- an overview or world-map renderer;
- a minimap;
- an XR tabletop or god-view presentation; and
- seed/world previews before canonical play is ready.

These consumers may choose different cameras, labels, budgets, and overlay
policy while sharing tile identities, field revisions, summaries, GPU
pipelines, and coverage/refinement scheduling. A map might render
`FarTerrainSummary` into color contours; the horizon renderer might displace a
grid from the same summaries. Neither should regenerate the underlying fields
through a private path.

The lab can deliberately expose more diagnostics than the game, and the game
can combine procedural terrain with authoritative edits and chunks in ways the
first lab does not. Reuse is at the semantic and service boundary, not at the
React component or whole-screen level.

## Ownership Direction

The shared-first boundary should be:

- `mclone-worldgen`: profile semantics, source identity, CPU reference
  evaluator, portable parameter/plan records, and any exact dual-backend
  arithmetic contract;
- `mclone-server`: generator-owned planning and canonical readiness, consuming
  an abstract completion backend rather than `wgpu`;
- `mclone-app-runtime`: bounded visual coverage/refinement scheduling, source
  revisions, capability policy, Terrain Lab request/view state that is useful
  outside one host, and optional compute-service assembly;
- `mclone-render-session`: resident procedural-tile or clipmap lifecycle,
  replacement readiness, and upload/compute admission;
- `mclone-render`: GPU buffers/textures, compute and draw pipelines, material
  sampling, timestamps, mono/per-eye/multiview drawing, and resource rebuild;
- `mclone-scene`: active-world orchestration, real/procedural arbitration,
  frame budgets, world switching, and diagnostics; and
- Terrain Lab web/native apps: URL/DOM or CLI/capture mechanics, adapter/device
  creation, surface lifecycle, and presentation of shared diagnostics; and
- game app/platform crates: adapter/device creation, surface/session
  lifecycle, raw capability collection, and presentation only.

The first proof established
[`mclone-terrain-view`](../../native/crates/mclone-terrain-view/) as that small
shared service. It owns the pure viewport/tile plan, stable tile identity,
progressive complete-level publication, bounded WGPU residency, production
compute and generated-grid draw, and aggregate comparison. The narrow
`mclone-terrain-lab` Wasm app owns the browser surface/device facade;
`tools/terrain-lab` owns URL/DOM mechanics and the animation-frame pump.
Terrain rules must not be hand-copied into app crates, a standalone
TypeScript package, or hidden as renderer policy merely because WGSL consumes
them.

GPU meshing of ordinary CPU-authoritative chunks is adjacent but independent.
It may produce a larger near-field throughput win and applies to every profile,
but it should not be conflated with synthesizing untouched first-party terrain
directly from its generator.

## Recommended Experiment Sequence

### Experiment 0: Contract And Baseline

Status: completed for the hosted single-tile boundary by Tacticals 227-228.

- define the visual source identity and logical surface sample;
- define the scale-aware summary, parent/child identity, and screen-space error
  inputs;
- record the current CPU Far LOD generation, CPU mesh, payload, upload, and
  settled draw costs for `mclone-overworld-v1`;
- add or reuse GPU timestamp attribution for compute and draw;
- select fixed plane and periodic-cylinder seeds/sites; and
- retain the existing CPU path as a bit-for-bit off/fallback control;
- establish a URL-addressed Terrain Lab shell that can show CPU reference
  output before the GPU kernel exists; and
- define rebuild-to-first-redraw latency as an explicit iteration metric.

### Experiment 1: One GPU Surface Tile

Status: completed for production fields through base surface by Tacticals
227-228.

- implement the smallest shared `wgpu` compute kernel for continuous Mclone
  terrain fields;
- write height, water, material, and normal/ambient facts to resident GPU data;
- draw one tile through a reusable grid in the web Terrain Lab and the smallest
  native validation host;
- perform no readback and no canonical chunk mutation;
- capture and inspect pixels at the first drawable milestone; and
- compare the tile against CPU source samples.

### Experiment 2: Coverage-First Progressive Refinement

Status: first unfiltered viewport-driven slice completed locally by Tactical
229. Complete nested levels, retained parents, stable aligned identities,
bounded scheduling/residency, 1-through-1,024 spacing, and 65.5 km coverage
are proven. Scale-aware summaries, band limiting, mixed-level seams, and
in-game structural/depth coverage remain open.

- cover a fixed visible region immediately with a bounded 64-cell-style grid;
- prove at least one extreme level where one tile represents hundreds of
  chunks in each dimension without materializing those chunks;
- band-limit coarse field evaluation and inspect stability while panning;
- refine selected regions through powers-of-two levels down to 16, 8, 4, and 2
  blocks;
- retain parents until children are complete;
- prove exact desired coverage with the existing structural/depth oracle;
- implement aligned seam, skirt, morph, or dither policy; and
- measure stationary startup, pan/zoom, sustained movement, and
  rebuild-to-first-redraw latency.

### Experiment 2.5: Canonical Terrain Workspace

Status: active under Tactical 232.

- show independently hideable canonical, CPU LOD, and GPU LOD panes with one
  synchronized location and camera;
- generate bounded exact chunks progressively in a cancelable Web Worker;
- reuse production first-party textures, block meshing, and translucent water;
- distinguish surface/final generation checkpoints from presentation-only
  water and vegetation visibility;
- expose canonical and procedural cache/readiness facts independently; and
- use the resulting side-by-side evidence to define the later in-game handoff.

### Experiment 3: Real-Terrain Handoff And Structured Overlays

- permit procedural terrain underneath not-yet-drawable normal chunks;
- preserve real/procedural XOR at final frame admission;
- transition geometry, material, water, and lighting coherently;
- upload bounded planned-stream records rather than rerunning an opaque GPU
  global search; and
- validate world switch, device rebuild, and source-revision invalidation.

### Experiment 4: Optional Semantic Readback

- dispatch several canonical requests ahead of need;
- read back only semantic columns first;
- expand them through the ordinary CPU chunk/material/tick path;
- prove exact output or keep the result diagnostic-only;
- compare end-to-end latency and device contention against the CPU worker; and
- retain CPU fallback and dedicated-server coverage.

### Experiment 5: Volumetric Lab

- choose one bounded 3D terrain specimen;
- compare sparse bricks plus GPU meshing against direct DDA/ray casting;
- include per-eye/multiview, occupancy update, memory, and worst-frame costs;
- keep the result presentation-only until simulation and persistence contracts
  are separately designed; and
- decide whether the evidence justifies a durable volumetric renderer.

Each experiment should become its own tactical when authorized. Completing one
does not silently authorize or commit to the next.

## Validation And Acceptance

### Correctness And Coherence

- fixed-seed CPU/GPU point and region comparisons;
- direct-coarse versus child-roll-up summary comparisons where both exist;
- maximum and percentile surface-height error;
- water-presence, water-height, material, biome-tint, and category mismatch;
- exact X-periodic cylinder seams where the profile supports them;
- neighbor tile and mixed-level crack checks;
- request-order, movement, cancellation, and stale-result independence;
- complete settled coverage with no real/procedural overlap;
- coherent geometry/material/light handoff;
- device-loss/resource-rebuild regeneration; and
- active/standby world identity isolation.

### Rendered Output

- capture and inspect the first tile before building the full hierarchy;
- inspect a continuous lab zoom from continent scale to the local surface;
- inspect desktop and phone-sized Terrain Lab layouts and touch interaction;
- inspect fixed lowland, coast, mountain, river, snow, and planned-stream
  views;
- inspect refinement and real-terrain transitions in motion;
- prove mono, stereo, and multiview ownership; and
- use `/tmp` for all debug and acceptance captures.

### Performance

- edit/rebuild-to-first-coarse-redraw time in the Terrain Lab;
- CPU terrain sampling and mesh time avoided;
- CPU worker occupancy and main-thread acceptance time;
- request and upload bytes;
- resident GPU bytes by level;
- compute dispatch count, duration, and oldest pending age;
- draw count, vertices/indices or procedural grid work, and fragment pressure;
- first coarse coverage time;
- time to each refinement level;
- time to canonical replacement;
- device/frame p50, p95, p99, and worst-frame cost;
- desktop native, browser WebGPU, Steam Deck, and Quest evidence before
  promoting platform defaults; and
- the disabled/fallback path remains byte- and behavior-stable where practical.

The relevant success question is not only whether a kernel evaluates samples
quickly. It is whether a new seed reaches a compelling, complete horizon
sooner without delaying input, canonical gameplay readiness, or frame
submission.

## Latest Receipt And Next Direction

Tactical 229 replaces the fixed single tile with a continuous viewport over
aligned 64-cell tiles. The URL now separates `blocks` across from `detail`;
legacy `spacing=` links retain their prior visible footprint. Auto chooses a
power-of-two spacing from panel CSS density. Manual `1:1` through `1:1024`
remains explicit, and the UI reports when the eight-tile-per-axis safety
budget raises effective detail.

The shared scheduler:

- starts with a level requiring at most two visible tiles per axis;
- admits center-first missing tiles under a four-tile and 8 ms CPU reference
  budget per browser frame;
- retains that complete parent while finer levels compile;
- publishes only complete levels;
- preloads one target-level tile margin;
- keeps up to 192 stable resident tiles keyed by seed, origin, and spacing
  within the renderer's fixed field/evaluator revision;
- discards obsolete queued work by viewport epoch; and
- aggregates asynchronous GPU readback over the complete target footprint.

The fixed seed `-98765`, center `(-304, 336)`, and about 2 km Auto view selected
`1:16` on both the desktop and Pixel 7 layouts. Headed-Wayland BrowserWebGPU
desktop side-by-side and phone stacked Compare pixels were inspected. Both
show the same geography, and map view fills each panel instead of letterboxing
an already aspect-correct viewport.

The local and hosted 65.5 km Auto maps selected `1:512` with 12 complete
visible tiles. Desktop and mobile receipts measured:

- base mean/P95 height error effectively zero;
- 100% ocean agreement;
- mean continentalness error around `1.8e-8`; and
- roughly 43–46 MiB of resident reference/GPU tile buffers after the complete
  multi-interaction smoke cache.

A manual `1:1` request at 4.1 km selected effective `1:16` and displayed
`1:1 -> 1:16` with its visible tile count. This is an explicit bounded-budget
result. At a close viewport, the same requested level becomes admissible.

The Lab now reports CPU reference compile wall time, WGPU encode/submit wall
time, first complete coarse pixel, first target pixel, resident/queued/evicted
tiles, and stale work. It does not label encode/submit as GPU execution:
portable timestamp queries are absent, so `GPU execution: unavailable` is the
honest answer to the earlier observation that CPU and GPU appeared equally
instantaneous.

The first Wasm run exposed `std::time::Instant` as an unsupported host call.
The renderer now accepts an injected monotonic clock from the Wasm facade;
scheduler policy remains shared and host-neutral.

Production asset version `4cff438b1e95-20260724111946` is live at
`https://mclone.kzahel.com/terrain/`, served by Worker version
`5575a252-3d41-48f9-b308-fa42f65220f7`. Dedicated hosted headed-Wayland
desktop and phone smokes exercised the complete interaction flow. Both settled
the final 65.5 km request at `1:512`, published all 12 visible tiles, and
drained the queue. The hosted desktop/mobile runs retained about 46/43 MiB
after their multi-interaction caches; their first complete coarse pixels
arrived within 79 ms and target pixels within 230 ms.

Tactical 230 corrects the misleading synchronized result. CPU reference
compilation and GPU dispatch now have independent queues; GPU work is
submitted before the bounded CPU compiler runs, and each panel publishes its
own finest complete level. Comparison identity is unchanged, and aggregate
error remains pending until both target sample sets exist.

The default cache is explicitly a session-local 192-tile LRU keyed by seed,
aligned origin, and spacing within the fixed field/evaluator revision. It
retains CPU samples, uploaded reference buffers, GPU-computed buffers, and
validation readbacks. `Cache off` invalidates residency when the generation
viewport changes, keeps only the active request, and skips preload. `Cold
current view` invalidates without changing coordinates.

The fixed local cold stress race disables cache and uses seed `-98765`, center
`(-304, 336)`, a 2 km Compare map, requested `1:2`, and a 12-tile-per-axis
diagnostic budget. Both desktop and phone recorded
`neither -> GPU only -> both` target readiness. Desktop GPU
target-plus-readback completed in 287.9 ms versus 2,595.5 ms for CPU target
publication; phone measured 665.6 ms versus 1,881.3 ms. Both retained
effectively zero base mean/P95 error and 100% ocean agreement. These are
end-to-end fixed-workload timings, not pure GPU execution measurements.

The targeted production upload changed only Terrain Lab. The existing Worker
version `5575a252-3d41-48f9-b308-fa42f65220f7` now serves JavaScript
`index-BcjjqS2k.js`, stylesheet `index-Jbtyl1cl.css`, and Wasm
`mclone_terrain_lab_bg-BZKifX35.wasm`. Hosted headed-Wayland BrowserWebGPU
desktop and phone smokes reproduced `neither -> GPU only -> both` with cache
off and zero hits. The desktop fixed race measured GPU
target-plus-readback at 278.2 ms versus 2,187.3 ms for CPU target publication;
the phone measured 686.4 ms versus 1,918.9 ms. Inspected side-by-side and
stacked captures showed matching coordinate-locked geography.

Tactical 232 now proves the canonical workspace end to end. The default route
shows exact final terrain, CPU LOD, and GPU LOD at one seed, center, scale, and
camera. The exact pane has a separately bounded radius, compiles production
chunks center-first in a replaceable Worker, transfers each result
independently, and incrementally updates the shared textured section renderer.
Final/surface generation, water/vegetation presentation, and exact/LOD cache
domains are separate controls.

Fixed-seed shared tests compare canonical final chunks directly with production
generation. Presentation filters preserve the pre-filter block/biome
fingerprint. Desktop and Pixel Playwright coverage exercised exact completion,
shared orbit, pane toggles, visibility-only remeshing, map cancellation,
cache-off work, and independent CPU/GPU publication. A manually exercised
25-chunk final footprint remained progressive after replacing combined-mesh
rebuilds with section and immediate-neighbor updates.

The targeted hosted upload changed only Terrain Lab objects. The unchanged
Cloudflare Worker now serves JavaScript `index-BbUZPeEi.js`, canonical Worker
`canonical-worker-DEYoRExd.js`, stylesheet `index-D63K5d8a.css`, and Wasm
`mclone_terrain_lab_bg-CRfewa58.wasm`. Each hashed object was byte-verified
before HTML switched. Hosted headed-Wayland desktop and Pixel smokes passed
against `https://mclone.kzahel.com/terrain/`.

The hosted Pixel run reached the first of nine exact final chunks in 170.1 ms
and completed the footprint in 423.6 ms. The desktop run measured 182.9 ms and
578.4 ms. Both retained effectively zero CPU/GPU base-height mean/P95 error,
100% ocean agreement, and about `2e-8` continentalness error through the
65.5 km view. The Pixel cold race published the GPU target plus readback in
1,010.2 ms and the CPU target in 3,052.8 ms. The desktop run, under other host
build load, measured 469.7 ms and 11,726.4 ms. These remain different
end-to-end boundaries, not pure GPU execution timings.

The exact pane also exposed a production asset truth: many current block
catalogue entries resolve to generated fallback atlas tiles because curated
first-party materials are incomplete. The Lab labels those fallbacks rather
than substituting reference Minecraft assets or presenting their purple/pink
appearance as a worldgen or renderer mismatch.

This proves useful navigation, bounded exact and resident generation,
progressive point-sampled detail, and the shared exact-versus-LOD review
boundary. It does not yet prove a truthful far summary. The 65.5 km
continentalness image still evaluates sub-footprint field energy at points, so
aliasing and temporal stability remain open correctness problems.

Tactical 233 removes the remaining presentation mismatch before far-summary
work. Exact and LOD panes now use one physical projection, CPU/GPU samples
carry a shared macro surface block ID with measured 100% agreement, and both
LOD lanes sample the first-party atlas through five filtered mip levels.
Exact terrain opts into the same smooth overview minification without changing
the game's vanilla-compatible default. Right drag and focused arrows pan the
shared URL center in map and 3D.

Local headed-Wayland desktop and Pixel smokes passed the complete interaction,
65.5 km, and cold-race flows. Material agreement remained 100% alongside zero
base mean/P95 height error, 100% ocean agreement, and about `2e-8`
continentalness error. The macro contract is deliberately uncarved: final
river/wetland clay and planned-stream materials still belong to the exact pane
until structured overlays reach the GPU path.

The targeted production upload changed only `/terrain/` objects and was built
from clean product commit `7a538223`. The unchanged Worker now serves
JavaScript `index-DC0tTMgU.js`, canonical Worker
`canonical-worker-C3EyhCMt.js`, stylesheet `index-BUU9cbcd.css`, and Wasm
`mclone_terrain_lab_bg-CEXO1oC3.wasm`. Every object was downloaded from R2 and
byte-verified before acceptance.

Hosted headed-Wayland desktop and Pixel smokes passed at
`https://mclone.kzahel.com/terrain/`, including desktop right-button pan,
focused arrow pan, exact completion, 65.5 km navigation, and the cold
independent scheduler race. Both reported 100% macro-material and ocean
agreement, zero review base mean/P95 height error, and about `2e-8`
continentalness error. The hosted cold race published GPU target plus
readback in 847.6/1,123.8 ms on desktop/Pixel versus CPU target publication in
14,134.1/12,758.1 ms. These remain end-to-end boundaries rather than pure GPU
execution.

Human review of the exact/LOD workspace then selected hydrology as the next
macro-content campaign. Tactical
[`234`](../tactical/234-terrain-lab-hydrology-and-provenance.md) owns
dependency-ordered preview stages, natural CPU/GPU watercourse parity,
scale-aware signed-contour preservation, sparse planned-stream overlays, and
production-derived diagnostic receipts. These are preview compiler stages,
not authoritative gameplay generator flags.

Tactical 234 is now complete. Natural rivers, variable banks, wetlands, pools,
submerged outlets, final materials, biome/surface recipes, and production
landform classes run in the GPU evaluator and the CPU reference lane. Planned
streams remain bounded CPU-produced records and are merged into both panes at
`1:1` through `1:4`. The reviewed stream at seed `-98765`, point
`(2369, -1977)`, is visible in both panes and names owner chunk `(147, -126)`.

The Lab exposes dependency-ordered `Base`, `Hydrology`, `Structured`,
`Surface`, and `Cover` stages plus river, wetland, landform, biome, surface,
and planned-stream diagnostics. Map/3D point picking returns a
production-derived receipt with ordered semantic decisions, hydrology fields,
structured ownership, and field/schema/evaluator/decoration revisions.
Content stage is part of URL and tile/cache identity; cache-off remains a real
cold path.

The final targeted upload serves preview schema v5 and GPU evaluator A5. Hosted
desktop and Pixel headed-WebGPU smokes reported zero natural final/base height
error and 100% river, visible-material, biome, surface, and reviewed landform
agreement. In the fixed uncached race, desktop GPU target plus readback took
1,579.4 ms versus 22,341.6 ms for CPU target publication; Pixel measured
2,010.3 ms versus 21,050.8 ms. These are end-to-end publication boundaries,
not pure GPU execution timing. The complete object hashes and validation
receipt live in Tactical 234.

Tactical
[`236`](../tactical/236-terrain-lab-multitouch-and-exact-footprints.md) is
also complete. Two-contact gestures now combine centroid pan and pinch zoom
in map and 3D across both procedural and canonical panes. The shared canonical
preview bound now offers center-first `7x7 = 49` and `9x9 = 81` exact
footprints. A phone-sized browser observed progressive publication through all
81 final-feature chunks in about four seconds, and the same byte-verified
bundle passed hosted desktop and phone headed-WebGPU smokes.

Tactical
[`237`](../tactical/237-terrain-lab-block-detail-zoom.md) is complete. The
shared viewport now moves continuously from a 131,072-block continental
overview to one block in map and 3D. Sub-tile procedural views retain and crop
an aligned spacing-one tile, while canonical rendering uses a
production-surface-focused projection that exposes complete block faces and
crisp authored texture texels. Inspected desktop and phone captures prove the
same one-block footprint across canonical, CPU LOD, and GPU LOD panes. The
byte-verified `/terrain/` bundle also passed hosted desktop and phone
headed-WebGPU regression smokes.

The next implementation direction is:

1. band-limit or aggregate other sub-sample field energy while preserving
   coast and mountain silhouettes;
2. add sparse structure and macro vegetation records/layers without moving
   bounded placement search into WGSL;
3. add a Rust/WGSL edit watcher and measure source-edit to first updated
   coarse pixel;
4. choose explicit mixed-level seam/transition behavior before using partial
   child coverage; and
5. decide whether the current Far LOD control plane should adopt the shared
   procedural content and exact-handoff contracts proven in the Lab.

## Open Questions

- Should the first resident shape remain chunk-granular, use a geometry
  clipmap, or combine chunk identities near the handoff with clipmap rings far
  away?
- Is an approximate current-profile evaluator visually close enough, or should
  the first experiment introduce a deliberately GPU-compatible field revision?
- Which structured records are small and stable enough to upload for streams,
  major landmarks, and later structures?
- How should water and river surfaces morph across sample levels without
  introducing slopes, open faces, or shoreline flicker?
- Which coarse lighting facts best hide the transition to exact packed light?
- How much compute can overlap rendering on each `wgpu` backend, and where does
  one queue simply serialize the work?
- Does desktop benefit more from direct procedural terrain, ordinary GPU
  meshing, or both under one budget?
- What is the smallest semantic readback that can materially accelerate
  canonical chunks?
- How are remote seed disclosure and server-streamed procedural descriptors
  represented without weakening authority or privacy?
- When do distant player edits justify sparse overlays rather than waiting for
  normal chunks?
- What field precision and world-coordinate decomposition avoid visible
  jitter kilometers from the origin?
- Which summary statistics preserve mountain silhouettes, islands, river
  networks, and authored landmarks at each footprint?
- At what level should a parent switch from direct procedural synthesis to a
  roll-up of resident canonical or edited descendants?
- Is a small dedicated Terrain Lab WASM payload practical while sharing the
  production evaluator and WGPU pipelines with the game?
- Should the minimal native diagnostic be a dedicated app, an offscreen
  harness, or a mode of an existing host after the shared service exists?
- At what point do volumetric bricks justify their memory and update cost over
  a surface shell?

## Related

- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`far-lod.md`](far-lod.md)
- [`../lod-architecture.md`](../lod-architecture.md)
- [`lighting.md`](lighting.md)
- [`dynamic-point-lights.md`](dynamic-point-lights.md)
- [`performance.md`](performance.md)
- [`world-height-and-volumetric-streaming.md`](world-height-and-volumetric-streaming.md)
- [`structure-lab.md`](structure-lab.md)
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md)
- [`../native-web.md`](../native-web.md)
- [`../runtime-data-model.md`](../runtime-data-model.md)
- [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md)
