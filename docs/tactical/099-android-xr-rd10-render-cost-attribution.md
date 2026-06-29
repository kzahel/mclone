# 099: Android XR RD10 Render-Cost Attribution And CPU-Bound Submission

Status: active; diagnostics landed (opt-in Meta performance-metrics probe,
commit `0df8733`, and Slice A render split); Slice B single-submit stereo path
landed; Slices C1/C2 (shared section-record prep + prepare sub-bucket attribution)
landed. This is the concrete continuation of [`096`](096-android-xr-quest-performance.md)
Slice 5 (GPU timing / render-mode attribution). **The ordered remaining plan now
lives in [`106`](106-android-xr-static-render-cpu-reduction.md)** (container-bound
cull diagnosis, cross-frame record cache, flatten cull/encode, single shared
dual-frustum cull, stereo multiview, batching / overlap). This doc remains the
attribution + diagnostics record; future optimization sessions should start at
`106`.

## Purpose

Render distance 10 (RD10) misses the 72 Hz budget on standalone Quest 3 even in
the frozen-render lane where all chunk generation, runtime poll, section sync,
traversal refresh, and GPU upload report `0.000ms`. The prior interpretation
(see the `2d200e1` frozen sweep record) guessed RD10 was "render-bound or
compositor/GPU-wait-bound." On-device Meta performance-metrics data overturns
that guess: **RD10 has GPU headroom and is CPU draw-submission / frame-pacing
bound.** This doc captures the evidence, the confidence level of each claim, the
Playbox cross-check, and an ordered optimization plan so the next session does
not re-derive it.

Scope: standalone Quest / Android XR APK only, not desktop-hosted OpenXR
streaming to a Quest client.

## Headline Finding

On the frozen RD10 lane (fixed pose `0,80,-96,180`, 179 drawn sections,
1.35M drawn indices, ~62 FPS, p50 16.1ms):

| Bucket | Measured | Source |
|---|---:|---|
| App GPU frametime (both eyes) | `7.040 ms` | `XR_META_performance_metrics` `app/gpu_frametime` |
| GPU utilization | `57.6 %` | `device/gpu_utilization` |
| Compositor GPU frametime | `1.561 ms` | `compositor/gpu_frametime` |
| App per-eye CPU wall (left + right) | `10.570 + 10.420 = 20.99 ms` | `MCLONE_ANDROID_XR_PERF_TERRAIN` |
| Slice A per-eye max poll wait | left `7.217 ms`, right `6.487 ms` | `MCLONE_ANDROID_XR_PERF_TERRAIN` |
| Slice A per-eye max prepare | left `3.504 ms`, right `3.419 ms` | `MCLONE_ANDROID_XR_PERF_TERRAIN` |
| Slice A per-eye max encode | left `1.638 ms`, right `1.621 ms` | `MCLONE_ANDROID_XR_PERF_TERRAIN` |
| Slice B frozen RD10 p50 / avg | `15.609 ms` / `15.721 ms` | `MCLONE_ANDROID_XR_PERF_SUMMARY` |
| Slice B single stereo poll wait max | `10.092 ms` | `MCLONE_ANDROID_XR_PERF_TERRAIN` |
| Slice B per-eye encode wall max | left `4.679 ms`, right `4.282 ms` | `MCLONE_ANDROID_XR_PERF_TERRAIN` |
| Compositor dropped frames (cumulative) | `103` | `compositor/dropped_frame_count` |
| SpaceWarp | off (`0`) | `compositor/spacewarp_mode` |
| Device CPU util avg / worst core | `35.7 %` / `~53 %` | `device/cpu_utilization_*` |

The GPU draws both eyes in ~7 ms with ~42% headroom, yet the app burns ~21 ms of
per-eye wall time. Slice A shows the largest measured per-eye bucket is the
blocking `device.poll` wait, with prepare still material and direct encode
smaller. Slice B removes per-eye submit/poll, improving frozen RD10 p50 from
~`16.1ms` to `15.6ms`, but a single shared stereo poll still waits up to
`10.1ms`. The compositor drops frames because the app frame exceeds the
13.889 ms budget, not because the GPU or compositor is saturated. The full
record is in
[`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
under the 2026-06-27 frozen RD10 Meta metrics and render-split records.

## What Landed (diagnostic only)

- New opt-in `XR_META_performance_metrics` probe (the Meta/"Facebook"
  perf-metrics extension), ported from the Playbox pattern:
  `native/apps/mclone-android-xr-client/src/perf_metrics.rs`.
- `meta_performance_metrics` enabled at instance creation when available.
- `--perf-metrics` flag plumbed through startup options →
  `run_android_openxr_mclone` → `run_mclone_frame_loop`, forwarded in
  `android-xr/validate-quest-openxr.sh`, with a
  `native:android-xr:perf:frozen:rd10:metrics` convenience lane.
- Emits `MCLONE_ANDROID_XR_PERF_METRICS` (compact app/compositor GPU+CPU
  frametime, utilization, dropped/stale frames, motion-to-photon) plus a
  per-counter `MCLONE_ANDROID_XR_PERF_METRICS_COUNTER` inventory. Off by
  default; degrades gracefully when the extension or counters are unavailable.
- Slice A adds opt-in per-eye max fields to `MCLONE_ANDROID_XR_PERF_TERRAIN`:
  prepare, encode, section encode, submit, and poll wait. It is enabled only for
  Android XR perf runs, with no render-behavior change in normal rendering.
- Slice B records both eyes into one encoder, submits once, and polls once after
  both eyes. The same terrain marker now also carries stereo finish/submit/poll
  max fields for the shared submission.
- Commits: `0df8733` (probe), `4ffa99c` (record).

The Oculus `v204.201.0` runtime enumerates 17 counters and returns valid data
on the first attempt. It does **not** expose `app/cpu_frametime` or
`compositor/cpu_frametime`, so those marker fields are `n/a`; CPU load is
covered by `device/cpu_utilization_*` (average, worst, per-core 0..7).

## mclone's render path findings

Code facts a future session should not have to rediscover.

- Before Slice B, each eye rendered independently and serially in `render_prepared_frame`
  (`native/crates/mclone-xr-scene/src/lib.rs:540-567`): left eye fully
  completes before the right eye is even encoded.
- Before Slice B, `render_eye_target` (`native/crates/mclone-xr-scene/src/lib.rs:874-948`)
  creates its **own** command encoder, records the pass, `queue.submit`s, then
  **blocks** on `device.poll(WaitForSubmissionIndex)` + `device.poll(Wait)`
  (`:936-944`). That is **two encoders, two submits, and four blocking polls
  per frame**, with no CPU/GPU overlap and no eye parallelism. It is why the
  per-eye wall time already includes GPU execution and why the compositor never
  waits on our GPU (we drained it ourselves), so the existing `wait_begin`
  stage cannot see GPU cost.
- Slice B changed this to one stereo encoder, one submit, and one
  `WaitForSubmissionIndex` poll after both eyes. It cut per-eye wall from
  ~`10.6ms` to ~`4.7ms`/`4.3ms`, but the single stereo poll still reaches
  `10.092ms` max.
- `TexturedSectionRenderer::render_with_options`
  (`native/crates/mclone-render/src/chunk.rs:1532-1607`) runs in full **per eye,
  per frame, even when frozen**:
  - builds a fresh `BTreeMap` over **all** sections (1,133) — `:1540`,
  - runs `cull_textured_sections` over all of them — `:1556`,
  - `queue.write_buffer`s the camera uniform — `:1557`,
  - issues **one `draw_indexed` per drawn section** (~179 opaque), unbatched,
    instances `0..1` — `:1586-1591`,
  - allocates a `Vec` and **sorts** the translucent set, one draw each —
    `:1593-1605`.
- In frozen mode the pose is fixed, so the cull result, draw list, and camera
  uniform are **identical every frame**, yet all of it is rebuilt from scratch
  twice per frame.

## Playbox cross-check

Playbox is a mature wgpu/OpenXR engine with extensive Quest iteration; its
tactical `046-quest-hz-ramp-performance-baselines.md` tests these exact
questions. Findings (Playbox file references are approximate; verify before
porting):

1. **Single encoder, single submit, single poll for both eyes.** Playbox
   records both eyes into one encoder (`src/render/mod.rs` ~2964), submits once
   (`~3279`), then polls **once after both eyes** (`src/xr/mod.rs` ~1632-1648).
   Its 046 doc explicitly records a "stereo single-poll" optimization that moved
   *from* per-eye polling *to* one poll and measured the win. mclone's per-eye
   blocking poll is the anti-pattern Playbox already removed.
2. **No wgpu `RenderBundle`s anywhere** (grep empty). Instead Playbox calls
   `prepare_frame_scene` **once per frame and shares it across both eyes**
   (`src/xr/mod.rs` ~3532) and **batches draws with instancing** — one
   `draw_indexed` per batch, not per object (`src/render/mod.rs` ~3315;
   `src/render/mesh_cache.rs` `DrawBatch`/`InstanceRaw`/`MAX_INSTANCES`).
3. **For Playbox the residual is GPU-wait, not CPU.** Its instrumented table
   shows `device.poll(Wait)` at ~60-70% of submit time and direct per-eye
   encode under ~1 ms combined, and it concludes "more CPU prep refactors are
   unlikely to help unless they reduce queued GPU work." **Critically, that
   conclusion is conditional on Playbox having already batched draws and shared
   prep.** mclone has done neither and issues ~358 unbatched draws with
   duplicated per-eye prep, so mclone is in the CPU/submission-bound regime
   Playbox had already left — consistent with the 7 ms GPU vs 21 ms wall gap.

So Playbox both validates the direction (kill per-eye poll, share prep, batch
draws) and warns: `device.poll(Wait)` overhead is itself non-trivial and mclone
runs it twice as often, so part of mclone's residual is poll overhead, not draw
encoding. Measure before assuming.

## Theories and confidence

| # | Claim | Confidence | Basis / what would raise it |
|---|---|---|---|
| T0 | RD10 is **not** GPU-fill-bound; GPU has ~40% headroom | **High** | Runtime-measured `app/gpu_frametime` 7.04 ms and `gpu_utilization` 57.6% independently agree. Single mid-window sample; a few more samples for a distribution would make it airtight. |
| T1 | Per-eye blocking submit/poll serialized eyes, blocked CPU on GPU, and hid GPU cost from the wait stage | **High / resolved by Slice B** | Direct from pre-Slice-B code (`lib.rs:936-944`); Playbox removed the same pattern. Slice B removed per-eye submit/poll and leaves one shared stereo wait. |
| T2 | mclone re-prepares (BTreeMap build + cull + translucent sort) and re-encodes ~358 unbatched draws per eye per frame, wasteful under a static view | **High** | Direct from code (`chunk.rs:1532-1607`). |
| T3 | The ~14 ms residual (21 ms wall − 7 ms GPU) was dominated first by per-eye poll wait; after Slice B the largest measured wait is one shared stereo poll | **High** | Slice A measured max poll wait at left `7.217ms`, right `6.487ms`. Slice B reports per-eye poll `0.000ms`, stereo poll `10.092ms`, and per-eye encode wall left/right `4.679ms`/`4.282ms`. Max fields are independent, not one additive frame. |
| T4 | Batching draws and/or render bundles + single-submit will recover most of the budget | **Medium** | Follows from T2/T3 but magnitude depends on the T3 split. Playbox's batching path is proven; render bundles are an mclone-specific option Playbox did not need. |
| T5 | "Keep it on the GPU / reuse last frame" by caching rendered pixels | **Low / rejected** | Unsafe in XR: each frame still submits its own layer and even a "static" headset has pose jitter. The correct reading of the intuition is *stop re-walking/re-culling/re-encoding on the CPU*, not *reuse the framebuffer*. Playbox does not cache pixels. |

## Recommended implementation slices (ordered)

Diagnostic-first, then the Playbox-proven structural fixes. Keep every change
behind the shared XR contracts; do not fork mclone rendering for Quest.

### Slice A - Split the per-eye wall into prepare / encode / submit / poll (done)

- Landed. Bracket, per eye: prepare (records map + cull + translucent sort),
  render-pass encode (the draw loop), `queue.submit`, and the
  `device.poll(Wait)` block. Mirror Playbox's 046 breakdown table.
- Surface as new max fields on the existing `MCLONE_ANDROID_XR_PERF_TERRAIN`
  marker (or a new `..._RENDER_SPLIT` line), same opt-in style as the metrics
  probe. ~Cheap, zero render-behavior change, no device feature.
- Result: poll wait is the largest max bucket, so chase Slice B first. Prepare
  is still large enough to keep Slice C/D relevant after the per-eye GPU stall
  is removed.
- Validation passed: `cargo check --manifest-path native/Cargo.toml --target
  aarch64-linux-android`; `pnpm native:android-xr:perf:frozen:rd10:metrics`;
  explicit force-stop + headset sleep cleanup verified.

### Slice B - Single encoder, single submit, single poll for both eyes (done)

- Landed. Acquire both eye swapchain images, record both eye render passes into one
  encoder, `queue.submit` once, `device.poll` once after both, then release
  both images and `xrEndFrame`. Remove the per-eye blocking poll.
- Playbox-proven; removes 3 of 4 polls and enables CPU(eye 2)/GPU(eye 1)
  overlap. Refactors `render_prepared_frame` / `render_eye_target` ownership of
  the encoder; keep `record_eye0_summary` semantics.
- Result on frozen RD10 metrics: p50 improved to `15.609ms`, frame avg to
  `15.721ms`, terrain frame max to `18.102ms`; per-eye wall max is now
  left `4.679ms`, right `4.282ms`; shared stereo poll max is `10.092ms`.
- Validation passed: `cargo check --manifest-path native/Cargo.toml --target
  aarch64-linux-android`; `pnpm native:android-xr:perf:frozen:rd10:metrics`;
  explicit force-stop + headset sleep cleanup verified. Attempted headset
  `screencap`, but Quest returned a zero-byte capture after the perf process
  had exited, so there is no visual screenshot artifact for this run.

### Slice C - Shared stereo section prep, then per-eye cull/sort

- Do **not** assume the live headset view is exactly unchanged. Even a still
  headset has small pose jitter, so exact view/projection cache keys are only
  reliable in the frozen perf lane.
- First split the terrain CPU prep into shared stereo work and per-eye work:
  build the section culling records once per frame from the current section set,
  then run each eye's frustum/occlusion cull and translucent sort from that
  shared record set.
- Keep per-eye cull and translucent sort view-dependent. Later live-XR caching
  can use conservative buckets/tolerances (section-set version, camera section
  or small position cell, coarse yaw/pitch, expanded frustum), but the first
  implementation should be exact and behavior-preserving.
- Once the shared record set exists, left/right cull + translucent sort become
  natural independent jobs for a later threading slice. Avoid threading first;
  make the work separable and measurable before adding scheduling complexity
  across desktop/Android/WASM.
- Slice C1 landed: build the section culling records once per stereo frame and
  reuse that record set for both eyes. Per-eye frustum/occlusion culling,
  uniform upload, translucent sort, and draw encoding remain view-dependent.
- Result on frozen RD10 metrics: `max_terrain_shared_records_ms=1.195`; per-eye
  prepare dropped from Slice B's `3.476ms` / `3.181ms` maxima to `2.330ms` /
  `2.134ms`; p50 improved from `15.609ms` to `14.431ms`. App GPU remained
  about `7.0ms`, so the next useful split is inside the remaining per-eye
  prepare bucket.
- Slice C2 is attribution-only: split each eye's prepare bucket into
  `cull`, `uniform_write`, `translucent_collect`, and `translucent_sort` maxima
  on an opt-in `MCLONE_ANDROID_XR_PERF_TERRAIN_PREP` line. Keep it separate
  from the existing terrain line so Android logcat does not truncate the marker
  before the stereo submit/poll fields. Use that measurement to choose between
  exact cull/sort reuse, left/right prep jobs, or moving on to draw
  encoding/bundles.
- Slice C2 result on frozen RD10 metrics: p50 stayed at `14.414ms`; shared
  records max `1.305ms`; prepare max left/right `2.324ms` / `2.291ms`; cull
  max left/right `2.148ms` / `2.113ms`; translucent collect max `0.383ms` /
  `0.296ms`; translucent sort max `0.046ms` / `0.028ms`. The remaining prepare
  bucket is cull-dominated. The next behavior-preserving implementation should
  extract a pure per-eye prepared-draw/cull result from command encoding so
  left/right cull can either be cached under conservative keys or run as
  independent jobs after shared records are built.

### Slice D - Cut per-draw CPU cost (batching or render bundles)

- Option D1 (Playbox path): batch sections into fewer draws via a combined
  vertex/index megabuffer + instancing or `multi_draw_indexed_indirect`.
- Option D2 (mclone-specific): pre-record the opaque + translucent section draw
  sequence into a wgpu `RenderBundle` and replay with `execute_bundles`; set the
  camera bind group on the pass outside the bundle so one bundle serves both
  eyes and survives across frames while the section set is stable. Good fit for
  heterogeneous per-section meshes and the frozen/static case.
- Choose D1 vs D2 after Slice A shows how much is encode vs prepare.

### Slice E (lower priority) - GPU timestamps / diagnostic render modes

- wgpu `TIMESTAMP_QUERY` per pass or a flat/cheap-material diagnostic mode
  (Playbox `RenderMode`) is now **lower priority**: the GPU bucket is already
  known small. Revisit only if Slices A-D close the CPU gap and GPU becomes the
  limiter, or to confirm fixed-foveated rendering is actually applied (the
  instance enables `fb_foveation*` but the applied level is unverified).

## Methodology caveats (carry forward)

- The frozen lane's per-eye timers fuse CPU + GPU because of the blocking poll,
  and zero out compositor wait. Use the Meta counters (or Slice A) for the real
  split, not `frame_wall` percentiles (which include xrWaitFrame pacing).
- The Meta sample is one steady-state read (frame-count warmup ~720), not a
  max/percentile. Fine for a constant frozen load; sample a few times if a
  distribution is needed.
- The validated APK was built from a worktree carrying an unrelated uncommitted
  `mclone-ui` change (links in transitively); it does not change terrain draw
  behavior but is noted in the record for honesty.
- Thermals/clocks can drift over long runs; `gpu_utilization` alongside
  frametime guards against misreading throttle as work.

## Guardrails

- Keep diagnostics opt-in (`--perf-metrics`, future `--perf-render-split`);
  never add always-on per-frame diagnostic cost to the headset loop (the prior
  76 ms diagnostics regression is the cautionary tale).
- Do not fork mclone rendering, chunk streaming, or session runtime for Quest;
  add platform adapters/diagnostics around the shared contracts.
- Always run the validator cleanup (force-stop, sleep headset) and verify
  `pidof com.kzahel.mclone.xr` empty, `mWakefulness=Asleep`,
  `mHoldingDisplaySuspendBlocker=false` after on-device runs.
- For pixel-affecting slices (B, D), capture a screenshot and look before
  moving on.

## Open questions

- After Slice B: is the shared stereo poll mostly true GPU execution, wgpu wait
  overhead, or frame-pacing interaction? Meta app GPU time remains about
  `7.145ms`, while app-side stereo poll max is `10.092ms`.
- Does the live (non-frozen) RD10 lane share the same CPU-bound profile, or does
  streaming/upload re-enter as the limiter once the runtime is unfrozen?
- Is fixed-foveated rendering actually applied to the eye swapchains? If not, it
  is a cheap GPU win even though GPU is not the current limiter.
- What is the right Quest default render distance once Slices A-D land and a
  CPU-side budget exists?

## References

- Records: `../quest-standalone-performance-records.md` (frozen RD10 Meta
  metrics row, and the `2d200e1` frozen sweep it supersedes in interpretation).
- Parent perf tactical: [`096`](096-android-xr-quest-performance.md).
- Probe: `native/apps/mclone-android-xr-client/src/perf_metrics.rs`; flag wiring
  in the same crate's `lib.rs`; forwarding in
  `android-xr/validate-quest-openxr.sh`.
- Hot path: `native/crates/mclone-xr-scene/src/lib.rs:540-567,874-948`;
  `native/crates/mclone-render/src/chunk.rs:1532-1607`.
- Playbox: `docs/tactical/046-quest-hz-ramp-performance-baselines.md`,
  `src/xr/mod.rs`, `src/render/mod.rs`, `src/render/mesh_cache.rs`,
  `src/xr/performance_metrics.rs` (the probe pattern).
- Commits: `0df8733` (probe), `4ffa99c` (record).
