# 162: Real-Chunk LOD Reduction Draft

Supersession note 2026-07-25: do not resume this design. Tactical
[`245`](245-retire-chunk-far-lod-runtime.md) removes the chunk-based LOD
runtime and retains this file only as historical evidence.

Status: deferred at Slice 4 by user direction on 2026-07-11. There is no
current commitment to resume reduced-real, persisted-source, reducer, extents,
or protocol work after Tactical 172; basic pure-synthetic far-LOD correctness
and stability take priority. This tactical remains the design record if that
decision is revisited. Slice 0A retained synthetic chunk patches, Slice 1
startup LOD prewarm, and Slice 2 coverage coordinator + source precedence
landed. The proposed bespoke Slice 3 LOD budget/queue does not proceed:
Tactical 166 — Shared Resident-Tile Substrate supersedes it with one shared
real-section/LOD residency, worker, budget, and upload mechanism, and 166 is
complete through Slice 4 multi-level rings. A second producer must not inherit
or mask synthetic coverage defects, but green Tactical 172 gates do not
automatically resume this work. Macro ordering lives in Tactical 171 —
Convergence And Parity Closeout.

Workstream: native Rust shared runtime/persistence/render boundary. Desktop
validation remains the likely first lane, but the target shape must stay shared
across flat, XR, Android, and web.

## Purpose

Record the desired initial shape for LOD data derived from persisted or loaded
real chunks.

This is distinct from the current surface LOD shortcut. The existing far terrain
path is a cheap synthetic surface shell that makes small render distances feel
larger. Real-chunk LOD should preserve reduced chunk shape from actual world
facts instead of forcing those facts through a heightfield.

## Position

Use two LOD sources:

```text
Synthetic surface LOD
  cheap seed/worldgen surface fallback
  heightfield-like
  no features, caves, structures, edits, or real lighting requirement

Reduced real-chunk LOD
  derived from loaded or persisted chunk records/snapshots
  limited Distant Horizons-style column/span data
  cache-only, non-authoritative
```

Render/source precedence:

```text
normal loaded chunk
  > reduced real-chunk LOD
  > synthetic surface LOD
  > nothing
```

This precedence is a draw/source rule, not a work-priority rule. It says which
representation wins when multiple representations are available. Work priority
still depends on startup state, chunk authority needs, frame health, and queue
age.

## Startup Coverage Policy

LOD should be allowed to improve the first view before the player is fully in
control, as long as it does not delay the authoritative spawn window without a
strict cap.

Split startup into two gates:

```text
Spawn authority gate
  underfoot collision facts
  small nearby real chunk window
  enough host/client state for movement and interaction to be honest

Visual coverage gate
  retained LOD coverage outside the normal chunk radius
  target radius: normal render distance + startup extra chunks
  synthetic surface or cached reduced-real tiles only at first
  hard time/tile budget; timeout degrades to current behavior
```

The playable transition should require the spawn authority gate. The visual
coverage gate may improve first impression, but it must be bounded:

```text
playable when:
  spawn authority complete
  and (visual coverage complete or visual coverage hit its time cap)
```

The first experiment should target a coarse shell, not final quality:

- start with synthetic surface LOD or already-cached reduced-real tiles
- use coarse startup spacing such as `8` or `16` blocks if needed
- refine to normal far-LOD spacing after playable
- default target can be `render_distance + 5` chunks for the first desktop
  measurement, then tune per platform
- keep this opt-in or diagnostics-gated until measured on Quest/Web

Do not spawn the player on LOD authority. LOD may cover blank space visually
while authoritative chunks arrive, but collision, raycast, block edits, entity
visibility, and chunk interest still require real host/client chunk facts.

## Scheduling And Budget Policy

Use ordered lanes instead of a single "LOD versus chunks" switch:

```text
P0 input, pose, session commands, local player state
P1 underfoot and nearby real chunk load/generate/light/publication
P2 normal visible chunk mesh compile/upload/drawable publication
P3 reduced-real LOD reduction from already available real chunk facts
P4 synthetic far LOD tile generation and mesh build
P5 discardable LOD cache writes
```

During startup, a bounded LOD prewarm burst is allowed after or alongside the
spawn authority gate, because no live frame comfort contract exists yet. During
live movement, optional LOD work should back off aggressively when normal chunk
or frame health is poor.

Initial adaptive rules:

- if recent frames are over budget or dropped, stop optional LOD generation and
  LOD uploads; keep already uploaded LOD visible unless LOD draw cost is the
  measured problem
- if the normal near-chunk pipeline has backlog, prefer real chunk
  load/generate/light and normal mesh/upload work
- if normal chunk work is caught up and measured headroom is stable, spend a
  small bounded budget on LOD tiles and raise that budget slowly
- if LOD queue age grows while frame health is acceptable, allow a small trickle
  so coverage eventually fills instead of starving forever
- all LOD jobs must be cancellable or cheaply discardable when coverage targets
  move

Frame slack is not free CPU. Render-thread admission, GPU upload, draw-resource
mutation, and post-submit overlap windows should use frame/headroom signals.
Local integrated worldgen, light, mesh compile, and LOD reduction workers are
peer CPU pressure and must still respect platform and thermal headroom.

## Coordinator Shape

Before real-chunk reduction or LOD persistence, add a CPU-side coverage
coordinator that tracks source, quality, and drawable state separately:

```text
NormalChunkCoverage
  chunk position
  authoritative snapshot revision
  light quality
  normal mesh compile/upload/drawable state

LodTileCoverage
  tile identity
  lod level / sample spacing
  source kind: synthetic surface | reduced real chunk | persisted reduced real
  quality: unlit | lit | stale | dirty | cached
  source chunk revisions / source record versions
  reduction/mesh/upload/drawable state
```

The coordinator decides desired coverage and replacement behavior:

- real chunk mesh becoming drawable suppresses overlapping LOD
- chunk unload reveals reduced-real LOD if available, otherwise synthetic LOD
- snapshot publication or chunk save may dirty impacted LOD tiles
- local block edits dirty impacted LOD tiles but must not block the edit
- old drawable LOD should remain visible until a better replacement is drawable
- source precedence applies per tile/cell, not by rebuilding one whole centered
  world mesh

This coordinator should align with the terrain render pipeline work in
[`128`](128-terrain-render-pipeline-coordination.md): resident lifecycle state,
admission, upload, and drawable publication should have named counters and
budgets. It does not need to wait for every 128 follow-up, but it should avoid
creating another centered full-set rebuild path.

## Desired Initial Shape

The first real-chunk LOD record should be semantic data, not GPU buffers.

Use a sparse column/span model:

```text
ReducedChunkLodTile
  tile identity
  lod level / sample spacing
  source provenance and versions
  columns

ReducedColumn
  x/z cell
  visible or representative vertical spans

ReducedSpan
  bottom_y
  top_y
  material/block class
  biome/tint identity
  optional sky/block light
  flags
```

This is intentionally DH-like. The initial scope should be much smaller than
Distant Horizons:

- no gameplay authority
- no collision, raycast, AI, spawning, or block query use
- no requirement to preserve every underground span
- no server/client LOD protocol design in this draft
- no Voxy-style 3D hierarchy or GPU-driven traversal

## Persistence

Persist reduced LOD only as a discardable cache family.

It must be safe to delete, skip on shutdown, invalidate on schema/profile/light
version changes, and rebuild from real chunks later. It should not share durable
chunk-save obligations.

Chunk save/publication may mark LOD regions dirty, but should not perform
reduction inline. Reduction and cache writes belong on an opportunistic
low-priority lane below normal chunk load/generate/light/publish, durable saves,
and render-section compilation.

Persisted LOD derived from edited chunks must not outrun the authoritative chunk
save protocol. Either write the reduced LOD after the durable chunk save is
acknowledged, or store source revisions/versions that can be validated on read
and discard the LOD tile if the authoritative source cannot prove it still
matches.

## Lighting

Synthetic surface LOD may use simple sky/daytime rendering only.

Reduced real-chunk LOD should carry sampled light when the source chunk is lit.
If the source is features-only or otherwise unlit, the reduced tile should be
marked lower quality and remain replaceable by a lit-derived tile.

## Guardrails

- Real chunks remain the only authoritative world representation.
- Reduced LOD never satisfies chunk interest.
- Synthetic LOD never overwrites or blocks reduced real-chunk LOD.
- Renderer buffers are derived products and should not be the persistence format.
- Keep source provenance explicit enough to prefer real lit data over cheap
  synthetic data.
- Startup LOD may improve presentation, but it must not satisfy the spawn
  authority gate.
- A camera-center move must not reset the whole LOD world; world-aligned retained
  tiles should be diffed, kept, added, and evicted.
- LOD budget decisions must expose skip reasons and queue age so starvation and
  frame-pressure tradeoffs are visible.

## Proposed Sequence

### Slice 0: Retained Synthetic LOD Tiles

Replace the current centered far-LOD mesh cache with world-aligned retained
tiles or chunk patches.

Deliverables:

- tile identity independent of camera center
- desired coverage diff: keep/add/evict instead of clear-and-rebuild
- retained synthetic surface data and derived mesh products
- movement across one chunk no longer clears the entire LOD world
- debug counters for retained tiles, pending tiles, generated tiles, evicted
  tiles, and skipped tiles

This slice fixes the current reset behavior and creates the tile vocabulary that
real reduced chunks can reuse later. It should not add persistence or real
column/span reduction.

Slice 0A landed with chunk-sized synthetic patches:

- `FarTerrainLodCache` now retains derived chunk-patch meshes keyed by
  `ChunkPos` instead of clearing all geometry when the camera-center key changes
- desired coverage is recomputed each frame, but retained overlapping patches
  are reused and only missing desired chunks are enqueued
- the merged draw mesh is rebuilt from retained patches that are still desired
- memory is bounded by retention radius and a hard patch cap:
  `DEFAULT_FAR_TERRAIN_LOD_EVICTION_MARGIN_CHUNKS = 2` and
  `MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES = 4096`
- patches outside the padded retention radius are evicted; if the hard cap is
  hit, non-desired and farther patches are evicted first
- focused tests cover incremental fill, one-chunk movement reuse, cap behavior,
  and far-move eviction

This first implementation intentionally keeps patch data as derived mesh
products. Slice 2/4 should introduce the semantic coverage/reduced-real tile
state before persistence or protocol work.

### Slice 1: Startup LOD Prewarm

Add an explicit startup visual-coverage prewarm path.

Deliverables:

- startup config: enabled, extra chunks, time cap, tile cap, sample spacing
- default experiment target: normal render distance plus `5` chunks
- separate spawn-authority and visual-coverage readiness facts
- first-playable policy that degrades when prewarm times out
- diagnostics: `first_playable_ms`, `lod_prewarm_ms`,
  `startup_lod_tiles_ready`, `startup_lod_tiles_target`,
  `startup_lod_timeout`, and first-frame LOD upload cost
- desktop screenshot before and after normal chunks replace LOD

This is the product experiment: prove whether cheap coarse coverage avoids empty
space without materially delaying entry to play.

Slice 1 landed with a shared startup prewarm path:

- `StartupLodPrewarmConfig` (shared `far_lod` module) carries enabled, extra
  chunks (default `render distance + 5`), time cap (default `2000ms`), tile cap
  (default `1024`), and the live sample spacing so prewarmed patches share the
  live `FarTerrainLodSourceKey` and are reused, not reset, by live rendering
- `FarTerrainLodCache::prewarm`/`coverage` advance the retained chunk patches
  with an explicit build budget and report `ready`/`target` tiles; this shares
  the exact retained-patch path as `mesh_for_camera`
- `LocalIntegratedStartupPump` runs the prewarm alongside spawn-authority
  loading and tracks readiness separately: `spawn_authority_ready` never depends
  on prewarm, and `playable_ready = spawn_authority_ready && (prewarm complete
  or timed out or disabled)`
- diagnostics on the startup step and the desktop completion log:
  `first_playable_ms`, `lod_prewarm_ms`, `startup_lod_tiles_ready`,
  `startup_lod_tiles_target`, `startup_lod_timeout`
- desktop opt-out via `--startup-lod-prewarm true|false`; auto-enabled with
  `--far-lod true`. Measured desktop (RD4, seed 12345): 280/280 tiles prewarmed
  in ~307ms, no timeout, before the playable transition

Not yet done here (deferred to later slices): a shared coverage coordinator
(Slice 2), LOD budget/queue policy (Slice 3), reduced real-chunk tiles
(Slice 4+), and any persistence.

### Slice 2: Coverage Coordinator And Source Precedence

Move LOD visibility decisions behind a shared CPU-side coordinator.

Deliverables:

- normal drawable coverage map separate from loaded chunk map
- LOD tile source/quality/drawable state
- precedence application per tile/cell: normal drawable, reduced real,
  synthetic, nothing
- replacement counters for normal-over-LOD and reduced-real-over-synthetic
- pop/replacement diagnostics for movement and startup
- explicit owner boundary between coordinator, render-session mesh handoff, and
  renderer draw buffers

This slice is where 162 becomes a coordination plan instead of only a data-model
draft.

Slice 2 landed with a shared CPU-side coverage coordinator:

- new `mclone-app-runtime` module `lod_coverage` owns `LodCoverageCoordinator`.
  It lives beside `far_lod` (not in `mclone-render-session`) because it consumes
  both the drawable normal-chunk set derived from render-session traversal
  readiness and the synthetic `FarTerrainLodCache`, both of which app-runtime
  already owns and shares across flat/XR/Android/web via `NativeSceneRuntime`.
  This keeps the desktop app pure glue.
- `NormalChunkCoverage` tracks per-chunk drawable state (`Loaded` vs `Drawable`)
  kept separate from the client loaded-chunk map. Only a `Drawable` normal chunk
  suppresses overlapping LOD; a loaded-but-not-drawable chunk keeps old LOD
  visible until the real mesh is actually drawable (no blank flicker on
  replacement), matching 128's "old output stays until replacement complete."
- `LodTileCoverage` models tile identity, source kind
  (`synthetic surface | reduced real chunk | persisted reduced real`), quality
  (`unlit | lit | stale | dirty | cached`), and a resident lifecycle
  (`Pending → Reducing → Meshing → Uploading → Drawable`) aligned with 128's
  admission/upload/drawable vocabulary. Slice 2 only exercises the synthetic
  source; the reduced-real slots are modeled so Slice 4 drops straight in.
- precedence is applied per tile/cell (`normal drawable > reduced real >
  synthetic > nothing`); there is no centered full-set rebuild. The coordinator
  is a pure decision component, driven each frame from `prepare_far_lod_mesh`
  after the synthetic mesh is built (via new `FarTerrainLodCache::current_mesh`
  and `drawable_lod_tiles` accessors, so the mesh borrow is released first).
- counters: `normal_over_lod`, `reduced_real_over_synthetic`,
  `lod_revealed_on_unload`, `lod_first_drawn`, `lod_evicted`, and
  `suppressed_without_replacement` (a genuine blank/pop distinguished from benign
  eviction by whether the cell is still loaded/drawable). Exposed via
  `lod_coverage_counters()` on the local, remote-dedicated, and enum runtimes.
- focused unit tests cover per-tile precedence (drawable-normal suppresses LOD,
  loaded-not-drawable keeps old LOD, unload re-reveals LOD, reduced-real replaces
  synthetic) and every counter. Desktop headless far-LOD captures (RD4, seed
  12345, `--far-lod true`) show coarse LOD around sparse real chunks at
  `playable` and detailed real chunks cleanly replacing overlapping LOD at
  `idle` with no blank seam.

Not yet done here (deferred): LOD budget/queue policy (Slice 3), reduced
real-chunk tiles and reducer (Slice 4+), edit dirtying (Slice 5), and any
persistence (Slice 6). Slice 2 keeps synthetic surface as the only real source;
LOD still never satisfies chunk interest, collision, raycast, edits, entities, or
gameplay authority.

### Slice 3: LOD Budget And Queue Policy

Status: superseded by Tactical 166 — Shared Resident-Tile Substrate Slices 1–3.
Do not implement this bespoke queue. The shared substrate owns the equivalent
resident lifecycle, worker admission, budget families, and upload admission.

> **Re-scoped 2026-07-09 into [`166`](166-shared-resident-tile-substrate.md).**
> Slice 3 as drafted below would give far LOD a *bespoke* budget/queue, which
> deepens the divergence from the real-section pipeline. Instead, far LOD becomes
> a producer on a shared residency/priority/budget substrate (166): the P1–P5
> lanes land as new `mclone-frame-budget` decision families plus cross-family
> ordering (not a new scheduler/queue), LOD gen/mesh moves onto the shared
> render-compile worker pool, and GPU residency uses per-tile slots in region
> arenas with region-level packed-index draws — reusing the real-section
> admission/upload policy rather than paralleling it. The deliverables below are
> absorbed as 166 Slice 3's acceptance criteria. Slices 4–6 (reduced-real tiles,
> edit dirtying, persistence) remain here but land as later producers on the 166
> substrate.

Integrate LOD work with the existing frame/pipeline accounting model.

Deliverables:

- budget classes for LOD reduction, synthetic generation, mesh build, upload,
  and cache write
- skip reasons: disabled, no target coverage, normal backlog, low headroom,
  dropped-frame backoff, stale target, cache-only miss, time cap
- queue-age and oldest-target diagnostics
- conservative platform defaults: desktop can be more permissive; Quest/Web
  back off earlier
- validation that LOD work cannot increase current-frame upload/ready-publish
  bursts without a budget entry

This should reuse the budget vocabulary from
[`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md) and the
terrain pipeline coordination direction in
[`128`](128-terrain-render-pipeline-coordination.md).

### Slice 4: In-Memory Reduced Real-Chunk LOD

Add the first semantic reduced-real tile format and reducer, in memory only.

Deliverables:

- `ReducedChunkLodTile`, `ReducedColumn`, and `ReducedSpan` data types
- reducer from already loaded/published chunk snapshots or records
- source provenance: chunk revisions, light quality/version, reducer version,
  material/profile version
- lit tiles replace unlit/lower-quality tiles
- mesh builder consumes semantic reduced tiles, not chunk snapshots directly
- no persistence and no protocol changes

This proves the real-chunk source precedence and quality replacement model while
keeping failure cheap.

### Slice 5: Edit Dirtying And Neighbor Impact

Wire authoritative mutations to LOD dirtiness without blocking gameplay.

Deliverables:

- block edits dirty impacted reduced-real tiles
- dirty regions coalesce
- rebuild is low-priority and cancellable
- normal chunks continue to draw over stale LOD
- tests for edits near tile boundaries and chunk boundaries

This should land before any persistent LOD cache is trusted across sessions.

### Slice 6: Discardable LOD Persistence

Persist reduced-real LOD only after in-memory semantics, dirtying, and budget
behavior are stable.

Deliverables:

- separate cache record family with schema/profile/light/reducer versions
- cache reads never satisfy chunk interest or gameplay queries
- cache writes are lower priority than durable chunk/entity/player/saved-data
  writes
- edited-source safety: write after durable save ack or validate source
  revisions on read
- safe delete/reset behavior
- remote dedicated-server behavior remains disabled or explicitly negotiated

This is intentionally late. It should reuse persistence actor concepts from
[`134`](134-shared-persistence-architecture.md) rather than adding a renderer
sidecar database.

## Sequencing Notes

- Slice 0 can start immediately and is the best first step because it fixes the
  visible reset problem without waiting for real LOD, persistence, or broad
  render-pipeline refactors.
- Slice 1 should follow quickly if Slice 0 validates cheap retained coverage;
  its success/failure decides whether startup LOD is a product feature or only a
  debug experiment.
- Slice 2 and Slice 3 are the dependency bridge to the broader coordinator work.
  They should track 128's resident lifecycle/admission vocabulary, but they do
  not need to wait for every terrain coordinator cleanup.
- Real reduced tiles should wait until retained tiles, startup coverage, source
  precedence, and budget diagnostics exist. Otherwise real LOD will inherit the
  same centered full-rebuild and invisible-priority problems as the current
  synthetic path.
- Persistence should wait until edit dirtying and source validation exist.
  Persisting early would make stale visual lies and cache invalidation harder to
  debug.

## Open Questions

- Tile dimensions and alignment.
- How aggressively to cap/merge spans.
- Whether reduced tiles are host-only, client-only, or both.
- How edited chunks dirty neighboring LOD records.
- First material/light fidelity target.
- Remote dedicated-server behavior.
- Startup prewarm default: enabled for desktop diagnostics only, or exposed as a
  product option after first measurement?
- First startup spacing: `4`, `8`, or `16` blocks?
- Should startup visual coverage require full target completion on desktop when
  it is fast, or always use the same timeout-shaped policy across platforms?
