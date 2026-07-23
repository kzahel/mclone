# GPU Procedural Terrain

Topic: `gpu-procedural-terrain`

Status: accepted research direction recorded 2026-07-23. No implementation
tactical is open yet. The first recommended proof is GPU-resident,
presentation-only terrain for `mclone-overworld-v1`: establish coarse visible
coverage immediately, refine it progressively, and let ordinary authoritative
chunks replace it when they become ready. Optional GPU-backed canonical chunk
generation and volumetric terrain remain separate later experiments.

## Scope

This topic owns the continuing direction for:

- reconstructing first-party procedural terrain directly on the GPU;
- showing coarse terrain before canonical chunks are generated, lit, meshed,
  and published;
- progressively refining one visible terrain representation through nested
  sample spacings such as 16, 8, 4, and 2 blocks;
- retaining procedural terrain on the GPU without CPU-built per-tile meshes;
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

There is no current terrain compute pipeline. Normal terrain and Far LOD arrive
at `mclone-render` as CPU-constructed mesh products.

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
semantics.

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
- **Browser/WebGPU:** retain the same visual contract where compute, storage,
  and device limits admit it. Worker/device ownership must follow the browser
  host boundary rather than leaking terrain policy into TypeScript.
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

## Ownership Direction

The shared-first boundary should be:

- `mclone-worldgen`: profile semantics, source identity, CPU reference
  evaluator, portable parameter/plan records, and any exact dual-backend
  arithmetic contract;
- `mclone-server`: generator-owned planning and canonical readiness, consuming
  an abstract completion backend rather than `wgpu`;
- `mclone-app-runtime`: bounded visual coverage/refinement scheduling, source
  revisions, capability policy, and optional compute-service assembly;
- `mclone-render-session`: resident procedural-tile or clipmap lifecycle,
  replacement readiness, and upload/compute admission;
- `mclone-render`: GPU buffers/textures, compute and draw pipelines, material
  sampling, timestamps, mono/per-eye/multiview drawing, and resource rebuild;
- `mclone-scene`: active-world orchestration, real/procedural arbitration,
  frame budgets, world switching, and diagnostics; and
- app/platform crates: adapter/device creation, surface/session lifecycle, raw
  capability collection, and presentation only.

The exact crate split is deferred until the first proof identifies a real
shared API. Terrain rules must not be hand-copied into app crates or hidden as
renderer policy merely because WGSL consumes them.

GPU meshing of ordinary CPU-authoritative chunks is adjacent but independent.
It may produce a larger near-field throughput win and applies to every profile,
but it should not be conflated with synthesizing untouched first-party terrain
directly from its generator.

## Recommended Experiment Sequence

### Experiment 0: Contract And Baseline

- define the visual source identity and logical surface sample;
- record the current CPU Far LOD generation, CPU mesh, payload, upload, and
  settled draw costs for `mclone-overworld-v1`;
- add or reuse GPU timestamp attribution for compute and draw;
- select fixed plane and periodic-cylinder seeds/sites; and
- retain the existing CPU path as a bit-for-bit off/fallback control.

### Experiment 1: One GPU Surface Tile

- implement the smallest shared `wgpu` compute kernel for continuous Mclone
  terrain fields;
- write height, water, material, and normal/ambient facts to resident GPU data;
- draw one tile through a reusable grid in mono and multiview;
- perform no readback and no canonical chunk mutation;
- capture and inspect pixels at the first drawable milestone; and
- compare the tile against CPU source samples.

### Experiment 2: Coverage-First Progressive Refinement

- cover a fixed visible region at 16-block spacing;
- refine selected regions through 8, 4, and 2 blocks;
- retain parents until children are complete;
- prove exact desired coverage with the existing structural/depth oracle;
- implement aligned seam, skirt, morph, or dither policy; and
- measure stationary startup plus sustained movement.

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
- inspect fixed lowland, coast, mountain, river, snow, and planned-stream
  views;
- inspect refinement and real-terrain transitions in motion;
- prove mono, stereo, and multiview ownership; and
- use `/tmp` for all debug and acceptance captures.

### Performance

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
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md)
- [`../runtime-data-model.md`](../runtime-data-model.md)
- [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md)
