# 130: Quest Thread Scheduling And Streaming Tail Attribution

Status: active — E2a/E2c and E8 landed; trace processor readout available;
next attribution target is per-eye encode and camera-command send tails
Workstream: Android XR / Quest frame pacing, shared native runtime threading,
terrain streaming tails

## Impetus

This doc is the output of a fresh-eyes review of the render-distance-7 Quest
frame-drop work tracked in `119`, `120`, and `128`. It does not replace the
Java-shape convergence plan in `128`; it argues that the plan is currently
being executed and measured on top of an unaddressed platform-level problem —
CPU thread scheduling on Quest — and that several of the recent measured
results are best explained by render-thread preemption rather than by the
intrinsic cost of the code inside the timed buckets.

Known context this doc takes as given (do not re-litigate):

- Steady-state (fully compiled, no streaming) orbits at RD7 do not drop
  frames. The GPU floor is roughly 8-10 ms and steady-state CPU work is low.
  The drops happen only while walking around loading/generating/compiling
  chunks. The problem is therefore entirely the dirty-to-drawable streaming
  window, not the render floor.
- Multiview was already tried (`107`). The production-style full-frame
  RD10 comparison was effectively flat (`13.916 ms` per-eye avg vs
  `13.842 ms` multiview avg). Do not propose multiview as the next lever.
- The `2 / 16 / 64` budget lane from `128` is the current measurement lane:
  Quest 3, Android XR per-eye, RD7 settled orbit, 2 render compile workers,
  45 seconds, via `android-xr/validate-quest-openxr.sh`.

## 2026-07-02 Implementation Update

Landed in this slice:

- Android XR enables `XR_KHR_android_thread_settings` when the Quest runtime
  advertises it and registers the OpenXR frame-loop thread as
  `RENDERER_MAIN` after session creation
  (`native/apps/mclone-android-xr-client/src/lib.rs`).
- The shared native render-section compile dispatcher no longer wakes every
  1 ms while idle. The queue now has a dispatcher condvar; enqueuing a
  pending compile request wakes the dispatcher, and shutdown wakes both the
  dispatcher and worker condvars
  (`native/crates/mclone-app-runtime/src/render_assets.rs`).

Not landed yet:

- E2b worker/dispatcher niceness. That still needs an explicit shared setting
  and A/B lane, not an implicit behavior change.
- Perfetto frame markers. Trace processor is installed and usable, but the app
  still does not emit per-frame trace slices that can align `android_main`
  scheduler state directly to an app frame index.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime render_compile`
  passed.
- `node ./scripts/run-native-bash.mjs ./android-xr/build-apk.sh` passed.
- Quest logcat confirmed `XR_KHR_android_thread_settings` was advertised and
  the renderer-main thread was registered in both post-change runs:
  `tid=25404`, `tid=26140`.

Measured standard lane, single baseline plus two post-change repeats:

| Run | Meta dropped frames | Frame avg / p95 / p99 / max (ms) | >2x / >4x budget | App over period | Runtime ready sections max | Ready publish max | Submit max | Notes |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Baseline, pre-slice | 36 | 15.840 / 18.150 / 22.771 / 61.669 | 12 / 1 | 82.1% | 36.665 ms | 15.774 ms | 5.572 ms | Trace captured during this run |
| E2a+E2c run 1 | 13 | 15.591 / 17.756 / 20.335 / 26.013 | 0 / 0 | 83.3% | 1.378 ms | 1.564 ms | 0.948 ms | Thread registered |
| E2a+E2c run 2 | 14 | 15.255 / 17.609 / 20.034 / 25.876 | 0 / 0 | 74.4% | 1.401 ms | 1.655 ms | 1.034 ms | Thread registered |

Interpretation:

- This strongly supports the scheduling-tail thesis: the extreme unrelated
  bucket tails collapsed on repeat runs, and Meta dropped frames fell from
  36 to 13/14. The old >2x/>4x budget spikes disappeared.
- It does **not** make RD7 a stable 72 Hz lane. Average app work is still
  ~15.1-15.5 ms against a 13.889 ms budget, and 74-83% of frames still exceed
  the period. This slice reduced severe streaming tail damage; it did not
  remove the steady per-frame app-work deficit.
- CPU utilization also moved in the expected direction on the sampled Meta
  counters: average CPU utilization fell from 71.9% baseline to 63.1%/66.4%
  post-change. Treat that as supporting context, not a primary result.

Current next step: use the coherent worst-frame snapshots to split the
remaining large terrain eye-encode spans. The latest snapshots show that the
old submit/ready/publish tails are no longer the top problem.

## 2026-07-02 Follow-Up: Trace Processor And Worst-Frame Logging

Trace processor install attempt:

- Installed the official Perfetto wrapper with:
  `mkdir -p ~/.local/bin && curl -L -o ~/.local/bin/trace_processor https://get.perfetto.dev/trace_processor && chmod +x ~/.local/bin/trace_processor`.
- First run succeeded and downloaded the native cached binary under
  `~/.local/share/perfetto/prebuilts/trace_processor_shell-*`.
- Smoke query succeeded:
  `~/.local/bin/trace_processor query /tmp/mclone-rd7-baseline.pftrace "select count(*) as thread_count from thread;"`
  loaded the 17.97 MB trace and returned `thread_count=589`.

Baseline trace readout, first pass:

- App process in the trace: `pid=24244`, process
  `com.kzahel.mclone.xr`.
- Important app threads found:
  `android_main` (`tid=24269`), three truncated `mclone-render-c*`
  threads (`tid=24345`, `24346`, `24347`), `mclone integrat`
  (`tid=24348`), `mclone-light-st` (`tid=24350`), plus OVR/runtime/audio
  helper threads.
- Running time over the ~20 s trace:
  `android_main` ~10.790 s, `mclone integrat` ~6.746 s,
  `mclone-light-st` ~0.219 s, render compile/dispatch threads ~0.153 s,
  ~0.189 s, and ~0.201 s.
- `android_main` runnable-not-running time totaled ~177 ms. The largest
  individual runnable gaps were ~3.035 ms, 2.562 ms, and 2.043 ms.

Interpretation:

- The trace processor path is now usable locally.
- Baseline trace confirms scheduler pressure exists, but this first readout
  does not prove the 61 ms bad frame by itself because the app did not emit
  per-frame trace slices. The new E8 app-side worst-frame snapshots should
  provide coherent per-frame bucket attribution, and a later trace slice can
  align scheduler state to those frames if we add trace markers.
- The baseline also shows `mclone integrat` is a major on-device CPU peer
  during the trace window, not just the render compile workers. Any future
  worker-priority or backpressure work should include local server/scheduler
  threads in the thread inventory.

Worst-frame logging implementation attempt:

- Added a top-5 app-work worst-frame ring in the Android XR perf harness.
- Ranking key is `app_work_ms = frame_wall_ms - wait_frame_ms`, so legitimate
  `xrWaitFrame` blocking does not make a frame look expensive.
- At perf summary time each ranked frame logs four correlated lines with the
  same `rank` and `sample_frame`:
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_TERRAIN`,
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_RUNTIME`, and
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_UPLOAD`.
- Android target compile passed:
  `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`.
- Android XR APK build passed:
  `node ./scripts/run-native-bash.mjs ./android-xr/build-apk.sh`.

Standard RD7 lane with worst-frame logging:

- Command: standard settled-orbit RD7 lane with 2 render compile workers and
  `2 / 16 / 64` budgets.
- Summary path:
  `/tmp/mclone-quest-openxr-perf-orbit-rd7-worst-frame-snapshots-2-16-64.txt`.
- Logcat path:
  `/tmp/mclone-quest-openxr-perf-orbit-rd7-worst-frame-snapshots-2-16-64-logcat.txt`.
- Result: Meta dropped frames `15`, frame avg/p95/p99/max
  `15.451 / 17.795 / 20.216 / 28.073 ms`, `over_2x_budget=1`,
  `app_work_avg_ms=15.325`, `app_over_period_pct=80.6`.
- Note: this APK was built while unrelated local `mclone-ui` debug-overlay
  worktree changes were present but inactive in this lane. Do not include
  those files in this scheduling/perf commit.

Worst-frame snapshot readout, rank 1:

- `sample_frame=1857`, `frame_wall_ms=28.073`, `app_work_ms=28.038`,
  `headroom_ms=-14.149`, `over_2x_budget=true`.
- High-level stages: `render_mclone_frame_ms=27.850`,
  `locomotion_ms=5.080`, `wait_frame_ms=0.035`. This was real app work, not
  `xrWaitFrame` blocking.
- Terrain: `terrain_frame_ms=22.431`, `runtime_upload_ms=6.078`,
  `shared_records_ms=1.143`, `left_eye_ms=5.952`, `right_eye_ms=8.437`,
  `stereo_poll_wait_ms=9.470`.
- Runtime: `sync_ms=2.915`, `prepare_ms=1.865`, `submit_ms=0.927`,
  `submit_snapshot_ms=0.850`, `submit_handoff_ms=0.076`,
  `ready_sections_ms=0.608`, `ready_publish_ms=0.946`. The previous
  giant submit/ready/publish tails are not present in this worst frame.
- Upload/admission state: `submitted_sections=32`,
  `request_target_sections=32`, `request_target_sections_single=16`,
  `request_payload_bytes=622304`, `dispatcher_pending_jobs=2`,
  `dispatcher_queued_compile_tasks=1`, `upload_removed_sections=19`,
  `accept_limited=true`.

Post-fix Perfetto capture:

- Captured during the run at `/tmp/mclone-rd7-post-scheduling.pftrace`
  (`16M`, 20 seconds).
- Post-fix trace app process: `pid=28211`, `android_main tid=28234`.
- Running time over the trace: `android_main` ~10.933 s,
  `mclone integrat` ~7.464 s, `mclone-light-st` ~1.390 s,
  `mclone-worldgen` ~0.665 s, render compile/dispatch threads ~0.002 s,
  ~0.395 s, and ~0.362 s.
- `android_main` runnable-not-running time totaled ~50 ms. The largest
  individual runnable gaps were ~0.563 ms, 0.345 ms, and 0.301 ms.
  This is much lower than the baseline trace's ~3.035/2.562/2.043 ms top
  gaps.

Interpretation after E8 + post-fix trace:

- The earlier E2a/E2c scheduling slice seems to have done its job: in the
  post-fix trace, multi-ms render-thread runnable gaps are no longer the
  obvious primary problem.
- The remaining worst frames are coherent now. They show broad app-side work:
  locomotion, terrain runtime upload/sync/prepare, eye rendering, and
  `stereo_poll_wait_ms`, while submit/handoff/ready-publish are small.
- The next likely lever is not another queue transport tweak. It is either
  reducing/smoothing the real per-frame work visible in the worst-frame
  snapshots, or adding trace markers so Perfetto can align `android_main`
  scheduler slices with app frame indices directly.
- Because `mclone integrat`, `mclone-light-st`, and `mclone-worldgen` are
  large CPU peers in the post-fix trace, future "worker niceness" or
  backpressure work should include local server/worldgen/light-store threads,
  not only render compile workers.

## 2026-07-02 Follow-Up: Locomotion And Budget Attribution

Implementation:

- Added `XrLocomotionTiming` in `mclone-xr-scene` and threaded it through the
  Android XR perf loop.
- New fields split the broad `locomotion_ms` bucket into input/setup,
  camera-apply, camera/player-pose commit, pose command send, position-update
  drain, interest-center update, and gameplay interaction.
- Worst-frame logs now also include
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_BUDGET`, which derives a coherent
  per-frame split between known render-frame work, unattributed render time,
  terrain work before the final poll wait, final terrain poll wait, eye CPU
  work, and eye poll wait.

Validation:

- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- `pnpm native:android-xr:perf:orbit:rd7:metrics` passed and wrote
  `/tmp/mclone-quest-openxr-perf-orbit-rd7-metrics.txt`.
- The apples-to-apples budgeted lane passed:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-quest-openxr-perf-orbit-rd7-attribution-2-16-64.txt --log /tmp/mclone-quest-openxr-perf-orbit-rd7-attribution-2-16-64-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.

Measured results:

| Run | Workers / budgets | Meta dropped frames | Frame avg / p95 / p99 / max (ms) | App work avg / p95 / max (ms) | App over period | Notable max buckets |
| --- | --- | ---: | --- | --- | ---: | --- |
| Stock RD7 orbit metrics | 1 / unbounded | 9 | 15.414 / 17.672 / 19.622 / 34.540 | 15.265 / 17.594 / 34.513 | 78.4% | `left_eye_encode=20.062`, `stereo_poll_wait=9.593`, `commit_server_command=5.994` |
| RD7 orbit attribution | 2 / 16 / 64 / completed-accept 2 | 13 | 15.518 / 17.843 / 20.361 / 44.086 | 15.346 / 17.738 / 44.055 | 78.7% | `left_eye_encode=29.429`, `stereo_poll_wait=9.501`, `commit_server_command=5.586` |

Budgeted-lane coherent worst-frame readout:

- Rank 1 (`sample_frame=659`) was not a streaming submit or locomotion frame:
  `frame_wall_ms=44.086`, `app_work_ms=44.055`,
  `render_mclone_frame_ms=43.916`, `locomotion_ms=0.040`.
  The terrain split was `terrain_before_poll_wait_ms=36.204` and
  `terrain_poll_wait_ms=7.001`; eye CPU was `31.961 ms`.
  The left eye alone reported `left_eye_ms=33.253` and
  `left_encode_ms=29.429`, while runtime submit/sync/upload were zero in that
  frame.
- Rank 2 was the same shape at lower magnitude:
  `frame_wall_ms=34.759`, `terrain_before_poll_wait_ms=26.454`,
  `terrain_poll_wait_ms=7.788`, `left_encode_ms=19.778`, and no runtime
  submit/sync/upload.
- Ranks 3-5 are the streaming-transition shape. They combine
  `locomotion_ms=4.459-5.595`, `terrain_poll_wait_ms=6.784-9.079`, and
  runtime sync/upload/prepare work. The new locomotion split shows the
  locomotion spike is almost entirely
  `commit_server_command_ms=4.452-5.586`, not interest-center math
  (`commit_interest_ms=0.001`) or camera application (`0.002`).
- In rank 3, the budgeted pipeline submitted two compile requests:
  `submitted_sections=32`, `request_target_sections_single=16`,
  `request_snapshots=10`, `request_snapshot_sections=60`,
  `request_payload_bytes=552416`, `dispatcher_pending_jobs=2`.
  Even there, compiler handoff itself was small
  (`compiler_ms=0.021`, `command_send_ms=0.019`).

Interpretation:

- The remaining RD7 problem is not one single bucket. There are at least two
  distinct bad-frame families:
  1. pure eye-encode frames where runtime streaming work is absent and one eye
     spends 20-30 ms in CPU-side encode;
  2. streaming-transition frames where runtime work is moderate, the final
     poll wait is 7-9 ms, and the camera/player-pose command send can burn
     ~5 ms.
- The broad `locomotion_ms` suspicion is now narrowed: the expensive part is
  `send_gameplay_command` during camera/player-pose commit. Interest update is
  not the cause.
- Meta performance metrics during both runs still report low app GPU time
  (`2.923-3.533 ms`) and modest GPU utilization (`23.5-29.0%`). Treat
  `stereo_poll_wait_ms` as a final submission/fence wait that may include
  driver/runtime scheduling effects, not as proof that terrain shaders are
  consuming 7-9 ms of GPU time.

Follow-up target from this result:

- Split per-eye encode in the per-eye path the same way the multiview summary
  already splits `sky`, `terrain`, `actor`, `screen_effect`, and
  `world_overlay` work. The current `left_encode_ms` / `right_encode_ms`
  number is too broad: `left_eye_section_encode_ms` is only ~1.8 ms in the
  worst frame, so the missing 20-30 ms is outside section command encoding.
  This was implemented in the next slice below.
- In parallel or immediately after, add a narrow timer around
  `runtime.send_gameplay_command(report.command)` in the camera commit path so
  we know whether the ~5 ms command-send tail is queue contention,
  downstream server pressure, or another scheduling point.

## 2026-07-02 Follow-Up: Per-Eye Encode Split

Implementation:

- Extended `FullFrameRenderTiming` with coarse pass timings for sky/clear,
  far LOD, terrain opaque, terrain translucent, actors, screen effects, and
  GUI.
- Extended `XrTerrainEyeRenderTiming` and Android XR worst-frame logs with
  left/right eye splits for the shared full-frame renderer plus XR-only
  fade, selection, world-line, world-panel, and encoder-finish work.
- New log lines:
  `MCLONE_ANDROID_XR_PERF_TERRAIN_EYE_SPLIT` for max buckets and
  `MCLONE_ANDROID_XR_PERF_WORST_FRAME_EYE_SPLIT` for coherent worst frames.

Validation:

- `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`
  passed.
- Budgeted RD7 lane passed:
  `node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-quest-openxr-perf-orbit-rd7-eye-split-2-16-64.txt --log /tmp/mclone-quest-openxr-perf-orbit-rd7-eye-split-2-16-64-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time`.

Measured result:

| Run | Workers / budgets | Meta dropped frames | Frame avg / p95 / p99 / max (ms) | App work avg / p95 / max (ms) | App over period | Notable max buckets |
| --- | --- | ---: | --- | --- | ---: | --- |
| RD7 eye-split attribution | 2 / 16 / 64 / completed-accept 2 | 13 | 14.654 / 16.716 / 19.662 / 37.326 | 14.206 / 16.566 / 35.099 | 57.2% | `right_eye_encode=23.650`, `right_actor=22.117`, `stereo_poll_wait=8.424`, `commit_server_command=8.592` |

Coherent worst-frame readout:

- Rank 1 (`sample_frame=638`) was the same bad-frame family as the previous
  eye-encode spike, but the split now attributes it:
  `right_encode_ms=23.650`, `right_full_frame_ms=23.026`, and
  `right_actor_ms=22.117`. Terrain was not the spike:
  `right_opaque_ms=0.532`, `right_translucent_ms=0.172`,
  `right_sky_ms=0.164`, `right_encoder_finish_ms=0.002`.
  Runtime submit/sync/upload were zero.
- Rank 2 repeated the same shape:
  `right_encode_ms=18.876`, `right_full_frame_ms=18.268`,
  `right_actor_ms=17.444`, with terrain and sky sub-ms.
- Ranks 3-5 were the streaming-transition family. Their eye splits were
  ordinary (`left_full_frame_ms=1.347-1.434`,
  `right_full_frame_ms=1.295-1.703`), while the expensive pieces were
  `commit_server_command_ms=5.189-8.592`, runtime sync/upload/prepare, and
  `stereo_poll_wait_ms=6.077-8.025`.

Interpretation:

- The 20-30 ms "eye encode" mystery is now narrowed to actor rendering, not
  sky, terrain section encoding, overlays, GUI, or encoder finish.
- Only two actors were present (`actors=2`, `drawn_actors=2`), so
  `right_actor_ms=22.117` is not normal per-actor CPU volume. It is either a
  stall/preemption point inside actor rendering or a specific actor-renderer
  path doing unexpectedly blocking work.
- The run average improved versus the prior budgeted sample, but the lane is
  noisy. Treat the attribution as the result, not a proven performance win.

Next implementation target:

- Split `ActorDrawResources::render_in_slot` on the XR per-eye path into its
  own smaller timings: per-actor preparation, uniform/buffer writes, render
  pass setup, draw calls, and any texture/material binding work. If the whole
  actor pass is only a scheduling victim, a Perfetto frame marker around this
  pass should show `android_main` runnable/not-running during the spike.
- Add the narrow `send_gameplay_command` timer from the prior section, since
  the locomotion-command tail is still present and reached
  `commit_server_command_ms=8.592` in this run.

## Central Thesis

The frame-breaking tails during streaming are not (only) the work inside the
buckets that report them. A large share is the render thread losing the CPU —
being preempted or migrated — while it happens to be inside whatever bucket
then eats the blame. Before the 2026-07-02 E2a/E2c slice, the application did
not tell the Quest OS which threads matter:

- There was no `XR_KHR_android_thread_settings` registration anywhere in the
  tree. The frame-loop thread was not registered as a renderer/performance
  thread. E2a has now changed this for Android XR when the runtime advertises
  the extension.
- There is no `XR_EXT_performance_settings` / CPU-GPU level usage.
- Render compile workers and the dispatcher pump spawn at default priority
  with no niceness or affinity:
  `native/crates/mclone-app-runtime/src/render_assets.rs:319-345`.
- The dispatcher pump used to idle on a 1 ms sleep-poll
  (`render_assets.rs:32`, `render_assets.rs:459-469`) — a thread waking
  1000x/sec on a device with very few performance cores. E2c has now changed
  this to a blocking queue-owned condvar.

On Quest, unregistered threads are ordinary `SCHED_OTHER` peers. A compile
worker that wakes with a ~16-section, ~300 KB request can preempt the render
thread or push it off its core mid-frame, and it stays busy for a long burst.

### Evidence from measurements already in `128` and `120`

Every row below is a previously measured result. Read them together:

| Evidence | Measured value | Why it points at scheduling |
| --- | --- | --- |
| `notify_one` on the render thread (wakeup-order experiment, `128`) | `notify_single_ms = 15.790` | Waking a thread costs microseconds. 15.8 ms inside `notify_one` means the render thread lost the CPU at the wake point — the woken worker (or something else) ran instead. |
| Post-enqueue residual before the unlock reorder (`128`) | `post_enqueue_single_ms = 17.958` | The measured span is mutex-guard drop plus function return — effectively zero work. Only descheduling explains 18 ms. |
| `submit_snapshot_ms` across runs (`120`/`128`) | `0.859`–`0.979` ms for many runs, then `11.037`, `12.748`, `32.169` ms in the recent coordinator runs | The snapshot code (clone ~5 chunk columns) did not change between those runs. A fixed CPU job whose measured cost varies 30x across runs is being interrupted, not slow. |
| Tail bucket migrates run-to-run (`128`) | worst bucket = command send, then ready sections (`24.015`), then upload apply (`19.191`–`20.588`), then submit snapshot (`32.169`), then eye encode (`24.578`–`39.845`) | A tail that hops between unrelated code spans has an external cause. Preemption lands wherever the render thread happens to be. |
| Dispatcher pump result (`128`) | render-frame `command_send_single_ms` dropped `15.791 → 0.003`, but frame pacing did not improve; tails reappeared downstream (upload apply `20.418`, ready sections `18.992`, left-eye encode `39.845`) | Removing the wakeup from the render frame removed that bucket's tail — and the *same magnitude* of tail immediately surfaced in other buckets. Consistent with the preemption source (busy workers) being untouched. |
| `mpsc` → `sync_channel` swap made it worse; slot queue helped only ~2 ms; queue-internal buckets (lock wait, slot write, push) all ~0 (`128`) | multiple runs | Three different queue implementations, same tail. The queue was never the cost. |
| Neutral diagnostic changes moved Meta dropped frames `62 → 75 → 64` (`128`) | run-to-run | The lane's noise floor is larger than most effects being measured. Scheduling nondeterminism is a classic source of exactly this variance. |
| Multiview flat (`107`) | per-eye `13.916` vs multiview `13.842` avg | If eye-encode tails were intrinsic command-recording volume, halving encoded views should have shown up. It did not — the encode bucket tails are inflated by something other than encode work. |
| Steady-state orbits clean (user-confirmed) | no dropped frames | The floor fits. Drops correlate with exactly the window where compile workers are busy — i.e., when there are peer threads to lose the CPU to. |

None of these is individually conclusive. Together they justify spending one
trace run and one small slice before any further pipeline restructuring.

### What this reframes

The `128` roadmap (dispatcher ownership, held capacity, upload coordinator,
frame decision) is good architecture and should stay. But its A/B measurements
were all taken in a lane where the render thread can silently lose 10-30 ms to
the scheduler. That means:

- Recent "no performance win" results (dispatcher boundary, shared
  coordinator, frame decision) may be understating wins.
- Recent budget-knob comparisons (`1/8/16` vs `2/16/64` vs unbounded) are
  within the contaminated noise floor and should be re-run after the
  scheduling fix.
- Do not conclude anything further from single 45-second runs until the
  variance source is addressed.

## Experiments

Ordered by information-per-effort. E1 and E2 come first and are cheap. E3+
depend on their outcome.

### E1: Perfetto scheduler trace of the standard lane (no code change)

Goal: directly observe, instead of infer, what the render thread is doing
during the worst frames.

Method:

- Run the standard RD7 settled-orbit lane. While it runs, capture ~20 s of
  scheduler trace from the headset:

  ```
  adb shell perfetto -o /data/misc/perfetto-traces/mclone.pftrace -t 20s \
      sched freq idle
  adb pull /data/misc/perfetto-traces/mclone.pftrace /tmp/
  ```

  Open in `ui.perfetto.dev`.

Read out, in priority order:

1. Find the frame-loop thread (the thread calling `xrWaitFrame`; check thread
   names in the app process). For the long frames: how much time is it in
   state Runnable (wants CPU, not running) vs Running vs Sleeping? Runnable
   gaps of multiple ms during a frame = preemption confirmed.
2. During those Runnable gaps, which threads occupy the CPU the render thread
   last ran on? Expect `mclone-render-compile*` workers and/or the
   `mclone-render-compile-dispatch` pump if the thesis is right.
3. Distinguish preemption from lock/allocator contention: futex-wait segments
   on the render thread point at allocator or mutex contention (see E7);
   Runnable-not-running segments point at scheduling.
4. Inventory every thread in the process during the lane. In particular:
   what server/worldgen/network/audio threads run on-device in this lane, how
   many, and how busy? The compile workers may not be the only peers.
5. Check which core the render thread runs on and whether it migrates cores
   mid-frame.

Success: the worst frames get an unambiguous attribution. This single trace
either validates E2 before writing it, or kills the thesis cheaply.

### E2: Thread scheduling slice

Three small parts, one slice. All are measured against the standard lane.

**(a) Register the XR frame-loop thread via `XR_KHR_android_thread_settings`.**

- The openxr `0.21` crate already models this: `ExtensionSet` has
  `khr_android_thread_settings`, and `instance.exts()` exposes the raw
  `AndroidThreadSettingsKHR` fn pointers. The Android XR app uses `libc`
  only to read the current Linux thread id with `gettid`.
- Enable the extension at the existing enable site
  (`native/apps/mclone-android-xr-client/src/lib.rs:1143-1151`), gated on
  `available.khr_android_thread_settings`.
- After session creation, on the frame-loop thread, call
  `xrSetAndroidApplicationThreadKHR(session, RENDERER_MAIN, gettid())`.
  If a distinct application main/game-logic thread exists, register it as
  `APPLICATION_MAIN`. Meta's runtime uses these hints for core placement and
  priority of exactly these threads.
- This is genuinely platform glue and belongs in the Android XR app crate /
  XR host boundary — it is an allowed platform-local behavior under the
  shared-first policy, and desktop OpenXR simply doesn't enable the
  extension.

**(b) Deprioritize compile workers and the dispatcher pump.**

- At worker thread startup (`render_assets.rs:335-345`) and pump startup
  (`render_assets.rs:319-322`), set niceness for the calling thread:
  `libc::setpriority(libc::PRIO_PROCESS, 0, nice)` (on Linux/Android,
  `who = 0` targets the calling thread). Suggested first value: `10`.
- Ship it as a shared runtime setting (e.g.
  `--render-compile-worker-nice N`, default `0` = current behavior) so the
  lane can A/B it and desktop behavior is unchanged by default. The setting
  lives in `mclone-app-runtime` beside the worker-count setting — shared, not
  XR-local.
- Rationale: compile workers should soak idle core time and never win a
  scheduling contest against the render thread. This is an intentional,
  documented divergence from vanilla (Java's dispatcher threads run at
  default priority) justified by the platform: desktop Java has many cores
  and no compositor budget; Quest does not. It preserves parity of the
  *shape* (bounded dispatcher capacity) while changing only OS priority.

**(c) Fix the dispatcher pump idle wait.**

- Replace the 1 ms sleep-poll in `run_render_section_compile_dispatcher`
  (`render_assets.rs:459-469`) with blocking on a condvar signaled when a
  slot is published (and on shutdown). The queue already owns a `Condvar`
  (`render_assets.rs:84`); add a second wait channel for the pump or widen
  the existing one.
- Removes 1000 wakeups/sec of steady scheduler noise and idle power draw.

Measurement:

- Standard lane, three runs per config, compare medians (see E8):
  baseline vs (a) alone vs (a)+(b)+(c).
- Expected if the thesis holds: the migrating multi-ms tails
  (`submit_snapshot`, `ready sections`, `upload apply`, eye encode maxima)
  collapse toward their many-run baseline values (~1 ms for snapshot);
  Meta dropped frames and `over_2x_budget` improve; run-to-run variance
  shrinks noticeably.
- Also expected: p99 improvement even in the unbudgeted (drain-all) policy,
  since preemption hits that path too.

Rollback rule: everything is behind an extension-availability check and a
default-off nice setting. If the lane is flat across three-run medians, keep
(c) (it is correct regardless), keep (a) (it is free and standard practice),
drop (b) back to default, and treat the preemption thesis as weakened — then
E5/E6 become the front of the queue.

### E3: Re-baseline the budget knobs after E2

All prior accept/upload budget comparisons were taken in the contaminated
lane. After E2 lands, re-run: unbounded (Java drain-all baseline) vs
`2 / 16 / 64` vs `1 / 8 / 16`, three runs each, medians. It is possible the
Java-shaped drain-all baseline simply wins once workers can't stall the
render thread — which would let several opt-in knobs be retired instead of
tuned. That outcome would be a real simplification win for `128`.

### E4: Worker count re-test after E2

The `120` Slice B result (2 workers = better average, worse tails) is exactly
the signature preemption would produce: more throughput, more render-thread
contention. With workers niced, re-test 1 vs 2 vs 3 workers. If tails no
longer degrade with worker count, the streaming window shortens with more
workers for free, which directly reduces the number of frames during which
drops can happen.

### E5: Event-driven deferred readiness (stop the per-frame deferred rescan)

Independent of scheduling, this is steady-state waste in the streaming
window and the clearest remaining Java-shape divergence on the CPU side.

Current shape: `plan_ready_render_sections`
(`native/crates/mclone-render-session/src/lib.rs:1217-1280`) and
`plan_ready_render_section_key` (`lib.rs:3670-3691`) re-test neighbor
readiness for every dirty/deferred section every frame and rebuild fresh
`BTreeSet`s. The RD7 lane shows ~3,552 deferred sections re-scanned per frame
(`128` Slice A table), most of which are frontier sections whose neighbor
chunks will not arrive until the player moves. The `120` attribution pass
measured the prepare bucket as high as `14.181 ms` in a max frame.

Java's counterpart costs nothing per frame: a chunk whose neighbors are
missing is simply not scheduled — `hasAllNeighbors()` is checked when the
traversal encounters the chunk (`LevelRenderer.java:848-871`,
`ChunkRenderDispatcher.java:462-466`), and there is no global deferred set to
rescan.

Change: keep deferred sections in per-chunk buckets and re-evaluate a bucket
only when one of its neighbor chunks arrives, unloads, or is re-marked dirty
(event-driven readiness invalidation — already hypothesized as Shortfall 3 in
`128`). The per-frame planning loop should touch only chunks whose readiness
could have changed since last frame.

Measurement: deferred-section scan count per frame (new counter), prepare
bucket max, plus the standard lane numbers.

### E6: Visibility-first admission (Java parity, with an XR nuance)

Java builds `chunksToCompile` from the frustum-culled traversal — only
visible-ish dirty chunks are scheduled, nearest first
(`LevelRenderer.java:848-871`). Our admission plans from the global dirty
set. During a walk/orbit, sections behind the camera compete for compile
capacity with sections the player is about to look at.

Direction: admission priority should be traversal/visibility-first, distance
second, background dirty work last — rather than frustum-*only*, because XR
head rotation flips "behind" too quickly to never compile it. This also pairs
naturally with E5: the traversal is the natural "which chunks to re-check"
driver, replacing the global scan.

Also worth folding in here: Java compiles **one section per task**, while our
requests batch up to 16 sections and ~300 KB (`128` Slice A: the worst single
request targeted 16 sections, ~306 KB). Coarse requests mean long
uninterruptible worker bursts, coarse cancellation, and coarse priority.
Smaller (per-section or per-2-4-section) tasks make worker occupancy smoother
and make distance/visibility priority meaningful. After E2, measure request
granularity 16 vs 4 vs 1 in the standard lane.

### E7: Allocator check (secondary; only if E1 shows futex waits)

Compile workers allocate mesh buffers heavily while the render thread also
allocates (fresh `BTreeSet`s per frame, snapshot clones). If E1 shows the
render thread blocked in futex during allocation rather than preempted,
try `mimalloc` (or `jemalloc`) as the Android global allocator behind a
feature flag and re-measure. Do not do this before E1 — it is a shot in the
dark without the trace.

### E8: Measurement hygiene (do alongside E1/E2)

Two cheap changes that make every later A/B trustworthy:

- **Coherent worst-frame snapshots.** The current perf lines are per-field
  maxima across the run (`128` explicitly notes the max aggregator is not a
  single-frame snapshot), which makes cross-bucket attribution impossible —
  a 20 ms `upload apply` and a 24 ms `ready sections` in the same log line
  may be different frames. Keep a small ring of the N worst frames and log
  each one's complete bucket breakdown as one line at run end.
- **Three runs per config, compare medians.** The lane has shown ±15% swings
  in dropped frames on neutral changes. Single-run conclusions have already
  mis-ranked options once (see `128` Slice B interpretation caveats).

## What Not To Do Next

- No more queue/channel/slot transport variants. Three shapes measured the
  same tail; the transport was never the cost.
- No budget-knob tuning before E2 + E8 land. The comparisons are inside the
  noise.
- No further ownership extractions justified primarily by expected perf wins.
  Extractions are fine when they clarify ownership (they have), but the last
  three were performance-neutral, as `128` itself records — the next perf
  claim needs the clean lane first.
- Do not treat worker deprioritization as license to skip the `128`
  coordinator work. The Java-shaped upload/publication phases and E5/E6 are
  still the correct destination; E1/E2 exist to make their measurements mean
  something.

## Success Criteria

- E1 produces a definitive attribution for the worst-frame Runnable/Running
  split (preemption vs lock contention vs genuine work), documented here.
- After E2, the standard lane's bucket maxima stop migrating between
  unrelated buckets, `submit_snapshot_ms` max returns to ~1 ms, and
  three-run median variance shrinks.
- Meta dropped frames in the RD7 lane improve materially with E2 alone, or
  the thesis is explicitly falsified and this doc's status is updated so
  `128` can proceed without this question hanging over its lane.
- The budget-knob and worker-count decisions in `119`/`120`/`128` are
  re-validated (or retired) against the clean lane.
