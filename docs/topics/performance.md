# Performance Topic

Status: active living index.

This document is the current map for native performance work. Tactical docs
remain the implementation records; this topic doc records priority, Java
alignment, current measured state, and where to look next.
The holistic accounting model for frame pacing, terrain streaming, local
integrated server work, remote-host contrast, workers, GPU upload, and debug
counter requirements lives in
[`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md).

Update this file whenever a performance slice changes the priority order,
invalidates an older recommendation, or establishes a new baseline.

## Current Baseline

Current validated slice: native block render-facts parity after non-cubic model
AO render parity. The movement-frame probe remains under budget, cold startup
lighting is no longer dominated by the sky graph drain, the dark foliage-top
render bug is fixed, the textured chunk shader uses Java's default lightmap
curve, and model faces now get Java-shaped mesh-side AO including non-cubic
shape weighting plus current-block `BlockStateBase.Cache` render facts.

Latest 120 Hz release movement-frame probe was captured during that slice at:

```text
/tmp/mclone-render-facts-movement-frame-release.json
```

Summary:

| Metric | Value |
|---|---:|
| probe mode | `movement_walk` |
| target | `120 Hz` / `8.333 ms` |
| frames | `240` |
| movement speed | `32 blocks/sec` |
| over-budget frames | `0 / 240` |
| p95 frame | `4.128 ms` |
| p99 frame | `5.889 ms` |
| max frame | `8.250 ms` |
| initial face count | `121,222` |
| initial index count | `727,332` |
| average headless frame | `2.887 ms` |

Release timedemo from the same slice:

| Metric | Value |
|---|---:|
| frames | `240` |
| scene build | `1,491.873 ms` |
| section count | `1,296` |
| face count | `405,407` |
| index count | `2,432,442` |
| average frame | `2.739 ms` |
| max frame | `13.450 ms` |
| average drawn sections | `90.167` |
| max drawn sections | `125` |

Interpretation: Java `canOcclude=false` for leaves/glass-style blocks raises
mesh face pressure because leaves no longer cull adjacent hidden solid faces.
The release movement-frame lane remains within 120 Hz budget, and timedemo
stays in the existing renderer envelope. Treat the higher face count as the new
baseline for Java-style leaf non-occlusion.

For durable historical trends, use [`../performance-records.md`](../performance-records.md).

Tactical 168 Slice 7d changed probe ownership, not the JSON contracts:
timedemo, startup-streaming, frame-budget, and movement-frame now drive the
same shared Mono scene host as desktop through `OffscreenDriver`. Timedemo and
both frame-probe pre/post key-set comparisons are exact, and
frame-budget/movement retain their 1,936-section initial target and accounting
meanings. Timedemo no longer builds the retired
static tracking-halo batch, so its resident/drawable counts and culling sample
set moved to the host's actual target. Do not compare its old numeric baseline
as a regression threshold; capture a fresh release-mode baseline before the
next performance judgment. The detailed migration evidence and debug sample
are recorded in [`168`](../tactical/168-unified-native-scene-host.md#slice-7-desktop-flat-onto-the-host-delete-flatclientdriver-orchestration).

Slice 7e moved XR camera-commit attribution into the shared optional
`EngineCameraCommitTiming` output. The existing XR locomotion/report fields and
their meanings are unchanged; this is instrumentation ownership convergence,
not a new performance baseline.

Tactical 166 — Shared Resident-Tile Substrate is complete through Slice 4.
Synthetic Far LOD now uses shared workers/admission/uploads plus 4/8/16 rings
with two-chunk hysteresis and bounded replacement residency. Its attached Quest
3 RD1 30-second moving orbit recorded zero skipped frames, zero dropped-frame
delta, zero over-period frames, app-work p95 `6.697ms`, average headroom
`8.254ms`, and GPU `2.665ms`; full-frame multiview also passed. Production
local-worker, IndexedDB, and remote browser movement probes recorded zero
replacement blanks. The next LOD performance judgment belongs to Tactical 162
— Real-Chunk LOD Reduction Draft Slice 4, the first reduced-real producer on
this substrate; do not reopen a parallel budget or worker path.

Standalone Quest Android XR is not covered by the desktop/headless
movement-frame, timedemo, or desktop-hosted OpenXR baselines. Track headset
frame pacing, real OpenXR refresh state, overlay diagnostics, and standalone
Quest baseline work in
[`096`](../tactical/096-android-xr-quest-performance.md). The first moving
Quest probe is automated no-clip flight at walking-like speed, with
render-distance scripts for 1, 5, and 10, current OpenXR refresh reporting,
and max-stage timing attribution. Durable headset rows live in
[`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md).
On-device `XR_META_performance_metrics` data initially suggested the RD10 frozen
lane had GPU headroom (`7ms` app GPU at `58%` utilization) while per-eye CPU wall
was `~21ms`. **E1 (2026-06-29, in-stream wgpu GPU timestamps) revised this:** the
blocking stereo poll wait is real GPU execution (`~10.7ms` both eyes, matching
the poll within `~0.5ms`), the Meta `app/gpu_frametime` counter under-reported,
and the frame is a balanced serial `CPU(~9ms) + GPU(~10.7ms)` — so CPU reductions
cannot hit 72Hz alone and CPU/GPU overlap plus GPU-side reduction become the key
levers. The attribution,
confidence levels, Playbox cross-check, and the landed diagnostic +
shared-records slices live in
[`099`](../tactical/099-android-xr-rd10-render-cost-attribution.md). The ordered
remaining plan — container-bound-cull diagnosis, cache prepared records across
frames, flatten cull/encode off `BTreeMap`, single shared dual-frustum cull,
stereo multiview encode, then batching / CPU-GPU overlap, plus the ship-distance
decision — lives in
[`106`](../tactical/106-android-xr-static-render-cpu-reduction.md). The unsafe
single-submit slice was reverted by `d0c5161` after headset validation exposed a
left-eye projection/uniform-lifetime bug; the new gating plan is
[`107`](../tactical/107-xr-stereo-uniform-ownership-and-multiview.md): orphaned
E1/E2 perf surfaces have been removed/fenced, and all current per-view
renderers now have slotted uniform/view-data ownership. Headset validation
confirmed the per-eye-submit baseline after that migration, and a desktop
one-submit slot proof exists. Multiview then landed and was validated on Quest.
Corrected Playbox-style busy/free metrics show it is average-app-work flat
because it still renders the per-eye union, though it improves p95/over-period
tail behavior; production stays on masked per-eye submit. For Quest perf rows,
compare `MCLONE_ANDROID_XR_PERF_HEADROOM` `app_work_*`, `headroom_*`, and
`app_over_period_*`. Legacy `frame_avg_ms` mostly tracks compositor-paced frame
cadence at the selected refresh rate and does not measure headroom. With the
cheap CPU wins (Slices F/G/H) landed and RD10 only borderline, the active plan
pivots to the `~10.7ms` GPU floor and CPU/GPU overlap in
[`117`](../tactical/117-android-xr-rd10-gpu-floor-and-frame-overlap.md):
fill-vs-geometry probe, render-scale, fixed-foveated rendering (accepted by the
runtime but no measured win), a solid render layer that drops the
early-Z-defeating `discard`, frame overlap (E4), batching, greedy meshing, app
spacewarp, and a ship-distance decision. 096/099/106/107 are closed references.

Current throughput strategy, as of the July 4 baseline pass, lives in
[`142`](../tactical/142-throughput-policy-with-quest-rd5-guardrail.md). Optimize
desktop native startup-streaming throughput at RD10/RD15, keep Quest OpenXR RD5
settled-orbit clean as the safety guardrail, and use Quest RD7 settled-orbit as
the pressure check. RD5 is not a throughput benchmark; it is the lower-distance
control that catches fixed XR/render regressions while desktop throughput work
changes shared worker/admission policy. The 142 revision attributes current
desktop throughput to the server publication valve — `1` feature chunk and `1`
light status published per **gameplay tick** (20 Hz), with the next feature job
gated on full publication of the previous one — capping production near `20`
chunks/sec (measured `13.5`-`17.1` across RD5-RD20). A reverted 2-line
publish-budget prototype (`1` → `32`) validated the model on 2026-07-04:
desktop RD10 full-view-ready `27.54s` → `9.23s` and playable entry `1.08s` →
`0.30s` with unchanged frame pacing; host-only cadence `60/20/60` measured
worse and is a retired lever; compile workers `2` left the bulk mesh drain
flat. This proves a desktop throughput bottleneck, not a Quest optimization by
itself: the Quest question is whether a larger publication drain remains safely
buffered by the 72 Hz client/render budgets or leaks through as server runner
spikes, update queue age, dirty/prepare/upload tails, or dropped-frame deltas.
The ownership split matters: in local integrated play, terrain/features, light,
publication, persistence, client apply, mesh/upload, and render all compete for
the same device CPU; in remote/multiplayer play, terrain/features/light/
publication/store move to the host while the headset still pays
network/decode/apply, mesh/upload, and render. Backpressure is the target shape
for decoupling those stages, but it must be proven with queue-age and
frame-pacing evidence before opening shared levers on Quest. The 2026-07-05
waterfall pass strengthened that split: desktop RD10/RD15 still scale at about
`19` target chunks/sec with empty client update queues, while Quest RD5
local-integrated chunk-view churn stays mostly paced (`skipped_delta=0`,
`app_work_p95=12.631ms`) but already shows update queue age (`167ms`),
upload/accept backpressure, and one `2x` frame. The remote-dedicated contrast
is not yet valid because the remote path still ignores `SendOnly` and does not
surface the needed client/network/update counters.
The follow-up fixed-count sweep on the same day showed the desktop full-view
gain saturates early: publish budget `4` moves RD10 from `27.492s` to
`9.681s`, while budgets `8/16` are flat and `32` adds tail risk. Under budget
`4`, disabling lighting only improves full-view to `8.154s`, but
`--render-compile-workers 2` cuts render quiescence from `20.089s` to
`11.412s` with no workers-4 gain.
The new raw/server ceiling split prevents misreading that desktop-shaped result
as a hardware limit: raw RD10-footprint feature generation is `551.5`
chunks/sec cold (`1243.2` warm), server-only loading is `126.7` chunks/sec with
lighting off, and server-only loading with light is `40.6` chunks/sec before
any client mesh/render work.
Next decomposition is the remaining render tail: persisted server reload now
measures RD10 view-ready at `0.523s` from in-memory records and `0.572s`
through temp SQLite, both from `625` already-lit records with no
worldgen/light work. Mesh CPU-only from persisted snapshots is measured at
`11.037s` for `8464` RD10 target sections, while GPU upload-only for the same
RD10 prebuilt mesh footprint is `0.133s` for `360 MB`. Tactical 150 then split
the apparent RD10 long tail: live seed `12345` quiescence was dominated by
scheduled lava/fluid simulation, not renderer throughput. With scheduled fluids
frozen and the conservative render compile queue-depth default
(`workers=1`, `max-pending=4`), clean Slice 5 RD10 startup-streaming reaches
full target view in `9.14-9.18s`, RD15 reaches full view in `19.43s`, and
persisted RD10 reaches actionable target render quiescence in `4.925s`.
Tactical [`153`](../tactical/153-vanilla-shaped-chunk-pipeline-capacity.md)
then closed the broad capacity/valve pass: shared render compile capacity now
exists but remains explicit/context-gated, retained-light redundant rechecks are
filtered, and publication count caps became cost-derived grants. The remaining
fresh-startup ceiling is not another generic pipeline valve; it is serial
sky-light graph/storage compute. Treat future work here as lighting-specific
unless a new measurement row re-promotes a capacity or frame-pacing valve.
Remote/local host-mode treatment stays with
[`151`](../tactical/151-remote-inbound-update-pipeline.md), and Quest soak /
RD7/RD10 pressure stays with the Quest pacing tacticals.
See the publication-valve record in
[`../performance-records.md`](../performance-records.md).

## Priority Queue

| Priority | Work | Java-shaped | Tactical | Status | Why It Matters |
|---|---|---:|---|---|---|
| P0 | Startup loading/progress presentation | Native policy | [`027`](../tactical/027-mclone-ui-foundation.md), [`028`](../tactical/028-headless-window-frame-unification.md), [`044`](../tactical/044-native-light-status-worker-and-disable-flag.md) | next recommended for desktop feel | Radius-5 lighting-enabled startup dropped to `1,931.110 ms`, but the desktop window path still waits for the first light-ready scene. Even with faster lighting, presenting progress or a partial scene will make regressions diagnosable instead of looking like a frozen app. |
| P1 | Liquid renderer light sampling | Yes | [`054`](../tactical/054-native-block-render-facts-parity.md), [`lighting topic`](lighting.md) | next recommended for visual lighting parity | Stored sky/block values reach the renderer, the shader uses Java's default lightmap curve, mesh-side AO includes non-cubic shape weighting, and current terrain-MVP AO facts now follow Java `BlockStateBase.Cache` behavior. Liquid light sampling from `LiquidBlockRenderer` is still not ported. |
| P2 | Sky neighbor skip-through propagation | Yes | [`049`](../tactical/049-native-sky-source-section-ownership.md), [`lighting topic`](lighting.md) | pending solver parity | Source-section ownership now lives in `SkyLightSectionStorage`, but native still lacks Java `SkyLightEngine.checkNeighborsAfterUpdate(...)` behavior for vertical gaps in light-storage sections. This remains important before live deltas, but it is no longer the current foliage-top visual blocker. |
| P3 | Live light deltas and light-section dirtying | Yes | [`lighting topic`](lighting.md), [`026`](../tactical/026-lighting-pipeline.md), [`031`](../tactical/031-native-section-block-delta-updates.md) | pending larger subsystem | Section block deltas currently leave light payloads unchanged. Correct live lighting needs Java-shaped light propagation/deltas and render dirtying by changed light sections. |
| P4 | Cancellable render compile tasks | Yes | [`034`](../tactical/034-native-render-compile-revisions-and-priority.md), [`033`](../tactical/033-native-async-render-section-compile-queue.md) | deferred | Native now avoids stale queued backlog and accepts unchanged sections. This remains useful for stress-orbit streaming, but current radius-5 evidence no longer puts render compile cancellation ahead of startup presentation or lighting parity. |
| P5 | GPU upload budgeting and buffer reuse | Broadly | [`024`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md), [`030`](../tactical/030-native-streaming-publish-and-render-budget.md) | conditional | Native uploads changed sections incrementally, and movement probes show upload cost is small. Do this when probes show upload/allocation cost is material again. |
| P6 | Release perf budgets and durable records | Native policy | [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md), [`030`](../tactical/030-native-streaming-publish-and-render-budget.md), [`033`](../tactical/033-native-async-render-section-compile-queue.md), [`034`](../tactical/034-native-render-compile-revisions-and-priority.md), [`../performance-records.md`](../performance-records.md) | ongoing | Once baselines stabilize, add budget thresholds that catch regressions without failing on normal host noise. |
| P7 | Streaming throughput with Quest guardrails | Native policy | [`142`](../tactical/142-throughput-policy-with-quest-rd5-guardrail.md), [`150`](../tactical/150-adaptive-frame-budget-controller.md), [`153`](../tactical/153-vanilla-shaped-chunk-pipeline-capacity.md), [`../performance-records.md`](../performance-records.md), [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md) | closed as broad capacity pass; lighting-specific follow-up | Optimize desktop RD10/RD15 startup-streaming throughput, not RD5. Tactical 150 promoted adaptive publication for local-integrated desktop/native-XR/Android-XR and the conservative render compile queue-depth baseline (`workers=1`, `max-pending=4`). Tactical 153 added whole-pipeline attribution, shared render compile capacity, sparse retained-light rechecks, cost-derived publication grants, light-status batch/priority evidence, and Quest metrics hygiene. Global render-capacity promotion remains declined because desktop 120 Hz still has a top-level over-2x outlier at desktop-derived `7/14`; the remaining fresh-startup ceiling is sky-light graph/storage compute, so the next throughput lever should be tracked as lighting work rather than another broad capacity default. |
| PX | Quest 72 Hz: RD5 guardrail, RD7 pressure, RD10 stress | Native policy | [`142`](../tactical/142-throughput-policy-with-quest-rd5-guardrail.md) (current guardrail policy), [`117`](../tactical/117-android-xr-rd10-gpu-floor-and-frame-overlap.md) (RD10 stress), [`119`](../tactical/119-android-xr-live-streaming-frame-pacing.md) (active checklist); [`099`](../tactical/099-android-xr-rd10-render-cost-attribution.md), [`106`](../tactical/106-android-xr-static-render-cpu-reduction.md), [`107`](../tactical/107-xr-stereo-uniform-ownership-and-multiview.md) (closed refs) | active; do not chase perfect RD7 before throughput work | E1 showed RD10 is a balanced serial `CPU(~9ms, now ~4ms) + GPU(~10.7ms)` frame (Meta `7ms` GPU counter under-reported; the poll wait is real GPU), so CPU-only work cannot make RD10 comfortably hit 72 Hz alone. Use `PERF_HEADROOM` app-work/headroom fields, not legacy `frame_avg_ms`, for Quest comparisons. Current throughput work must keep RD5 clean and avoid materially worsening RD7; RD10 remains the stress lane, not the primary pass/fail gate for desktop throughput policy. |

## Java Reference Anchors

Use these local sources when implementing or reviewing performance work:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`
  - chunk packets mark all chunk sections dirty
  - section/block update packets apply changed blocks to the client level
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
  - `setBlockDirty`, `setBlocksDirty`, and `setSectionDirtyWithNeighbors`
    define section dirty granularity
  - `updateRenderChunks(...)` performs graph-aware visible-section traversal
  - `compileChunksUntil(...)` consumes render compile/upload work under a time
    budget
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java`
  - owns reusable render chunk slots and dirty flags
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
  - owns render chunk compile tasks, upload queue, and `hasAllNeighbors()`
  - creates `RenderChunkRegion` with a one-block halo for real neighbor block
    reads
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/VisGraph.java`
  and `VisibilitySet.java`
  - local section face-connectivity graph used by traversal
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
  - records changed blocks by section and broadcasts block/section update
    packets
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
  - mailbox-shaped light scheduling and batched publication

## Completed Or De-Risked

- Desktop frame pacing controls and headless frame-budget probe:
  [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md)
- Completed chunk publication slicing and initial streaming attribution:
  [`030`](../tactical/030-native-streaming-publish-and-render-budget.md)
- Bounded synchronous render-section rebuild/upload work:
  [`030`](../tactical/030-native-streaming-publish-and-render-budget.md)
- CPU render-section dirty cache and incremental GPU section updates:
  [`024`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md)
- Java `VisGraph` / `VisibilitySet` first pass:
  [`025`](../tactical/025-render-section-visibility-graph.md)
- Section block deltas replacing runtime full chunk snapshot publication:
  [`031`](../tactical/031-native-section-block-delta-updates.md)
- Neighbor-ready render boundary first pass:
  [`032`](../tactical/032-native-neighbor-stable-render-boundaries.md)
- Section-precise render dirtying/rebuilds for section block deltas:
  [`024`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md),
  [`031`](../tactical/031-native-section-block-delta-updates.md)
- Async CPU render-section compile queue first pass:
  [`033`](../tactical/033-native-async-render-section-compile-queue.md)
- Movement-shaped release frame probe:
  [`029`](../tactical/029-native-frame-pacing-and-streaming-hitches.md),
  [`033`](../tactical/033-native-async-render-section-compile-queue.md)
- Render compile queue revision/priority refinement:
  [`034`](../tactical/034-native-render-compile-revisions-and-priority.md)
- Shared initial light batch:
  [`045`](../tactical/045-native-shared-initial-light-batch.md)
- Retained initial light world:
  [`046`](../tactical/046-native-retained-initial-light-world.md)
- Light graph drain instrumentation and mixed graph map optimization:
  [`047`](../tactical/047-native-light-graph-drain-instrumentation.md)
- Empty-section light setup:
  [`048`](../tactical/048-native-sky-empty-section-light-setup.md)
- Sky source-section ownership:
  [`049`](../tactical/049-native-sky-source-section-ownership.md)
- Leaf sky render parity:
  [`050`](../tactical/050-native-leaf-sky-render-parity.md)
- LightTexture render parity:
  [`051`](../tactical/051-native-light-texture-render-parity.md)
- Model AO render parity:
  [`052`](../tactical/052-native-model-ao-render-parity.md)
- Non-cubic AO render parity:
  [`053`](../tactical/053-native-non-cubic-ao-render-parity.md)
- Block render-facts parity:
  [`054`](../tactical/054-native-block-render-facts-parity.md)

## Tactical Index

Primary performance tacticals:

- [`020-native-movement-perf-and-render-culling.md`](../tactical/020-native-movement-perf-and-render-culling.md)
- [`023-render-mesh-culling-parity-and-perf.md`](../tactical/023-render-mesh-culling-parity-and-perf.md)
- [`024-render-section-dirty-cache-and-upload-diffs.md`](../tactical/024-render-section-dirty-cache-and-upload-diffs.md)
- [`025-render-section-visibility-graph.md`](../tactical/025-render-section-visibility-graph.md)
- [`029-native-frame-pacing-and-streaming-hitches.md`](../tactical/029-native-frame-pacing-and-streaming-hitches.md)
- [`030-native-streaming-publish-and-render-budget.md`](../tactical/030-native-streaming-publish-and-render-budget.md)
- [`031-native-section-block-delta-updates.md`](../tactical/031-native-section-block-delta-updates.md)
- [`032-native-neighbor-stable-render-boundaries.md`](../tactical/032-native-neighbor-stable-render-boundaries.md)
- [`033-native-async-render-section-compile-queue.md`](../tactical/033-native-async-render-section-compile-queue.md)
- [`034-native-render-compile-revisions-and-priority.md`](../tactical/034-native-render-compile-revisions-and-priority.md)
- [`096-android-xr-quest-performance.md`](../tactical/096-android-xr-quest-performance.md)
- [`099-android-xr-rd10-render-cost-attribution.md`](../tactical/099-android-xr-rd10-render-cost-attribution.md)
- [`106-android-xr-static-render-cpu-reduction.md`](../tactical/106-android-xr-static-render-cpu-reduction.md)
- [`107-xr-stereo-uniform-ownership-and-multiview.md`](../tactical/107-xr-stereo-uniform-ownership-and-multiview.md)
- [`117-android-xr-rd10-gpu-floor-and-frame-overlap.md`](../tactical/117-android-xr-rd10-gpu-floor-and-frame-overlap.md)
- [`119-android-xr-live-streaming-frame-pacing.md`](../tactical/119-android-xr-live-streaming-frame-pacing.md)
- [`140-streaming-throughput-frame-pacing-baselines.md`](../tactical/140-streaming-throughput-frame-pacing-baselines.md)
- [`142-throughput-policy-with-quest-rd5-guardrail.md`](../tactical/142-throughput-policy-with-quest-rd5-guardrail.md)

Related subsystem tacticals:

- [`021-scheduled-fluid-ticks.md`](../tactical/021-scheduled-fluid-ticks.md)
- [`026-lighting-pipeline.md`](../tactical/026-lighting-pipeline.md)
- [`028-headless-window-frame-unification.md`](../tactical/028-headless-window-frame-unification.md)
- [`044-native-light-status-worker-and-disable-flag.md`](../tactical/044-native-light-status-worker-and-disable-flag.md)
- [`045-native-shared-initial-light-batch.md`](../tactical/045-native-shared-initial-light-batch.md)
- [`046-native-retained-initial-light-world.md`](../tactical/046-native-retained-initial-light-world.md)
- [`047-native-light-graph-drain-instrumentation.md`](../tactical/047-native-light-graph-drain-instrumentation.md)
- [`048-native-sky-empty-section-light-setup.md`](../tactical/048-native-sky-empty-section-light-setup.md)
- [`049-native-sky-source-section-ownership.md`](../tactical/049-native-sky-source-section-ownership.md)
- [`050-native-leaf-sky-render-parity.md`](../tactical/050-native-leaf-sky-render-parity.md)
- [`051-native-light-texture-render-parity.md`](../tactical/051-native-light-texture-render-parity.md)
- [`052-native-model-ao-render-parity.md`](../tactical/052-native-model-ao-render-parity.md)
- [`053-native-non-cubic-ao-render-parity.md`](../tactical/053-native-non-cubic-ao-render-parity.md)

## Validation Lanes

Fast gates:

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm --silent native:frame-budget:smoke
pnpm --silent native:movement-frame:smoke
git diff --check
```

Release comparison:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- \
  --frame-budget-probe --frame-budget-frames 120 --target-hz 120
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- \
  --movement-frame-probe --frame-budget-frames 240 --target-hz 120
```

Movement and render pressure:

```bash
pnpm --silent native:movement:smoke
pnpm --silent native:timedemo:smoke
```

For pixel-affecting native work, capture screenshots to `/tmp` and inspect
them before considering the slice complete.

## Update Policy

- Keep this topic doc short enough to scan.
- Record detailed raw historical baselines in
  [`../performance-records.md`](../performance-records.md), not here.
- When a tactical recommendation is superseded, update the old tactical with a
  pointer to the newer tactical and update this priority queue.
- Prefer Java-shaped work when it addresses the measured bottleneck or prevents
  correctness/performance churn.
- Do not add performance thresholds until there is a stable release-mode
  baseline and the threshold would catch a real regression rather than host
  noise.
