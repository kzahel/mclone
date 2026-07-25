# 166: Shared Resident-Tile Substrate (Real Sections + LOD)

Retirement note 2026-07-25: the extraction remains a useful execution record,
but its synthetic chunk-based LOD producer is rejected and scheduled for
removal by Tactical [`245`](245-retire-chunk-far-lod-runtime.md). Only the
ordinary real-section consumers of generic residency, upload, budgeting, and
compile machinery remain supported.

Status: complete 2026-07-11. Slices 1–3 landed the behavior-preserving
resident-tile/cache extraction, shared budget vocabulary, and synthetic far-LOD
producer on shared workers, admission, residency, and per-tile region arenas.
Tactical 171 — Convergence And Parity Closeout Milestone D then integrated and
proved that producer in the production browser local-worker, IndexedDB, and
remote-WebSocket lanes. Slice 4 has now landed 4/8/16-block multi-level rings,
hysteresis, replacement-before-suppress transitions, cross-level edge rebuilds,
and bounded diagnostics. Tactical 162 — Real-Chunk LOD Reduction Draft Slice 4
is the next producer. Opened after a far-LOD stutter investigation found that
far LOD reimplemented a crude, unbudgeted copy of the real-section
residency/upload pipeline instead of sharing it. This tactical owns the shared
substrate; it **pauses and re-scopes** Tactical 162 — Real-Chunk LOD Reduction
Draft Slice 3+ (see Relationship).

Workstream: native Rust shared runtime/render-session/render boundary. Desktop
validation first, but the target shape must stay host-neutral across flat, XR,
Android, and web. This is squarely a shared-first refactor, not a platform lane.

## Impetus

Far LOD (`far_lod.rs` + `FarTerrainLodRenderer`) and normal terrain
(`mclone-render-session` + `chunk.rs`) both want the same thing: keep
world-aligned tiles resident, rebuild/upload only the tiles that changed, admit
that work under a frame budget, and evict tiles that leave range. Normal terrain
has all of this and it is measured and tuned (tacticals
[`128`](128-terrain-render-pipeline-coordination.md),
[`150`](150-adaptive-frame-budget-controller.md),
[`120`](120-vanilla-render-compile-backpressure.md)). Far LOD has none of it:

- generation is "incremental" only by count: `advance_for_camera` runs
  **synchronously on the frame thread** with a fixed
  `DEFAULT_FAR_TERRAIN_LOD_CHUNK_BUILD_BUDGET = 4` patches per call — a count
  constant, not a frame-health signal, and not on the render-compile worker
  pool real sections use;
- meshing re-concatenates **every** retained patch into one monolithic
  `FarTerrainLodMesh` on every chunk-center change and every ready-normal-chunk
  change (`far_lod.rs::rebuild_mesh`; the drawable normal-chunk set is folded
  into the build key, so each newly drawable real chunk also trips a full
  rebuild),
- GPU upload re-writes the **entire** merged vertex+index buffer on every
  revision bump (`FarTerrainLodRenderer::upload_mesh`), amplified per eye/frame
  slot in XR, and
- none of it consults `mclone-frame-budget` or any frame-health signal.

The renderer also carries a latent correctness bug that argues for deletion
rather than repair: vertex data is slotted per view
(`vertex_buffer_slot_size * PER_VIEW_UNIFORM_SLOT_COUNT`) but the index buffer
and `uploaded_index_count` are global, so two view slots holding different
revisions can draw one revision's vertices with another's indices.

The result is unbudgeted, whole-world re-mesh + re-upload on every chunk step —
unusable in XR because the framedrops cause discomfort during movement.

The fix is not "give far LOD its own budget/queue" (that deepens the divergence).
The fix is to make far LOD a **second producer** on the same
residency/priority/budget substrate normal sections already use, so there is one
system that coordinates real chunks and reduced-resolution chunks by policy.

## Position

There is one shared substrate and several producers. The substrate owns
*mechanism*; producers own *representation*.

```text
Shared substrate (mechanism)
  resident tile cache: per-tile slot, dirty flag, world-aligned keep/add/evict
  build admission + priority lanes, fed by mclone-frame-budget
  budgeted per-tile upload admission (policy, queue, lifecycle accounting)
  eviction/diff algorithm and resident stats
  chunk-granular cross-producer precedence (the existing LOD coordinator)

Producers (representation) — NOT unified with each other
  real section        level 0, authoritative snapshot -> voxel greedy mesh, textured+lit
  synthetic far LOD   level N, seed/worldgen surface  -> coarse surface shell, flat color
  reduced-real LOD    level N, real snapshot reduced  -> DH-like spans (162 Slice 4, later)
```

What is shared: the tile lifecycle, the admission/budget lanes, the upload
admission *policy* (budgets, queue, lifecycle accounting), the eviction/diff
algorithm, and the precedence coordinator.

What is **not** shared, on purpose (anti-over-abstraction guardrail): the
mesher, the vertex format, the draw pipeline/shader, the data source, and the
**GPU buffer layout**. A real section and a surface shell are different meshes
with different shaders and — critically — different draw-batching needs (see
GPU Residency And Draw Batching). Do not force a common tile *data model* or a
common buffer scheme; unify how tiles *live and get scheduled*, not what a tile
*is* or how its bytes sit on the GPU.

## Locked Invariant: Chunk-Granular Tiles Only

Every tile — at any LOD level — covers exactly one chunk column footprint in
X/Z. There is no multi-chunk tile and no "negative" LOD level where one tile
spans several chunks.

Consequences (why this invariant is worth locking):

- tile keys stay chunk-aligned in X/Z, so the substrate can map any tile to a
  `ChunkPos` for range checks, eviction, and precedence against normal chunks;
- cross-producer precedence stays chunk-granular (a drawable real chunk
  suppresses the LOD tile at the same chunk — already how
  `LodCoverageCoordinator` works);
- we avoid the quadtree / mip-pyramid tile merge-and-split machinery that
  Distant Horizons-style multi-chunk far tiles require.

LOD level changes **resolution within** the chunk footprint (sample spacing),
never the footprint itself. The Y axis may still differ per producer (real
sections are 16³ subchunks, many per column; LOD tiles are one column patch) —
the substrate stays agnostic to Y granularity by being generic over the tile key
(see Coordinator/Substrate Shape). The invariant is specifically about X/Z
footprint.

### Distance-scaling outlook (where this invariant actually ends)

Two hard facts bound how far chunk-granular tiles can go:

- **Spacing floor:** a 16-block chunk footprint cannot go coarser than
  16-block sample spacing (one sample per chunk). The 4/8/16 ring example below
  is already at the ceiling; further reduction *requires* multi-chunk tiles.
- **CPU bookkeeping, not GPU, is the binding constraint.** With region-arena
  draw batching (below), GPU cost stops scaling with tile count: radius 128
  chunks (~2 km) is ~66k tiles ≈ roughly 10–40 MB of vertex data (packed vs
  current fat format) and a few hundred region draws — comparable to today's
  real-section draw counts at RD10. What scales linearly is per-tile CPU state:
  coverage sets, coordinator maps, desired-set diffs. Those must be
  event-driven (recomputed on center/set/config change, not per frame), and the
  `MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES = 4096` cap gets raised as part of any
  far-distance experiment.

Future trigger (recorded, not now): when a far-distance experiment pushes past
roughly kilometer scale, per-tile CPU bookkeeping and the 16-block spacing floor
— not draw calls — are what force multi-chunk tiles (DH-style merge pyramids).
Revisit the invariant at that point as an explicit, measured decision, not a
drift.

## LOD Levels As Concentric Rings

Before Slice 4, far LOD was a single ring at 4-block sample spacing outside
render distance. The landed shape is multiple concentric rings at increasing
spacing with distance
(e.g. 4 near, 8 further, 16 furthest — 16 being the chunk-granular ceiling),
each ring still chunk-granular.

Model:

- a tile's identity carries its `lod_level` (0 = real section; 1..N = coarser
  spacings);
- a tile's **desired** level is a pure function of its chunk distance from the
  camera (a distance→level band lookup); real render distance is the level-0
  band, then LOD bands stack outward;
- when the camera moves and a tile's band changes, its desired level changes.
  That is handled the same way as any other staleness: **mark the tile dirty /
  needs-regenerate at the new level**, build the new-level tile through the
  normal budgeted admission path, and keep the old-level tile drawable until the
  new one is drawable (replacement-before-suppress, the 128/162 invariant), then
  evict the old level.

Properties that keep this cheap and stable:

- **Work per chunk step is O(ring), not O(area).** A one-chunk camera step
  moves each band boundary by one chunk, so only the boundary rings of tiles
  change desired level; those rebuilds trickle through the budgeted lanes like
  any other work. Nothing global ever happens.
- **Band hysteresis (required, Slice 4).** A tile flips level only when it is
  ≥ H chunks (H = 2 to start) past the band boundary in the new direction.
  Without hysteresis, oscillating movement astride a boundary thrashes
  rebuild work at every step.
- **Cross-level seams stay tile-local.** Patches are independently meshed and
  already resolve edges against neighbor surface samples
  (`append_neighbor_inner_drop_face`, backed by a +1-chunk surface retention
  ring). A spacing-4 tile bordering a spacing-8 tile extends this same
  mechanism — drop faces/skirts to the neighbor's sampled height at the
  coarser spacing — never cross-tile mesh stitching. A tile's level change
  marks its four X/Z neighbors dirty so their edge faces re-resolve; neighbor
  dirtying is still O(ring).
- **Transition double-residency is bounded and observable.** Between "new
  level drawable" and "old level evicted," both levels of a tile are resident.
  Budgeted admission means a moving player accumulates a band of
  double-resident tiles; Slice 4 exposes a per-level double-residency counter
  and bounds the band (evict-oldest when the band exceeds a cap).

No new machinery is needed for multi-level beyond: the level dimension on the
key, the distance→level band policy with hysteresis, and
dirty-on-level-change (self + neighbors). It rides the same
resident/dirty/admit/upload/evict path as everything else.

## Coordinator / Substrate Shape

Generalize `CachedTexturedRenderSections` (`section_cache.rs`) into a reusable
resident-tile cache rather than duplicating it. The reusable core is:

```text
ResidentTileCache<K, M>
  slots: map<K, { metadata: M, dirty: bool }>
  by_chunk index for chunk-granular lookup/precedence
  apply_build_report(ready, rebuilt, removals) -> keep/add/remove diff
  dirty tracking: mark/clear/iterate dirty keys
  resident stats (count, vertex/index, owned bytes)
```

`K` must expose `chunk_pos()` and `lod_level()`; the cache is otherwise agnostic
to Y granularity and mesh type. Concrete instances:

- real: `ResidentTileCache<RenderSectionKey, TexturedRenderSectionMetadata>`;
  compiled mesh payloads remain transient upload-queue items, preserving the
  CPU-mesh eviction contract from Tactical 163 — Render Section CPU Mesh
  Eviction;
- synthetic LOD: `ResidentTileCache<LodTileKey, LodTileMetadata>` where
  `LodTileKey = { chunk: ChunkPos, level: u8 }`.

Genericity is deliberately minimal: share the diff/dirty/eviction algorithm and
the stats; keep the concrete mesh/vertex/shader types per producer. If review
finds the generics get gnarly (trait bounds leaking into `chunk.rs` or producer
code), the sanctioned fallback is two concrete caches that call shared
free-function helpers — the constraint that matters is one diff algorithm and
one budget owner, not one type.

Crate ownership (decided; was an implicit gap):

- **`mclone-render-session`** owns the substrate mechanism: `ResidentTileCache`,
  the upload admission policy/queue/lifecycle accounting (today's `upload.rs`
  vocabulary: `RenderSectionUploadCoordinator`, `RenderSectionUploadFramePolicy`),
  and the admission-lane wiring. It already owns `section_cache.rs`; the
  substrate is its generalization, not a new home.
- **`mclone-app-runtime`** owns the producers and policy: the synthetic LOD
  producer (`far_lod.rs`), the `LodCoverageCoordinator`, the distance→level band
  policy, hysteresis, and startup prewarm.
- **`mclone-render`** owns draw pipelines and GPU buffer layouts: per-section
  buffers for real sections, region arenas for LOD tiles (below).

Desired-coverage computation must become event-driven as part of this work:
today `far_lod_chunk_positions` rebuilds and sorts an O(outer²) position list
every frame. Under the substrate, desired coverage recomputes only on
chunk-center change, normal-drawable-set change, or config change.

Cross-producer precedence stays where it already is: the chunk-granular
`LodCoverageCoordinator` (tactical 162 Slice 2) decides, per chunk, whether the
drawable real section or a LOD tile is shown. It already models source/quality/
lifecycle per tile; it just needs to drive per-tile visibility of *resident LOD
tiles* instead of gating a monolithic merged mesh.

## Priority And Budget

One budget owner, ordered priority lanes:

```text
P1 underfoot / near real section build + light + publish
P2 normal visible real section mesh compile + upload
P3 reduced-real LOD reduction from already-available snapshots
P4 synthetic far LOD tile generation + mesh + upload
P5 discardable LOD cache writes (162 Slice 6, later)
```

**Vocabulary correction (important for implementers):** "lane" is priority
vocabulary, not a scheduler object. `mclone-frame-budget` is not a single queue
today and this tactical must not introduce one. It is a **decision-family
panel** — `BudgetDecisionFamily` (`RenderAdmission`, `SectionUpload`,
`RenderCompileWorkers`, `FeaturePublication`, …) with per-family budgets, EWMA
cost estimators, and a grant/spend ledger — consulted at several distinct
admission points. Implementing P1–P5 means:

- adding LOD decision families (e.g. `LodBuildAdmission`, `LodUpload`) to the
  existing panel, with skip reasons and queue-age telemetry like every other
  family; and
- a cross-family ordering policy applied at the existing admission points
  (real-section families outrank LOD families when both want the same frame
  headroom).

Building a new queue/scheduler struct alongside the panel would violate 150
rule 10 in spirit while citing this doc. Tripwire: if a Slice 2 diff contains a
new scheduler type rather than new `BudgetDecisionFamily` variants plus ordering
policy, stop and re-read this section.

**Worker pool (resolved; was an open question): one pool.** LOD generation and
meshing move off the frame thread onto the render-compile worker pool in
Slice 3 as a requirement, not an option. The inline synchronous generation in
`prepare_far_lod_mesh` is part of the stutter, and admitting LOD jobs through
the same `RenderCompileWorkers` capacity derivation is how the budget stays
genuinely unified instead of two pools fighting for cores.

Adaptive rules (from 162's Scheduling section, now actually wired):

- frames over budget or dropped → stop optional LOD build/upload; keep already
  drawable LOD visible unless LOD *draw* cost is the measured problem;
- normal near-chunk backlog → prefer real section work;
- normal work caught up + stable headroom → spend a small, slowly-raised LOD
  budget;
- LOD queue age growing while frame health is fine → allow a trickle so coverage
  fills instead of starving;
- all LOD build/upload jobs cancellable / cheaply discardable when the desired
  level or coverage moves.

Startup keeps the bounded prewarm burst (162 Slice 1): before the first playable
frame there is no live frame-comfort contract, so an explicit, time/tile-capped
LOD prewarm is allowed. This is where the "always render terrain, no empty spots"
goal lives safely — as a bounded startup fill, not a live-frame stall. After
Slice 3 the prewarm drives the producer through the shared queue with a
startup-mode budget instead of the private `mesh_for_camera` path.

This aligns with 150 rule 10 (no new platform-local budget policy): LOD gets no
bespoke budget constant; it becomes families/outputs of the existing controller.

## GPU Residency And Draw Batching (Region Arenas)

The monolithic `FarTerrainLodRenderer` has exactly one virtue: one draw call.
Replacing it with one `wgpu::Buffer` pair per tile would fix the upload
pathology but buy a draw-call pathology: LOD tiles are tiny (a spacing-4 patch
is ~30 quads with edge faces; a spacing-16 patch is ~5), and typical coverage is
~700–2,000 tiles (up to the 4,096 cap). Thousands of ~50-triangle draws is a
genuinely bad shape on Quest — plausibly worse than the stutter it replaces.
Real sections tolerate per-section draws because a 16³ voxel mesh amortizes its
draw; a flat-color shell patch does not.

The resolution is to **decouple mesh granularity from draw granularity**. Tiles
never merge at the mesh layer; grouping happens one layer down, in buffer
allocation and index packing:

- **Region arenas.** Tiles of one LOD level are suballocated into region-owned
  buffers, keyed `RegionKey { region_pos, lod_level }` (starting size 16×16
  chunks per region — see open questions). Each region owns one vertex buffer
  and one index buffer.
- **Fixed-size vertex slots per tile.** Tile geometry is bounded by level
  (worst-case quads per spacing), so slab allocation is trivial and waste is a
  rounding error at these sizes. Uploading a rebuilt tile = one
  `queue.write_buffer` into its slot — the budgeted incremental upload,
  unchanged in policy, admitted through the `LodUpload` family.
- **Packed per-region index buffer.** Each region maintains the concatenated
  indices of its currently *visible* tiles (visibility = the coordinator's
  per-tile decision). When a tile in the region changes visibility or is
  rebuilt, that one region re-packs its index list. This is the only "group"
  operation anywhere in the design: region-local, index-only (vertices never
  move), and ≤ ~200 KB memcpy worst case for a dense 16×16 spacing-4 region —
  nothing like today's whole-world vertex re-concat. A 16×16 region also stays
  under 65k vertices worst-case, keeping u16 indices on the table.
- **Draw = one `draw_indexed` per region per level**, after per-region frustum
  culling (which the monolithic mesh never had). Tens of draws at today's
  coverage; a few hundred at kilometer-scale radius.

Deliberately boring portability property: this needs **no
multi-draw-indirect**, so it works identically on WebGPU, where wgpu's
`MULTI_DRAW_INDIRECT` feature does not exist. Multi-draw / GPU-driven culling
(Voxy-style) is a later, native-only optimization to consider *only if* region
re-pack shows up in a profile — an optimization, not a portability fork.

Store vertices **region-relative** (u16 offsets + per-region transform). Two
wins: the fat 28-byte vertex (7×f32) shrinks to ~8–12 bytes, and float32
precision stops degrading at very large world coordinates — which otherwise
bites exactly when far-far rendering gets pushed.

Substrate boundary restated for this layer: the substrate owns upload
*admission* (budgets, queue, lifecycle, accounting — extended from today's
`RenderSectionUploadCoordinator`); the producer's renderer owns buffer layout
underneath that contract. Real sections keep per-section buffers; the LOD
producer owns region arenas; both answer to the same admission policy. This
extends the existing "draw pipeline is per-producer" line one notch down, and
means Slice 1 extracts *less*, not more.

Mixed levels compose without new machinery: level is part of the region key, so
during a band transition the same area is covered by tiles in the level-1 region
*and* the level-2 region; the coordinator's per-tile visibility bit decides
which region's packed index list contains it. Replacement-before-suppress =
build new tile (budgeted), flip two visibility bits, re-pack two regions.

After this, a chunk step touches only the ring of tiles that changed and the
regions they live in — not the whole world — and both the upload bytes and the
draw count are bounded and observable. This is the change that removes the
movement stutter, especially the per-eye/per-slot amplification in XR.

## Regression Tripwires (all slices)

These apply to **every** slice; per-slice gates below add to them. They exist so
an implementing agent can mechanically answer "did I regress anything?" before
moving on.

Functional:

- workspace `cargo test` green; existing `far_lod` and `lod_coverage` unit
  tests keep passing or are ported 1:1 — no test deleted without a named
  replacement in the same slice;
- `pnpm native:desktop-offscreen:smoke` — capture and **look at** the
  screenshot (repo native-validation rule), at the first drawable milestone of
  the slice, not only at the end;
- far-LOD visual fixture: desktop offscreen captures at RD4, seed 12345,
  `--far-lod true`, at `playable` and `idle` readiness (the 162 Slice 2
  fixture): coarse LOD ring present, real chunks cleanly replace overlapping
  LOD, no blank seam;
- coordinator counters stay honest: in the steady scenarios above,
  `suppressed_without_replacement` delta stays `0`;
- web build/smoke (`pnpm native:web:build`, `pnpm native:web:smoke`) for any
  shared-crate change, per the platforms.md validation routing.

Perf (compare against the 150 baselines recorded in
`docs/performance-records.md` and `docs/quest-standalone-performance-records.md`;
"within noise" means inside the recorded run spread):

- desktop `pnpm native:startup-streaming:perf` (RD10, 6000 frames) and the RD15
  variant (`--render-distance 15 --startup-streaming-frames 9000`): playable /
  full-view / quiescent within spread, no new over-budget frames;
- `pnpm native:movement:smoke` and `pnpm native:timedemo:smoke`;
- Quest `pnpm native:android-xr:perf:orbit:rd5:metrics` and
  `...:rd7:metrics` vs the pinned gates (`skipped_delta=0`, over-2x `0`, app
  p95 within gate) — required for any slice that touches upload, admission, or
  the XR frame path; desktop-only slices may defer Quest to the next slice
  boundary but not past it.

Invariance:

- **far-LOD-off is byte-identical.** `--far-lod` defaults off; with it off,
  behavior, counters, and perf must be indistinguishable from the pre-slice
  commit through Slice 3 acceptance. LOD work must never tax the default path.
- **Real sections never regress.** Slices 1–2 are byte-for-byte for real
  sections (see per-slice gates); Slices 3–4 keep the real-section perf lanes
  within spread.

Stop rules:

- a failed gate **blocks the next slice** — bisect or revert; do not stack a
  new slice on an unexplained regression;
- baselines are re-pinned only as an explicit recorded decision with the
  measurement attached (150's re-pin rule), never silently absorbed;
- if a "temporary" inline/synchronous path survives its slice (e.g. LOD gen
  still on the frame thread after Slice 3), that is a gate failure, not a
  follow-up.

## Proposed Sequence (additive; real sections never regress)

Each slice ends with the all-slice tripwires above plus its own gate proven
before moving on.

### Slice 1: Extract the resident-tile substrate (real-only, behavior-preserving)

Work: generalize the section cache diff/dirty/eviction/stats into
`ResidentTileCache` and generalize the upload admission policy/queue
(`upload.rs`) into the substrate form — **policy and accounting only; buffer
layout stays in `mclone-render` untouched**. Real sections adopt the substrate
as the level-0 producer with byte-for-byte identical behavior. No LOD touched.
Pure refactor behind a perf gate.

Gate:

- a scripted fixed run (fixed seed/pose/`--freeze-time`) produces an identical
  `RenderSectionCacheUpdate` counter stream and a pixel-identical
  `native:desktop-offscreen:smoke` screenshot vs the pre-slice commit;
- desktop RD10/RD15 startup-streaming and Quest RD5/RD7 orbit within spread of
  the 150 baselines; no new over-budget frames;
- generics tripwire: if `ResidentTileCache<K, M>` bounds leak into `chunk.rs`
  or force churn in real-section call sites, fall back to two concrete caches
  + shared free functions *within this slice* — do not carry gnarly generics
  forward as debt.

Landed evidence (2026-07-11):

- `mclone-render-session::resident_tile` now owns
  `ResidentTileCache<K, M>`, `ResidentTileKey`, the neutral
  `ResidentTileUploadCoordinator<K, P>`, payload byte/key facts, neutral queue
  stats/drains, release lifecycle accounting, and the shared upload frame
  policy. `CachedTexturedRenderSections` and
  `RenderSectionUploadCoordinator` remain section-named facades, so no generic
  bound leaked into render-session, renderer, scene, or app call sites.
- Real sections use compact `TexturedRenderSectionMetadata` as level-zero
  resident metadata. Full meshes remain transient upload payloads, preserving
  Tactical 163 — Render Section CPU Mesh Eviction. No far-LOD producer, shader,
  GPU layout, frame-budget family, or default changed.
- The focused render-session suite passed `110/110`, including new independent
  multi-level-key cache and upload tests plus all existing real-section golden
  lifecycle tests. Client-experience ledger tests passed `23/23`; the shared
  scene suite passed `92/92`; direct render-session and scene Wasm checks and
  `pnpm native:web:build` passed.
- The fixed 960x540 before/final captures reported the same `64` resident and
  `11` drawn sections, and both files have SHA-256
  `052647136ff313c20bb91216f478772e715fcae48406c4a3262d84e1081151f2`.
  `/tmp/mclone-t166-s1-final.png` was inspected: textured spruce terrain,
  actors, foliage, and sky were clean. The inspected 1280x640 synthetic-stereo
  capture `/tmp/mclone-t166-s1-xr-emulation-final.png` retained distinct eye
  pixels, terrain, and both UI composites.
- Repeated release fresh/frozen startup-streaming finished the RD10 target in
  `6053.809ms` and quiesced in `6547.870ms`; RD15 finished in `11137.282ms` and
  quiesced in `11961.494ms`. Both recorded `0` over-budget and `0` over-2x
  frames. The deterministic frame-budget stress probe also recorded `0/240`
  over-budget frames and no accounting-conservation violation before and after.
- The final release Quest APK built and attached Quest 3 per-eye frame-overlap
  orbit gates passed. RD5 recorded dropped delta `2`, app-work p95 `12.205ms`,
  average headroom `+3.234ms`, `0.1%` over-period, and zero skipped frames. The
  first final RD7 sample had a runtime dropped-frame anomaly after a `48.530s`
  settle, so the Tactical 150 — Adaptive Frame-Budget Controller repeat rule
  was applied; the repeat recorded dropped delta `1`, app-work p95 `12.700ms`,
  average headroom `+2.609ms`, `0.5%` over-period, and zero skipped frames. Both
  accepted rows are inside the Tactical 150 gates. Evidence is under
  `/tmp/mclone-t166-s1-final-*`; no capture or perf artifact is committed.

### Slice 2: Shared budgeted admission lanes

Work: add the LOD decision families to the `mclone-frame-budget` panel and the
cross-family ordering policy at the existing admission points. Real-section
admission behavior is preserved (the controller already governs it); this is
wiring, not new policy. LOD families exist but are inert (no producer submits
to them yet).

Gate:

- decision-trace diff on a fixed probe (`pnpm native:frame-budget:perf`):
  existing families' decisions unchanged for the same telemetry; new families
  present, inert, with skip reasons visible;
- no new scheduler/queue type in the diff — only `BudgetDecisionFamily`
  variants, panel config, and ordering policy (see Priority And Budget
  tripwire);
- same real-section perf gate as Slice 1.

Landed evidence (2026-07-11):

- Frame-pipeline schema 9 adds `LodBuildAdmission` and `LodUpload` after the
  four real render families in the explicit
  `RENDER_BUDGET_DECISION_FAMILY_ORDER`. Both use the existing controller and
  panel. While no producer exists, they report `inactive-no-producer`, zero
  elapsed/unit/pending grants, zero queue depth, and unavailable queue age and
  cost. No scheduler, queue, worker pool, or producer was added.
- The production `native:frame-budget:perf` report now serializes the scene
  host's real final decision panel instead of an empty placeholder. Its fixed
  240-frame run reported the three scheduler families, four real render
  families, then the two inert LOD families; it recorded `0` over-budget,
  `0` over-2x, and `0` accounting-conservation violations. Deterministic
  controller tests compare the real-family reports with and without the inert
  rows over cold start, clean growth, frame-miss pressure, and target-period
  change; every real report is exactly equal.
- The fixed 960x540 before/final captures both reported `64` resident and `11`
  drawn sections and are byte-identical with SHA-256
  `052647136ff313c20bb91216f478772e715fcae48406c4a3262d84e1081151f2`.
  `/tmp/mclone-t166-s2-final.png` was inspected: textured spruce terrain,
  foliage, cow, chicken, and sky were clean. The inspected browser smoke
  captures rendered the production WebGPU canvas and HUD with `64` resident
  sections.
- The required RD4 far-LOD-on captures were inspected at both `playable` and
  `idle` readiness. `/tmp/mclone-t166-s2-far-lod-playable.png` showed the
  coarse surface shell surrounding the transitioning real-section coverage;
  `/tmp/mclone-t166-s2-far-lod-idle.png` showed clean real-section replacement,
  continuous background LOD, and no blank boundary seam. Existing coordinator
  replacement/counter tests remained green.
- The full native workspace, focused diagnostics/frame-budget/app-runtime
  suites, `92/92` shared-scene tests, scene-host purity gate, Wasm build, and
  browser worker/WebGPU smoke passed. Movement and timedemo smokes passed; the
  latter rendered 60 frames at `3.811ms` average and `6.891ms` maximum.
- Comparable frozen-fluid release startup lanes stayed inside the Slice 1
  spread: RD10 reached full view in `6057.983ms` and quiesced in `6598.687ms`;
  RD15 reached full view in `11186.835ms` and quiesced in `12163.226ms`. Both
  recorded `0` over-budget and `0` over-2x frames. A detached same-host run of
  the pre-slice commit also reproduced the longer non-frozen fluid tail
  (`49863.597ms` versus final `49654.794ms`), proving that tail was not caused
  by the inert families.
- The release Quest APK built and attached Quest 3 per-eye frame-overlap orbit
  gates passed. RD5 recorded dropped/skipped delta `0/0`, app-work p95
  `12.339ms`, average headroom `+3.101ms`, `0.1%` over-period, and zero
  conservation violations. The first RD7 row had a runtime dropped-frame
  counter anomaly and was rejected under Tactical 150 — Adaptive Frame-Budget
  Controller's repeat rule; the accepted repeat recorded dropped/skipped delta
  `0/0`, app-work p95 `12.702ms`, average headroom `+2.605ms`, `0.5%`
  over-period, and zero conservation violations. Device logs show both LOD
  families inert with zero grants. Evidence is under
  `/tmp/mclone-t166-s2-*`; no capture or perf artifact is committed.

### Slice 3: Far LOD becomes a producer (kills the stutter)

Work: move synthetic far LOD onto the substrate: per-tile resident LOD slots in
region arenas, generation+meshing on the render-compile worker pool admitted at
P4, per-tile upload through the `LodUpload` family, visibility via the
coordinator, desired-coverage recompute event-driven. Startup prewarm drives
the same path with a startup-mode budget. **Delete** `FarTerrainLodMesh`
monolithic merge and `FarTerrainLodRenderer` whole-buffer upload.

Gate:

- deletion is real: no code path remains that concatenates all patches or
  re-uploads the world (`rebuild_mesh`/`upload_mesh` shapes are gone, not
  bypassed);
- no LOD generation or meshing on the frame thread (assert via the worker-pool
  admission counters);
- new counters exist and are exercised: region draw count, per-frame LOD upload
  bytes, LOD build queue depth/age, per-family skip reasons;
- desktop movement with `--far-lod true`: per-chunk-step LOD upload bytes
  bounded by the changed ring (not world size); region draw count bounded by
  live regions;
- XR movement at view distance 1 + far LOD 12: no movement stutter
  attributable to LOD (capture and look; coverage counters + budget
  skip-reason counters; add a `--far-lod` variant of the Quest orbit/strafe
  metrics lane as part of this slice's validation deliverables) — and the gate
  measures **draw cost, not only upload**: GPU frame time and draw count with
  far LOD on vs off recorded on-device, so the draw-call tradeoff this design
  exists to avoid is verified, not assumed;
- multiview guardrail: LOD tiles render in both the per-eye path and the
  full-frame multiview path (captures of both), with no shared mutable per-eye
  uniforms across a submission;
- real-section perf lanes still within spread; far-LOD-off still
  byte-identical.

Landed evidence (2026-07-11):

- `LodTileKey` is the shared core identity. Synthetic tiles now compile below
  real sections on the existing render-compile worker threads, with one real
  slot reserved under shared capacity. The event-driven cache uses
  `ResidentTileCache` and `ResidentTileUploadCoordinator`; startup prewarm
  drives that same asynchronous path. `FarTerrainLodMesh`, `rebuild_mesh`, and
  the old whole-buffer `upload_mesh` path were deleted.
- `FarTerrainLodRenderer` now owns fixed tile slots in 16-by-16-chunk region
  arenas. It uploads only changed tile vertex ranges, repacks a dirty region's
  visible indices, and submits one draw per visible region. Per-eye rendering
  has distinct per-region uniforms; supported adapters lazily create the true
  two-layer multiview pipeline so desktop adapters do not validate an unusable
  multiview shader.
- The formerly inert `LodBuildAdmission` and `LodUpload` families now report
  queue depth, age, grants, and skip reasons whenever the producer is enabled.
  Real work retains priority. A bounded cold-start floor admits at most four
  builds and sixteen uploads before the first complete frame report; inactive
  traces remain zero. Debug/offscreen and Quest markers expose region draws and
  per-frame upload bytes.
- Focused post-proof validation passed `92/92` scene tests, the browser Wasm
  build, thin-adapter purity, formatting, the real-work-priority worker test,
  and the far-LOD/runtime/render-session/frame-budget suites. The full native
  workspace test run, desktop movement smoke, release timedemo, Quest release
  APK build, per-eye launch, and full-frame multiview validation also passed.
- Far-LOD-off remained byte-identical to Slice 2: the fixed 960x540 capture has
  SHA-256 `052647136ff313c20bb91216f478772e715fcae48406c4a3262d84e1081151f2`
  with `64` resident and `11` drawn sections. The inspected RD2 overview
  `/tmp/mclone-t166-s3-far-lod-overview.png` showed a continuous coarse shell
  around detailed terrain with four region draws and `226944` upload bytes on
  the final initial frame. No capture is committed.
- The inspected 1280x640 per-eye capture
  `/tmp/mclone-t166-s3-xr-per-eye.png` showed the LOD horizon in both eyes with
  `268608` differing pixels. Production Quest full-frame multiview swapchain
  readback passed after actual LOD became drawable: `127` submitted/runtime
  frames, zero skips, two layers at 1680x1760, one far-LOD region draw,
  `12512` far-LOD upload bytes, and `287862` differing pixels against a minimum
  of `2956`. Evidence is in
  `/tmp/mclone-t166-s3-quest-multiview-proof-logcat.txt`.
- Paired Quest 3 RD1 30-second frame-overlap orbits had no skipped frames,
  dropped-frame delta, or over-period frames. Far LOD off measured GPU
  `1.827ms`, app-work p95 `4.211ms`, and zero region draws/uploads; far LOD on
  measured GPU `2.536ms`, app-work p95 `6.780ms`, up to six region draws, and
  `616032` upload bytes during bounded ring changes. The clean comparison is
  retained under `/tmp/mclone-t166-s3-quest-rd1-far-lod-{off,on}.txt`.
- Tactical 171 — Convergence And Parity Closeout Milestone D carried the same
  producer into the production browser host and resident shared-memory render
  compiler. Local-worker, IndexedDB, and remote-WebSocket probes each reached
  64 resident/visible tiles, four region draws, `317152` uploaded bytes, and a
  `32930`-pixel off/on horizon difference with no compiler fallback or result
  overflow. The `FarLod` browser exception is removed; Slice 4 can now evolve
  quality on the proven cross-platform substrate.

### Slice 4: Multi-level rings

Work: add the `lod_level` dimension, the distance→level band policy with
hysteresis (H = 2 chunks to start), dirty-on-level-change (self + the four X/Z
neighbors for edge re-resolve). Multiple concentric rings (4/8/16). Old-level
tiles stay drawable until the new-level tile is drawable, then evict, with the
double-residency band counted and capped.

Gate:

- crossing band boundaries during movement shows no pop/blank and no new
  stutter (capture and look, plus `suppressed_without_replacement` delta 0);
- oscillating movement astride a band boundary does not thrash: per-tile level
  flips per minute bounded (hysteresis proven by counter, not by eyeball);
- double-residency counter exposed, band bounded under sustained movement;
- tile counts and per-level residency visible in diagnostics;
- cross-level seams: captures at band boundaries show no cracks (neighbor edge
  re-resolve working);
- all Slice 3 counters remain healthy at multi-level coverage.

Landed evidence (2026-07-11):

- The synthetic producer now maps each desired chunk to `LodTileKey { chunk,
  level }` using three equal distance bands over the configured Far LOD range.
  The default base spacing produces 4/8/16-block levels. A two-chunk hysteresis
  guard retains the prior level around each boundary; a focused oscillation
  test holds a watched tile at zero flips while raw distance crosses back and
  forth.
- Desired and currently drawable keys are tracked separately. A prior level
  remains visible until its replacement upload is resident, then visibility
  switches atomically and the old GPU tile is removed through the existing
  lifecycle queue. Transitions are capped at 256; current/max double residency,
  per-level resident/visible counts, total flips, and maximum flips per tile are
  exposed in shared debug and browser reports.
- A level change dirties the changed tile and its four X/Z neighbors. Compact
  native/browser worker requests carry west/east/north/south sample spacing, so
  tile-local boundary skirts sample the adjacent level's 4/8/16 grid. Stale
  seam-signature completions are discarded and requeued without adding a
  worker, budget, upload, or renderer path.
- The focused Far LOD suite passed `28/28`, including 4/8/16 band selection,
  hysteresis, cross-level worker inputs, all-three-level residency, and a
  replacement transition with no blank frame. Scene tests passed `92/92`;
  web-client tests and ABI locks, direct browser Wasm/typecheck, renderer and
  render-session tests, workspace check, formatting, thin-adapter purity,
  frame-budget, movement-frame, movement, timedemo, normal app, asset-pack, and
  movement browser lanes all remained green.
- Production local-worker, IndexedDB, and remote-WebSocket browser probes each
  filled all 912 desired/visible tiles, then moved three chunks and settled
  levels `172/304/436`. Each recorded 68 level flips, at most one flip per tile,
  maximum double residency 2, zero `suppressed_without_replacement`, 14 region
  draws, no shared-result fallback/overflow, and valid inspected static/moved
  captures. Final maximum frame gaps were `16.980ms`, `15.795ms`, and
  `17.680ms`; off/on captures changed `220824`, `220873`, and `220563` pixels.
  Evidence remains under `/tmp/mclone-native-web-far-lod-*` and
  `/tmp/mclone-t166-s4-final-web-*`.
- The inspected native 1280×720 overview showed detailed spruce/snow terrain
  surrounded by a continuous three-level shell with 12 region draws. The exact
  Far-LOD-off 960×540 canary stayed byte-identical at SHA-256
  `052647136ff313c20bb91216f478772e715fcae48406c4a3262d84e1081151f2`,
  with 64 resident and 11 drawn sections.
- The final release Quest 3 full-frame multiview proof submitted 128 runtime
  frames
  with zero skips, two 1680×1760 layers, one initial Far LOD region draw,
  `12512` upload bytes, and `305711` differing pixels. The 30-second RD1
  per-eye frame-overlap orbit moved `24.853` blocks with 2160 submitted frames,
  zero skipped or dropped-frame delta, zero over-period frames, app-work p95
  `6.697ms`, average headroom `8.254ms`, and GPU `2.665ms`. Evidence is under
  `/tmp/mclone-t166-s4-final-quest-*`; no capture or performance artifact is
  committed.

### Later: reduced-real LOD as a third producer

162 Slice 4+ (reduced-real tiles, edit dirtying, discardable persistence) drop in
as a third producer sourced from the same snapshots real sections consume — cheap
now because the substrate, budget, and precedence already exist. This tactical
does not implement them; it makes them a producer slot instead of a second
pipeline.

## Guardrails

- Chunk-granular tiles only; no multi-chunk / negative LOD (locked invariant;
  spacing floor is 16 — see distance-scaling outlook for the recorded trigger).
- Real sections stay authoritative; LOD never satisfies chunk interest,
  collision, raycast, edits, entities, or gameplay authority (carry from 162).
- Real-section behavior stays byte-for-byte through Slices 1–2 (perf-gated);
  far-LOD-off stays byte-identical through Slice 4 acceptance.
- One budget owner: LOD build/upload competes through the same
  `mclone-frame-budget` decision-family panel as real work; lanes are new
  decision families + ordering policy, **never** a new scheduler/queue object;
  no bespoke LOD budget constant (150 rule 10).
- One worker pool: after Slice 3, LOD generation/meshing never runs on the
  frame thread and never gets a private thread pool.
- Substrate owns admission/upload policy and the diff/dirty/eviction algorithm;
  producers own mesher, vertex format, shader, data source, and GPU buffer
  layout. Don't extract buffer allocation into the substrate.
- No whole-world concat or whole-buffer re-upload path may survive Slice 3 —
  including "temporary fallback" variants.
- Multiview-aware: both producers keep their per-eye + full-frame multiview
  paths (repo XR render-path guardrail); LOD region buffers must not share
  mutable per-eye uniforms across one submission.
- Replacement-before-suppress: old drawable tile (any level) stays until its
  replacement is drawable (128/162 invariant), with the double-residency band
  counted and capped.
- No silent caps: any bound the substrate applies (tile caps, band caps, budget
  denials) must surface as a counter/skip reason, not silent truncation.

## Resolved Questions (were open in the first draft)

- **Per-tile buffers vs batching:** region arenas — per-tile meshing and
  residency, region-level index packing and draws. Naive one-buffer-per-tile is
  rejected for draw-call cost; monolithic is rejected for upload cost.
- **LOD build workers:** one pool. LOD jobs go through the render-compile
  worker pool and its budget-derived capacity.
- **Crate ownership:** substrate mechanism in `mclone-render-session`;
  producers/coordinator/band policy in `mclone-app-runtime`; pipelines and
  buffer layouts in `mclone-render`.
- **Multi-draw-indirect:** not needed; packed per-region index buffers keep one
  portable path across native and WebGPU. Revisit only if region re-pack shows
  up in profiles, as a native-only optimization.
- **Genericity and Y granularity:** `ResidentTileCache<K, M>` is generic over
  the key/metadata while remaining agnostic to real-section versus LOD-column Y
  shape.
- **Region allocation:** 16×16 chunk regions with fixed per-tile vertex slots;
  visibility changes repack only the region-local index buffer.
- **Distance bands:** three equal bands over the configured extra radius use
  levels 1/2/3 and default 4/8/16 spacing. Hysteresis is two chunks on every
  platform; adaptive/per-platform band policy is not part of this tactical.

## Open Questions

- Whether measured far-level density justifies regions larger than the landed
  16×16 chunks or best-fit allocation instead of fixed slots.
- Packed vertex format details (u16 region-relative positions + rgba8 vs
  keeping f32 for the first slice).
- Reduced-real provenance/versioning: deferred to 162 Slice 4; only the producer
  slot is reserved here.

## Relationship To Other Tacticals

- [`162`](162-real-chunk-lod-reduction-draft.md): its Slice 3 (LOD budget/queue)
  is **re-scoped into this tactical** — far LOD becomes a producer on the shared
  substrate rather than getting a bespoke budget. 162 Slices 4–6 (reduced-real
  tiles, edit dirtying, persistence) build on this substrate as later producers.
- [`128`](128-terrain-render-pipeline-coordination.md): the real-section
  residency/admission/upload pipeline this generalizes. Slice 1 must preserve its
  behavior and vocabulary.
- [`150`](150-adaptive-frame-budget-controller.md): the budget owner. LOD
  admission/upload becomes controller decision families, not new constants; its
  baselines and re-pin rule are the perf gates for every slice.
- [`120`](120-vanilla-render-compile-backpressure.md): the compile/upload
  backpressure baseline the shared admission follows.
- [`121`](121-surface-lod-first-slice.md): the origin of the synthetic far-LOD
  producer.
