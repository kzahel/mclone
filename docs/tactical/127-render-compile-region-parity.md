# 127: Render Compile Region Parity

Status: proposed. Shared native Rust renderer/runtime workstream.

## Purpose

Track the remaining Java-parity gap in how terrain render compile jobs see world
data. Tactical `120` owns live pacing and backpressure. This note owns the
compile-input shape: replacing ad-hoc snapshot payloads with a Java-shaped small
render-region view that is correct, cheap to hand to workers, and shared across
native desktop, Android, XR, and web where practical.

## Reference Shape

Primary Java files:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/RenderChunkRegion.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`

Relevant behavior:

- `RenderChunk.createCompileTask()` cancels stale work and builds one
  `RebuildTask` for one 16x16x16 render chunk.
- The task captures a `RenderChunkRegion` from the live `Level`, not the whole
  view distance.
- `RenderChunkRegion` stores the relevant `LevelChunk` grid and a padded block
  state array for the local render chunk region.
- `RebuildTask.compile(...)` iterates only the target render chunk's 16 cubed
  block positions, but rendering can query the region for neighbor block state,
  fluid state, block entities, light engine, shade, and tint.
- `RenderChunk.hasAllNeighbors()` prevents far chunk rebuilds from compiling
  against missing cardinal neighbors, while nearby chunks are allowed through.
- `LevelRenderer.compileChunksUntil(...)` schedules compile tasks under a frame
  deadline, while `ChunkRenderDispatcher` gates execution on free
  `ChunkBufferBuilderPack`s and publishes uploads later.

## Current Native Shape

The shared render loop now follows several Java pacing ideas: compile capacity,
deadline-driven admission, old-mesh retention, stale-result checks, and
completed-result acceptance. The input payload is still not Java-shaped.

Current native compile request:

- carries an owned `Vec<ChunkSnapshot>`,
- is moved to native worker threads,
- now includes only the target render-section chunks plus four horizontal mesh
  neighbors for native lanes,
- still deep-clones full chunk columns on the render/main frame.

Current web compile request:

- uses the same `RenderSectionCompiler` trait,
- sends delta upserts/evictions into a resident worker-side snapshot mirror,
- compiles from that mirror rather than from a complete per-request snapshot
  payload.

The native targeted snapshot pass is a measured performance win, but it is not
the final parity target. It approximates the small-region idea with whole chunk
columns, and it currently models horizontal face-culling neighbors more than the
full Java `RenderChunkRegion` contract.

### Local Block Edit Stress Case

Block add/remove stalls are related to this tactical, but only through the
compile-submit side of the problem. A local edit can dirty one or more render
sections and trigger several small compile submissions in quick succession. If
each submission performs avoidable chunk-column cloning, request construction, or
handoff work on the frame path, that cost becomes visible while mining/placing
blocks, especially when the player is moving.

The one-frame sky/background flash is still primarily a render-publication
coherence issue owned by `113`, `119`, and `120`: keep old visible meshes until
replacement sections or dirty groups are ready. Region parity should support that
policy by making local edit rebuild inputs cheap and explicit, not by encouraging
Quest/XR to copy Java's synchronous nearby rebuild behavior.

## Parity Gaps

- **Region shape:** Java uses a padded block region around a single render
  chunk. Native uses whole `ChunkSnapshot` columns, which are larger than
  necessary and not explicit about the exact sampled block bounds.
- **Neighbor completeness:** The current native five-column payload is enough
  for cardinal cross-chunk face culling, but Java's padded region can support
  corner/edge samples used by AO, model rendering, tint, fluid, light, and block
  entity lookups. Audit native mesh access before assuming cardinal-only
  neighbors are always sufficient.
- **Ownership:** Java rebuild tasks reference/copy a compact region. Native
  currently deep-clones `ChunkSnapshot` data into each request. Web has a
  resident mirror, but that is a transport strategy rather than a shared region
  abstraction.
- **Vertical scope:** Java captures the target render chunk's vertical range
  plus padding. Native sends whole chunk columns, including sections outside the
  target section's mesh neighborhood.
- **Worker state:** Java workers receive a task plus a free buffer pack. Native
  workers receive owned snapshots and build from scratch. A future native
  resident mirror may match Java's "world data already exists outside the task"
  behavior better, but it still needs an explicit small-region read contract.
- **Local edit coherence:** Java can rebuild nearby player-dirty chunks
  synchronously. Native should not make region ownership opaque in a way that
  makes synchronous/local edit publication harder.

## Candidate Slices

### A. Submit-Cost Attribution

Status: first measurement landed in `120`.

Split remaining native `runtime_submit_ms` into:

- target-section snapshot-position selection,
- chunk snapshot clone/copy,
- request construction,
- compiler handoff/channel send.

Use this to decide whether the next optimization should be region ownership or
handoff scheduling.

First Quest RD7 result:

- max submit total: 14.130 ms,
- max snapshot selection/clone: 0.979 ms,
- max handoff bucket: 13.873 ms.

Read: native per-request deep clones are no longer the measured long pole after
the targeted-neighborhood pass. The handoff bucket still needs a finer split
before we choose between queue mechanics, ready-plan dirty-state bookkeeping, or
region ownership work.

### B. Explicit Compile Region Contract

Status: planned.

Introduce a shared render compile input type that names:

- target render sections,
- required chunk positions,
- exact padded block bounds per target section or chunk,
- light/block-entity/tint access expectations.

This can first wrap existing snapshots, but the contract should be named as a
region, not as an arbitrary full-window snapshot vector.

### C. Java-Region Parity Audit

Status: planned.

Audit native meshing and block model rendering reads against Java
`RenderChunkRegion`:

- visible face culling,
- ambient occlusion side/corner samples,
- non-cubic model shape reads,
- fluid rendering neighbor reads,
- packed light reads,
- biome/tint reads,
- block entity reads.

Use the audit to decide whether target plus cardinal neighbors is sufficient for
current MVP content, or whether the native region must include diagonal
neighbors and/or exact padded block slices now.

### D. Native Resident Snapshot / Shared Ownership

Status: planned after attribution.

If future measurements show clone/copy dominates `runtime_submit_ms`, replace
per-request deep clones with one of:

- worker-side resident snapshot mirror with explicit upserts/evictions,
- shared immutable chunk snapshot ownership (`Arc`-style) with revision checks,
- compact copied `RenderChunkRegion` block arrays sized to the Java region.

The desired outcome is that live frame submission moves handles or compact
regions, not whole chunk columns.

### E. Web/Native Convergence

Status: planned.

Keep the `RenderSectionCompiler` trait shared, but do not force native and web
to share a serialized transport. Instead, converge them on one logical
compile-region contract:

- native can use Rust structs, channels, and shared ownership,
- web can keep `SharedArrayBuffer`/mirror mechanics,
- both must expose the same target sections, revision semantics, neighbor
  readiness, and compile-region diagnostics.

## Validation

- Unit tests: compile request payload/region selection should prove unrelated
  chunks are excluded and all required neighbor data is present.
- Java parity tests: targeted chunk-boundary fixtures should cover corner AO,
  fluid faces, light reads, and block-entity lookup once those features are
  in-scope.
- Local edit stress: add/remove block bursts while moving should remain a named
  performance probe for submit cost, result acceptance, and coherent publication.
- Performance: Quest Android XR settled-orbit render distance 7 remains the
  product lane for live pacing. Track `MCLONE_ANDROID_XR_PERF_TERRAIN_RUNTIME`
  and keep default result acceptance unbounded unless measured otherwise.

## Current Recommendation

The first submit split says snapshot clone/copy is not the immediate
performance blocker. Split `submit_prepared_sync_plan(...)` internals next:
request construction, compiler `submit`, and ready-plan/dirty-state mutation.
Keep resident/shared region ownership as the parity target, but do not treat it
as the next performance fix unless the finer split reverses the conclusion.
