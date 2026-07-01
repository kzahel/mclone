# 119: Android XR Live Streaming Frame Pacing

Status: active checklist. Companion to
[`030`](030-native-streaming-publish-and-render-budget.md) for the shared/native
streaming architecture and [`117`](117-android-xr-rd10-gpu-floor-and-frame-overlap.md)
for RD10 GPU-floor and CPU/GPU overlap work.

## Purpose

Keep Quest standalone / Android XR live RD7 and RD10 movement from missing
frames in bursts when chunks, render sections, uploads, and draw-record
maintenance arrive at the same time.

The product/comfort target lane is now render distance 7 at scale 1.0. The
stress target remains render distance 10 at scale 1.0. RD7 should answer "is
this stable enough to use"; RD10 should answer "does this expose bursty work or
tail regressions clearly."

This file is the durable checklist for:

- what the Java 1.17.1 reference engine does for render compile pacing,
- where our native/XR pipeline matches that shape,
- where we are currently coarser or less frame-aware than the reference,
- what has already been tried on Quest,
- what remains worth trying next.

Current priority read as of the RD7 settled-orbit runs:

1. Use RD7 settled orbit as the primary product-style movement lane; use RD10 as
   the stress lane.
2. Prepared-record dirty churn from unchanged ready-set reassertions is fixed;
   remaining rebuilds track real section/update work.
3. Naive fixed section-accept budgeting is measurable but not a win yet; it
   spreads small section changes across many more prepared-record rebuild
   frames.
4. Coalesce or incrementally maintain prepared records for small accepted
   section changes, then re-test accept/upload budgets with frame overlap.
5. Defer greedy meshing, draw arenas, and other geometry-policy changes until
   counters show the remaining tail is dominated by geometry, draw submission,
   or upload bytes rather than ready-set/record maintenance.

## Pipeline Model

Think of the live path as five queues feeding the headset frame:

1. Server/world work completes chunk data.
2. Published snapshots mark render chunks/sections dirty.
3. A render compile worker builds CPU meshes.
4. The XR frame path accepts completed meshes and creates/updates GPU resources.
5. The terrain draw path maintains the ready-section/prepared-record structures
   used by cull, encode, and submit.

The steady-state frame can be cheap. The hitch problem is not steady drawing by
itself; it is multiple queues paying their "done" cost in the same headset
frame, while the renderer also has a real GPU floor and limited CPU/GPU overlap.

At 72 Hz the app has `13.889ms`. For Quest comparisons, use
`MCLONE_ANDROID_XR_PERF_HEADROOM` fields:

- `app_work_*`
- `headroom_*`
- `app_over_period_*`

Do not use legacy `frame_avg_ms` as the primary headroom signal. It mostly
tracks compositor pacing once the runtime is waiting near the refresh period.

## Java 1.17.1 Reference Shape

Relevant files:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`

The reference client does not do greedy meshing for world terrain. It traverses
blocks, asks the block/model renderer for baked quads, checks whether faces are
visible, and emits quads individually.

For pacing, the important shape is:

- `ChunkRenderDispatcher` owns a compile queue, worker buffer packs, an upload
  queue, and a mailbox.
- Worker count is bounded by processor count and memory.
- Compile tasks are scheduled only when a free buffer pack is available.
- `LevelRenderer.compileChunksUntil(...)` uses a frame-time deadline and recent
  average compile cost to decide how many chunk compile tasks to schedule.
- Uploads are still drained broadly through `uploadAllPendingUploads()`.
- Some nearby dirty chunks may rebuild synchronously on the render thread.

Interpretation for us: Java is more time-aware than a fixed "N chunks per
frame" scheduler, but it is not a perfect mobile XR pacing model. Its unbounded
upload drain and occasional synchronous rebuild are acceptable compromises for a
desktop 60 Hz Java client, not policies to copy blindly on Quest.

### Reference Divergence: Local Edit Coherence Versus XR Frame Budget

Java's synchronous near/player-dirty rebuild path is evidence for why local
edits can look visually immediate, but it is not a policy we should inherit for
Quest. On Android XR, the headset frame deadline is harder than immediate
terrain repair: a missed frame is worse than a few frames of stale terrain.

For local block edits, gameplay/client state should update immediately, while
render repair should be bounded and coherent. Prefer high-priority async section
compiles plus atomic/grouped publication: old visible section meshes may remain
until the replacement dirty group is ready, but we should avoid publishing a
partial group that exposes sky/background where an adjacent face should be.

Synchronous rebuilds may stay useful for desktop diagnostics or opportunistic
desktop work when measured headroom exists, but they should not be the default
shared or Quest policy. This matches
[`113`](113-block-edit-render-coherence.md): the production target is coherent
eventual render publication, not Java-style render-thread immediacy.

## Current Native/XR Shape

Things we already do reasonably well:

- Server completed-worldgen publication is sliced: one completed target chunk
  per scheduler poll.
- Light publication is also sliced.
- Shared app runtime uses a render compile chunk budget of one.
- `mclone-render-session` keeps one render compile job in flight and queues
  dirty chunks instead of rebuilding everything inline.
- Android XR has an optional render-section upload budget:
  `--xr-render-section-upload-budget`.
- Android XR has optional per-eye frame overlap:
  `--xr-frame-overlap`.
- The Quest perf harness records app-work/headroom fields, not just paced frame
  wall time.

Current gaps versus the reference or versus what mobile XR needs:

- There is no adaptive, headroom-aware controller. Budgets are constants or
  launch flags, not computed from live frame headroom, backlog, and comfort
  state.
- The main render compile budget is by dirty chunk column. One accepted chunk can
  produce many vertical render sections, so the actual CPU/GPU work admitted in
  a frame is still lumpy.
- A completed render compile result can contain many section meshes. Accepting
  the result is not yet section-count or byte-count budgeted.
- The XR upload budget limits GPU upload count, but it does not fully budget the
  CPU-side cache apply, ready-section updates, removals, or prepared-record
  maintenance that happen around upload.
- `TexturedSectionDrawResources::set_traversal_ready_sections(...)` currently
  dirties prepared records whenever the live path calls it. During live XR this
  can rebuild shared records even when the ready set did not semantically
  change.
- `runtime_sync_ms` is too coarse. We need sub-buckets that separate result
  receive/drain, finish/apply, cache insertion, ready-set diff, prepared-record
  rebuild, GPU buffer creation/write, removal apply, and snapshot staging.
- There is no explicit starvation or quality policy for queued uploads: nearest
  visible first, max stale age, max queued sections, or graceful lower-quality
  fill.
- We still allocate/update per-section GPU resources rather than a section arena
  or indirect/batched draw representation.
- Greedy meshing has not been implemented. It could reduce GPU bytes and
  indices, but it is a parity-sensitive geometry-policy change and may move cost
  from GPU to CPU.

## Current Evidence

### RD7 Stable-Lane Baseline

The first committed RD7 live baseline (`c87ae7a`) shows RD7 is better than RD10
but still not a clean 72 Hz product lane at scale `1.0` from the current
`0,120,-96,180` view:

| Lane | app work avg / p95 / p99 / max | FPS | over period | Notes |
|---|---:|---:|---:|---|
| Flight default | `8.360 / 18.850 / 29.565 / 64.889ms` | `69.61` | `9.8%` | `max_runtime_sync_ms=44.386`, `max_terrain_shared_records_ms=24.230` |
| Flight overlap | `6.685 / 14.122 / 28.115 / 48.444ms` | `69.85` | `5.3%` | runtime spike moved to `max_runtime_prefetch_sync_ms=42.745` |
| Stationary default | `16.319 / 18.289 / 25.364 / 35.139ms` | `60.94` | `99.1%` | steady view draws `172` sections / `1.369M` indices |
| Stationary overlap | `13.494 / 14.600 / 18.275 / 26.322ms` | `71.01` | `21.1%` | much better, but still borderline |

Interpretation: RD7 should stay as the comfort/product lane, but passing RD7 now
requires more than just lowering distance from RD10. Frame overlap helps
materially, while live flight still exposes the same runtime-sync /
prepared-record burst class and stationary RD7 still shows a near-budget steady
render cost.

The settled-orbit lane (`abe9d6d`) is the better product-style movement proxy
because it waits for the populated RD7 scene, then moves around the local chunk
cluster:

| Lane | app work avg / p95 / p99 / max | FPS | missed 72 Hz slots | over period | Notes |
|---|---:|---:|---:|---:|---|
| Settled orbit default | `15.674 / 20.856 / 30.028 / 60.227ms` | `62.88` | `~410 / 3240` (`12.7%`) | `71.3%` | `max_runtime_sync_ms=33.504`, `max_terrain_shared_records_ms=27.010` |
| Settled orbit overlap | `12.493 / 14.692 / 22.826 / 58.398ms` | `70.25` | `~79 / 3241` (`2.4%`) | `9.5%` | `max_runtime_prefetch_sync_ms=44.080`, `max_terrain_shared_records_ms=18.175` |

Interpretation: frame overlap is a real product-lane win, but RD7 at scale `1.0`
still is not locked. The remaining work should target prepared-record/runtime
burst spikes and likely add a quality/headroom lever before making overlap a
default.

Prepared-record dirty diff first pass:

| Lane | app work avg / p95 / p99 / max | FPS | missed 72 Hz slots | over period | Record-cache notes |
|---|---:|---:|---:|---:|---|
| Settled orbit default | `15.041 / 21.149 / 30.707 / 59.273ms` | `65.01` | `~315 / 3241` (`9.7%`) | `58.6%` | `ready_set_changed=15/2926`, `prepared_rebuilds=174`, `max_shared_records_ms=7.352` |
| Settled orbit overlap | `11.791 / 14.142 / 22.437 / 49.540ms` | `70.36` | `~74 / 3241` (`2.3%`) | `5.7%` | `ready_set_changed=15/3167`, `prepared_rebuilds=175`, one `26.887ms` record rebuild outlier |

Interpretation: the unchanged-ready-set rebuild class is mostly gone. The
remaining record rebuilds line up with real section/update work
(`174-175` rebuilds for `182-203` upload-work frames), so the next pacing slice
should budget completed-section acceptance/upload application and then consider
incremental prepared-record maintenance if tails remain.

Section accept budget 4 first pass:

| Lane | app work avg / p95 / p99 / max | FPS | missed 72 Hz slots | over period | Record-cache notes |
|---|---:|---:|---:|---:|---|
| Settled orbit default accept4 | `15.440 / 21.676 / 30.272 / 70.603ms` | `63.65` | `~376 / 3241` (`11.6%`) | `68.9%` | `prepared_rebuilds=797`, `queued_upload_sections=96`, `queued_upload_removed_sections=224` |
| Settled orbit overlap accept4 | `12.136 / 15.028 / 26.242 / 63.753ms` | `69.89` | `~95 / 3241` (`2.9%`) | `8.0%` | `prepared_rebuilds=773`, `queued_upload_sections=28`, `queued_upload_removed_sections=220` |

Interpretation: the opt-in budget successfully caps accepted uploads/removals
to `4` per frame, but it worsens RD7 settled orbit as a fixed policy. It turns
section changes into many small record-dirty frames, increasing prepared-record
rebuilds from `174-175` to `773-797`. Keep the flag as a diagnostic/probe; the
next implementation slice should make prepared-record maintenance cheaper or
coalesced before relying on section acceptance budgets.

### Frame Overlap Live RD10

From the live RD10 A/B recorded in
[`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
and [`117`](117-android-xr-rd10-gpu-floor-and-frame-overlap.md):

| Lane | app work avg / p95 / p99 / max | FPS | over period | Notes |
|---|---:|---:|---:|---|
| Stationary default | `19.251 / 23.594 / 26.611 / 42.276ms` | `51.72` | `100.0%` | `max_terrain_runtime_upload_ms=20.496`, `max_runtime_sync_ms=17.446` |
| Stationary overlap | `15.206 / 16.932 / 24.488 / 30.853ms` | `65.11` | `87.3%` | upload moved into prefetch path; still not 72 Hz |
| Flight default | `8.081 / 15.623 / 31.759 / 73.828ms` | `69.31` | `6.6%` | `max_terrain_runtime_upload_ms=48.837`, `max_runtime_sync_ms=48.176` |
| Flight overlap | `6.619 / 15.156 / 29.742 / 56.720ms` | `69.26` | `6.0%` | fewer drops, but MTP worsened and FPS stayed flat |

Interpretation: frame overlap is a useful opt-in, especially for frozen and
stationary RD10, but it does not solve live flight bursts by itself.

### Upload Budget Probe

Older RD10 flight probes from `106` tested upload budgets:

| Lane | avg / p50 / p95 / p99 / max | over 2x | MTP | queued upload sections |
|---|---:|---:|---:|---:|
| Unbounded | `14.722 / 13.933 / 28.904 / 46.438 / 74.961ms` | `71` | `32.128ms` | `0` |
| Budget 4 | `14.466 / 13.862 / 26.129 / 41.733 / 62.810ms` | `62` | `26.002ms` | `46` |
| Budget 8 | `14.472 / 13.956 / 24.065 / 40.213 / 105.262ms` | `54` | `25.601ms` | `16` |

Interpretation: upload budgeting improved some tail and motion-to-photon
signals, but it did not make the lane shippable. The likely reason is that the
budget controlled only one part of the burst; CPU-side section acceptance and
record maintenance could still spike.

### Prepared-Record Suspicion

Live flight recorded large `max_terrain_shared_records_ms` values, including
about `48ms` in the default frame-overlap A/B and about `14ms` in the overlap
run. This points at ready-section/prepared-record churn as a separate burst
source from mesh compile and GPU upload.

The specific code path to inspect first:

- `native/crates/mclone-xr-scene/src/lib.rs`: live XR calls
  `set_traversal_ready_sections(...)` during runtime polling/upload.
- `native/crates/mclone-render/src/chunk.rs`:
  `TexturedSectionDrawResources::set_traversal_ready_sections(...)` marks
  records dirty.
- `prepare_render_records(...)` rebuilds when records are dirty.
- `build_prepared_records(...)` walks visibility sections and rebuilds the
  prepared representation.

Hypothesis: if the ready set is unchanged or only slightly changed, we should
avoid marking all records dirty and should eventually maintain prepared records
incrementally.

## Checklist

### A. Instrument The Burst

Status: partially landed. `MCLONE_ANDROID_XR_PERF_RECORD_CACHE` now reports
ready-set diff calls and prepared-record rebuild count/avg/max. Runtime sync
sub-buckets for completed-result receive/apply/cache insertion/removal are still
open.

Add Android XR perf markers for:

- runtime poll total,
- completed compile result receive/drain,
- compile result finish/apply,
- CPU mesh/cache insertion,
- old-section removal apply,
- pending upload queue push/pop,
- GPU buffer creation/write bytes,
- ready-section calculation,
- ready-section diff/equality check,
- prepared-record rebuild,
- snapshot clone/staging.

Success condition: a live RD7 settled-orbit or RD10 stress run can explain each
`runtime_sync_ms` or `shared_records_ms` spike with a named sub-phase.

### B. Dirty Prepared Records Only On Real Ready-Set Change

Status: first pass landed.

Teach `set_traversal_ready_sections(...)` to compare or hash the incoming ready
set against the current one before setting `records_dirty`.

The landed pass also skips empty section-update apply calls so they do not dirty
records. RD7 settled-orbit counters show the ready set only changes about `15`
times in a 45-second run, while thousands of unchanged ready-set calls no longer
force rebuilds.

Expected win: remove full prepared-record rebuilds from frames where the live
path merely reasserts the same ready set.

Result: confirmed for the unchanged-ready-set case. The remaining rebuilds are
mostly tied to real section/upload work, so this item now feeds Slice C.

Risks:

- Shared renderer path, not Quest-only.
- Must preserve correctness when sections are added, removed, hidden, revealed,
  or traversal mode changes.

Validation:

- `cargo test --manifest-path native/Cargo.toml`
- desktop timedemo/smoke lane if render records changed materially,
- Quest settled-orbit RD7 metrics and frame-overlap as the primary product-lane
  comparison,
- Quest stationary RD7 metrics and frame-overlap,
- Quest flight RD7 metrics and frame-overlap,
- Quest stationary RD10 metrics and frame-overlap as the stress comparison,
- Quest flight RD10 metrics and frame-overlap as the stress comparison.

### C. Budget Completed Section Acceptance

Status: first opt-in XR draw-resource probe landed; fixed budget `4` measured
as worse for RD7 settled orbit.

Do not only budget GPU upload. Stage completed section meshes and admit them to
the live render cache by section count or byte count per frame. Keep old visible
section resources until replacements are accepted and uploaded.

For local block edits specifically, this should preserve stale-but-coherent old
meshes until the replacement section or dirty group is ready to publish. Do not
trade a short-lived stale face for a partial publication that reveals
sky/background through an occluded neighbor boundary.

Expected win: a single compile result cannot force all vertical section CPU
apply work into one headset frame.

Result so far: `--xr-render-section-accept-budget 4` limits XR draw-resource
acceptance to four rebuilt/removed sections per frame, but without incremental
prepared-record maintenance it spreads the same section churn across hundreds of
record rebuild frames and worsens average/p95/p99 in RD7 settled orbit.

Open design questions:

- Budget by section count, vertex/index bytes, or measured apply time?
- Prioritize nearest/visible sections first or preserve compile-result order?
- How do we expose stale-but-visible sections in diagnostics?
- How do we coalesce or incrementally update prepared records so accepting a few
  section changes does not trigger a full prepared-record rebuild each time?

### D. Re-run Upload Budgets With Current Metrics

Status: still open after the accept4 result.

Re-test `--xr-render-section-upload-budget` with the current headroom markers and
frame-overlap path.

Suggested lanes:

- `native:android-xr:perf:orbit:rd7:metrics`,
- `native:android-xr:perf:orbit:rd7:frame-overlap`,
- `native:android-xr:perf:flight:rd7:metrics`,
- `native:android-xr:perf:flight:rd7:frame-overlap`,
- `native:android-xr:perf:stationary:rd7:metrics`,
- `native:android-xr:perf:stationary:rd7:frame-overlap`,
- `native:android-xr:perf:flight:rd10:metrics`,
- `native:android-xr:perf:flight:rd10:frame-overlap`,
- `--xr-render-section-upload-budget 1`,
- `--xr-render-section-upload-budget 2`,
- `--xr-render-section-upload-budget 4`,
- `--xr-render-section-upload-budget 8`,
- repeat promising upload budgets with `--xr-frame-overlap`.

Success condition: p95/p99/max app work and MTP improve without visible
starvation, and queued upload sections do not grow without bound.

### E. Add A Headroom-Aware Live Controller

Status: open.

Replace fixed budgets with a conservative adaptive controller:

- target the current refresh period (`13.889ms` at 72 Hz),
- track recent app-work/headroom moving averages and tail frames,
- lower section accept/upload budgets when headroom is low,
- raise budgets slowly when headroom and queue age allow it,
- preserve a minimum budget for nearby visible sections,
- expose the chosen budgets and backlog in perf output.

Keep this opt-in until comfort and visual fill behavior are validated.

### F. Split Render Compile Results More Finely

Status: open.

Current chunk-level compile admission can produce many vertical render sections.
Investigate section-level compile targets or section-level result publication,
especially for tall columns where only a few sections are near or visible.

This is closer to a structural pipeline change than a quick XR tweak. It should
remain shared-first and not fork Android XR meshing.

### G. Co-design Uploads And Record Maintenance

Status: next priority after accept4.

If B and C show record rebuild is still a major tail source, move toward:

- incremental prepared-record maintenance,
- a section GPU arena instead of per-section allocation churn,
- draw batching or indirect records that can update a small changed range.

This overlaps with Slice J in
[`117`](117-android-xr-rd10-gpu-floor-and-frame-overlap.md).

### H. Geometry Reduction / Greedy Meshing

Status: open, not first.

Java 1.17.1 does not greedy-mesh world terrain. Native currently follows the
same broad model: visible block faces become quads, with neighbor culling,
lighting, and AO.

Greedy meshing could still be a valid native performance option if counters show
we are vertex/index/draw or upload-byte limited in live RD7 or RD10. Treat it as
an opt-in experiment first:

- start with counters for mergeable solid faces by texture, light, tint, and AO,
- restrict an initial prototype to simple solid opaque faces,
- preserve cutout, translucent, animated, tinted, and non-cube model behavior,
- compare CPU compile cost against GPU/upload savings.

Do not let this become the first response to a pacing problem if the immediate
spike is actually prepared-record rebuild or CPU apply work.

## Attempt Log

### Completed / Known

- `030` landed shared/native publication slicing and queued render-section build
  boundaries for desktop-style streaming hitches.
- `106` tested Quest RD10 upload budgets. They improved some tail metrics and
  MTP but left visible pacing problems and queued uploads.
- `117` landed `--xr-frame-overlap`. It is a useful opt-in and a frozen RD10
  win, but live RD10 flight remains below the target.
- RD7 settled-orbit landed as the preferred product-style movement lane. It
  keeps a populated scene visible while moving around nearby chunks; frame
  overlap cuts missed 72 Hz slots from about `12.7%` to `2.4%`, but p95 remains
  slightly over budget.
- Prepared-record dirty diff landed: unchanged ready-set reassertions no longer
  dirty records, empty section-update applies are no-ops, and the RD7
  settled-orbit overlap lane improved to `11.791ms` avg / `14.142ms` p95 with
  `5.7%` over-period frames. It still is not locked.
- `--xr-render-section-accept-budget` landed as an opt-in Android XR probe.
  Budget `4` caps accepted uploads/removals but worsens RD7 settled orbit by
  increasing prepared-record rebuild frames to `773-797`; do not make fixed low
  accept budgets default without record-maintenance work.
- Solid render-layer split landed for vanilla-shaped render-layer correctness,
  but it was not a measured RD10 performance win.
- Full-frame multiview was correctness-valid but performance-flat or tail-worse
  for the current production-style path, so production remains masked per-eye.

### Not Yet Tried

- Runtime sync sub-bucket instrumentation.
- Section or byte budget for completed-result acceptance in the shared render
  session, with prepared-record coalescing/incremental updates.
- Current-metric upload-budget A/B combined with frame overlap.
- Headroom-aware adaptive accept/upload controller.
- Section-level render compile result splitting.
- Incremental prepared records and/or section GPU arena.
- Solid-only greedy meshing prototype after counters justify it.

## Next Recommended Slice

Do G next: make prepared-record maintenance cheaper for small section changes,
then re-run accept/upload budgets.

1. Inspect `TexturedSectionDrawResources` record invalidation and prepared
   record build ownership for section add/remove/update.
2. Prototype coalescing or incremental maintenance so a few accepted sections do
   not force a full prepared-record rebuild every frame.
3. Keep the existing accept-budget flag as the probe harness, but do not treat
   budget `4` as a product policy.
4. Re-test RD7 settled orbit default and frame-overlap, first unbounded and then
   accept/upload budget variants.
5. Only then move to adaptive budgets or section-level render compile result
   splitting if the record-maintenance tail is under control.

This is now better targeted than greedy meshing: the latest counter evidence
says low fixed acceptance budgets expose prepared-record rebuild amplification,
not a raw geometry-count limit.
