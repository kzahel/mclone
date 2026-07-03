# 131: Quest CPU-GPU Overlap And Frame Cost Hygiene

Status: active — F1a implemented and measured; redundant device-idle poll
removed, but no material RD7 win because submission waits still dominate
Workstream: Android XR frame pacing, XR scene GPU synchronization, actor
renderer resource lifetime, server-update application pacing

## Impetus

`130`'s E2a/E2c slice validated the scheduling thesis: Meta dropped frames fell
`36 → 13/14`, the migrating multi-ms bucket tails collapsed
(`ready sections 36.665 → 1.4 ms`, `>2x budget 12 → 0`), and the post-fix trace
shows `android_main` runnable gaps down from ~3 ms to ~0.5 ms. The coherent
worst-frame snapshots then isolated two remaining bad-frame families:

1. actor-pass spikes (`right_actor_ms=22.117` with only 2 actors), and
2. streaming-transition frames combining `commit_server_command` dirty-mark
   work (~4-6 ms), terrain final poll wait (6-9 ms), and moderate runtime work.

Meanwhile the lane's average app work still rides the 72 Hz budget line
(`13.554-15.346 ms` vs `13.889 ms`), so the strategic need is now twofold:
kill the two spike families, and recover several milliseconds of *average*
frame time to create durable headroom.

This pass audited the code paths behind those snapshots. It found two concrete
mechanisms and proposes an ordered set of slices. Everything below is grounded
in current code (file:line) and the measurements already recorded in `130`.

## Finding 1: the render thread blocks on GPU completion multiple times per frame — then waits to device idle

`wait_for_xr_submission` (`native/crates/mclone-xr-scene/src/lib.rs:3898-3912`)
does two things:

1. `device.poll(wgpu::PollType::WaitForSubmissionIndex(submission))` — a full
   CPU block until that submission's GPU work completes, and then
2. `device.poll(wgpu::PollType::Wait)` — a block until the **whole device is
   idle**, which also waits on any other outstanding submissions and drains
   unrelated deferred work.

Call sites: the per-eye path after each eye's submit when `wait_after_submit`
(`lib.rs:3849-3861`), the deferred-overlap path's final right-eye wait
(`lib.rs:1841-1848`), overlay compose (`lib.rs:1534-1538`), and the multiview
path (`lib.rs:2101-2105`, `2366-2376`).

Why this matters, from `130`'s own measurements:

- Meta performance metrics during the attribution runs report app GPU time of
  only `2.923-3.533 ms` and GPU utilization `23.5-29.0%` — yet worst frames
  show `terrain_poll_wait_ms=5.747-8.380` and per-eye poll waits summing
  `6-8 ms`. The app is not GPU-bound; it is CPU-blocked on synchronization
  points waiting for a mostly idle GPU (completion latency + driver/runtime
  scheduling), twice or more per frame.
- These waits are counted inside `app_work_ms`, so they directly inflate the
  `app_over_period` percentage and eat the headroom the scheduling fix
  recovered.

The OpenXR Vulkan contract does not require CPU-side completion before
`xrReleaseSwapchainImage`: the app must have *submitted* the rendering work to
the queue from the graphics binding; the runtime performs queue-level
synchronization itself. The upstream `openxrs` Vulkan example releases after
submit with no device wait. The infrastructure for living without these waits
largely exists already: `107` built slotted per-view uniform ownership exactly
so submissions can remain in flight, and the deferred-overlap mode already
defers the right-eye wait to do runtime prefetch work.

### Slices

**F1a — remove the redundant device-idle poll.** Delete the second
`device.poll(PollType::Wait)` from `wait_for_xr_submission`.
`WaitForSubmissionIndex` already guarantees the labeled submission is
complete; waiting for device idle on top of it is strictly broader and can
stall on the other eye's work or pending uploads. Near-zero risk; measure the
standard lane before/after.

### 2026-07-03 F1a Result

Implementation:

- Removed the trailing `device.poll(wgpu::PollType::Wait)` from
  `wait_for_xr_submission`.
- Kept `WaitForSubmissionIndex(submission)`, so the per-eye path still blocks
  until the just-submitted eye work completes. This was intentionally the
  narrow low-risk slice, not full no-wait pipelining.

Validation:

- `rustfmt --edition 2024 native/crates/mclone-xr-scene/src/lib.rs` passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene` passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- `cargo check --manifest-path native/Cargo.toml -p mclone-native-client`
  passed.
- Standard RD7 lane passed:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-quest-openxr-perf-orbit-rd7-no-device-idle-2-16-64.txt --log /tmp/mclone-quest-openxr-perf-orbit-rd7-no-device-idle-2-16-64-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.
- Skip-actors control passed:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --xr-skip-actors --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-quest-openxr-perf-orbit-rd7-no-device-idle-skip-actors-2-16-64.txt --log /tmp/mclone-quest-openxr-perf-orbit-rd7-no-device-idle-skip-actors-2-16-64-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.

Measured comparison:

| Run | Actors | Meta dropped frames | Frame avg / p95 / p99 / max (ms) | App work avg / p95 / max (ms) | App over period | Poll / spike readout |
| --- | --- | ---: | --- | --- | ---: | --- |
| Pre-F1a eye-split attribution (`130`) | on | 13 | 14.654 / 16.716 / 19.662 / 37.326 | 14.206 / 16.566 / 35.099 | 57.2% | Rank 1 actor spike: `right_actor_ms=22.117`; `stereo_poll_wait_ms=8.424` max |
| F1a standard | on | 12 | 14.716 / 16.789 / 19.625 / 54.132 | 14.197 / 16.682 / 54.103 | 57.1% | Actor spike persisted and worsened in this sample: `left_actor_ms=38.320`; `stereo_poll_wait_ms=8.939` max |
| Pre-F1a skip-actors command split (`130`) | off | 12 | 14.095 / 15.725 / 18.514 / 26.970 | 13.554 / 15.488 / 25.398 | 35.8% | Top frames were dirty-mark + poll-wait; `stereo_poll_wait_ms=8.442` max |
| F1a skip-actors control | off | 17 | 14.127 / 15.843 / 18.335 / 26.570 | 13.522 / 15.598 / 24.215 | 37.9% | No actor spike; one `stereo_poll_wait_ms=15.093` outlier remained |

Interpretation:

- F1a is correct cleanup, but not a demonstrated performance win in this
  lane. Average app work was effectively flat in both standard and
  skip-actors runs.
- The removed device-idle wait was not the dominant part of
  `terrain_poll_wait_ms`. The remaining `WaitForSubmissionIndex` wait is
  still enough to produce 6-8 ms streaming-transition waits, and one
  skip-actors outlier reached `15.093 ms`.
- The actor allocation spike remains the clearest actor-enabled tail. This
  run reproduced it as `left_actor_ms=38.320` with only two actors, again
  independent of terrain streaming work.
- Because F1a did not materially reduce poll waits, do not treat it as proof
  that F1b no-wait pipelining is safe or sufficient. It instead strengthens
  the need for Finding 4's `CLOCK_THREAD_CPUTIME_ID` attribution before the
  larger sync change: we need to distinguish CPU-busy render work from
  blocked submission wait per frame.

Next:

- Keep F1a. It removes a strictly broader wait and did not break validation.
- Implement Finding 4's thread CPU-time attribution next, then retry the
  wait-path experiments with wall-vs-CPU evidence.
- In parallel priority terms, the actor buffer lifetime fix remains the
  strongest concrete spike fix because actor-enabled worst frames are still
  dominated by per-frame actor resource churn.

**F1b — stop waiting for eye completion at all (pipelining).** Submit left
eye, submit right eye, release swapchain images, end frame; let `xrWaitFrame`
pace the loop. Run-ahead is bounded to one frame by the frame loop itself.
Invariants to verify (this is the `107` lesson — the reverted single-submit
optimization failed on uniform lifetime):

- every per-view uniform/buffer slot ring must be deep enough for
  eyes x frames-in-flight, not just the two eyes of one frame;
- terrain section upload buffers must not be written while a prior frame's
  submission may still read them (wgpu tracks `queue.write_buffer` ordering,
  so uploads through the queue are safe; anything mapped or reused manually is
  the risk surface);
- validate visually on-headset — any flicker/tearing in eye output means a
  sync gap, revert and diagnose before retrying.

Expected effect: several ms of average `app_work_ms` reduction (the current
per-frame wait cost is the GPU's ~3 ms plus scheduling latency, paid 2-3x),
and the `terrain_poll_wait` / `stereo_poll_wait` component of the
streaming-transition bad-frame family shrinks toward zero. This is the
single largest *average* lever visible in the current data, and average
headroom is what the lane now lacks.

If F1b proves out, the deferred-overlap prefetch mode and the
`wait_after_submit` flag can likely be retired rather than tuned — the
overlap becomes the default shape instead of an opt-in.

## Finding 2: the actor pass allocates fresh GPU buffers per eye, per frame

`ActorDrawResources::render_in_slot`
(`native/crates/mclone-render/src/entity.rs:356-435`):

- rebuilds the actor mesh on the CPU (`actor_mesh(...)`, line 373) — once per
  **eye**, so twice per frame for the same world state;
- creates a brand-new vertex buffer and index buffer via
  `device.create_buffer_init` (lines 391-400) — again per eye — and drops the
  previous frame's buffers to deferred destruction. The multiview path does
  the same (lines 469-475).

That is four fresh Vulkan buffer+memory allocations per frame plus four
deferred frees, forever, for content that changes only by animation pose.
This is the classic mechanism behind sporadic 20 ms stalls that look like
"actor encode": buffer/memory allocation occasionally takes a slow path in
the driver or in wgpu's allocator bookkeeping, and deferred frees pile into
the maintain/poll calls (which Finding 1's device-idle waits then absorb —
the two findings compound). `130`'s skip-actors probe already proved the
family is real (`max 37.326 → 26.879 ms`, app-over-period `57.2% → 36.2%`)
and that only 2 actors were present — `22 ms` is not per-actor CPU volume.

Note the fix pattern is already in this function: the uniform write goes
through a slotted per-view ring (`self.renderer.uniforms.write_slot`,
line 386). Extend the same ownership to geometry:

- build `actor_mesh` once per frame and share it across both eyes (the mesh
  is view-independent; only the uniforms differ per eye);
- keep persistent, grow-only vertex/index buffers (per view slot if needed
  for F1b's in-flight invariant) and update them with `queue.write_buffer`;
- expected: actor bucket becomes sub-ms steady, and the rank-1 actor spike
  family disappears with actors enabled. Verify with the same eye-split
  worst-frame lane; `--xr-skip-actors` remains the control.

This also generalizes: audit other per-frame `create_buffer_init` /
`create_texture` sites on the frame path (selection outline, world lines,
overlays) for the same churn. Per-frame GPU resource creation on the render
thread should be treated as a defect class, not a style choice.

## Finding 3: endorse the dirty-mark decoupling, add pacing and a resident-slot endgame

`130`'s locomotion-command split correctly showed the ~5 ms
`commit_server_command` tail is `apply_dirty_mark_ms=3.9-4.1` for an
opportunistically drained batch of 15 chunk snapshots + 15 unloads, not queue
send. The planned fix (pose commands stop draining/applying server updates;
the normal runtime poll owns that) is right. Three additions:

1. **Pace the application, don't just move it.** Applying 30 chunk-scale
   updates in one frame will produce the same spike inside the runtime poll
   bucket. Vanilla never sees this shape locally: chunk packets arrive spread
   across network ticks and are applied as received. The integrated server
   removed that natural pacing. Apply at most K snapshot/unload updates per
   frame from the drained queue (K measured; start ~4) — this is
   vanilla-shaped pacing recovered, not a new invented policy.
2. **Check interest hysteresis.** Every worst frame showed exactly
   15 loads + 15 unloads — the orbit is swapping a full chunk row each
   boundary crossing. If the interest region has no hysteresis margin, a path
   that oscillates near a boundary will thrash load/unload. Verify the
   unload margin is wider than the load margin (vanilla keeps chunks resident
   beyond the strict view distance and unloads lazily).
3. **The endgame is the resident-slot dirty bit.** `apply_dirty_mark` costing
   ~4 ms for 30 updates is BTreeSet-insertion churn (mark chunk neighborhood
   dirty = hundreds of ordered-set inserts). In Java, `setDirty` is a boolean
   store on a resident `RenderChunk` slot. This is the same signature as
   `130` E5's per-frame deferred rescan — both are costs of moving keys
   between sets instead of flipping state on resident slots. It strengthens
   the case for `128`'s resident-lifecycle refactor as the shared fix for
   marking, readiness, and deferral rather than three local optimizations.

## Finding 4: attribution — split CPU-busy from blocked, and add percentiles

Two cheap harness upgrades, in the spirit of `130` E8, that the next slices
need:

1. **Per-frame thread CPU time.** `app_work_ms = frame_wall_ms -
   wait_frame_ms` still counts GPU/compositor blocking (Finding 1) as "app
   work". Read `CLOCK_THREAD_CPUTIME_ID` at frame start/end (one
   `clock_gettime` pair, negligible cost) and log `cpu_busy_ms` beside the
   wall buckets in the summary and worst-frame snapshots. `wall - cpu_busy`
   inside the frame = blocked time. This makes "we do too much work" vs "we
   wait too long" unambiguous per frame, and it is the direct success metric
   for F1a/F1b.
2. **Per-stage percentiles.** The worst-frame ring answers tails; the
   remaining deficit is the *average*. Add p50/p95 per major stage (terrain
   runtime, eye CPU, poll waits, locomotion) so the composition of the
   ~14-15 ms typical frame is visible, not only its worst outliers.

## Finding 5: finish `130` E2b — broadened to all background threads

The post-fix trace shows the compile workers are no longer meaningful CPU
peers (~0.4 s over 20 s each), but `mclone integrat` ran ~7.5 s, plus
`mclone-light-st` and `mclone-worldgen`. The niceness slice should land as a
shared background-thread-class policy — one spawn-site helper in
`mclone-app-runtime` that tags threads (render-critical vs gameplay vs
background-batch) and applies platform policy (nice on Android; no-op default
on desktop) — covering the integrated server tick, worldgen, light store,
compile workers, and the dispatcher pump together, rather than a
compile-worker-only flag. With the render thread now registered via
`XR_KHR_android_thread_settings`, this is defense in depth, but on a
core-constrained device the server tick colliding with the frame loop is
still live risk, and it will grow at RD10.

## Recommended order

1. **F1a** (delete the device-idle poll) — landed and measured; correct
   cleanup, but no material RD7 performance win.
2. **Finding 4** attribution (thread CPU time + percentiles) — makes
   everything after it measurable.
3. **Finding 2** actor buffer lifetime fix — kills bad-frame family 1.
4. **Finding 3** decoupling + K-per-frame pacing — kills family 2's CPU half.
5. **F1b** no-wait pipelining — recovers the average headroom; family 2's
   wait half.
6. **Finding 5** background-thread policy, then re-run `130` E3/E4
   (budget-knob and worker-count baselines) in the now-clean lane, then
   proceed to `130` E5/E6 (event-driven deferral, visibility-first
   admission) which remain the correct terrain-pipeline destination.

Each step: standard RD7 lane, three runs, compare medians, one change at a
time. Keep `--xr-skip-actors` as the control for step 3.

## What not to do

- Do not tune upload/accept budget values before F1a/F1b land — the poll
  waits currently sit inside the same frames those budgets try to shape, and
  they will confound the comparison.
- Do not treat `stereo_poll_wait`/`terrain_poll_wait` as GPU cost. Meta
  metrics already show the GPU under 30% utilized; these are synchronization
  stalls, and the fix is removing the sync, not reducing GPU work.
- Do not fix the actor spike by capping/skipping actors in product lanes;
  the mechanism (per-frame buffer allocation) will reappear with every new
  per-frame-allocated renderer. Fix the lifetime pattern.

## Success criteria

- `cpu_busy_ms` vs blocked split exists in summaries and worst frames.
- Average `app_work_ms` in the standard lane drops clearly below the
  13.889 ms budget (target: p95 under budget), driven by measured reductions
  in poll-wait blocked time.
- With actors enabled, no worst-frame snapshot attributes >2 ms to the actor
  pass across three runs.
- Streaming-transition worst frames no longer contain multi-ms
  `apply_dirty_mark` or opportunistic update application inside the
  locomotion phase, and chunk-update application cost appears as bounded
  per-frame work under runtime attribution.
- Meta dropped frames over three-run medians improve from the current ~12-15
  baseline, and `130` E3-E6 proceed against a lane whose noise floor is
  small enough to rank policies.
