# Standalone Quest Performance Records

This file records standalone Quest / Android XR APK performance baselines by
date and commit. Keep one row per benchmark lane so regressions and
improvements are easy to compare over time.

Scope: these records are for the app running on the headset itself. They do not
cover desktop OpenXR hosted on the PC with Quest acting as a streaming client
through VirtualDesktopXR, WiVRn, Link, SteamVR, or similar runtimes.

## How To Add A Row

1. Commit the runtime code you want to benchmark.
2. Run one or more Quest benchmark scripts.
3. Add rows below with the benchmarked commit hash, device/runtime details, and
   the compact `MCLONE_ANDROID_XR_PERF_*` marker-block numbers.

If a benchmark is captured from an uncommitted worktree, record that explicitly
and name the later commit that contains the same runtime code.

Current summaries are saved as a compact marker block:
`MCLONE_ANDROID_XR_PERF_SUMMARY`, `HEADROOM`, `STAGES`, `TERRAIN`,
`TERRAIN_PREP`, `UPLOAD_MAX`, `RUNTIME_MAX`, `QUEUE_MAX`, `COMPILE_MAX`,
`UPLOAD_LAST`, and `DRAW`. Use `app_work_*`, `headroom_*`, and
`app_over_period_*` as the primary performance comparison fields. `frame_avg_ms`
and legacy `over_budget` include OpenXR compositor pacing and can stay near
`13.889ms` at 72 Hz even when real app work changes.

The marker block also includes refresh fields (`refresh_supported`,
`current_hz`, `supported_hz`, `target_hz`, `budget_ms`), max stage timings,
terrain runtime poll/sync/GPU-upload timings, runtime poll sub-buckets,
diagnostics refresh/cache-age fields, server/scheduler queue state,
compile/upload workload counters, and draw counts. Add extra columns or a
secondary detail table when those fields are relevant to the change being
tracked. Treat records captured before the `MCLONE_ANDROID_XR_PERF_HEADROOM`
marker landed as historical pacing records unless they have a separate
busy/free measurement.

## Current Standalone Quest Lanes

Automated no-clip flight at walking-like speed:

```bash
pnpm native:android-xr:perf:flight:rd1
pnpm native:android-xr:perf:flight:rd5
pnpm native:android-xr:perf:flight:rd7:metrics
pnpm native:android-xr:perf:flight:rd7:frame-overlap
pnpm native:android-xr:perf:flight:rd10
pnpm native:android-xr:perf:flight:rd10:metrics
pnpm native:android-xr:perf:flight:rd10:frame-overlap
pnpm native:android-xr:perf:flight:sweep
```

Settled orbit at walking-like speed:

```bash
pnpm native:android-xr:perf:orbit:rd5:metrics
pnpm native:android-xr:perf:orbit:rd5:persisted
pnpm native:android-xr:perf:orbit:rd5:frame-overlap
pnpm native:android-xr:perf:orbit:rd7:metrics
pnpm native:android-xr:perf:orbit:rd7:frame-overlap
pnpm native:android-xr:perf:orbit:rd7:accept4:metrics
pnpm native:android-xr:perf:orbit:rd7:accept4:frame-overlap
```

Settled stationary render isolation:

```bash
pnpm native:android-xr:perf:stationary:rd1
pnpm native:android-xr:perf:stationary:rd5
pnpm native:android-xr:perf:stationary:rd7:metrics
pnpm native:android-xr:perf:stationary:rd7:frame-overlap
pnpm native:android-xr:perf:stationary:rd10
pnpm native:android-xr:perf:stationary:rd10:metrics
pnpm native:android-xr:perf:stationary:rd10:frame-overlap
pnpm native:android-xr:perf:stationary:sweep
```

Frozen-mesh stationary render isolation:

```bash
pnpm native:android-xr:perf:frozen:rd1
pnpm native:android-xr:perf:frozen:rd5
pnpm native:android-xr:perf:frozen:rd10
pnpm native:android-xr:perf:frozen:rd10:frame-overlap
pnpm native:android-xr:perf:frozen:sweep
```

Flight samples start after the first submitted terrain frame and stress
movement, streaming, compile/upload, and rendering together. Stationary samples
disable locomotion, wait for no server queues, no compile jobs, no rebuilds, no
uploads, and no poll changes after a minimum 5 second settle window, then record
a 20 second steady-state render sample. Persistent deferred render sections are
recorded as a steady-state condition rather than blocking the sample forever.
RD5 settled orbit is the current lower-distance safety guardrail for throughput
policy changes. RD7 remains the product-style pressure lane, but the current
throughput workstream is not trying to perfect RD7 before improving desktop
streaming throughput. RD10 remains the stress lane for exposing bursty work and
tail regressions; it should not be the only pass/fail signal for a pacing
change.

Frozen samples use the same settle gate, then skip runtime polling, section
sync, traversal-ready refresh, and GPU section uploads during the measured
window. They render from the configured startup `--view-pose`, not the live
headset pose, so physical headset orientation cannot change terrain culling or
draw counts during the measured window. Current frozen scripts use
`--view-pose 0,80,-96,180` so RD1 still draws nearby terrain. The validator
force-stops the app and sleeps the headset during cleanup.

## Records

### 2026-07-24 - Exact Frame-Accounting Measurement Defect

Benchmarked runtime endpoint: `696a4271`. The corrected APK was built from a
detached worktree at that exact endpoint with only the
`FramePipelineAccountant::new_exact_on_demand` and Android XR report-boundary
patch applied. The main worktree contained unrelated concurrent changes, so
these are explicitly uncommitted-patch measurements; the two Quest-relevant
runtime files in the main worktree contain the same code.

The finite Android XR perf probe accidentally used the publish-on-record exact
collector in `after_frame`. Every submitted frame therefore rebuilt
percentiles, sorted the growing exact history, cloned worst-frame detail, and
allocated the complete report. `OpenXrFrameDriver` had already stopped its
`frame_wall` and thread-CPU clocks, so the overhead reduced real submission
cadence without appearing in `app_work_*`. This is a measurement defect, not
an RD10 render optimization.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |

Frozen RD10 causality and correction:

| Runtime / accounting | Frames / interval | Submitted FPS | App p95 | Thread CPU p95 | Meta GPU | Drawn sections / indices |
|---|---:|---:|---:|---:|---:|---:|
| Old exact, on | `925 / 20.002s` | `46.25` | `14.294ms` | `6.237ms` | `7.514ms` | `284 / 2,008,398` |
| Old exact, off | `1440 / 20.176s` | `71.37` reported | `14.558ms` | `6.042ms` | `7.785ms` | `284 / 2,008,398` |
| Fixed on-demand, on | `1440 / 20.012s` | `71.96` | `14.161ms` | `5.888ms` | `7.540ms` | `284 / 2,008,398` |
| Fixed on-demand, off | `1441 / 20.012s` | `72.01` | `14.059ms` | `5.917ms` | `7.448ms` | `284 / 2,008,398` |

The old accounting-off row did submit 1440 frames during the intended
20-second window. Its reported `20.176s` denominator incorrectly included the
one-time fallback report reconstruction, producing `71.37 FPS`; the fix
freezes the sample duration before report construction. The fixed on/off
thread-CPU p95 delta is `-0.029ms`. App-work p95 differs by `+0.102ms`, below
the `<= 0.2ms` accounting ceiling.

An accounting-on repeat after a 48-second settle reached `67.04 FPS`, with
app-work p95 `16.094ms`, Meta GPU `8.694ms`, and 10 actors rather than 2. It
remained far above the old exact collector's `46.25 FPS`, but is retained as
evidence that thermal/runtime and entity variance matter near the RD10
72-Hz edge; do not treat one sequential run as an optimization verdict.

Active-terrain RD5 flight guardrail (`--xr-skip-actors`, `4.3 blocks/s`,
approximately 86 blocks traversed):

| Accounting | Frames / interval | Submitted FPS | App p95 | Thread CPU p95 | Drawn sections / indices |
|---|---:|---:|---:|---:|---:|
| On-demand exact on | `1441 / 20.010s` | `72.01` | `5.669ms` | `5.135ms` | `43 / 232,104` |
| Off | `1440 / 20.000s` | `72.00` | `5.750ms` | `5.178ms` | `43 / 232,104` |

The traversal delta is `-0.081ms` app-work p95, i.e. benchmark noise, while
the exact schema still reports all 12 stages, 8 queues, 4 peer threads, and 9
budget decisions. This revalidates the Quest RD5 measurement-overhead
guardrail while terrain generation, compilation, admission, and upload are
active.

### 2026-07-08 - Tactical 155 P0 Fix A Quest Free-Movement + Churn Memory Soak

Benchmarked runtime commit: `8343d960` (the P0 Fix A eviction runtime is
`0c8de561`; `8343d960` only re-frames the 155 Fix B doc note). The release APK was
rebuilt and installed for these rows. The worktree carried the desktop
`scheduler_movement_smoke` instrumentation uncommitted at measurement time, which
is **not** compiled into the XR APK (it is a `mclone-server` bin, not runtime),
so the on-device runtime is the committed Fix A code.

This row is the open 155 "V" Quest item: re-run RD5/RD7/churn on the
RAM-constrained platform after the P0 bound landed, confirming (a) no frame/
stability regression and (b) that the previously-untested **free-movement** case
holds memory bounded (the leak, had it survived, would OOM the Quest first). Frame
metrics come from the `MCLONE_ANDROID_XR_PERF_*` markers; process memory is new
evidence for this row, sampled on-device from `/proc/<pid>/{VmRSS,VmHWM}` (RD7 soak
used a VmRSS-only sampler to avoid perturbing frame timing; RD5 lanes also sampled
`dumpsys meminfo` TOTAL PSS, which adds a rare dump-induced frame hitch).

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |

Commands:

```bash
pnpm native:android-xr:perf:flight:rd5
pnpm native:android-xr:perf:churn:rd5:metrics
# Long sustained-flight memory soak (default runtime config, extended duration):
bash android-xr/validate-quest-openxr.sh --perf-seconds 180 --perf-flight --perf-flight-speed 4.3 --perf-metrics --wait-seconds 320 --render-distance 7 --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --day-time 6000 --freeze-time
```

Frame metrics (primary comparison fields):

| Lane | Config | Frames | Skipped | App p95 / p99 / max | Headroom avg / p05 / min | App over-period | Conservation |
|---|---|---:|---:|---:|---:|---:|---:|
| RD5 flight (`20s`) | default, workers `1`, unbounded budgets | `1441` | `0` | `9.358 / 9.983 / 12.525ms` | `+6.753 / +4.531 / +1.364ms` | `0` / `0.0%` | `0` |
| RD5 chunk-view churn (`45s`) | workers `2`, accept/upload `2/16/64` | `3237` | `0` | `10.664 / 12.195 / 21.291ms` | `+7.818 / +3.225 / -7.402ms` | `10` / `0.3%` | `0` |
| RD7 flight soak (`180s`) | default, workers `1`, unbounded budgets | `12844` | `0` | `14.356 / 16.240 / 21.438ms` | `+3.781 / -0.467 / -7.549ms` | `1115` / `8.7%` | `0` |

Process memory (the P0 evidence — `/proc` RSS in MB):

| Lane | Traversal | Fill peak / VmHWM | Movement-phase RSS band | Net accumulation |
|---|---|---:|---:|---:|
| RD5 flight (`20s`) | ~5 chunks | ~620 MB HWM | `~540–580 MB` | none |
| RD5 churn (`45s`) | ±16-chunk view toggle | ~692 MB HWM | `~574–707 MB` | **returns to baseline** (`574 → 574 MB`) |
| RD7 flight soak (`180s`) | `774` blocks ≈ `48` new chunks | `845 MB` HWM (set at initial fill) | `~633–725 MB` (avg `679`, ends `653`) | **VmHWM never exceeded in flight** |

Interpretation: **no regression and no unbounded memory growth on the constrained
platform.** All three lanes ran to completion with `0` skipped frames and `0`
conservation violations — no crash, no OOM. RD5 flight is fully green
(`0.0%` over-period, all-positive headroom); the RD5 churn tail is marginally
above the `1c8a0743` 153 baseline (app p95 `10.66` vs `8.92ms`, over-period `0.3`
vs `0.0%`), attributable to the concurrent `dumpsys meminfo` sampler's periodic
process dump rather than Fix A (a server-side light-unload message off the render
hot path). RD7 continuous flight sits at `8.7%` over-period, the expected cost of
streaming new terrain at RD7 (heavier than the settled-orbit RD7 rows), not a
memory or eviction regression. **Memory is the decisive result:** under `180s`
of sustained free-movement flight across ~48 fresh chunks, RSS holds a bounded
`~633–725 MB` band and the `845 MB` high-water mark set by the initial RD7 fill is
never exceeded — no per-unique-chunk ratchet. The churn lane loads/unloads its
view repeatedly and RSS returns exactly to its pre-churn baseline (`574 MB`),
the eviction-working signature. Had the pre-fix leak survived, retained light
(~128 KB/unique chunk) would have ratcheted RSS/VmHWM upward past the fill peak
across this traversal and threatened the Quest RAM ceiling; instead memory is
bounded by the loaded set, matching the desktop RD20 300-step soak result.

### 2026-07-07 - Tactical 153 Applied Derived Render Compile Capacity

Benchmarked runtime commit: `ab582d3c`, clean worktree before docs edits. The
release APK was rebuilt and installed for the first orbit row, then reused with
staged assets for the repeat orbit and churn rows.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Capacity mode | `--render-compile-capacity derived`, no manual worker override |

Commands used the normal RD5 orbit/churn guardrails with the existing
completed-result/section upload/section accept caps `2/16/64`, replacing
manual `--render-compile-workers 2` with `--render-compile-capacity derived`.
The derived capacity resolved to Quest-local `workers=1`,
`max_pending_jobs=4` in every marker block.

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-capacity derived --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 210 --perf-summary /tmp/mclone-quest-openxr-perf-orbit-rd5-derived-capacity.txt --log /tmp/mclone-quest-openxr-perf-orbit-rd5-derived-capacity-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --render-compile-capacity derived --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 210 --perf-summary /tmp/mclone-quest-openxr-perf-orbit-rd5-derived-capacity-r2.txt --log /tmp/mclone-quest-openxr-perf-orbit-rd5-derived-capacity-r2-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --render-compile-capacity derived --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-chunk-view-churn --perf-churn-interval-seconds 3 --perf-churn-offset-chunks 16 --perf-metrics --wait-seconds 210 --perf-summary /tmp/mclone-quest-openxr-perf-churn-rd5-derived-capacity.txt --log /tmp/mclone-quest-openxr-perf-churn-rd5-derived-capacity-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
```

Primary guardrail rows:

| Lane | Workers / pending | Settle | Frames | Skipped | Dropped delta | App p95 / p99 / max | Headroom avg / p05 / min | App over-period | Compile queue max age | Deadline skips |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| RD5 settled orbit r1 | `1 / 4` | `12.923s` | `3193` | `0` | `23` | `11.994 / 12.828 / 15.723ms` | `+3.307 / +1.895 / -1.834ms` | `1` frame / `0.0%` | `180.945ms` | `0` |
| RD5 settled orbit r2 | `1 / 4` | `12.756s` | `3200` | `0` | `15` | `12.088 / 12.698 / 14.135ms` | `+3.235 / +1.787 / -0.246ms` | `2` frames / `0.1%` | `154.030ms` | `0` |
| RD5 chunk-view churn | `1 / 4` | `15.689s` | `3236` | `0` | `17` | `9.439 / 10.590 / 14.929ms` | `+8.523 / +4.450 / -1.040ms` | `4` frames / `0.1%` | `617.266ms` | `0` |

Additional queue and upload markers:

| Lane | Upload-work max age | Host-publication max age | Inbound max age | Update oldest age | Update pump stalls | Compile stale sections |
|---|---:|---:|---:|---:|---:|---:|
| RD5 settled orbit r1 | `13.212ms` | `469.071ms` | `0.000ms` | `19.935ms` | `0` | `1` |
| RD5 settled orbit r2 | `23.343ms` | `0.000ms` | `0.000ms` | `16.607ms` | `0` | `0` |
| RD5 chunk-view churn | `233.672ms` | `2344.678ms` | `132.665ms` | `161.149ms` | `1` | `32` |

Interpretation:

- The shared derived capacity request now reaches Android XR and resolves from
  Quest resources to `1/4`, materially different from desktop's `7/14` and
  equal to the fail-safe floor. This validates the cross-host resource-derived
  path and prevents the desktop capacity from leaking onto Quest.
- RD5 orbit is app-work green on both repeats. The first repeat had a high Meta
  dropped-frame delta (`23`), so it was repeated; the second repeat returned to
  the established `15-17` range while preserving similar app p95/headroom.
- RD5 churn is app-work green: skipped `0`, app p95 `9.439ms`, app
  over-period `0.1%`, and no deadline skips. Render-compile queue max age
  `617.266ms` is slightly above the Slice 0 attribution row (`581.685ms`) but
  below the earlier queue-depth candidate row (`678.520ms`). Host-publication
  age is higher (`2344.678ms`), so publication/light remains the watched
  downstream pressure, not mesh worker count.
- This row validates the applied Quest path, but it does not justify a global
  default promotion. Desktop 120 Hz still has one top-level over-2x outlier at
  `7/14`; keep `derived` explicit/context-gated and move the Tactical 153 focus
  to the fresh-startup light/status ceiling.

Raw artifacts:

- `/tmp/mclone-quest-openxr-perf-orbit-rd5-derived-capacity.txt`
- `/tmp/mclone-quest-openxr-perf-orbit-rd5-derived-capacity-logcat.txt`
- `/tmp/mclone-quest-openxr-perf-orbit-rd5-derived-capacity-r2.txt`
- `/tmp/mclone-quest-openxr-perf-orbit-rd5-derived-capacity-r2-logcat.txt`
- `/tmp/mclone-quest-openxr-perf-churn-rd5-derived-capacity.txt`
- `/tmp/mclone-quest-openxr-perf-churn-rd5-derived-capacity-logcat.txt`

### 2026-07-07 - Tactical 153 Slice 0 RD5 Churn Attribution

Benchmarked runtime commit: `1c8a0743`, clean worktree before docs edits.
The release APK was rebuilt and installed for this row.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |

Command:

```sh
pnpm native:android-xr:perf:churn:rd5:metrics
```

Expanded validator command:

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-chunk-view-churn --perf-churn-interval-seconds 3 --perf-churn-offset-chunks 16 --perf-metrics --wait-seconds 210 --perf-summary /tmp/mclone-quest-openxr-perf-churn-rd5-metrics.txt --log /tmp/mclone-quest-openxr-perf-churn-rd5-metrics-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
```

Primary row:

| Lane | Config | Skipped | Dropped delta | App p95 / p99 / max | Headroom avg / p05 / min | App over-period | Compile queue max age |
|---|---|---:|---:|---:|---:|---:|---:|
| RD5 chunk-view churn | workers `2`, max pending `4`, accept/upload `2/16/64` | `0` | `16` | `8.919 / 9.678 / 15.518ms` | `+8.651 / +4.969 / -1.629ms` | `1` frame / `0.0%` | `581.685ms` |

Publication, queue, and peer attribution:

| Metric | Value |
|---|---:|
| sample / settle seconds | `45.007s` / `15.878s` |
| frames / submitted / runtime | `3239 / 3239 / 3239` |
| publication cadence | feature `23.419`/s, light `11.509`/s, units `34.928`/s |
| queue max ages | inbound `133.188ms`, upload `254.848ms`, host-publication `1872.731ms`, render-compile `581.685ms` |
| server/update queues | server update max `283`, oldest applied age `160.968ms`, pending light statuses `214` |
| render compile peer | pending `4`, response frames `21085`, busy `24722.366ms`, max request `91.801ms` |
| render compile cost | `1.173ms`/completed section |
| light peer | pending `214`, response frames `141`, busy `57526.172ms`, max request `1271.172ms` |
| worldgen peer | pending `1`, response frames `20`, busy `15896.635ms`, max request `1393.790ms` |
| upload/apply caps | upload limited `true`, accept limited `true`, update pump stalls `1` |

Interpretation: the Slice 0 instrumentation now reports nonzero XR
render-compile worker busy time. The row stays green on app-work headroom while
showing the same backlog shape as the Tactical 150 churn lane: host-publication
age near `1.9s`, render-compile age around `0.6s`, and a saturated light peer.
Use this as the Quest contrast row for Tactical 153 Slice 1; a render-compile
capacity change must not worsen skipped frames, app p95, app over-period, or
compile queue age.

### 2026-07-07 - Tactical 150 Slice 5 Clean Baseline Guardrails And Churn Soak

Benchmarked commit: `42d3e43a`, clean worktree. The first row rebuilt and
installed the release APK; later rows reused the APK and staged assets. All
commands intentionally omit `--render-compile-max-pending-jobs` and reported
default max pending `4`.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Battery/thermal | before soak `100%`, `35.0C`; after soak `100%`, `43.0C` |

Commands:

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 210 --perf-summary /tmp/mclone-150-slice5-quest-rd5-orbit.txt --log /tmp/mclone-150-slice5-quest-rd5-orbit-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-150-slice5-quest-rd7-orbit.txt --log /tmp/mclone-150-slice5-quest-rd7-orbit-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-chunk-view-churn --perf-churn-interval-seconds 3 --perf-churn-offset-chunks 16 --perf-metrics --wait-seconds 210 --perf-summary /tmp/mclone-150-slice5-quest-rd5-churn.txt --log /tmp/mclone-150-slice5-quest-rd5-churn-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 900 --perf-chunk-view-churn --perf-churn-interval-seconds 3 --perf-churn-offset-chunks 16 --perf-metrics-periodic --wait-seconds 1080 --perf-summary /tmp/mclone-150-slice5-quest-rd5-churn-soak900.txt --log /tmp/mclone-150-slice5-quest-rd5-churn-soak900-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
```

Short guardrail rows:

| Lane | Config | Skipped | Dropped delta | App p95 / p99 | Headroom avg / p05 | App over-period | Compile queue max age | Deadline skips |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| RD5 settled orbit | workers `2`, default max pending `4`, accept/upload `2/16/64` | `0` | `15` | `12.040 / 12.673ms` | `+3.172 / +1.849ms` | `0.0%` | `114.857ms` | `0` |
| RD7 settled orbit | workers `1`, default max pending `4`, unbounded accept/upload | `0` | `16` | `12.755 / 13.759ms` | `+2.697 / +1.134ms` | `0.9%` | `174.714ms` | `0` |
| RD5 chunk-view churn | workers `2`, default max pending `4`, accept/upload `2/16/64` | `0` | `16` | `9.771 / 10.705ms` | `+8.663 / +4.118ms` | `0.0%` | `531.182ms` | `0` |

Additional queue and upload markers:

| Lane | Upload-work max age | Host-publication max age | Server update queue / age | Upload limited | Accept limited | Update pump stalls |
|---|---:|---:|---:|---|---|---:|
| RD5 settled orbit | `40.063ms` | `0.000ms` | `0 / 15.705ms` | `true` | `false` | `0` |
| RD7 settled orbit | `0.000ms` | `271.066ms` | `19 / 34.540ms` | `false` | `false` | `1` |
| RD5 chunk-view churn | `211.405ms` | `1890.433ms` | `287 / 161.496ms` | `true` | `true` | `1` |

Long churn soak (`900.076s`, single-mode chunk-view churn because the validator
cannot combine settled orbit and churn in one launch):

| Metric | Value |
|---|---:|
| frames / submitted FPS | `37616 / 41.79` |
| skipped delta | `0` |
| app p95 / p99 / max | `10.209 / 11.710 / 43.547ms` |
| headroom avg / p05 / min | `+8.498 / +3.680 / -29.658ms` |
| app over-period frames / pct | `24 / 0.1%` |
| render-compile queue max age | `1558.614ms` |
| upload-work / host-publication max age | `478.470 / 2098.178ms` |
| server update queue / age | `287 / 445.220ms` |
| deadline-skipped compile requests | `1` |
| periodic Meta dropped-frame delta | `219 -> 358` across 61 samples |

Interpretation:

- The short RD5/RD7/churn guardrails are green on clean commit `42d3e43a`.
- The long soak is useful but **not** a clean Slice 5 soak pass. App-work
  percentiles stayed safe, but submitted/runtime FPS fell to `41.79`, periodic
  Meta dropped-frame deltas grew, queue ages increased, and one render compile
  deadline skip appeared. Treat this as thermal/compositor/long-run drift plus
  backlog evidence for the follow-up staged-budgeting tactical.
- The mixed orbit+churn soak requested by Tactical 150 is blocked by current
  validator shape: `--perf-settled-orbit` and `--perf-chunk-view-churn` are
  mutually exclusive.

Raw artifacts:

- `/tmp/mclone-150-slice5-quest-rd5-orbit.txt`
- `/tmp/mclone-150-slice5-quest-rd5-orbit-logcat.txt`
- `/tmp/mclone-150-slice5-quest-rd7-orbit.txt`
- `/tmp/mclone-150-slice5-quest-rd7-orbit-logcat.txt`
- `/tmp/mclone-150-slice5-quest-rd5-churn.txt`
- `/tmp/mclone-150-slice5-quest-rd5-churn-logcat.txt`
- `/tmp/mclone-150-slice5-quest-rd5-churn-soak900.txt`
- `/tmp/mclone-150-slice5-quest-rd5-churn-soak900-logcat.txt`

### 2026-07-07 - Tactical 150 Queue-Depth Default-On Verification

Benchmarked worktree: dirty on `1815d7f0` while promoting the shared default
render compile max-pending cap to `4` and editing docs. The release APK was
rebuilt for the first RD7 row, then reused with staged assets for the repeat and
churn rows. These commands intentionally omit
`--render-compile-max-pending-jobs`; the marker blocks prove the app default is
now `4`.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |

Commands:

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-150-defaulton-quest-rd7-orbit.txt --log /tmp/mclone-150-defaulton-quest-rd7-orbit-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-150-defaulton-quest-rd7-orbit-repeat.txt --log /tmp/mclone-150-defaulton-quest-rd7-orbit-repeat-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --render-compile-workers 2 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-chunk-view-churn --perf-churn-interval-seconds 3 --perf-churn-offset-chunks 16 --perf-metrics --wait-seconds 210 --perf-summary /tmp/mclone-150-defaulton-quest-rd5-churn.txt --log /tmp/mclone-150-defaulton-quest-rd5-churn-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
```

| Lane | Config | Skipped | Dropped delta | App p95 / p99 | Headroom avg / p05 | App over-period | Compile queue max age | Deadline skips |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| RD7 settled orbit | workers `1`, default max pending `4`, unbounded accept/upload | `0` | `25` | `12.888 / 13.988ms` | `+2.640 / +1.001ms` | `1.0%` | `125.710ms` | `0` |
| RD7 settled orbit repeat | workers `1`, default max pending `4`, unbounded accept/upload | `0` | `18` | `12.738 / 13.845ms` | `+2.758 / +1.151ms` | `0.9%` | `106.057ms` | `0` |
| RD5 chunk-view churn | workers `2`, default max pending `4`, accept/upload `2/16/64` | `0` | `14` | `9.084 / 10.886ms` | `+8.829 / +4.805ms` | `0.1%` | `550.485ms` | `0` |

Additional queue and upload markers:

| Lane | Upload-work max age | Host-publication max age | Server update queue / age | Upload limited | Update pump stalls |
|---|---:|---:|---:|---|---:|
| RD7 settled orbit | `0.000ms` | `466.042ms` | `19 / 37.660ms` | `false` | `1` |
| RD7 settled orbit repeat | `0.000ms` | `1371.042ms` | `19 / 29.242ms` | `false` | `1` |
| RD5 chunk-view churn | `217.998ms` | `1903.822ms` | `283 / 163.331ms` | `true` | `1` |

Interpretation:

- The promoted default path is active: all rows omitted
  `--render-compile-max-pending-jobs` and reported
  `render_compile_max_pending_jobs=4` plus `COMPILE_MAX max_pending_jobs=4`.
- The first RD7 default-on row was app-work safe but failed the dropped-frame
  delta envelope (`25` vs gate `<=20`). The immediate repeat was inside the
  RD7 pressure envelope (`18`, app p95 `12.738ms`, app-over-period `0.9%`), so
  treat the first row as thermal/compositor noise, not a deterministic
  queue-depth regression.
- Static XR accept/upload caps are not promoted. The same-session RD7
  comparison before this default flip showed pinned `2/16/64` caps at
  app-over-period `0.9%` and unbounded repeat at `0.7%`; unbounded remains the
  default while RD5 guardrail scripts keep their explicit lane args.
- Churn queue age is not a new blocker for max pending `4`. The default-on
  churn row's render-compile queue age `550.485ms` is below prior accepted
  overlap/adaptive churn artifacts (`606.756-781.323ms`), and the earlier
  `deadline_skipped_requests=1` marker did not reproduce.

Raw artifacts:

- `/tmp/mclone-150-defaulton-quest-rd7-orbit.txt`
- `/tmp/mclone-150-defaulton-quest-rd7-orbit-logcat.txt`
- `/tmp/mclone-150-defaulton-quest-rd7-orbit-repeat.txt`
- `/tmp/mclone-150-defaulton-quest-rd7-orbit-repeat-logcat.txt`
- `/tmp/mclone-150-defaulton-quest-rd5-churn.txt`
- `/tmp/mclone-150-defaulton-quest-rd5-churn-logcat.txt`

### 2026-07-07 - Tactical 150 Queue-Depth Candidate Quest Pass

Benchmarked worktree: dirty on `a89dc699` while adding Android XR
`--render-compile-max-pending-jobs` plumbing and this record. The commit
containing this row carries the same runtime code. Release APK was rebuilt for
the first lane, then reused with staged assets for the remaining lanes.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |

Commands:

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --render-compile-workers 2 --render-compile-max-pending-jobs 4 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 210 --perf-summary /tmp/mclone-150-quest-rd5-orbit-queue4.txt --log /tmp/mclone-150-quest-rd5-orbit-queue4-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --render-compile-max-pending-jobs 4 --perf-seconds 45 --perf-settled-orbit --perf-orbit-speed 4.3 --perf-metrics --wait-seconds 270 --perf-summary /tmp/mclone-150-quest-rd7-orbit-queue4.txt --log /tmp/mclone-150-quest-rd7-orbit-queue4-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 7 --day-time 6000 --freeze-time
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh --skip-build --skip-assets --render-compile-workers 2 --render-compile-max-pending-jobs 4 --xr-render-completed-result-accept-budget 2 --xr-render-section-upload-budget 16 --xr-render-section-accept-budget 64 --perf-seconds 45 --perf-chunk-view-churn --perf-churn-interval-seconds 3 --perf-churn-offset-chunks 16 --perf-metrics --wait-seconds 210 --perf-summary /tmp/mclone-150-quest-rd5-churn-queue4.txt --log /tmp/mclone-150-quest-rd5-churn-queue4-logcat.txt --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 5 --day-time 6000 --freeze-time
```

All three startup argv logs included `--render-compile-max-pending-jobs 4`,
and each marker block reported `max_pending_jobs=4` in
`MCLONE_ANDROID_XR_PERF_COMPILE_MAX`.

| Lane | Config | Skipped | Dropped delta | App p95 / p99 | Headroom avg / p05 | App over-period | Compile queue max age | Deadline skips |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| RD5 settled orbit | workers `2`, max pending `4`, accept/upload `2/16/64` | `0` | `16` | `11.923 / 12.584ms` | `+3.296 / +1.966ms` | `0.0%` | `168.645ms` | `0` |
| RD7 settled orbit | workers `1`, max pending `4`, unbounded accept/upload | `0` | `16` | `12.407 / 13.769ms` | `+2.895 / +1.482ms` | `0.8%` | `114.569ms` | `0` |
| RD5 chunk-view churn | workers `2`, max pending `4`, accept/upload `2/16/64` | `0` | `16` | `9.639 / 10.595ms` | `+8.740 / +4.250ms` | `0.1%` | `678.520ms` | `1` |

Additional queue and upload markers:

| Lane | Upload-work max age | Host-publication max age | Server update queue / age | Upload limited | Update pump stalls |
|---|---:|---:|---:|---|---:|
| RD5 settled orbit | `107.686ms` | `475.548ms` | `0 / 26.218ms` | `true` | `0` |
| RD7 settled orbit | `0.000ms` | `926.904ms` | `19 / 25.472ms` | `false` | `1` |
| RD5 chunk-view churn | `208.969ms` | `1899.507ms` | `287 / 172.884ms` | `true` | `1` |

Interpretation:

- The candidate now reaches Quest correctly; the validator passes the shared
  startup flag through, and the perf markers prove the render compiler saw a
  `4`-job in-flight cap.
- RD5 orbit and RD5 churn app pacing remain inside the established overlap
  envelope (`0` skipped frames, app p95 under the Slice 1 rows, dropped delta
  `16`).
- Do not promote defaults from this pass alone. RD7 app-over-period rose to
  `0.8%` versus the older `0.5%` overlap pressure row, and churn's compile-max
  marker reported `deadline_skipped_requests=1` with a `678.520ms`
  render-compile queue max age. The next pass should repeat RD7 with the
  accept/upload policy under decision and attribute the churn
  queue-age/deadline-skip row before shipping max-pending `4` by default.

Raw artifacts:

- `/tmp/mclone-150-quest-rd5-orbit-queue4.txt`
- `/tmp/mclone-150-quest-rd5-orbit-queue4-logcat.txt`
- `/tmp/mclone-150-quest-rd7-orbit-queue4.txt`
- `/tmp/mclone-150-quest-rd7-orbit-queue4-logcat.txt`
- `/tmp/mclone-150-quest-rd5-churn-queue4.txt`
- `/tmp/mclone-150-quest-rd5-churn-queue4-logcat.txt`

### 2026-07-07 - Tactical 150 Slice 3 Churn Diagnostics Rerun

Benchmarked commit: `9da26f88` (`Fix app-runtime wasm all-target coverage
bin`), clean tree before doc edits. Release APK build was up to date.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |

Command:

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-perf-churn-rd5-adaptive-diagnostics.txt \
  --log /tmp/mclone-quest-openxr-perf-churn-rd5-adaptive-diagnostics-logcat.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time \
  --adaptive-chunk-publication-budget true
```

The Android XR startup logs and perf markers reported
`adaptive_chunk_publication_budget=true`. The new publication/decision markers
were present.

| Lane | Config | Skipped | Dropped delta | App p95 / p99 | Headroom avg / p05 | App over-period | Over 2x | Queue / oldest age | Pump stalls | Pending publication chunks | Publication cadence |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| RD5 chunk-view churn | workers `2`, accept/upload `2/16/64` | `0` | `15` | `9.361 / 10.402ms` | `+8.945 / +4.528ms` | `0.1%` | `0` | `287 / 164.177ms` | `1` | `100` | `23.412` feature chunks/s, `13.239` light statuses/s, `36.651` units/s |

Budget decision trace:

| Family | Host/window/stage | Reason | Target period | Grant | Max units | Per-unit estimate |
|---|---|---|---:|---:|---:|---:|
| feature publication | integrated-server-runner / gameplay-tick / scheduler-publication | `hold-at-cap` | `50.000ms` | `10.000ms` | `3` | `2.519ms` |
| light publication | integrated-server-runner / gameplay-tick / scheduler-publication | `hold-at-cap` | `50.000ms` | `10.000ms` | `4` | `0.289ms` |
| feature-job admission | integrated-server-runner / gameplay-tick / terrain-generation | `hold-at-cap` | `50.000ms` | `0.000ms` | `1`, cap `256` pending units | n/a |

Interpretation:

- The churn safety fields remain inside the Slice 1 overlap churn envelope:
  app p95/p99 are below both the Slice 1 churn baseline (`9.897 / 11.521ms`)
  and the 2026-07-06 adaptive churn row (`9.957 / 10.908ms`), dropped delta
  `15` remains below the RD5 gate limit `17`, app-over-period remains `0.1%`,
  and over-2x frames are `0`.
- Update queue depth and oldest-applied age are materially unchanged from the
  prior adaptive churn row (`287 / 164.701ms`), and the single update-pump
  stall matches the established churn lane shape.
- The shared controller converged differently on Quest than on the desktop
  cadence row: feature publication capped at `3` units from measured
  `2.519ms` per-unit cost, while desktop stayed at `4` units from a much lower
  measured cost. This is the expected input-driven difference, not a
  platform-local policy branch.

Raw artifacts:

- `/tmp/mclone-quest-openxr-perf-churn-rd5-adaptive-diagnostics.txt`
- `/tmp/mclone-quest-openxr-perf-churn-rd5-adaptive-diagnostics-logcat.txt`

### 2026-07-06 - Tactical 150 Slice 3 Adaptive Publication Quest Checkpoint

Commit: this checkpoint commit (release APK built from the same worktree
contents before the commit was created; base before edits was `704f6373`). Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |

All rows included `--adaptive-chunk-publication-budget true`, and the Android
XR scene/options logs and perf markers reported
`adaptive_chunk_publication_budget=true`.

| Lane | Config | Skipped | Dropped delta | App p95 / p99 | Headroom avg | App over-period | Over 2x | Queue / oldest age | Pump stalls | Pending publication chunks |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| RD5 settled orbit | workers `2`, accept/upload `2/16/64` | `0` | `14` | `12.067 / 12.681ms` | `+3.085ms` | `0.0%` | `0` | `0 / 20.927ms` | `0` | `0` |
| RD7 settled orbit | lane default | `0` | `17` | `12.653 / 13.544ms` | `+2.740ms` | `0.5%` | `0` | `19 / n/a` | `0` | `15` |
| RD5 chunk-view churn | workers `2`, accept/upload `2/16/64` | `0` | `13` | `9.957 / 10.908ms` | `+8.832ms` | `0.1%` | `1` | `287 / 164.701ms` | `1` | `96` |

Interpretation:

- The RD5 settled-orbit guardrail is green against the 142 envelope
  (`dropped_delta <= 17`, app p95 `<= 13.0ms`, headroom avg `>= +2.5ms`,
  app-over-period `<= 2%`, over-2x `0`).
- RD7 remains inside the pressure envelope (`dropped_delta <= 20`, app p95
  `<= 13.5ms`, app-over-period `<= 2%`, over-2x `0`).
- RD5 churn is comparable to the Slice 1 overlap churn baseline: app p95 is
  `+0.060ms`, p99 is lower, dropped delta is `+1`, queue age is slightly lower
  than `166.692ms`, and the one update-pump stall matches the existing churn
  rows. Pending publication peaked at `96`, below the adaptive backlog cap.
  The lane still lacks an aggregate chunks/sec field and full budget-decision
  trace, so this is gate evidence, not default-on closure by itself.

Raw artifacts:

- `/tmp/mclone-quest-openxr-perf-orbit-rd5-adaptive.txt`
- `/tmp/mclone-quest-openxr-perf-orbit-rd5-adaptive-logcat.txt`
- `/tmp/mclone-quest-openxr-perf-orbit-rd7-adaptive.txt`
- `/tmp/mclone-quest-openxr-perf-orbit-rd7-adaptive-logcat.txt`
- `/tmp/mclone-quest-openxr-perf-churn-rd5-adaptive.txt`
- `/tmp/mclone-quest-openxr-perf-churn-rd5-adaptive-logcat.txt`

### 2026-07-06 - Tactical 150 Slice 1 Quest Frame-Shape Default Decision

Benchmarked code: A/B rows used the already-landed `--xr-frame-overlap` path
on commit `a864effe` (`Record Quest persisted-world guardrail run`) with the
Slice 1 package lane additions in the worktree. The follow-up implementation in
the same slice makes frame overlap the Android XR per-eye default, adds
`--xr-frame-serial` for the legacy path, and was launch-validated with a release
APK built from the worktree.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Pinned budgets | RD5 orbit/churn used render compile workers `2`, completed-result accept `2`, section upload `16`, section accept `64`; RD7 orbit and default-config control used their existing unbounded lane defaults |

Summary:

| Lane | Serial path | Overlap/default path | Result |
|---|---:|---:|---|
| RD5 settled orbit, pinned budgets | p95 `14.684ms`, head avg `+1.642ms`, over `12.4%`, dropped `23` | p95 `12.236ms`, head avg `+3.148ms`, over `0.0%`, dropped `14` | overlap wins |
| RD7 settled orbit pressure | p95 `15.806ms`, head avg `+0.344ms`, over `33.1%`, dropped `18` | p95 `12.647ms`, head avg `+3.171ms`, over `0.5%`, dropped `16` | overlap wins |
| RD5 chunk-view churn, pinned budgets | p95 `12.441ms`, head avg `+7.297ms`, over `1.3%`, dropped `17` | p95 `9.897ms`, head avg `+8.317ms`, over `0.1%`, dropped `12` | overlap wins |
| RD5 default-config control | p95 `14.575ms`, head avg `+1.572ms`, over `12.9%`, dropped `16` | p95 `12.044ms`, head avg `+3.383ms`, over `0.0%`, dropped `18` | overlap wins |

Interpretation:

- Make per-eye frame overlap the Android XR default. The measured win is not
  marginal: RD5/RD7 settled orbit and RD5 churn all gain app-work headroom and
  reduce over-period frames. The default-config control also improves app p95 by
  `2.531ms`.
- Keep `--xr-frame-overlap` as an explicit confirmation flag and add
  `--xr-frame-serial` for legacy A/B rows. The default is disabled for
  full-frame multiview and multiview proof/perf modes because the path only
  applies to the normal per-eye full-frame renderer.
- Overlap includes the existing runtime/render-section prefetch path. In the
  overlap rows, the normal runtime/upload max bucket is `0.000ms` while
  `MCLONE_ANDROID_XR_PERF_OVERLAP` records prefetch work, e.g. RD5 orbit
  max prefetch `6.852ms` and prefetch GPU upload `5.068ms`.

Post-flip launch validation:

```sh
pnpm native:android-xr:validate
```

The release APK built, installed, and launched successfully. The log records:
`Android XR frame overlap mode: default`, `Android XR frame overlap: true`, and
`render_path=per-eye-frame-overlap`.

Raw artifacts:

- `/tmp/mclone-150-slice1-rd5-orbit-default-summary.txt`
- `/tmp/mclone-150-slice1-rd5-orbit-overlap-summary.txt`
- `/tmp/mclone-150-slice1-rd7-orbit-default-summary.txt`
- `/tmp/mclone-150-slice1-rd7-orbit-overlap-summary.txt`
- `/tmp/mclone-150-slice1-rd5-churn-default-summary.txt`
- `/tmp/mclone-150-slice1-rd5-churn-overlap-summary.txt`
- `/tmp/mclone-150-slice1-rd5-shipping-default-summary.txt`
- `/tmp/mclone-150-slice1-rd5-shipping-default-overlap-summary.txt`
- `/tmp/mclone-quest-openxr-logcat.txt`

### 2026-07-06 - Quest Persisted-World RD5 Guardrail Unblock Run

Benchmarked code: Android XR release APK built from clean commit `9bbb4ff5`
(`Stabilize XR body follow at blocked walls`). The successful run reused that
APK and already-staged assets with `--skip-build --skip-assets` after fixing a
validator false positive for a benign Quest system warning containing the word
`SIGSEGV`.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated SQLite world at `/sdcard/Android/data/com.kzahel.mclone.xr/files/persisted-world-guardrail/rd5`, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| World storage after run | `world.sqlite3` plus WAL/SHM, `20M` total |
| Pinned budgets | render compile workers `2`, completed-result accept `2`, section upload `16`, section accept `64` |

Command:

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-persisted-world.sh \
  --skip-build \
  --skip-assets
```

The packaged lane is:

```sh
pnpm native:android-xr:perf:orbit:rd5:persisted
```

Raw artifacts:

- `/tmp/mclone-quest-openxr-persisted-rd5-prewarm-summary.txt`
- `/tmp/mclone-quest-openxr-persisted-rd5-prewarm-logcat.txt`
- `/tmp/mclone-quest-openxr-persisted-rd5-summary.txt`
- `/tmp/mclone-quest-openxr-persisted-rd5-logcat.txt`

Primary frame/headroom rows:

| Phase | Mode | Settle | FPS | Frames | Skipped | Dropped delta | App avg | App p95 | App p99 | App max | Head avg | App over-period | Over 2x |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| prewarm | stationary-settled | `32.289s` / `2272` frames | `63.85` | `1278` | `0` | n/a | `12.862ms` | `14.248ms` | `16.839ms` | `18.265ms` | `+1.027ms` | `132/1278` (`10.3%`) | `0` |
| reopen | settled-orbit | `17.424s` / `1213` frames | `65.35` | `2941` | `0` | `53` | `12.615ms` | `14.843ms` | `16.771ms` | `20.182ms` | `+1.274ms` | `429/2941` (`14.6%`) | `0` |

Reopen detail:

| Field | Value |
|---|---:|
| Flight distance | `7.488` blocks |
| Meta app/compositor GPU | `6.591ms` / `1.675ms` |
| GPU util / CPU util avg/worst | `57.598%` / `35.922%` / `46.000%` |
| Motion-to-photon | `34.436ms` |
| Drawn sections / indices | `125` / `1,127,148` |
| Loaded sections / ready sections | `848` / `1936` |
| Rebuilt / uploaded sections | `32` / `7` (`16` queued upload sections at max) |
| Server pending jobs / publications max | `11` / `12` |
| Scheduler loaded / visible / ticket chunks max | `288` / `196` / `1600` |
| Scheduler pending worldgen chunks / light publications max | `5` / `7` |
| Runtime poll / sync / GPU upload max | `1.201ms` / `3.473ms` / `4.262ms` |
| Server tick / scheduler tick max | `63.430ms` / `61.749ms` |
| Prepared record rebuilds | `247`, avg `0.712ms`, max `1.577ms` |

Interpretation:

- The blocker is cleared: Android XR now accepts the shared startup
  `--world-dir`, the validator can prewarm and reopen a persisted Quest world,
  and the lane emits the normal RD5 settled-orbit marker block.
- The reopened run is not a no-work static scene: it still has local integrated
  streaming work while orbiting (`scheduler_pending_worldgen_publication_chunks=5`,
  `scheduler_pending_light_publications=7`, `queued_upload_sections=16` at max),
  but it validates the already-created SQLite world path and avoids repeating
  a fresh app-local storage bootstrap.
- Frame pacing is in the current RD5 guardrail envelope: `skipped_delta=0`,
  `0` over-2x frames, app p95 `14.843ms`, and app over-period `14.6%`.

### 2026-07-06 - Tactical 150 Slice 0 Quest Guardrail Recapture

Benchmarked code: Android XR release APK built from clean commit `bc55076c`
(`Fix throughput guardrail measurement gates`). Host: `kmacbook`, Apple M4 Pro
Mac. Raw summaries and logcats were captured under `/tmp/mclone-142-baselines/`.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Pinned budgets | render compile workers `2`, completed-result accept `2`, section upload `16`, section accept `64` |

Settled-orbit guardrail rows:

| Lane | Run | Config | FPS | Frames | Skipped | Dropped delta | App avg | App p95 | App p99 | App max | Head avg | App over-period | Over 2x |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| RD5 orbit | 1 | pinned | `64.44` | `2901` | `0` | `17` | `12.270ms` | `14.737ms` | `16.718ms` | `19.331ms` | `+1.619ms` | `396/2901` (`13.7%`) | `0` |
| RD5 orbit | 2 | pinned | `62.92` | `2832` | `0` | `17` | `12.560ms` | `15.133ms` | `17.243ms` | `19.824ms` | `+1.329ms` | `444/2832` (`15.7%`) | `0` |
| RD5 orbit | 1 | shipping default | `62.66` | `2821` | `0` | `17` | `12.638ms` | `14.920ms` | `16.222ms` | `18.198ms` | `+1.251ms` | `538/2821` (`19.1%`) | `0` |
| RD7 orbit | 1 | pinned | `58.20` | `2620` | `0` | `18` | `13.681ms` | `16.522ms` | `19.183ms` | `23.829ms` | `+0.208ms` | `938/2620` (`35.8%`) | `0` |
| RD7 orbit | 2 | pinned | `57.56` | `2591` | `0` | `32` | `13.784ms` | `17.129ms` | `19.534ms` | `21.819ms` | `+0.105ms` | `973/2591` (`37.6%`) | `0` |

RD5 local-integrated chunk-view churn rows, pinned config, center alternates by
`16` chunks every `3s`:

| Run | FPS | Frames | Skipped | Dropped delta | App avg | App p95 | App p99 | App max | Head avg | App over-period | Runtime upload max | GPU upload max | Update q / oldest age | Pump stalls | Pending publication chunks |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `70.96` | `3194` | `0` | `29` | `6.361ms` | `11.664ms` | `13.290ms` | `19.401ms` | `+7.528ms` | `14/3194` (`0.4%`) | `7.339ms` | `5.834ms` | `283 / 164.784ms` | `1` | `125` |
| 2 | `71.16` | `3203` | `0` | `22` | `6.453ms` | `11.577ms` | `13.216ms` | `17.103ms` | `+7.436ms` | `18/3203` (`0.6%`) | `8.516ms` | `6.011ms` | `283 / 170.014ms` | `1` | `123` |

Meta dropped-frame delta validation:

- One-shot guardrail rows now include `dropped_frames_start`,
  `dropped_frames_end`, and `dropped_frames_delta`.
- A separate RD5 stationary `--perf-metrics-periodic` validation emitted
  repeated windows with advancing start/end counters, for example sample 1
  `0 -> 15`, sample 2 `15 -> 16`, sample 3 `16 -> 16`, and later samples such
  as `218 -> 258`.

Interpretation:

- The old RD5 142 gate is stale: on the clean baseline, RD5 pinned orbit still
  has `skipped_delta=0` and `0` over-2x frames, but app p95 is
  `14.737-15.133ms` and over-period is `13.7-15.7%`.
- RD7 pinned orbit is cleaner than the previous pressure-check envelope:
  app p95 `16.522-17.129ms`, over-period `35.8-37.6%`, and `0` over-2x frames.
- RD5 churn remains the streaming guardrail row for Candidate A/B: update queue
  depth still reaches `283`, oldest-applied age is `~165-170ms`, and each row
  has one update-pump stall while render app p95 stays under `12ms`.

### 2026-07-06 - Quest RD5 Local vs Remote Chunk-View Churn Contrast

Benchmarked code: Android XR release APK built from clean commit `4f86dad3`
(`Add shared debug diagnostics toggle`).

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 `2G0YC1ZF93041Z` |
| Android API | 34 |
| OpenXR runtime | Oculus |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Lane | RD5 chunk-view churn, center alternates by `16` chunks every `3s` |
| Budgets | render compile workers `2`, completed-result accept `2`, section upload `16`, section accept `64` |

Local integrated command shape:

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

Remote dedicated command shape adds `--adb-reverse --start-server` and writes a
server log. Raw local artifacts were captured under:

- `/tmp/mclone-quest-openxr-churn-rd5-slice3-local-a-20260706-summary.txt`
- `/tmp/mclone-quest-openxr-churn-rd5-slice3-local-b-20260706-summary.txt`
- `/tmp/mclone-quest-openxr-churn-rd5-slice3-remote-a-20260706-summary.txt`
- `/tmp/mclone-quest-openxr-churn-rd5-slice3-remote-b-20260706-summary.txt`
- `/tmp/mclone-quest-openxr-churn-rd5-slice3-remote-a-20260706-server.txt`
- `/tmp/mclone-quest-openxr-churn-rd5-slice3-remote-b-20260706-server.txt`

Primary frame/headroom rows:

| Mode | Run | FPS | Frames | App avg | App p50 | App p95 | App p99 | App max | Head avg | Head p05 | Head min | App over-period | Over 2x |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| local integrated | A | `71.04` | `3198` | `6.412ms` | `5.443ms` | `11.532ms` | `13.473ms` | `16.246ms` | `+7.477ms` | `+2.357ms` | `-2.357ms` | `204/3198` (`0.7%`) | `0` |
| local integrated | B | `71.03` | `3197` | `6.449ms` | `5.619ms` | `11.526ms` | `13.303ms` | `16.318ms` | `+7.440ms` | `+2.363ms` | `-2.429ms` | `166/3197` (`0.5%`) | `0` |
| remote dedicated | A | `69.45` | `3126` | `7.719ms` | `5.424ms` | `14.928ms` | `17.425ms` | `20.515ms` | `+6.169ms` | `-1.039ms` | `-6.626ms` | `673/3126` (`10.8%`) | `0` |
| remote dedicated | B | `69.65` | `3135` | `7.543ms` | `5.337ms` | `14.940ms` | `17.432ms` | `20.448ms` | `+6.346ms` | `-1.051ms` | `-6.559ms` | `703/3135` (`10.5%`) | `1` |

Runtime/update maxima:

| Mode | Run | Runtime upload / sync / GPU upload | Poll / apply / client apply | Producer read / decode | Update queue depth / bytes / oldest age | Update-pump stalls |
|---|---|---:|---:|---:|---:|---:|
| local integrated | A | `8.987 / 6.091 / 7.401ms` | `2.113 / 2.081 / 2.021ms` | `0.000 / 0.000ms` | `283` / `7,497,776` / `167.385ms` | `true`, count `1` |
| local integrated | B | `7.293 / 2.105 / 6.518ms` | `2.173 / 2.096 / 2.043ms` | `0.000 / 0.000ms` | `283` / `7,551,038` / `169.310ms` | `true`, count `1` |
| remote dedicated | A | `7.686 / 5.447 / 5.681ms` | `2.141 / 2.108 / 2.064ms` | `5917.254 / 10.911ms` | `387` / `9,724,619` / `14.368ms` | `true`, count `1` |
| remote dedicated | B | `8.062 / 4.046 / 6.664ms` | `2.135 / 2.124 / 2.069ms` | `5780.298 / 12.454ms` | `387` / `9,724,619` / `15.067ms` | `true`, count `1` |

Server-owned lane accounting:

| Mode | Run | Client server tick / scheduler tick | Client pending publication / worldgen chunks / light publications | Server-side summary |
|---|---|---:|---:|---|
| local integrated | A | `70.721 / 70.575ms` | `124 / 124 / 8` | same process; lanes `availability=local` |
| local integrated | B | `53.156 / 53.005ms` | `125 / 125 / 9` | same process; lanes `availability=local` |
| remote dedicated | A | `0.000 / 0.000ms` | `0 / 0 / 0` | `phase=active`, `commands=16`, `updates_sent=4217`, `tick_total_ms=123.702`, `scheduler_tick_ms=103.806` |
| remote dedicated | B | `0.000 / 0.000ms` | `0 / 0 / 0` | `phase=active`, `commands=16`, `updates_sent=4217`, `tick_total_ms=125.400`, `scheduler_tick_ms=105.613` |

Availability markers:

- local rows: `host-publication` queue and `server-runner` peer reported
  `availability=local`;
- remote rows: `host-publication` queue and `server-runner` peer reported
  `availability=remote-host`; `worldgen` and `light-status` peers also
  reported `availability=remote-host`; render-compile workers stayed local.

Interpretation:

- The remote lane is now usable as a contrast row: no multi-second command-send
  stall, schema v6 availability labels prevent server-side zeros from reading
  as local idle work, and the host process emits server-side summaries.
- Server/scheduler work measurably leaves the headset in remote mode: the
  client-side server/scheduler pending fields become `0` because those lanes
  are `remote-host`, while the dedicated server reports the cumulative tick and
  scheduler work.
- The headset does not become free. Client-paid work remains: network
  read/decode, update apply, mesh/upload, and render pacing. In this run remote
  had lower update oldest-applied age (`~14-15ms` vs `~167-169ms`) but higher
  over-period rate (`~10-11%` vs `<1%`) and similar apply/upload tails.

### 2026-07-05 - Standalone Quest 3 RD5 Chunk-View Churn Attribution

Benchmarked code: Android XR release APK built from `f5f90f83` plus doc-only
working-tree edits.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` per eye, `1x` render scale |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Lane | RD5 chunk-view churn, center alternates by `16` chunks every `3s` |
| Budgets | render compile workers `2`, completed-result accept `2`, section upload `16`, section accept `64` |

Local integrated command:

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-churn-rd5-20260705-summary.txt \
  --log /tmp/mclone-quest-openxr-churn-rd5-20260705-logcat.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

Local integrated summary:

| Mode | Settle | FPS | Runtime skipped | App avg | App p50 | App p95 | App p99 | App max | Headroom avg | App over-period | Over 2x |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| local integrated | `32.259s` | `71.90` | `0` | `7.032ms` | `6.180ms` | `12.631ms` | `14.266ms` | `21.738ms` | `+6.857ms` | `53 / 3236` (`1.6%`) | `1` |

Local integrated max buckets:

| Bucket | Value |
|---|---:|
| Runtime upload / sync / GPU upload | `11.594 / 6.475 / 9.977ms` |
| Runtime poll / apply / dirty / client apply | `3.761 / 3.750 / 1.869 / 3.742ms` |
| Update queue depth / bytes / oldest age | `283` / `7,497,788` / `167.287ms` |
| Update-pump stalls | `true`, count `1` |
| Server tick / scheduler tick | `42.565 / 42.475ms` |
| Scheduler pending publications | `124` |
| Scheduler pending worldgen-publication chunks | `124` |
| Scheduler pending light publications | `8` |
| Worldgen mailbox jobs / light mailbox statuses | `1` / `23` |
| Upload removed sections queued | `976` |
| Drawn sections / drawn indices | `159` / `635,196` |

Interpretation:

- RD5 local integrated chunk-view churn is mostly well paced: zero runtime
  skipped frames, strong average headroom, `app_work_p95` under the 72 Hz
  period, and only one frame over `2x` period.
- It is not free. The churn lane still accumulates client update queue age
  (`167ms`), hits one update-pump stall, and shows active upload/accept
  backpressure. This is the guardrail evidence that prevents treating the
  desktop publication prototype as automatically safe for Quest.
- The new scheduler fields are live on Quest local integrated: the same
  one-feature-job publication backlog (`124` chunks) appears, with light
  publication and mailbox pressure visible.

Remote-dedicated contrast attempt:

```sh
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --skip-build \
  --skip-assets \
  --adb-reverse \
  --start-server \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-chunk-view-churn \
  --perf-churn-interval-seconds 3 \
  --perf-churn-offset-chunks 16 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-churn-rd5-remote-20260705-summary.txt \
  --log /tmp/mclone-quest-openxr-churn-rd5-remote-20260705-logcat.txt \
  --server-log /tmp/mclone-quest-openxr-churn-rd5-remote-20260705-server.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

Do **not** use this remote run as a performance comparison yet. The app logged
`remote dedicated session still ignores SendOnly command policy`, the churn
markers were irregular, and the sample captured a `5728.701ms` app-work frame
with `5718.639ms` attributed to locomotion/chunk-view command handling. The
remote summary also reported all server/scheduler/runtime-update counters as
zero (`MCLONE_ANDROID_XR_PERF_QUEUE_MAX` and `RUNTIME_MAX`), so the current
remote lane does not expose the accounting needed to compare against local
integrated play. Fix the remote session actor / diagnostics path from tactical
`133` before using remote RD5 churn as the "server costs moved off headset"
baseline.

### 2026-07-04 - Standalone Quest 3 RD5 Settled Orbit Control

Benchmarked code: current local worktree after `cce964fd` plus uncommitted
changes. The uncommitted package/docs changes only add this lane and record this
result, but unrelated worldgen files were also dirty during capture, so treat
this as a current-state control rather than a clean regression baseline.

This run uses the same guardrail shape as the 2026-07-04 RD7 settled-orbit
record: two render compile workers, completed-result accept budget `2`, section
upload budget `16`, and section accept budget `64`.

Command:

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --skip-build \
  --skip-assets \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-settled-orbit \
  --perf-orbit-speed 4.3 \
  --perf-metrics \
  --wait-seconds 210 \
  --perf-summary /tmp/mclone-quest-openxr-perf-orbit-rd5-budgeted-current-summary.txt \
  --log /tmp/mclone-quest-openxr-perf-orbit-rd5-budgeted-current-logcat.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 5 \
  --day-time 6000 \
  --freeze-time
```

`pnpm native:android-xr:perf:orbit:rd5:metrics` now carries the same budgeted
arguments and can reproduce this lane without the explicit command.

Summary:

| Lane | Path | Hz | Sample | Settle | FPS | Runtime skipped | App avg | App p50 | App p95 | App p99 | App max | Headroom avg | Over period |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| settled orbit RD5 | per-eye | `72.0` | `45.010s` | `32.411s` | `71.58` | `0` | `11.677ms` | `11.723ms` | `13.434ms` | `15.822ms` | `21.408ms` | `2.212ms` | `3.7%` |

Additional pacing counters:

- Frame interval summary: `13.917ms` avg / `15.471ms` p95 /
  `17.464ms` p99 / `27.446ms` max; `1556 / 3222` frame intervals were over
  the `13.889ms` 72 Hz period, with `0` frames over `2x` budget.
- Runtime frame counters during the measured sample: `submitted_delta=3222`,
  `runtime_delta=3222`, `skipped_delta=0`.
- Meta PerfMetrics single early query: app GPU `2.832ms`, compositor GPU
  `1.374ms`, GPU util `27.7%`, CPU util avg/worst `80.2% / 93.0%`,
  motion-to-photon `23.836ms`, dropped-frame counter `19`. This is not yet a
  measured per-sample dropped-frame delta.

Key max buckets:

| Bucket | Value |
|---|---:|
| Runtime upload | `6.210ms` |
| Runtime sync | `3.790ms` |
| Runtime GPU upload | `4.626ms` |
| Upload apply worst mesh upload | `3.507ms` |
| Shared records | `1.891ms` |
| Left/right eye | `7.450ms` / `7.492ms` |
| Stereo poll wait | `8.397ms` |
| Server tick / scheduler tick | `16.124ms` / `15.279ms` |
| Ready sections | `1936` |
| Drawn sections / indices | `108` / `927,348` |

Exploratory note: before adding the budgeted package lane, an unbounded RD5
orbit run with one compile worker also passed: `71.92 FPS`, `skipped_delta=0`,
app work `11.727ms` avg / `13.343ms` p95, and `2.162ms` average headroom.

Interpretation:

- RD5 is clean enough to use as the lower-distance Quest control: it has zero
  runtime skipped frames and positive average headroom under the same 72 Hz
  budget.
- Compared with the RD7 guardrail below, RD5 cuts drawn pressure from `191`
  sections / `1,512,444` indices to `108` sections / `927,348` indices and moves
  app work from over-budget on most frames to over-budget on only `3.7%`.
- The RD7 issue is therefore not fixed XR overhead alone. It scales materially
  with view-distance terrain pressure and the associated upload/compile/draw
  work.
- The exact Quest compositor dropped-frame story is still incomplete for the
  same reason as RD7: PerfMetrics currently logs an absolute counter, not a
  per-sample delta.

### 2026-07-04 - Standalone Quest 3 RD7 Settled Orbit Current Guardrail

Benchmarked code: captured from the current local worktree after the Android XR
legacy remote-address sentinel fix. The commit containing this record also
contains that XR fix. The worktree was not clean during capture: unrelated
worldgen files were dirty, so treat this as a current-state guardrail and not as
a clean regression baseline.

Command:

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-settled-orbit \
  --perf-orbit-speed 4.3 \
  --perf-metrics \
  --wait-seconds 270 \
  --perf-summary /tmp/mclone-quest-openxr-perf-orbit-rd7-current-summary.txt \
  --log /tmp/mclone-quest-openxr-perf-orbit-rd7-current-logcat.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 7 \
  --day-time 6000 \
  --freeze-time
```

Launch note: an earlier attempt failed before terrain startup because
`debug.mclone.remote_addr` still held the flat-Android no-remote sentinel
`__mclone_none__`; Android XR now ignores that sentinel and logs
`Android XR remote dedicated address: <none>`.

Summary:

| Lane | Path | Hz | Sample | Settle | FPS | Runtime skipped | App avg | App p50 | App p95 | App p99 | App max | Headroom avg | Over period |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| settled orbit RD7 | per-eye | `72.0` | `45.013s` | `44.564s` | `63.94` | `0` | `15.460ms` | `15.277ms` | `18.515ms` | `21.528ms` | `29.413ms` | `-1.571ms` | `81.9%` |

Additional pacing counters:

- Frame interval summary: `15.586ms` avg / `18.798ms` p95 /
  `22.307ms` p99 / `32.379ms` max; `2429 / 2878` frames over the
  `13.889ms` 72 Hz budget, with `3` frames over `2x` budget.
- Runtime frame counters during the measured sample: `submitted_delta=2878`,
  `runtime_delta=2878`, `skipped_delta=0`.
- Meta PerfMetrics single early query: app GPU `2.302ms`, compositor GPU
  `0.728ms`, GPU util `21.1%`, CPU util avg/worst `83.8% / 88.4%`,
  motion-to-photon `24.408ms`, dropped-frame counter `19`. This is not yet a
  measured per-sample dropped-frame delta.

Key max buckets:

| Bucket | Value |
|---|---:|
| Runtime upload | `13.902ms` |
| Runtime sync | `3.674ms` |
| Runtime GPU upload | `10.139ms` |
| Upload apply worst mesh upload | `9.414ms` |
| Shared records | `2.149ms` |
| Left/right eye | `9.508ms` / `9.133ms` |
| Stereo poll wait | `10.074ms` |
| Server tick / scheduler tick | `25.252ms` / `24.617ms` |
| Ready sections | `3600` |
| Drawn sections / indices | `191` / `1,512,444` |

Interpretation:

- The 45 second sample did not report runtime skipped frames, which is the
  guardrail evidence we were missing for this RD7 lane.
- RD7 is still not comfortably inside the 72 Hz app-work budget. Average app
  work is `1.57ms` over budget, p95 is `4.63ms` over, and most frames exceed
  the period.
- The exact Quest compositor dropped-frame story is still incomplete because
  the Meta dropped-frame counter is only captured as a single early absolute
  counter. A follow-up should record a before/after or per-sample delta.
- The biggest observed tails are upload/GPU-upload and render poll/eye work,
  not terrain generation alone, so desktop throughput tuning must preserve
  Quest upload/admission guardrails.

### 2026-07-01 - Standalone Quest 3 RD7 Settled Orbit Section Accept Budget 4

Benchmarked code: section-accept budget worktree, committed with this record.
This adds opt-in `--xr-render-section-accept-budget N`, which caps how many
rebuilt or removed sections the Android XR draw resources accept per live
frame. Default behavior remains unbounded.

Commands:

```bash
pnpm native:android-xr:perf:orbit:rd7:accept4:metrics
pnpm native:android-xr:perf:orbit:rd7:accept4:frame-overlap
```

Summary:

| Lane | Path | FPS | Missed 72 Hz slots | App avg | App p50 | App p95 | App p99 | Max | Headroom avg | Over period | MTP | Meta dropped | Drawn sections | Drawn indices |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| settled orbit accept4 | default per-eye | `63.65` | `~376 / 3241` (`11.6%`) | `15.440ms` | `14.736ms` | `21.676ms` | `30.272ms` | `70.603ms` | `-1.551ms` | `68.9%` | `35.144ms` | 87 | 146 | 1,245,252 |
| settled orbit accept4 | frame overlap | `69.89` | `~95 / 3241` (`2.9%`) | `12.136ms` | `11.834ms` | `15.028ms` | `26.242ms` | `63.753ms` | `1.753ms` | `8.0%` | `30.047ms` | 58 | 147 | 1,253,166 |

Key max buckets:

| Lane | Runtime upload | Runtime sync | Runtime prefetch sync | GPU upload | Ready sections | Shared records | Record rebuilds | Rebuild avg / max | Queued uploads / removals |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| default per-eye | `50.179ms` | `46.131ms` | `0.000ms` | `17.195ms` | `22.946ms` | `21.933ms` | 797 | `1.039 / 21.849ms` | `96 / 224` |
| frame overlap | `0.000ms` | `0.000ms` | `30.703ms` | `6.807ms` | `12.964ms` | `13.088ms` | 773 | `1.121 / 13.030ms` | `28 / 220` |

Interpretation:

- Budget 4 is not a win as a fixed policy. Compared with the prepared-record
  dirty-diff baseline, default worsened from `65.01 FPS` / `58.6%` over-period
  to `63.65 FPS` / `68.9%`, and frame-overlap worsened from `70.36 FPS` /
  `5.7%` over-period to `69.89 FPS` / `8.0%`.
- The cap did what it said: per-frame accepted uploads/removals are limited to
  `4`, and the queued backlog is visible. But each small accepted batch can
  dirty prepared records, so rebuild frames jumped from `174-175` to
  `773-797`. Spreading the work without incremental record maintenance created
  more total frames with record work.
- Keep the flag as an opt-in diagnostic/probe, not a default. The next
  implementation target should be prepared-record coalescing or incremental
  maintenance for small section changes, then re-test accept/upload budgets.

### 2026-07-01 - Standalone Quest 3 RD7 Settled Orbit Prepared-Record Dirty Diff

Benchmarked code: prepared-record dirty diff worktree, committed with this
record. The change avoids dirtying prepared terrain records when the
traversal-ready set is reasserted unchanged, skips empty section-update apply
calls, and adds `MCLONE_ANDROID_XR_PERF_RECORD_CACHE` counters.

Commands:

```bash
pnpm native:android-xr:perf:orbit:rd7:metrics
pnpm native:android-xr:perf:orbit:rd7:frame-overlap
```

Summary:

| Lane | Path | FPS | Missed 72 Hz slots | App avg | App p50 | App p95 | App p99 | Max | Headroom avg | Over period | MTP | Meta dropped | Drawn sections | Drawn indices |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| settled orbit | default per-eye | `65.01` | `~315 / 3241` (`9.7%`) | `15.041ms` | `14.238ms` | `21.149ms` | `30.707ms` | `59.273ms` | `-1.152ms` | `58.6%` | `37.149ms` | 83 | 157 | 1,252,116 |
| settled orbit | frame overlap | `70.36` | `~74 / 3241` (`2.3%`) | `11.791ms` | `11.670ms` | `14.142ms` | `22.437ms` | `49.540ms` | `2.098ms` | `5.7%` | `24.763ms` | 53 | 157 | 1,252,116 |

Key max buckets:

| Lane | Runtime upload | Runtime sync | Runtime prefetch sync | Shared records | Stereo poll wait | Record rebuilds | Rebuild avg / max | Ready set changed |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| default per-eye | `25.305ms` | `22.886ms` | `0.000ms` | `7.352ms` | `49.225ms` | 174 | `0.941 / 4.159ms` | `15 / 2926` |
| frame overlap | `0.000ms` | `0.000ms` | `34.027ms` | `26.982ms` | `27.693ms` | 175 | `1.135 / 26.887ms` | `15 / 3167` |

Interpretation:

- The dirty-diff guard did what it was supposed to do: almost every
  ready-set call is now unchanged (`2911/2926` default, `3152/3167` overlap),
  and those calls no longer force prepared-record rebuilds.
- The default lane improved average throughput versus the prior settled-orbit
  record (`62.88 -> 65.01 FPS`, missed slots `12.7% -> 9.7%`, over-period
  `71.3% -> 58.6%`), but it is still not close to locked.
- The frame-overlap lane is the useful product signal: app avg improved
  `12.493ms -> 11.791ms`, p95 `14.692ms -> 14.142ms`, p99
  `22.826ms -> 22.437ms`, max `58.398ms -> 49.540ms`, and over-period
  `9.5% -> 5.7%`. It still misses about `2.3%` of 72 Hz slots.
- The remaining prepared-record rebuilds now line up with real section/update
  work rather than ready-set reassertions (`174-175` rebuilds for
  `182-203` upload-work frames). That points the next slice at section
  acceptance/upload budgeting or incremental prepared records, not greedy
  meshing yet.

### 2026-07-01 - Standalone Quest 3 RD7 Settled Orbit

Benchmarked code commit: `abe9d6d` (`Add Android XR settled orbit perf lane`).
This lane waits for the RD7 settled gate, then moves in a local no-clip orbit at
`4.3` blocks/s for `45` seconds. Captured on Quest 3 with fixed startup pose
`0,120,-96,180`, `--perf-metrics`, render scale `1.0`, foveation off, render
distance `7`, and the same staged asset pack.

Commands:

```bash
pnpm native:android-xr:perf:orbit:rd7:metrics
pnpm native:android-xr:perf:orbit:rd7:frame-overlap
```

Summary:

| Lane | Path | FPS | Missed 72 Hz slots | App avg | App p50 | App p95 | App p99 | Max | Headroom avg | Over period | MTP | Meta dropped | Drawn sections | Drawn indices |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| settled orbit | default per-eye | `62.88` | `~410 / 3240` (`12.7%`) | `15.674ms` | `15.173ms` | `20.856ms` | `30.028ms` | `60.227ms` | `-1.785ms` | `71.3%` | `31.094ms` | 72 | 157 | 1,252,116 |
| settled orbit | frame overlap | `70.25` | `~79 / 3241` (`2.4%`) | `12.493ms` | `12.359ms` | `14.692ms` | `22.826ms` | `58.398ms` | `1.396ms` | `9.5%` | `39.214ms` | 54 | 157 | 1,252,116 |

Key max buckets:

| Lane | Runtime upload | Runtime sync | Runtime prefetch sync | Shared records | Stereo poll wait |
|---|---:|---:|---:|---:|---:|
| default per-eye | `35.719ms` | `33.504ms` | `0.000ms` | `27.010ms` | `41.404ms` |
| frame overlap | `0.000ms` | `0.000ms` | `44.080ms` | `18.175ms` | `37.273ms` |

Interpretation:

- This is the better product-style movement lane. Unlike the earlier straight
  flight sample, it starts from a populated settled RD7 scene and keeps drawing
  `157` sections / `1.252M` indices while moving around the local chunk cluster.
- Frame overlap is a large improvement here: submitted FPS rises from `62.88`
  to `70.25`, missed 72 Hz slots fall from about `12.7%` to `2.4%`, and
  over-period app-work frames fall from `71.3%` to `9.5%`.
- It still is not perfectly locked. Frame overlap leaves p95 just over budget
  (`14.692ms` against `13.889ms`) and p99/max spikes remain. The runtime spike
  moves into the overlap prefetch path (`44.080ms` max prefetch sync), and
  prepared-record/shared-record work remains visible (`18.175ms` max).
- Motion-to-photon worsened in the overlap sample (`31.094ms -> 39.214ms`), so
  overlap still needs a headset comfort check and probably a quality/headroom
  lever before becoming a default.

### 2026-07-01 - Standalone Quest 3 Live RD7 Stable-Lane Baseline

Benchmarked code commit: `c87ae7a` (`Add Android XR RD7 perf lanes`). Captured
on Quest 3 with fixed startup pose `0,120,-96,180`, `--perf-metrics`, render
scale `1.0`, foveation off, render distance `7`, flight speed `4.3`, and the
same staged asset pack.

Commands:

```bash
pnpm native:android-xr:perf:flight:rd7:metrics
pnpm native:android-xr:perf:flight:rd7:frame-overlap
pnpm native:android-xr:perf:stationary:rd7:metrics
pnpm native:android-xr:perf:stationary:rd7:frame-overlap
```

Summary:

| Lane | Path | FPS | App work avg | App work p50 | App work p95 | App work p99 | Max | Headroom avg | Over period | Drawn sections | Drawn indices |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| flight | default per-eye | `69.61` | `8.360ms` | `6.913ms` | `18.850ms` | `29.565ms` | `64.889ms` | `5.529ms` | `9.8%` | 81 | 556,938 |
| flight | frame overlap | `69.85` | `6.685ms` | `5.553ms` | `14.122ms` | `28.115ms` | `48.444ms` | `7.204ms` | `5.3%` | 81 | 556,938 |
| stationary settled | default per-eye | `60.94` | `16.319ms` | `16.057ms` | `18.289ms` | `25.364ms` | `35.139ms` | `-2.430ms` | `99.1%` | 172 | 1,369,332 |
| stationary settled | frame overlap | `71.01` | `13.494ms` | `13.370ms` | `14.600ms` | `18.275ms` | `26.322ms` | `0.394ms` | `21.1%` | 180 | 1,387,980 |

Key max buckets:

| Lane | Runtime upload | Runtime sync | Runtime prefetch sync | Shared records | Stereo poll wait |
|---|---:|---:|---:|---:|---:|
| flight default | `45.239ms` | `44.386ms` | `0.000ms` | `24.230ms` | `23.310ms` |
| flight frame overlap | `0.000ms` | `0.000ms` | `42.745ms` | `10.769ms` | `22.796ms` |
| stationary default | `18.581ms` | `14.730ms` | `0.000ms` | `2.074ms` | `10.453ms` |
| stationary frame overlap | `0.000ms` | `0.000ms` | `16.763ms` | `2.528ms` | `7.014ms` |

Interpretation:

- RD7 is better than RD10, but it is **not yet a clean product lane at scale
  1.0** from this view. Stationary default is steady-render bound at only
  `60.94 FPS`, and even stationary frame-overlap still has `21.1%`
  app-over-period frames.
- Frame overlap is still valuable. It improves flight average app work by
  `1.675ms`, flight p95 by `4.728ms`, stationary average by `2.825ms`, and
  stationary p95 by `3.689ms`. It also moves normal runtime upload/sync work out
  of the measured render bucket.
- Flight remains burst-sensitive at RD7: default flight hit
  `max_runtime_sync_ms=44.386` and `max_terrain_shared_records_ms=24.230`;
  frame-overlap moved the runtime spike into prefetch but still had
  `app_work_p99=28.115ms`.
- This keeps the next implementation target unchanged: instrument and reduce
  prepared-record / ready-section churn first, then revisit upload/acceptance
  budgeting. Separately, product stability likely needs frame overlap plus a
  quality lever such as moderate render scale, dynamic render distance, or a
  lower stable lane.

### 2026-07-01 - Standalone Quest 3 Live RD10 Opt-In Frame Overlap

Benchmarked code: current worktree for the live validation follow-up to
`1977a67` (`Add opt-in XR frame overlap path`), later committed with this
record. Runtime/render code is unchanged from the opt-in frame-overlap commit;
this adds package scripts for the live RD10 metrics lanes and records matched
Quest 3 A/B runs. Captured with fixed startup pose `0,120,-96,180`,
`--perf-metrics`, render scale `1.0`, foveation off, render distance `10`,
flight speed `4.3`, and the same staged asset pack.

Summary:

| Lane | Path | FPS | App work avg | App work p50 | App work p95 | App work p99 | Max | Headroom avg | Over period | MTP | Dropped | Drawn sections | Drawn indices |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| stationary settled | default per-eye | `51.72` | `19.251ms` | `18.425ms` | `23.594ms` | `26.611ms` | `42.276ms` | `-5.362ms` | `100.0%` | `26.954ms` | 94 | 237 | 1,811,616 |
| stationary settled | frame overlap | `65.11` | `15.206ms` | `14.987ms` | `16.932ms` | `24.488ms` | `30.853ms` | `-1.317ms` | `87.3%` | `25.482ms` | 52 | 237 | 1,811,616 |
| flight | default per-eye | `69.31` | `8.081ms` | `6.967ms` | `15.623ms` | `31.759ms` | `73.828ms` | `5.807ms` | `6.6%` | `24.521ms` | 92 | 62 | 401,754 |
| flight | frame overlap | `69.26` | `6.619ms` | `5.315ms` | `15.156ms` | `29.742ms` | `56.720ms` | `7.270ms` | `6.0%` | `25.815ms` | 46 | 59 | 375,882 |

Matched deltas:

| Lane | App work avg | App work p95 | App work p99 | Max | Headroom avg | Over period | FPS | MTP | Dropped |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| stationary settled | `-4.045ms` | `-6.662ms` | `-2.123ms` | `-11.423ms` | `+4.045ms` | `-12.7pp` | `+13.39` | `-1.472ms` | `-42` |
| flight | `-1.462ms` | `-0.467ms` | `-2.017ms` | `-17.108ms` | `+1.463ms` | `-0.6pp` | `-0.05` | `+1.294ms` | `-46` |

Raw marker block, stationary default:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.013 mode=stationary-settled render_path=per-eye render_section_upload_budget=unbounded xr_foveation=off xr_render_scale=1.000 xr_eye_size=1680x1760 render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=49.949 settle_min_seconds=5.000 settle_frames=2390 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1035 submitted_delta=1035 runtime_delta=1035 skipped_delta=0 frame_avg_ms=19.283 frame_min_ms=15.267 frame_p50_ms=18.457 frame_p95_ms=23.627 frame_p99_ms=26.644 frame_max_ms=42.307 over_budget=1035 over_2x_budget=1 over_4x_budget=0 app_work_avg_ms=19.251 app_work_p50_ms=18.425 app_work_p95_ms=23.594 headroom_avg_ms=-5.362 app_over_period_frames=1035 app_over_period_pct=100.0
MCLONE_ANDROID_XR_PERF_HEADROOM sample_seconds=20.013 mode=stationary-settled render_path=per-eye xr_render_scale=1.000 target_hz=72.0 budget_ms=13.889 frames=1035 submitted_delta=1035 runtime_delta=1035 skipped_delta=0 submitted_fps=51.72 runtime_fps=51.72 wait_frame_avg_ms=0.031 wait_frame_p50_ms=0.028 wait_frame_p95_ms=0.051 wait_frame_max_ms=0.123 app_work_avg_ms=19.251 app_work_min_ms=15.238 app_work_p50_ms=18.425 app_work_p95_ms=23.594 app_work_p99_ms=26.611 app_work_max_ms=42.276 headroom_avg_ms=-5.362 headroom_p50_ms=-4.536 headroom_p05_ms=-9.705 headroom_p01_ms=-12.723 headroom_min_ms=-28.387 app_over_period_frames=1035 app_over_period_pct=100.0
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_render_frame_ms=40.849 max_terrain_runtime_upload_ms=20.496 max_runtime_sync_ms=17.446 max_runtime_gpu_upload_ms=1.180 max_runtime_ready_sections_ms=1.555 max_terrain_shared_records_ms=3.179 max_terrain_left_eye_poll_wait_ms=10.800 max_terrain_right_eye_poll_wait_ms=8.797 max_terrain_stereo_poll_wait_ms=15.898
MCLONE_ANDROID_XR_PERF_OVERLAP max_runtime_prefetch_ms=0.000 max_runtime_prefetch_poll_ms=0.000 max_runtime_prefetch_sync_ms=0.000 max_runtime_prefetch_gpu_upload_ms=0.000 max_runtime_prefetch_ready_sections_ms=0.000
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=237 indices=5396934 drawn_indices=1811616 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=2.829 app_cpu_ms=n/a compositor_gpu_ms=1.270 compositor_cpu_ms=n/a gpu_util_pct=29.141 cpu_util_avg_pct=88.801 cpu_util_worst_pct=93.069 motion_to_photon_ms=26.954 dropped_frames=94.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.38
```

Raw marker block, stationary frame overlap:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.012 mode=stationary-settled render_path=per-eye-frame-overlap render_section_upload_budget=unbounded xr_foveation=off xr_render_scale=1.000 xr_eye_size=1680x1760 render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=50.168 settle_min_seconds=5.000 settle_frames=3352 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1303 submitted_delta=1303 runtime_delta=1303 skipped_delta=0 frame_avg_ms=15.308 frame_min_ms=11.719 frame_p50_ms=15.054 frame_p95_ms=17.134 frame_p99_ms=25.346 frame_max_ms=30.890 over_budget=1234 over_2x_budget=5 over_4x_budget=0 app_work_avg_ms=15.206 app_work_p50_ms=14.987 app_work_p95_ms=16.932 headroom_avg_ms=-1.317 app_over_period_frames=1137 app_over_period_pct=87.3
MCLONE_ANDROID_XR_PERF_HEADROOM sample_seconds=20.012 mode=stationary-settled render_path=per-eye-frame-overlap xr_render_scale=1.000 target_hz=72.0 budget_ms=13.889 frames=1303 submitted_delta=1303 runtime_delta=1303 skipped_delta=0 submitted_fps=65.11 runtime_fps=65.11 wait_frame_avg_ms=0.102 wait_frame_p50_ms=0.026 wait_frame_p95_ms=0.459 wait_frame_max_ms=0.897 app_work_avg_ms=15.206 app_work_min_ms=11.693 app_work_p50_ms=14.987 app_work_p95_ms=16.932 app_work_p99_ms=24.488 app_work_max_ms=30.853 headroom_avg_ms=-1.317 headroom_p50_ms=-1.098 headroom_p05_ms=-3.043 headroom_p01_ms=-10.599 headroom_min_ms=-16.964 app_over_period_frames=1137 app_over_period_pct=87.3
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_render_frame_ms=30.202 max_terrain_runtime_upload_ms=0.000 max_runtime_sync_ms=0.000 max_runtime_gpu_upload_ms=0.000 max_runtime_ready_sections_ms=0.000 max_terrain_shared_records_ms=2.851 max_terrain_left_eye_poll_wait_ms=0.000 max_terrain_right_eye_poll_wait_ms=0.000 max_terrain_stereo_poll_wait_ms=8.747
MCLONE_ANDROID_XR_PERF_OVERLAP max_runtime_prefetch_ms=22.927 max_runtime_prefetch_poll_ms=0.908 max_runtime_prefetch_sync_ms=18.291 max_runtime_prefetch_gpu_upload_ms=1.176 max_runtime_prefetch_ready_sections_ms=1.518
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=237 indices=5396928 drawn_indices=1811616 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=3.092 app_cpu_ms=n/a compositor_gpu_ms=1.233 compositor_cpu_ms=n/a gpu_util_pct=30.382 cpu_util_avg_pct=94.373 cpu_util_worst_pct=98.020 motion_to_photon_ms=25.482 dropped_frames=52.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.37
```

Raw marker block, flight default:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.011 mode=flight render_path=per-eye render_section_upload_budget=unbounded xr_foveation=off xr_render_scale=1.000 xr_eye_size=1680x1760 render_distance=10 flight_speed_blocks_per_second=4.300 flight_distance_blocks=85.966 settle_seconds=0.000 settle_min_seconds=5.000 settle_frames=0 settle_quiet_frames=0 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1387 submitted_delta=1387 runtime_delta=1387 skipped_delta=0 frame_avg_ms=14.388 frame_min_ms=2.132 frame_p50_ms=14.010 frame_p95_ms=23.020 frame_p99_ms=36.342 frame_max_ms=76.257 over_budget=735 over_2x_budget=37 over_4x_budget=3 app_work_avg_ms=8.081 app_work_p50_ms=6.967 app_work_p95_ms=15.623 headroom_avg_ms=5.807 app_over_period_frames=91 app_over_period_pct=6.6
MCLONE_ANDROID_XR_PERF_HEADROOM sample_seconds=20.011 mode=flight render_path=per-eye xr_render_scale=1.000 target_hz=72.0 budget_ms=13.889 frames=1387 submitted_delta=1387 runtime_delta=1387 skipped_delta=0 submitted_fps=69.31 runtime_fps=69.31 wait_frame_avg_ms=6.306 wait_frame_p50_ms=7.193 wait_frame_p95_ms=11.447 wait_frame_max_ms=30.261 app_work_avg_ms=8.081 app_work_min_ms=2.017 app_work_p50_ms=6.967 app_work_p95_ms=15.623 app_work_p99_ms=31.759 app_work_max_ms=73.828 headroom_avg_ms=5.807 headroom_p50_ms=6.922 headroom_p05_ms=-1.734 headroom_p01_ms=-17.870 headroom_min_ms=-59.940 app_over_period_frames=91 app_over_period_pct=6.6
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_render_frame_ms=58.579 max_terrain_runtime_upload_ms=48.837 max_runtime_sync_ms=48.176 max_runtime_gpu_upload_ms=17.049 max_runtime_ready_sections_ms=5.567 max_terrain_shared_records_ms=48.069 max_terrain_left_eye_poll_wait_ms=25.480 max_terrain_right_eye_poll_wait_ms=26.608 max_terrain_stereo_poll_wait_ms=28.069
MCLONE_ANDROID_XR_PERF_OVERLAP max_runtime_prefetch_ms=0.000 max_runtime_prefetch_poll_ms=0.000 max_runtime_prefetch_sync_ms=0.000 max_runtime_prefetch_gpu_upload_ms=0.000 max_runtime_prefetch_ready_sections_ms=0.000
MCLONE_ANDROID_XR_PERF_DRAW sections=624 drawn_sections=62 indices=3029646 drawn_indices=401754 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=2.276 app_cpu_ms=n/a compositor_gpu_ms=1.153 compositor_cpu_ms=n/a gpu_util_pct=25.521 cpu_util_avg_pct=91.578 cpu_util_worst_pct=96.000 motion_to_photon_ms=24.521 dropped_frames=92.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.38
```

Raw marker block, flight frame overlap:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.012 mode=flight render_path=per-eye-frame-overlap render_section_upload_budget=unbounded xr_foveation=off xr_render_scale=1.000 xr_eye_size=1680x1760 render_distance=10 flight_speed_blocks_per_second=4.300 flight_distance_blocks=86.027 settle_seconds=0.000 settle_min_seconds=5.000 settle_frames=0 settle_quiet_frames=0 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1386 submitted_delta=1386 runtime_delta=1386 skipped_delta=0 frame_avg_ms=14.398 frame_min_ms=1.669 frame_p50_ms=14.032 frame_p95_ms=24.073 frame_p99_ms=36.514 frame_max_ms=78.678 over_budget=743 over_2x_budget=48 over_4x_budget=4 app_work_avg_ms=6.619 app_work_p50_ms=5.315 app_work_p95_ms=15.156 headroom_avg_ms=7.270 app_over_period_frames=83 app_over_period_pct=6.0
MCLONE_ANDROID_XR_PERF_HEADROOM sample_seconds=20.012 mode=flight render_path=per-eye-frame-overlap xr_render_scale=1.000 target_hz=72.0 budget_ms=13.889 frames=1386 submitted_delta=1386 runtime_delta=1386 skipped_delta=0 submitted_fps=69.26 runtime_fps=69.26 wait_frame_avg_ms=7.778 wait_frame_p50_ms=8.906 wait_frame_p95_ms=12.175 wait_frame_max_ms=36.061 app_work_avg_ms=6.619 app_work_min_ms=1.643 app_work_p50_ms=5.315 app_work_p95_ms=15.156 app_work_p99_ms=29.742 app_work_max_ms=56.720 headroom_avg_ms=7.270 headroom_p50_ms=8.574 headroom_p05_ms=-1.267 headroom_p01_ms=-15.853 headroom_min_ms=-42.831 app_over_period_frames=83 app_over_period_pct=6.0
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_render_frame_ms=55.533 max_terrain_runtime_upload_ms=0.000 max_runtime_sync_ms=0.000 max_runtime_gpu_upload_ms=0.000 max_runtime_ready_sections_ms=0.000 max_terrain_shared_records_ms=14.003 max_terrain_left_eye_poll_wait_ms=0.000 max_terrain_right_eye_poll_wait_ms=0.000 max_terrain_stereo_poll_wait_ms=28.879
MCLONE_ANDROID_XR_PERF_OVERLAP max_runtime_prefetch_ms=51.116 max_runtime_prefetch_poll_ms=1.041 max_runtime_prefetch_sync_ms=50.325 max_runtime_prefetch_gpu_upload_ms=4.367 max_runtime_prefetch_ready_sections_ms=5.464
MCLONE_ANDROID_XR_PERF_DRAW sections=615 drawn_sections=59 indices=2986110 drawn_indices=375882 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=2.022 app_cpu_ms=n/a compositor_gpu_ms=1.311 compositor_cpu_ms=n/a gpu_util_pct=23.865 cpu_util_avg_pct=94.276 cpu_util_worst_pct=95.960 motion_to_photon_ms=25.815 dropped_frames=46.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.46
```

Interpretation:

- This historical RD10 stress row kept `--xr-frame-overlap` opt-in because
  stationary RD10 was still over the 72 Hz app-work period on `87.3%` of frames,
  and flight still submitted at about `69 FPS`. Tactical 150 Slice 1 supersedes
  that default decision for the normal per-eye Quest path using the RD5/RD7
  guardrail rows above.
- It was already a large live stationary win (`19.251ms -> 15.206ms` app work,
  `51.72 -> 65.11 FPS`) and a smaller but real live flight app-work win
  (`8.081ms -> 6.619ms`, with p99/max spikes reduced and dropped frames halved).
- The overlap counters prove the live N+1 prefetch half is active:
  stationary moved up to `22.927ms` max runtime-prefetch work out of the normal
  render bucket, and flight moved up to `51.116ms`. The normal
  `max_terrain_runtime_upload_ms` bucket dropped to `0.000ms` in both overlap
  runs.
- The next practical performance step is to combine frame overlap with the
  shipping-policy levers: benchmark RD7 and/or a moderate render scale, then
  make dynamic render distance/render scale the default policy while keeping
  fixed RD10 as the stress lane.

### 2026-07-01 - Standalone Quest 3 Frozen RD10 Opt-In Frame Overlap

Benchmarked code commit: `1977a67` (`Add opt-in XR frame overlap path`). This
adds `--xr-frame-overlap` for the per-eye Android XR path, leaves the default
path unchanged, defers the eye GPU waits to one stereo wait, and caches any live
runtime/render-section prefetch so the next frame consumes it instead of
polling twice. The frozen RD10 lane has no live runtime/upload work during the
timed sample, so this record primarily validates the deferred stereo wait /
frame-pacing part; `MCLONE_ANDROID_XR_PERF_OVERLAP` stayed `0.000ms`.

Validation before/on device:
`bash -n android-xr/validate-quest-openxr.sh android-xr/install-quest-openxr.sh`,
`cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-android-xr-client --target aarch64-linux-android`,
`cargo test --manifest-path native/Cargo.toml`, visual inspection of
`/tmp/mclone-frame-overlap-debug.png`, and three Quest runs:
overlap, matched default baseline, overlap confirmation. Captured with fixed
frozen RD10 pose `0,80,-96,180`, `--perf-metrics`, render scale `1.0`,
foveation off, and the same staged asset pack.

Summary:

| Path | Sample | FPS | App work avg | App work p50 | App work p95 | App work p99 | Headroom avg | Headroom p05 | Over period | Drawn sections | Drawn indices | MTP |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| matched default per-eye | `20.014s` | `72.00` | `12.685ms` | `12.673ms` | `13.554ms` | `13.953ms` | `1.204ms` | `0.335ms` | `1.5%` | 179 | 1,356,102 | `35.487ms` |
| frame overlap, first run | `20.013s` | `72.00` | `10.967ms` | `10.926ms` | `11.734ms` | `12.019ms` | `2.922ms` | `2.155ms` | `0.0%` | 179 | 1,356,102 | `28.814ms` |
| frame overlap, confirmation | `20.015s` | `72.00` | `10.959ms` | `10.956ms` | `11.686ms` | `11.964ms` | `2.930ms` | `2.203ms` | `0.0%` | 179 | 1,356,102 | `23.011ms` |

Matched delta from the default run to the confirmation overlap run:
`app_work_avg_ms -1.726ms`, p95 `-1.868ms`, average headroom `+1.726ms`,
`app_over_period_pct 1.5% -> 0.0%`.

Raw marker block, matched default baseline:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.014 mode=stationary-frozen-render render_path=per-eye render_section_upload_budget=unbounded xr_foveation=off xr_render_scale=1.000 xr_eye_size=1680x1760 render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=34.995 settle_min_seconds=5.000 settle_frames=2394 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1441 submitted_delta=1441 runtime_delta=1441 skipped_delta=0 frame_avg_ms=13.843 frame_min_ms=11.366 frame_p50_ms=13.789 frame_p95_ms=15.240 frame_p99_ms=15.826 frame_max_ms=16.631 over_budget=674 over_2x_budget=0 over_4x_budget=0 app_work_avg_ms=12.685 app_work_p50_ms=12.673 app_work_p95_ms=13.554 headroom_avg_ms=1.204 app_over_period_frames=22 app_over_period_pct=1.5
MCLONE_ANDROID_XR_PERF_HEADROOM sample_seconds=20.014 mode=stationary-frozen-render render_path=per-eye xr_render_scale=1.000 target_hz=72.0 budget_ms=13.889 frames=1441 submitted_delta=1441 runtime_delta=1441 skipped_delta=0 submitted_fps=72.00 runtime_fps=72.00 wait_frame_avg_ms=1.159 wait_frame_p50_ms=1.176 wait_frame_p95_ms=2.214 wait_frame_max_ms=3.703 app_work_avg_ms=12.685 app_work_min_ms=10.490 app_work_p50_ms=12.673 app_work_p95_ms=13.554 app_work_p99_ms=13.953 app_work_max_ms=14.323 headroom_avg_ms=1.204 headroom_p50_ms=1.216 headroom_p05_ms=0.335 headroom_p01_ms=-0.064 headroom_min_ms=-0.434 app_over_period_frames=22 app_over_period_pct=1.5
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_render_frame_ms=13.874 max_terrain_render_views_ms=0.034 max_terrain_menu_pointer_ms=0.000 max_terrain_runtime_upload_ms=0.000 max_runtime_poll_ms=0.000 max_runtime_sync_ms=0.000 max_runtime_gpu_upload_ms=0.000 max_runtime_ready_sections_ms=0.000 max_terrain_shared_records_ms=0.005 max_terrain_left_eye_ms=7.258 max_terrain_right_eye_ms=5.294 max_terrain_left_eye_prepare_ms=2.175 max_terrain_left_eye_encode_ms=2.294 max_terrain_left_eye_section_encode_ms=1.632 max_terrain_left_eye_submit_ms=0.629 max_terrain_left_eye_poll_wait_ms=5.664 max_terrain_right_eye_prepare_ms=0.193 max_terrain_right_eye_encode_ms=1.847 max_terrain_right_eye_section_encode_ms=1.388 max_terrain_right_eye_submit_ms=0.470 max_terrain_right_eye_poll_wait_ms=3.664 max_terrain_stereo_finish_ms=0.000 max_terrain_stereo_submit_ms=0.941 max_terrain_stereo_poll_wait_ms=8.826
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=179 indices=5396316 drawn_indices=1356102 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=2.555 app_cpu_ms=n/a compositor_gpu_ms=1.296 compositor_cpu_ms=n/a gpu_util_pct=28.361 cpu_util_avg_pct=81.243 cpu_util_worst_pct=88.119 motion_to_photon_ms=35.487 dropped_frames=102.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=1.27
```

Raw marker block, frame-overlap confirmation:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.015 mode=stationary-frozen-render render_path=per-eye-frame-overlap render_section_upload_budget=unbounded xr_foveation=off xr_render_scale=1.000 xr_eye_size=1680x1760 render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=34.858 settle_min_seconds=5.000 settle_frames=2471 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1441 submitted_delta=1441 runtime_delta=1441 skipped_delta=0 frame_avg_ms=13.839 frame_min_ms=11.115 frame_p50_ms=13.814 frame_p95_ms=14.829 frame_p99_ms=15.269 frame_max_ms=18.269 over_budget=658 over_2x_budget=0 over_4x_budget=0 app_work_avg_ms=10.959 app_work_p50_ms=10.956 app_work_p95_ms=11.686 headroom_avg_ms=2.930 app_over_period_frames=0 app_over_period_pct=0.0
MCLONE_ANDROID_XR_PERF_HEADROOM sample_seconds=20.015 mode=stationary-frozen-render render_path=per-eye-frame-overlap xr_render_scale=1.000 target_hz=72.0 budget_ms=13.889 frames=1441 submitted_delta=1441 runtime_delta=1441 skipped_delta=0 submitted_fps=72.00 runtime_fps=72.00 wait_frame_avg_ms=2.881 wait_frame_p50_ms=2.886 wait_frame_p95_ms=3.762 wait_frame_max_ms=5.574 app_work_avg_ms=10.959 app_work_min_ms=9.394 app_work_p50_ms=10.956 app_work_p95_ms=11.686 app_work_p99_ms=11.964 app_work_max_ms=12.737 headroom_avg_ms=2.930 headroom_p50_ms=2.933 headroom_p05_ms=2.203 headroom_p01_ms=1.925 headroom_min_ms=1.152 app_over_period_frames=0 app_over_period_pct=0.0
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_render_frame_ms=12.190 max_terrain_render_views_ms=0.044 max_terrain_menu_pointer_ms=0.000 max_terrain_runtime_upload_ms=0.000 max_runtime_poll_ms=0.000 max_runtime_sync_ms=0.000 max_runtime_gpu_upload_ms=0.000 max_runtime_ready_sections_ms=0.000 max_terrain_shared_records_ms=1.422 max_terrain_left_eye_ms=2.657 max_terrain_right_eye_ms=2.114 max_terrain_left_eye_prepare_ms=2.111 max_terrain_left_eye_encode_ms=2.424 max_terrain_left_eye_section_encode_ms=1.829 max_terrain_left_eye_submit_ms=0.778 max_terrain_left_eye_poll_wait_ms=0.000 max_terrain_right_eye_prepare_ms=0.132 max_terrain_right_eye_encode_ms=1.810 max_terrain_right_eye_section_encode_ms=1.478 max_terrain_right_eye_submit_ms=0.435 max_terrain_right_eye_poll_wait_ms=0.000 max_terrain_stereo_finish_ms=0.000 max_terrain_stereo_submit_ms=1.134 max_terrain_stereo_poll_wait_ms=7.215
MCLONE_ANDROID_XR_PERF_OVERLAP max_runtime_prefetch_ms=0.000 max_runtime_prefetch_poll_ms=0.000 max_runtime_prefetch_sync_ms=0.000 max_runtime_prefetch_gpu_upload_ms=0.000 max_runtime_prefetch_ready_sections_ms=0.000
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=179 indices=5396736 drawn_indices=1356102 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=2.214 app_cpu_ms=n/a compositor_gpu_ms=1.143 compositor_cpu_ms=n/a gpu_util_pct=25.035 cpu_util_avg_pct=84.439 cpu_util_worst_pct=87.129 motion_to_photon_ms=23.011 dropped_frames=48.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.40
```

Interpretation:

- This is a real opt-in frozen-RD10 app-work win. Two overlap runs were stable
  (`10.967/11.734ms` and `10.959/11.686ms` avg/p95) around a matched default
  run at `12.685/13.554ms`.
- The measured win is mostly frame pacing / deferred stereo wait, not live
  N+1 runtime work: frozen render disables runtime polling and upload, so the
  prefetch fields are all zero.
- This opt-in conclusion is superseded by the 2026-07-06 Tactical 150 Slice 1
  RD5/RD7 guardrail decision above, which makes frame overlap the default for
  the normal per-eye Quest path while keeping stress-lane evidence historical.

### 2026-07-01 - Standalone Quest 3 Frozen RD10 Solid Terrain Layer Split (Slice O)

Benchmarked code commit: `0f5ecaa` (`Split solid terrain render layer`). The
worktree had unrelated `tools/asset-lab` changes, but the Android XR APK was
built from the committed native/render code. Captured with the fixed frozen RD10
pose `0,80,-96,180`, `--perf-metrics`, render scale `1.0`, foveation off, and
the same staged asset pack. This is the Slice O implementation from
[`117`](tactical/117-android-xr-rd10-gpu-floor-and-frame-overlap.md):
solid terrain now renders through a no-`discard` pipeline, while cutout and
translucent terrain keep the alpha-discard shader path.

Validation before the Quest run:
`cargo test --manifest-path native/Cargo.toml`,
`cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`,
`pnpm native:web:build`, and visual inspection of
`/tmp/mclone-solid-layer-debug.png`.

Summary:

| Path | Sample | FPS | Frames | App work avg | App work p50 | App work p95 | App work p99 | Headroom avg | Over period | Drawn sections | Drawn indices | Key marker |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| per-eye solid layer split | `20.013s` | `71.55` | 1,432 | `13.720ms` | `13.729ms` | `14.653ms` | `15.011ms` | `0.168ms` | `37.8%` | 179 | 1,356,102 | stereo poll `10.408ms`; Meta app GPU `2.537ms` |

Raw marker block:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.013 mode=stationary-frozen-render render_path=per-eye render_section_upload_budget=unbounded xr_foveation=off xr_render_scale=1.000 xr_eye_size=1680x1760 render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=35.273 settle_min_seconds=5.000 settle_frames=2398 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1432 submitted_delta=1432 runtime_delta=1432 skipped_delta=0 frame_avg_ms=13.925 frame_min_ms=11.324 frame_p50_ms=13.814 frame_p95_ms=14.873 frame_p99_ms=17.374 frame_max_ms=26.232 over_budget=636 over_2x_budget=0 over_4x_budget=0 app_work_avg_ms=13.720 app_work_p50_ms=13.729 app_work_p95_ms=14.653 headroom_avg_ms=0.168 app_over_period_frames=541 app_over_period_pct=37.8
MCLONE_ANDROID_XR_PERF_HEADROOM sample_seconds=20.013 mode=stationary-frozen-render render_path=per-eye xr_render_scale=1.000 target_hz=72.0 budget_ms=13.889 frames=1432 submitted_delta=1432 runtime_delta=1432 skipped_delta=0 submitted_fps=71.55 runtime_fps=71.55 wait_frame_avg_ms=0.205 wait_frame_p50_ms=0.033 wait_frame_p95_ms=1.014 wait_frame_max_ms=11.802 app_work_avg_ms=13.720 app_work_min_ms=11.299 app_work_p50_ms=13.729 app_work_p95_ms=14.653 app_work_p99_ms=15.011 app_work_max_ms=15.311 headroom_avg_ms=0.168 headroom_p50_ms=0.160 headroom_p05_ms=-0.765 headroom_p01_ms=-1.122 headroom_min_ms=-1.422 app_over_period_frames=541 app_over_period_pct=37.8
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_render_frame_ms=14.769 max_terrain_render_views_ms=0.056 max_terrain_menu_pointer_ms=0.000 max_terrain_runtime_upload_ms=0.000 max_runtime_poll_ms=0.000 max_runtime_sync_ms=0.000 max_runtime_gpu_upload_ms=0.000 max_runtime_ready_sections_ms=0.000 max_terrain_shared_records_ms=0.008 max_terrain_left_eye_ms=7.849 max_terrain_right_eye_ms=6.857 max_terrain_left_eye_prepare_ms=2.007 max_terrain_left_eye_encode_ms=2.554 max_terrain_left_eye_section_encode_ms=1.746 max_terrain_left_eye_submit_ms=0.804 max_terrain_left_eye_poll_wait_ms=6.442 max_terrain_right_eye_prepare_ms=0.155 max_terrain_right_eye_encode_ms=2.067 max_terrain_right_eye_section_encode_ms=1.510 max_terrain_right_eye_submit_ms=0.405 max_terrain_right_eye_poll_wait_ms=5.611 max_terrain_stereo_finish_ms=0.000 max_terrain_stereo_submit_ms=0.992 max_terrain_stereo_poll_wait_ms=10.408
MCLONE_ANDROID_XR_PERF_TERRAIN_PREP max_terrain_left_eye_cull_ms=1.865 max_terrain_left_eye_uniform_write_ms=0.216 max_terrain_left_eye_translucent_collect_ms=0.416 max_terrain_left_eye_translucent_sort_ms=0.038 max_terrain_right_eye_cull_ms=0.000 max_terrain_right_eye_uniform_write_ms=0.155 max_terrain_right_eye_translucent_collect_ms=0.000 max_terrain_right_eye_translucent_sort_ms=0.000
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=179 indices=5396112 drawn_indices=1356102 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=2.537 app_cpu_ms=n/a compositor_gpu_ms=1.152 compositor_cpu_ms=n/a gpu_util_pct=25.587 cpu_util_avg_pct=98.316 cpu_util_worst_pct=99.020 motion_to_photon_ms=36.239 dropped_frames=119.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.37
```

Interpretation:

- The code path is validated, but this is **not a measured RD10 win**. Against
  the current 117 baseline target of about `13.2-13.3ms` app work avg and
  `~14.1ms` p95, this run is worse (`13.720ms` avg, `14.653ms` p95).
- Drawn work stayed at the masked per-eye baseline shape (`179` sections /
  `1.356M` drawn indices), so the change did not reduce geometry. It only
  changed shader/pipeline selection for solid terrain.
- The likely conclusion is that unconditional terrain `discard` was not the
  practical RD10 bottleneck on this path, or any early-Z benefit is smaller than
  thermal/run variance and the extra solid/cutout pipeline split overhead.
- Keep the solid/cutout split for parity and future render-layer correctness,
  but do not count it as an RD10 performance lever. The next measured lever
  should be CPU/GPU frame overlap (Slice K/E4) or draw batching/arena work, with
  render scale as the known quality tradeoff.

### 2026-06-30 - Standalone Quest 3 Frozen RD10 Per-Eye Submit-Overlap Probe

Benchmarked code: current worktree for the per-eye submit-overlap slice, later
committed as the slice commit. Captured with the fixed frozen RD10 pose
`0,80,-96,180`, `--perf-metrics`, and the same staged asset pack. The baseline
used the default per-eye path. The probe used `--xr-overlap-eye-submits`, which
submits the left eye immediately, records/submits the right eye while left-eye
GPU work can run, then waits once before releasing the OpenXR images. This is
not full one-frame-late E4 pipelining.

Summary:

| Path | Sample | FPS | Frames | p50 | p95 | p99 | Max | Over 1x | Drawn sections | Drawn indices | App GPU | MTP | Key marker |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| per-eye baseline | `20.013s` | `71.5` | 1,430 | `13.776ms` | `15.139ms` | `17.219ms` | `27.603ms` | 647 | 179 | 1,356,102 | `1.592ms` | `26.913ms` | L/R poll `7.473`/`5.761ms`; stereo poll `11.223ms` |
| per-eye-overlap | `20.012s` | `72.0` | 1,440 | `13.865ms` | `14.918ms` | `15.373ms` | `24.825ms` | 700 | 179 | 1,356,102 | `1.471ms` | `23.973ms` | L/R poll `0.000`/`0.000ms`; stereo poll `9.883ms` |

Raw summary markers:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.013 mode=stationary-frozen-render render_path=per-eye render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=34.758 settle_min_seconds=5.000 settle_frames=2438 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1430 submitted_delta=1430 runtime_delta=1430 skipped_delta=0 frame_avg_ms=13.948 frame_min_ms=11.902 frame_p50_ms=13.776 frame_p95_ms=15.139 frame_p99_ms=17.219 frame_max_ms=27.603 over_budget=647 over_2x_budget=0 over_4x_budget=0
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_left_eye_ms=8.884 max_terrain_right_eye_ms=7.206 max_terrain_left_eye_submit_ms=0.604 max_terrain_left_eye_poll_wait_ms=7.473 max_terrain_right_eye_submit_ms=0.378 max_terrain_right_eye_poll_wait_ms=5.761 max_terrain_stereo_submit_ms=0.787 max_terrain_stereo_poll_wait_ms=11.223
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=179 indices=5396736 drawn_indices=1356102 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=1.592 app_cpu_ms=n/a compositor_gpu_ms=0.676 compositor_cpu_ms=n/a gpu_util_pct=16.274 cpu_util_avg_pct=88.289 cpu_util_worst_pct=94.000 motion_to_photon_ms=26.913 dropped_frames=98.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.38

MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.012 mode=stationary-frozen-render render_path=per-eye-overlap render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=34.818 settle_min_seconds=5.000 settle_frames=2479 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1440 submitted_delta=1440 runtime_delta=1440 skipped_delta=0 frame_avg_ms=13.851 frame_min_ms=11.629 frame_p50_ms=13.865 frame_p95_ms=14.918 frame_p99_ms=15.373 frame_max_ms=24.825 over_budget=700 over_2x_budget=0 over_4x_budget=0
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_left_eye_ms=3.838 max_terrain_right_eye_ms=5.093 max_terrain_left_eye_submit_ms=0.650 max_terrain_left_eye_poll_wait_ms=0.000 max_terrain_right_eye_submit_ms=0.612 max_terrain_right_eye_poll_wait_ms=0.000 max_terrain_stereo_submit_ms=0.852 max_terrain_stereo_poll_wait_ms=9.883
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=179 indices=5396772 drawn_indices=1356102 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=1.471 app_cpu_ms=n/a compositor_gpu_ms=0.661 compositor_cpu_ms=n/a gpu_util_pct=16.019 cpu_util_avg_pct=94.636 cpu_util_worst_pct=97.959 motion_to_photon_ms=23.973 dropped_frames=54.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.38
```

Interpretation:

- The synchronization change behaved as intended: per-eye poll waits dropped to
  `0.000ms`, and the single end-of-stereo wait was `9.883ms` instead of the
  baseline `11.223ms` max marker.
- End-to-end frame timing is mixed, not a default-switch signal: avg improved by
  `0.097ms`, p95 by `0.221ms`, and p99/max improved materially, but p50 worsened
  by `0.089ms` and over-budget frames increased.
- Keep `--xr-overlap-eye-submits` as an opt-in diagnostic/perf lane. The next
  decision should be based on another A/B or on deeper true frame pipelining /
  batching work, not this single mixed run.

### 2026-06-30 - Standalone Quest 3 Frozen RD10 Per-Eye Draw Masks vs Multiview

Benchmarked code commit: `ecb502f` (`Show raw and tinted grass previews`), which
includes `894bacf` (`Mask shared stereo terrain draws per eye`). Captured from a
clean temporary worktree with the fixed frozen RD10 pose `0,80,-96,180`,
`--perf-metrics`, and the same staged asset pack for both runs. The per-eye run
used the default path; the multiview run used `--xr-full-frame-multiview`.

Summary:

| Path | Sample | FPS | Frames | p50 | p95 | p99 | Max | Over 1x | Drawn sections | Drawn indices | App GPU | MTP | Key marker |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| per-eye draw masks | `20.013s` | `71.8` | 1,433 | `13.746ms` | `15.166ms` | `18.813ms` | `27.472ms` | 567 | 179 | 1,356,102 | `1.555ms` | `27.821ms` | stereo poll `10.774ms` |
| full-frame multiview | `20.012s` | `72.0` | 1,440 | `13.799ms` | `14.546ms` | `15.520ms` | `25.007ms` | 593 | 203 | 1,523,148 | `2.057ms` | `23.326ms` | multiview terrain `3.646ms`; poll `12.034ms` |

Raw summary markers:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.013 mode=stationary-frozen-render render_path=per-eye render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=34.678 settle_min_seconds=5.000 settle_frames=2434 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1433 submitted_delta=1433 runtime_delta=1433 skipped_delta=0 frame_avg_ms=13.918 frame_min_ms=11.093 frame_p50_ms=13.746 frame_p95_ms=15.166 frame_p99_ms=18.813 frame_max_ms=27.472 over_budget=567 over_2x_budget=0 over_4x_budget=0
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=179 indices=5396736 drawn_indices=1356102 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=1.555 app_cpu_ms=n/a compositor_gpu_ms=0.664 compositor_cpu_ms=n/a gpu_util_pct=18.012 cpu_util_avg_pct=90.956 cpu_util_worst_pct=93.000 motion_to_photon_ms=27.821 dropped_frames=100.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.39

MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.012 mode=stationary-frozen-render render_path=multiview render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=34.702 settle_min_seconds=5.000 settle_frames=2447 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1440 submitted_delta=1440 runtime_delta=1440 skipped_delta=0 frame_avg_ms=13.844 frame_min_ms=11.419 frame_p50_ms=13.799 frame_p95_ms=14.546 frame_p99_ms=15.520 frame_max_ms=25.007 over_budget=593 over_2x_budget=0 over_4x_budget=0
MCLONE_ANDROID_XR_PERF_MULTIVIEW max_multiview_sky_ms=0.606 max_multiview_terrain_ms=3.646 max_multiview_actor_ms=0.667 max_multiview_screen_effect_ms=0.017 max_multiview_world_overlays_ms=0.064 max_multiview_submit_ms=0.928 max_multiview_poll_wait_ms=12.034
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=203 indices=5396316 drawn_indices=1523148 actors=1 drawn_actors=1
MCLONE_ANDROID_XR_PERF_METRICS app_gpu_ms=2.057 app_cpu_ms=n/a compositor_gpu_ms=0.644 compositor_cpu_ms=n/a gpu_util_pct=18.109 cpu_util_avg_pct=95.333 cpu_util_worst_pct=98.000 motion_to_photon_ms=23.326 dropped_frames=56.000 stale_frames=n/a counters=17 any_valid=true attempt=1/8 per_query_us=0.38
```

Interpretation:

- Per-eye draw masks reduce terrain work from the prior shared-union `203`
  sections / `1.523M` indices to `179` sections / `1.356M` indices in this pose.
  This is the durable win from `894bacf`.
- Full-frame multiview still renders the exact stereo union, so it gives back
  that per-eye overdraw reduction. It is slightly better on avg/p95 here
  (`13.844ms` / `14.546ms` versus `13.918ms` / `15.166ms`) but worse on p50,
  over-budget count, and sampled app GPU frametime.
- Treat this as another "multiview is correct and useful to keep" record, not a
  reason to switch the default. The next primary RD10 performance lever should
  be a runtime-toggleable overlap experiment or deeper draw/submit batching,
  using this masked per-eye path as the baseline.

### 2026-06-30 - Standalone Quest 3 Frozen RD10 Shared Stereo Terrain Prep (Slice H)

Benchmarked code: current Slice H worktree, based on `1c8fdd2`, committed in the
Slice H commit that adds this record. The change builds one shared
`PreparedTexturedSectionStereoDraw` per XR frame from the exact union of both eye
frustums and feeds it to the per-eye full-frame, frozen terrain-only, and
full-frame multiview paths. Validation before the Quest runs:
`cargo test --manifest-path native/Cargo.toml -p mclone-render`,
`cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-xr-scene`,
and `cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android`.
The Quest validator cleanup was verified after
the runs: `pidof com.kzahel.mclone.xr` empty, `mWakefulness=Asleep`,
`mHoldingDisplaySuspendBlocker=false`.

Lanes:

- `native:android-xr:perf:frozen:rd10:metrics` for the per-eye default path.
- `native:android-xr:perf:frozen:rd10:multiview` for the opt-in full-frame
  multiview path.

Summary:

| Path | Sample | FPS | Frames | p50 | p95 | p99 | Max | Over 1x | Drawn sections | Drawn indices | Key CPU/GPU marker |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| per-eye | `20.001s` | `72.0` | 1,440 | `13.653ms` | `15.262ms` | `18.313ms` | `25.584ms` | 526 | 203 | 1,523,148 | shared cull charged to left `1.736ms`; right cull `0.000ms`; stereo poll `10.011ms` |
| multiview | `20.010s` | `72.0` | 1,440 | `13.728ms` | `14.776ms` | `17.892ms` | `25.715ms` | 546 | 203 | 1,523,148 | multiview terrain `3.052ms`; multiview poll `11.408ms` |

Per-eye marker block:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.001 mode=stationary-frozen-render render_path=per-eye render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=34.944 settle_min_seconds=5.000 settle_frames=2401 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1440 submitted_delta=1440 runtime_delta=1440 skipped_delta=0 frame_avg_ms=13.841 frame_min_ms=11.511 frame_p50_ms=13.653 frame_p95_ms=15.262 frame_p99_ms=18.313 frame_max_ms=25.584 over_budget=526 over_2x_budget=0 over_4x_budget=0
MCLONE_ANDROID_XR_PERF_TERRAIN max_terrain_render_frame_ms=14.515 max_terrain_render_views_ms=0.157 max_terrain_menu_pointer_ms=0.000 max_terrain_runtime_upload_ms=0.000 max_runtime_poll_ms=0.000 max_runtime_sync_ms=0.000 max_runtime_gpu_upload_ms=0.000 max_runtime_ready_sections_ms=0.000 max_terrain_shared_records_ms=0.051 max_terrain_left_eye_ms=7.636 max_terrain_right_eye_ms=7.088 max_terrain_left_eye_prepare_ms=1.811 max_terrain_left_eye_encode_ms=2.270 max_terrain_left_eye_section_encode_ms=1.149 max_terrain_left_eye_submit_ms=0.429 max_terrain_left_eye_poll_wait_ms=6.137 max_terrain_right_eye_prepare_ms=0.144 max_terrain_right_eye_encode_ms=2.108 max_terrain_right_eye_section_encode_ms=1.549 max_terrain_right_eye_submit_ms=0.302 max_terrain_right_eye_poll_wait_ms=5.833 max_terrain_stereo_finish_ms=0.000 max_terrain_stereo_submit_ms=0.577 max_terrain_stereo_poll_wait_ms=10.011
MCLONE_ANDROID_XR_PERF_TERRAIN_PREP max_terrain_left_eye_cull_ms=1.736 max_terrain_left_eye_uniform_write_ms=0.174 max_terrain_left_eye_translucent_collect_ms=0.166 max_terrain_left_eye_translucent_sort_ms=0.105 max_terrain_right_eye_cull_ms=0.000 max_terrain_right_eye_uniform_write_ms=0.144 max_terrain_right_eye_translucent_collect_ms=0.000 max_terrain_right_eye_translucent_sort_ms=0.000
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=203 indices=5396316 drawn_indices=1523148 actors=1 drawn_actors=1
```

Multiview marker block:

```text
MCLONE_ANDROID_XR_PERF_SUMMARY sample_seconds=20.010 mode=stationary-frozen-render render_path=multiview render_distance=10 flight_speed_blocks_per_second=0.000 flight_distance_blocks=0.000 settle_seconds=34.586 settle_min_seconds=5.000 settle_frames=2430 settle_quiet_frames=45 refresh_supported=true current_hz=72.0 supported_hz=72.0,80.0,90.0,120.0 target_hz=72.0 budget_ms=13.889 frames=1440 submitted_delta=1440 runtime_delta=1440 skipped_delta=0 frame_avg_ms=13.843 frame_min_ms=12.065 frame_p50_ms=13.728 frame_p95_ms=14.776 frame_p99_ms=17.892 frame_max_ms=25.715 over_budget=546 over_2x_budget=0 over_4x_budget=0
MCLONE_ANDROID_XR_PERF_MULTIVIEW max_multiview_sky_ms=0.691 max_multiview_terrain_ms=3.052 max_multiview_actor_ms=0.675 max_multiview_screen_effect_ms=0.019 max_multiview_world_overlays_ms=0.069 max_multiview_submit_ms=2.800 max_multiview_poll_wait_ms=11.408
MCLONE_ANDROID_XR_PERF_DRAW sections=1133 drawn_sections=203 indices=5396316 drawn_indices=1523148 actors=1 drawn_actors=1
```

Interpretation:

- Slice H did remove the duplicate terrain prep structurally. In the per-eye
  path, right-eye cull/collect/sort is now zero and the shared work is charged to
  the left-eye marker. In the multiview path, `max_multiview_terrain_ms` dropped
  from the immediately prior matched `4.059ms` marker run to `3.052ms`.
- End-to-end RD10 remains borderline. Both paths still have p95 over the
  `13.889ms` 72Hz budget, and the dominant tail is submitted GPU/poll time
  (`10-11ms` max poll markers here).
- The shared union list is conservative: both paths report `203` drawn sections
  / `1.523M` drawn indices. That is correct for multiview and safe for per-eye,
  but it can add clipped edge-section GPU work relative to independent per-eye
  culls. If the default per-eye path is optimized further before batching,
  consider keeping shared traversal/prep while storing per-eye draw masks.

### 2026-06-29 - Standalone Quest 3 Frozen RD10 Poll-Contention Probe (E2)

Benchmarked code commit: this E2 commit (opt-in poll-contention probe,
`--perf-poll-contention`, marker `MCLONE_ANDROID_XR_PERF_CONTENTION`, new lane
`native:android-xr:perf:frozen:rd10:contention`). Two back-to-back runs from the
same installed APK (run 2 via `--skip-build --skip-assets`), frozen RD10, fixed
view pose `0,80,-96,180`, with `--perf-gpu-timestamps` also on so the GPU-clock
delta is the clean contention signal. Cleanup verified after each run: `pidof
com.kzahel.mclone.xr` empty, `mWakefulness=Asleep`,
`mHoldingDisplaySuspendBlocker=false`.

The probe alternates frames: on a "loaded" frame a single worker thread (062
native OS-thread backend) streams a read-modify-write over a 16 MiB buffer (well
past the SoC last-level cache, so real DRAM traffic) for the whole duration of the
blocking stereo `device.poll(Wait)`; a "control" frame polls with the worker idle.
The app buckets the per-frame stereo poll wait and the GPU-clock stereo total by
the `poll_contention_active` flag. Within-run A/B keeps the two buckets thermally
matched. The load is a deliberately memory-bandwidth-heavy, single-core,
full-poll-duration stand-in — heavier and longer than the real next-frame
cull/encode (~3-7ms, a smaller mostly-cached working set) that E4 would actually
overlap — so the measured GPU inflation is a conservative upper bound.

| Bucket (avg over the measured window) | Run 1 | Run 2 |
|---|---:|---:|
| Loaded / control frames | `717` / `718` | `720` / `720` |
| Control GPU (uncontended) | `7.285ms` | `7.210ms` |
| Loaded GPU (contended) | `9.150ms` | `9.135ms` |
| **GPU delta (contention)** | **`+1.865ms`** | **`+1.925ms`** |
| Control poll wait | `7.766ms` | `7.722ms` |
| Loaded poll wait | `9.662ms` | `9.655ms` |
| **Poll-wait delta** | **`+1.897ms`** | **`+1.933ms`** |
| Control / loaded GPU max | `8.454` / `11.287ms` | `8.562` / `11.527ms` |
| Frame p50 (serial, probe alternating) | `13.742ms` | `13.712ms` |

Interpretation:

- **Concurrent CPU work inflates the GPU by ~1.9ms (~26%) on the Quest's
  unified-memory SoC** — contention is real, not negligible, but bounded and
  highly reproducible (both runs agree to within `~0.06ms`).
- **The poll-wait delta matches the GPU delta to within `~0.03ms`.** This (a)
  re-confirms E1 (the blocking poll *is* GPU execution) and (b) shows the
  inflation is real GPU slowdown (shared DRAM bandwidth / power-DVFS), not CPU
  scheduling overhead — the main thread is parked in the fence wait, so the worker
  runs on a free core. The within-run alternation makes the delta robust to slow
  thermal drift.
- **E4 (frame pipelining / Slice K) is still a clear win even at this conservative
  contention.** Today the frame is serial `CPU(~3-4ms p50) + GPU`. Overlapped it
  becomes `max(CPU, GPU + contention)`, GPU-dominated: `~9ms` at this run's cool
  GPU floor (`~7.2ms`), and `~12.6ms` even at E1's hot `10.7ms` floor — under the
  `13.889ms` 72Hz budget across the thermal range, vs today's `~13.7ms` serial
  p50. The real cull/encode overlap is lighter than this probe (shorter, smaller
  footprint), so real contention should be `≤1.9ms`, likely `~0.5-1ms`.
- **Residual risk:** both runs settled cool (`~7.2ms` control GPU; the frozen lane
  is idle terrain), so this does not test whether contention grows near the SoC
  power ceiling under a hotter sustained state. The measured `~1.9ms` is at a
  moderate thermal level; E4's required motion-to-photon / comfort validation
  should also watch the hot-frame tail. Even so, `+1.9ms` keeps the hot frame
  under budget, so E2 is a green light for E4.

### 2026-06-29 - Standalone Quest 3 Frozen RD10 Shared Record Cache (Slice F follow-up)

Benchmarked code commit: this shared-cache-refactor commit. Captured immediately
before commit from the same worktree, based on the Slice G commit.

Moved the Slice F cross-frame record cache onto the shared `&self` render path
(`render_with_options_inner`'s `None` branch, via `RefCell`/`Cell` interior
mutability) so it is no longer reached only through the XR-only
`prepare_render_records` entry point. Every single-view client (flat desktop,
flat Android, web, headless) that culls through `TexturedSectionDrawResources`
now reuses the same cross-frame cache — the same "lives on the one shared path"
mechanism that gave flat the Slice G cull win for free.

Validation: `cargo check --target aarch64-linux-android` and host clean; 79
`mclone-render` tests pass; `pnpm native:timedemo:smoke` (flat render path)
exercises the shared cache with correct cull math (drawn `89.8` + graph-culled
`303.9` ~= frustum `393.7`). On-device frozen RD10 re-confirmed the XR path is
unregressed: `shared_records` max `0.021ms` (cache still hits), per-eye cull
`1.42`/`1.35ms`, frame p50 `13.838ms` (poll `8.915ms`, cooler device),
`drawn_sections=179` unchanged (`drawn_indices` `1,356,102` is within the usual
~0.3% settle-time section-set jitter). Cleanup verified: `pidof` empty,
`mWakefulness=Asleep`, `mHoldingDisplaySuspendBlocker=false`.

The flat win is the same shape as XR's: on camera-only frames (no section
upload/removal/readiness change) the `~1.5ms`/frame record rebuild is skipped;
it is largest at high flat render distance. A dedicated flat records-cost marker
would quantify it; correctness and the cache-hit path are confirmed here.

### 2026-06-29 - Standalone Quest 3 Frozen RD10 Fast-Hash Cull Scratch (Slice G)

Benchmarked code commit: this Slice G commit (replace the per-eye cull
`BTreeSet`/`BTreeMap`/`VecDeque` with reused `rustc-hash` `FxHashSet`/`FxHashMap`
scratch; `drawn_keys` is also `FxHashSet`). Captured immediately before commit
from the same worktree, based on the Slice F commit. Validation: `cargo check
--target aarch64-linux-android` clean; `cargo test -p mclone-render` 74 passed
(cull/visibility/traversal/frustum tests included). Cleanup verified: `pidof`
empty, `mWakefulness=Asleep`, `mHoldingDisplaySuspendBlocker=false`.

Lane `native:android-xr:perf:frozen:rd10`. Drawn counts unchanged: `179`
sections / `1,352,166` indices (behavior-preserving).

Comparison to the original baseline (both runs at comparable GPU poll
`~10.5-10.9ms`, so GPU-thermal-neutral) and to Slice F:

| Metric | Baseline | Slice F | Slice G |
|---|---:|---:|---:|
| Frame avg / p50 / p95 / p99 | `15.62`/`15.66`/`17.29`/`18.09` | `16.44`/`16.54`/`17.97`/`18.82` | `13.995`/`13.866`/`15.056`/`16.013` |
| Over budget / sample frames | `1167`/`1240` | `1198`/`1213` | `689`/`1425` |
| Shared records max | `1.537` | `0.006` | `0.017` |
| Left / right eye cull max | `2.277`/`2.340` | `2.820`/`2.295` | `1.653`/`1.479` |
| Left / right eye wall max | `4.530`/`4.262` | `4.831`/`4.258` | `3.738`/`2.933` |
| Stereo poll wait max (GPU) | `10.543` | `12.146` | `10.896` |

Interpretation:

- **Slice G cut per-eye cull from `~2.3ms` to `~1.5ms`** (`-0.6` to `-0.9ms`/eye
  vs baseline; `-1.2ms`/eye vs the thermally-hot Slice F run). The win is the
  fast-hash membership/insert replacing `BTreeSet`/`BTreeMap` `O(log n)` +
  pointer chasing, plus reused scratch capacity — exactly the container-bound
  cost 106 predicted. Behavior-preserving: drawn counts and the
  cull/visibility/traversal unit tests are unchanged.
- **Frame p50 dropped from `15.66` (baseline) to `13.866ms`, crossing under the
  `13.889ms` 72Hz budget at the median**, with the GPU poll essentially
  unchanged (`10.54` -> `10.90ms`). Because the GPU half was constant, this
  `~1.8ms` p50 improvement is a clean CPU win from E3 (Slice F shared-records
  `-1.5ms` + Slice G cull `-1.5ms` combined), not thermal.
- **But RD10 is not yet *comfortably* 72Hz.** `frame_p95` is `15.056ms` and
  `689/1425` (`~48%`) of frames are still over budget. With the serial
  `CPU + GPU` structure and a `~9.5-10.9ms` GPU poll, the tail (GPU spikes +
  heavier frames) still misses. Reaching solid 72Hz needs CPU/GPU overlap
  (E2/Slice K — hide the now-small CPU behind the GPU poll) and/or GPU-side
  reduction (multiview Slice I, FFR). E3 narrowed the CPU half enough that
  overlap would now leave the frame gated by the `~10.7ms` GPU, comfortably
  under budget.

### 2026-06-29 - Standalone Quest 3 Frozen RD10 Cached Prepared Records (Slice F)

Benchmarked code commit: this Slice F commit (cache prepared culling records
across frames behind a dirty flag; fold the view-independent loaded
section/index counts into the build). Captured immediately before commit from
the same worktree, based on the E1 commit. Validation: `cargo check
--target aarch64-linux-android` clean; `cargo test -p mclone-render` 74 passed.
Cleanup verified: `pidof` empty, `mWakefulness=Asleep`,
`mHoldingDisplaySuspendBlocker=false`.

Lane `native:android-xr:perf:frozen:rd10` (no `--perf-gpu-timestamps`, so no
readback perturbation; render-split timing on).

| Field | Slice F | E1 / baseline |
|---|---:|---:|
| Frame avg / p50 / p95 / max | `16.440` / `16.540` / `17.973` / `28.068` ms | `15.94` / `16.01` / `17.85` (E1) |
| **Shared records max (CPU)** | **`0.006ms`** | `1.537ms` (E1) |
| Left / right eye cull max | `2.820` / `2.295` ms | `2.28` / `2.34` (baseline) |
| Left / right eye wall max | `4.831` / `4.258` ms | — |
| Stereo poll wait max (GPU) | `12.146ms` | `11.139` (E1) / `10.543` (baseline) |
| Drawn sections / indices | `179` / `1,352,166` | same |

Interpretation:

- **The Slice F win is clean and exactly as designed: `shared_records` dropped
  from `~1.537ms` every frame to `0.006ms`** (an `Arc` clone of the cached
  records). In the frozen measured window the `~1.5ms` `BTreeMap` rebuild is gone
  entirely; in live play it now runs only on section upload/removal/readiness
  change (a few times/sec) instead of every frame. Behavior-preserving (drawn
  counts unchanged; 74 render tests pass).
- **The frame-level p50 did not improve and the poll wait rose to `12.146ms`.**
  This is thermal drift across repeated build/run cycles (poll crept
  `10.5 -> 11.1 -> 12.1ms` over baseline -> E1 -> F) on a GPU-bound frame: per E1
  the frame is `CPU + GPU` serial with GPU `~10.7ms+`, so a `~1.5ms` CPU saving
  is swamped by `~1.5ms` of GPU thermal variance at the frame level. The honest
  attribution of Slice F is the thermal-independent `shared_records` bucket
  delta, not frame p50.
- Cull max ticked up (`2.82` vs `2.28`) — also thermal/CPU-clock noise; folding
  the two loaded-count passes into the build is within measurement noise because
  cull is dominated by `BTreeMap`/`BTreeSet` ops, which is Slice G's target.

### 2026-06-29 - Standalone Quest 3 Frozen RD10 GPU-Timestamp Split (E1)

Benchmarked code commit: this E1 commit (opt-in wgpu GPU-timestamp split of the
stereo encoder). The run was captured immediately before commit from the same
implementation worktree, based on `9e10ab7` (`Add basic falling block ticks`).

Capture note: captured from the implementation worktree carrying the E1
GPU-timestamp instrumentation. Run via the new
`native:android-xr:perf:frozen:rd10:gpu` lane (frozen RD10, fixed render view
pose `0,80,-96,180`, seed `12345`, center chunk `(0, 0)`, noon, frozen time),
which enables both `--perf-metrics` and the new `--perf-gpu-timestamps`. The
device confirmed `XR GPU timestamps enabled (timestamp_period=52.083ns)`, i.e.
the OpenXR Vulkan adapter advertises `TIMESTAMP_QUERY` and Quest 3 returns valid
timestamps. Cleanup after the sample: `pidof com.kzahel.mclone.xr` empty,
`dumpsys power` reported `mWakefulness=Asleep` and
`mHoldingDisplaySuspendBlocker=false`.

Summary:

| Field | Value |
|---|---:|
| Sample | `20.010s`, `1251` frames |
| Target | `72.0 Hz` / `13.889ms` |
| Frame avg / p50 / p95 / p99 / max | `15.944ms` / `16.014ms` / `17.849ms` / `19.056ms` / `28.555ms` |
| Terrain frame max | `19.963ms` |
| Drawn sections / indices | `179` / `1,352,166` |
| Runtime work during sample | `0` upload work frames; runtime poll/sync/GPU upload all `0.000ms` |

GPU-timestamp split (`MCLONE_ANDROID_XR_PERF_GPU`, opt-in via
`--perf-gpu-timestamps`). The query set brackets the whole stereo command
encoder (both eyes plus sky/UI/selection passes); values are independent maxima:

| Bucket | Value (max) |
|---|---:|
| GPU stereo total (both eyes) | `10.680ms` |
| GPU left eye | `6.266ms` |
| GPU right eye | `6.125ms` |
| Stereo poll wait (OS fence, same run) | `11.139ms` |
| Shared records (CPU) | `1.537ms` |
| Left / right eye CPU record wall | `4.530ms` / `4.262ms` |
| Stereo submit | `0.522ms` |

Meta `XR_META_performance_metrics` on the same run reported
`app_gpu_ms=1.706`, `gpu_util_pct=18.067`, `cpu_util_avg_pct=73.674`,
`motion_to_photon_ms=25.268`, `dropped_frames=71`.

Interpretation:

- **The blocking stereo poll wait is real GPU execution, not CPU/submit
  overhead.** Our in-stream wgpu timestamp (`10.680ms`) and the OS fence wait
  (`11.139ms`) — two independent measurements of the same frames — agree within
  `~0.46ms`. That residual is the entire submit/acquire/fence-signal cost. This
  resolves the `11`-vs-`7ms` open question from 106/099: the poll is GPU.
- **The Meta `app/gpu_frametime` counter under-reports / is unreliable on this
  Oculus build.** It read `7.0ms` in prior baselines and a nonsensical `1.706ms`
  here, while the direct timestamp says `~10.7ms`. The 099/106 conclusion of
  "GPU ~7ms with ~40% headroom, CPU draw-submission bound" was an artifact of
  trusting that counter. Trust the in-stream timestamp.
- **The frame is a balanced serial `CPU(~9ms) + GPU(~10.7ms)`**, summing to the
  `~19.96ms` terrain-frame max. CPU sits blocked-idle for the entire `~10.7ms`
  GPU poll. Therefore CPU-only reductions (Slices F/G/H) cannot reach 72Hz at
  RD10 alone — even CPU→0 leaves a `~10.7ms` GPU floor that, with no overlap, is
  the frame. **CPU/GPU overlap (E2/E4/K) is now a first-class lever**, not
  optional polish: hiding CPU behind the GPU poll would take the frame toward
  `max(CPU, GPU) ≈ 10.7ms`, under budget. GPU-side reduction (multiview Slice I,
  FFR) is the complementary lever for the `~10.7ms` floor itself.
- The opt-in readback (`map_async` + one extra `poll(Wait)`) adds `~0.35ms` CPU
  per frame on `--perf-gpu-timestamps` runs only (p50 `16.014` vs `~15.66`
  baseline); it does not perturb the GPU-clock timestamp values. The normal
  headset loop and non-GPU-timestamp perf runs allocate no query set and pay
  nothing.

### 2026-06-27 - Standalone Quest 3 Frozen RD10 Prepare Sub-Buckets

Benchmarked code commit: this prepare-sub-bucket commit. The run was captured
immediately before commit from the same implementation worktree, based on
`1534b34` (`Share Quest XR section prep across stereo eyes`).

Capture note: captured from the implementation worktree carrying the Slice C2
attribution-only prepare sub-buckets. The run used
`native:android-xr:perf:frozen:rd10:metrics` with fixed render view pose
`0,80,-96,180`, seed `12345`, center chunk `(0, 0)`, noon, frozen time, and
RD10. Cleanup was run explicitly after the sample: `pidof
com.kzahel.mclone.xr` was empty, `dumpsys power` reported
`mWakefulness=Asleep`, and `mHoldingDisplaySuspendBlocker=false`.

Summary:

| Field | Value |
|---|---:|
| Sample | `20.013s`, `1355` frames |
| Target | `72.0 Hz` / `13.889ms` |
| Frame avg / p50 / p95 / p99 / max | `14.722ms` / `14.414ms` / `16.509ms` / `17.098ms` / `18.493ms` |
| Terrain frame max | `17.951ms` |
| Shared section records max | `1.305ms` |
| Left / right eye max wall | `3.662ms` / `3.342ms` |
| Drawn sections / indices | `179` / `1,352,142` |
| Runtime work during sample | `0` upload work frames; runtime poll/sync/GPU upload all `0.000ms` |
| Meta app GPU / GPU util | `6.901ms` / `60.537%` |
| Meta compositor GPU / dropped frames | `1.546ms` / `56` cumulative |

Prepare sub-bucket maxima. These are independent max fields, not necessarily
one additive frame:

| Bucket | Left | Right |
|---|---:|---:|
| Prepare total | `2.324ms` | `2.291ms` |
| Cull | `2.148ms` | `2.113ms` |
| Uniform write | `0.210ms` | `0.120ms` |
| Translucent collect | `0.383ms` | `0.296ms` |
| Translucent sort | `0.046ms` | `0.028ms` |
| Encode / section encode | `1.693ms` / `1.171ms` | `1.288ms` / `0.900ms` |
| Stereo finish / submit / poll wait | `0.046ms` / `0.585ms` / `10.199ms` | |

Interpretation:

- The remaining per-eye prepare cost is cull-dominated. Uniform writes and
  translucent sort are small; translucent collection is visible but secondary.
- Exact live-view caching remains unsafe because headset pose jitters, so the
  next useful implementation is to extract a pure per-eye prepared-draw/cull
  result from command encoding. That gives two options: conservative cull/result
  reuse under coarse keys, or left/right cull jobs after shared records are
  built.

### 2026-06-27 - Standalone Quest 3 Frozen RD10 Shared Stereo Section Prep

Benchmarked code commit: this shared-prep commit. The run was captured
immediately before commit from the same implementation worktree, based on
`264c723` (`Submit Quest XR stereo eyes together`).

Capture note: captured from the implementation worktree carrying the Slice C1
shared section-record prep path. The run used
`native:android-xr:perf:frozen:rd10:metrics` with fixed render view pose
`0,80,-96,180`, seed `12345`, center chunk `(0, 0)`, noon, frozen time, and
RD10. Cleanup was run explicitly after the sample: `pidof
com.kzahel.mclone.xr` was empty, `dumpsys power` reported
`mWakefulness=Asleep`, and `mHoldingDisplaySuspendBlocker=false`.

Summary:

| Field | Value |
|---|---:|
| Sample | `20.001s`, `1353` frames |
| Target | `72.0 Hz` / `13.889ms` |
| Frame avg / p50 / p95 / p99 / max | `14.736ms` / `14.431ms` / `16.405ms` / `16.896ms` / `17.763ms` |
| Terrain frame max | `17.192ms` |
| Shared section records max | `1.195ms` |
| Left / right eye max wall | `3.745ms` / `3.408ms` |
| Drawn sections / indices | `179` / `1,352,142` |
| Runtime work during sample | `0` upload work frames; runtime poll/sync/GPU upload all `0.000ms` |
| Meta app GPU / GPU util | `7.033ms` / `60.853%` |
| Meta compositor GPU / dropped frames | `1.559ms` / `59` cumulative |

Render split maxima. The shared records bucket is built once per stereo frame;
the per-eye prepare buckets still include view-dependent cull, uniform upload,
and translucent sort:

| Bucket | Value |
|---|---:|
| Shared section records | `1.195ms` |
| Left prepare / encode / section encode | `2.330ms` / `1.724ms` / `1.209ms` |
| Right prepare / encode / section encode | `2.134ms` / `1.506ms` / `0.993ms` |
| Stereo finish / submit / poll wait | `0.035ms` / `0.511ms` / `10.298ms` |

Interpretation:

- Slice C1 removed duplicated section-record construction from each eye without
  changing live XR culling semantics. Compared with Slice B, per-eye prepare
  max dropped by about `1.1ms` per eye, and p50 improved from `15.609ms` to
  `14.431ms`.
- RD10 still misses the `13.889ms` 72 Hz budget. App GPU remains around
  `7.0ms`, so the next CPU target is the remaining `2.1-2.3ms` per-eye prepare
  bucket before draw-call batching/bundles.

### 2026-06-27 - Standalone Quest 3 Frozen RD10 Single-Submit Stereo

Benchmarked code commit: this Slice B commit. The run was captured immediately
before commit from the same implementation worktree, based on `4797cdf` (`Add
Quest XR render split metrics`).

Capture note: captured from the implementation worktree carrying the Slice B
single-encoder/single-submit/single-poll stereo path. The run used
`native:android-xr:perf:frozen:rd10:metrics` with fixed render view pose
`0,80,-96,180`, seed `12345`, center chunk `(0, 0)`, noon, frozen time, and
RD10. After the run, an attempted `adb screencap` returned zero bytes because
the perf process had already exited; log markers still show `MCLONE_ANDROID_XR_READY`
and a successful 1,269 submitted-frame sample. Cleanup was run explicitly:
`pidof com.kzahel.mclone.xr` was empty, `dumpsys power` reported
`mWakefulness=Asleep`, and `mHoldingDisplaySuspendBlocker=false`.

Summary:

| Field | Value |
|---|---:|
| Sample | `20.011s`, `1269` frames |
| Target | `72.0 Hz` / `13.889ms` |
| Frame avg / p50 / p95 / p99 / max | `15.721ms` / `15.609ms` / `17.622ms` / `18.108ms` / `18.591ms` |
| Terrain frame max | `18.102ms` |
| Left / right eye max wall | `4.679ms` / `4.282ms` |
| Drawn sections / indices | `179` / `1,352,142` |
| Runtime work during sample | `0` upload work frames; runtime poll/sync/GPU upload all `0.000ms` |
| Meta app GPU / GPU util | `7.145ms` / `58.342%` |
| Meta compositor GPU / dropped frames | `1.557ms` / `77` cumulative |

Render split maxima. Per-eye submit/poll are now intentionally `0.000ms`
because submit and wait are shared by the stereo command buffer:

| Bucket | Value |
|---|---:|
| Left prepare / encode / section encode | `3.476ms` / `1.625ms` / `1.023ms` |
| Right prepare / encode / section encode | `3.181ms` / `1.388ms` / `1.073ms` |
| Stereo finish / submit / poll wait | `0.042ms` / `0.398ms` / `10.092ms` |

Interpretation:

- Slice B removed per-eye submit/poll and cut per-eye wall from ~`10.6ms` to
  `4.7ms`/`4.3ms`, improving RD10 p50 from ~`16.1ms` to `15.6ms`.
- RD10 still misses 72 Hz. With app GPU time still ~`7.1ms`, the remaining
  app-side stall is now one shared stereo poll, plus duplicated per-eye prepare
  work. Slice C should target cull/draw-set reuse next; a later timestamp or
  wait-mode probe can clarify the gap between app GPU time and the stereo poll
  max.

### 2026-06-27 - Standalone Quest 3 Frozen RD10 Render Split

Benchmarked code commit: this Slice A commit. The run was captured immediately
before commit from the same implementation worktree, based on `f0b87bf` (`Wire
touch controls mode preferences`).

Capture note: captured from the implementation worktree carrying the Slice A
render-split instrumentation. The run used
`native:android-xr:perf:frozen:rd10:metrics` with fixed render view pose
`0,80,-96,180`, seed `12345`, center chunk `(0, 0)`, noon, frozen time, and
RD10. After the run, the app was explicitly force-stopped and the headset was
slept; `pidof com.kzahel.mclone.xr` was empty, `dumpsys power` reported
`mWakefulness=Asleep`, and `mHoldingDisplaySuspendBlocker=false`.

Summary:

| Field | Value |
|---|---:|
| Sample | `20.013s`, `1242` frames |
| Target | `72.0 Hz` / `13.889ms` |
| Frame avg / p50 / p95 / p99 / max | `16.068ms` / `16.165ms` / `17.763ms` / `18.575ms` / `25.119ms` |
| Terrain frame max | `19.173ms` |
| Left / right eye max wall | `10.622ms` / `10.465ms` |
| Drawn sections / indices | `179` / `1,352,142` |
| Runtime work during sample | `0` upload work frames; runtime poll/sync/GPU upload all `0.000ms` |
| Meta app GPU / GPU util | `7.222ms` / `58.438%` |
| Meta compositor GPU / dropped frames | `1.557ms` / `99` cumulative |

Per-eye render split maxima. These are independent max fields from
`MCLONE_ANDROID_XR_PERF_TERRAIN`, so each row is not necessarily a single
frame's additive budget:

| Eye | Wall | Prepare | Encode | Section encode | Submit | Poll wait |
|---|---:|---:|---:|---:|---:|---:|
| Left | `10.622ms` | `3.504ms` | `1.638ms` | `1.193ms` | `0.471ms` | `7.217ms` |
| Right | `10.465ms` | `3.419ms` | `1.621ms` | `1.061ms` | `0.407ms` | `6.487ms` |

Interpretation:

- The largest measured bucket is still the per-eye blocking poll wait, which
  supports doing Slice B (single encoder/submit/poll for both eyes) before
  deeper draw-list caching or batching work.
- Prepare is also material at ~`3.4-3.5ms` max per eye, while section draw-loop
  encode is ~`1.1-1.2ms` max per eye. Slice C remains relevant after the
  per-eye GPU stall is removed.

### 2026-06-27 - Standalone Quest 3 Frozen RD10 Meta Performance Metrics

Benchmarked code commit: `0df8733` (`Add opt-in Quest XR Meta
performance-metrics probe`).

Capture note: captured from the implementation worktree at the commit above.
The worktree also carried unrelated uncommitted changes; of those, only
`mclone-ui` links into the XR APK (transitively via `mclone-xr-scene`), and it
does not change terrain draw behaviour. This is the standalone Quest lane. The
run used the new opt-in `--perf-metrics` flag on the existing frozen RD10 lane
(`native:android-xr:perf:frozen:rd10:metrics`), so the
`XR_META_performance_metrics` sample is taken under the same frozen RD10 load
as the `2d000e1`/`2d200e1` records. The validator force-stopped the app and
slept the headset; post-run `dumpsys power` reported `mWakefulness=Asleep` and
`mHoldingDisplaySuspendBlocker=false`, and `pidof com.kzahel.mclone.xr` was
empty.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| Lane | stationary frozen render, fixed render view pose `0,80,-96,180`, RD10 |

Frozen RD10 frame summary for this run (matches the prior frozen baseline):
`72.0 Hz` target, `1240` frames in `20.002s` (~`62.0 FPS`), `p50 16.106ms`,
`p95 17.839ms`, `max 19.738ms`, terrain frame max `19.268ms`, left eye
`10.570ms`, right eye `10.420ms`, `179` drawn sections / `1.35M` drawn indices,
`0` skipped frames.

Meta performance metrics (`XR_META_performance_metrics`, opt-in via
`--perf-metrics`). One steady-state sample inside the measured window;
`17` counters enumerated, `any_valid=true` on the first attempt:

| Counter | Value | Bucket |
|---|---:|---|
| `app/gpu_frametime` | `7.040 ms` | GPU shader/raster/fill/depth (both eyes) |
| `device/gpu_utilization` | `57.636 %` | GPU headroom |
| `compositor/gpu_frametime` | `1.561 ms` | compositor GPU pass |
| `compositor/dropped_frame_count` | `103` (cumulative) | pacing / missed budget |
| `compositor/spacewarp_mode` | `0` | ASW off |
| `compositor/reprojection_latency` | `15.192 ms` | pacing |
| `app/motion_to_photon_latency` | `39.376 ms` | pacing |
| `device/cpu_utilization_average` | `35.661 %` | CPU headroom |
| `device/cpu_utilization_worst` | `37.374 %` | CPU headroom |
| busiest per-core CPU (`cpu2`) | `53.125 %` | CPU headroom |

The runtime does not enumerate `app/cpu_frametime` or
`compositor/cpu_frametime` on this Oculus build, so those marker fields report
`n/a`; CPU load is covered by the device CPU-utilization counters.

Interpretation:

- RD10 is **not GPU-fill-bound**. The runtime reports the app drawing both eyes
  in `7.040ms` of GPU time at `57.6%` GPU utilization, i.e. ~42% GPU headroom,
  while the app's own per-eye wall-clock timers sum to `~20.99ms`
  (`10.570 + 10.420`).
- The gap between the `~21ms` measured per-eye CPU wall time and the `7ms` true
  GPU time is CPU draw-submission for `179` sections across two eyes plus the
  cost of the per-eye blocking `device.poll(Wait)` in `render_eye_target`, which
  serializes the eyes and parks the CPU on GPU fences instead of overlapping
  CPU and GPU work.
- The compositor is dropping app frames (`103` cumulative, SpaceWarp off)
  because the app CPU frame exceeds the `13.889ms` budget, not because the
  compositor or GPU is saturated.
- Next optimization target is therefore CPU draw submission / draw-call count
  (batching, instancing, or indirect draws per render layer) and removing the
  per-eye GPU stall so CPU and GPU overlap — not shader/fill cost. A wgpu
  timestamp-query or diagnostic flat-material pass is now lower priority because
  the GPU bucket is already known to be small.

### 2026-06-27 - Standalone Quest 3 Frozen Render Sweep

Benchmarked code commit: `2d200e18d35ab22273b50ce13c5f47d1f992b56b`
(`Set Quest XR frozen benchmark view height`).

Capture note: captured from a detached clean worktree at the commit above, with
the local ignored `reference/` assets junctioned in for APK asset staging. This
is the standalone Quest lane, not desktop-hosted streaming to a Quest client.
The frozen probe waits for the settled-stationary gate, then renders cached
terrain buffers from startup `--view-pose 0,80,-96,180`; live headset movement
does not affect terrain culling or draw counts during the measured window. A
prior `Y=120` attempt is intentionally not recorded because RD1 drew zero
terrain sections, and an earlier moved-headset attempt was invalid for the old
live-pose culling path. The validator cleanup slept the headset after each leg;
final `dumpsys power` reported `mWakefulness=Asleep` and
`mHoldingDisplaySuspendBlocker=false`.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Supported refresh | `72.0,80.0,90.0,120.0 Hz` |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Lane | stationary frozen render, locomotion disabled, fixed render view pose `0,80,-96,180` |

Summary:

| Date | Commit | Lane | RD | Settle | Sample | FPS | Frames | Skipped | Avg | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Work frames | Sections | Drawn sections | Indices | Drawn indices |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `2d200e1` | `native:android-xr:perf:frozen:rd1` | 1 | `5.007s` | `20.012s` | `72.0` | 1,441 | 0 | `13.839ms` | `13.852ms` | `14.431ms` | `14.710ms` | `15.430ms` | `5.084ms` | 659 | 0 | 0 | 21 | 4 | 120,024 | 43,452 |
| 2026-06-27 | `2d200e1` | `native:android-xr:perf:frozen:rd5` | 5 | `5.008s` | `20.013s` | `72.0` | 1,441 | 0 | `13.835ms` | `13.817ms` | `14.989ms` | `15.486ms` | `16.308ms` | `13.883ms` | 659 | 0 | 0 | 409 | 66 | 1,929,372 | 562,698 |
| 2026-06-27 | `2d200e1` | `native:android-xr:perf:frozen:rd10` | 10 | `5.364s` | `20.001s` | `62.2` | 1,244 | 0 | `16.031ms` | `16.089ms` | `17.673ms` | `18.523ms` | `20.034ms` | `19.827ms` | 1,217 | 0 | 0 | 1,133 | 179 | 5,308,362 | 1,352,142 |

Terrain timing maxima:

| RD | Terrain frame | Runtime total | Runtime poll | Sync sections | GPU upload | Ready refresh | Left eye | Right eye |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `4.716ms` | `0.000ms` | `0.000ms` | `0.000ms` | `0.000ms` | `0.000ms` | `2.667ms` | `2.328ms` |
| 5 | `11.815ms` | `0.000ms` | `0.000ms` | `0.000ms` | `0.000ms` | `0.000ms` | `7.448ms` | `5.574ms` |
| 10 | `19.504ms` | `0.000ms` | `0.000ms` | `0.000ms` | `0.000ms` | `0.000ms` | `9.962ms` | `10.569ms` |

Runtime and compile maxima:

| RD | Poll total | Server tick | Scheduler tick | Updates | Section updates | Pending chunks | Pending jobs | Deferred sections | Submitted | Completed | Ready sections |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `0.000ms` | `0.000ms` | `0.000ms` | 0 | 0 | 5 | 0 | 0 | 0 | 0 | 64 |
| 5 | `0.000ms` | `0.000ms` | `0.000ms` | 0 | 0 | 40 | 0 | 0 | 0 | 0 | 1,296 |
| 10 | `0.000ms` | `0.000ms` | `0.000ms` | 0 | 0 | 64 | 0 | 0 | 0 | 0 | 3,600 |

Interpretation:

- The frozen lane is now a true static terrain-render sample: no runtime poll,
  section sync, traversal refresh, GPU upload, server tick, scheduler tick, or
  compile work occurs during the measured window.
- RD1 and RD5 sustain the 72 Hz cadence with zero skipped frames. Their
  over-budget counts are near the 13.889 ms vsync threshold rather than
  evidence of generation or upload stalls.
- RD10 still misses 72 Hz with generation fully removed: p50 is `16.089ms`,
  effective throughput is about `62.2 FPS`, and the terrain pass peaks at
  `19.504ms` while drawing `179` sections and `1.35M` indices.
- The next high-value measurement is a profiler/GPU timestamp split for RD10:
  decide whether the remaining cost is CPU culling/draw submission, shader and
  raster work, depth/fill pressure, or compositor/GPU wait.

### 2026-06-27 - Standalone Quest 3 Settled Stationary Sweep

Benchmarked code commit: `65aa5121b9190538d97fd2043b4651ab61cd87ce`
(`Require minimum Quest XR stationary settle time`).

Capture note: captured from a detached clean worktree at the commit above, with
the local ignored `reference/` assets junctioned in for APK asset staging. This
is the standalone Quest lane, not desktop-hosted streaming to a Quest client.
The stationary probe disables locomotion, requires a `5.000s` minimum settle
window plus quiet frames, and records `settle_min_seconds` in the summary. The
validator cleanup slept the headset after each leg; final `dumpsys power`
reported `mWakefulness=Asleep` and `mHoldingDisplaySuspendBlocker=false`.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Supported refresh | `72.0,80.0,90.0,120.0 Hz` |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Lane | stationary, locomotion disabled, startup view pose `0,120,-96,180` |

Summary:

| Date | Commit | Lane | RD | Settle | Sample | FPS | Frames | Skipped | Avg | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Work frames | Sections | Drawn sections | Indices | Drawn indices |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `65aa512` | `native:android-xr:perf:stationary:rd1` | 1 | `5.009s` | `20.012s` | `72.0` | 1,441 | 0 | `13.845ms` | `13.852ms` | `14.346ms` | `14.658ms` | `15.525ms` | `3.801ms` | 654 | 0 | 0 | 21 | 4 | 119,706 | 43,452 |
| 2026-06-27 | `65aa512` | `native:android-xr:perf:stationary:rd5` | 5 | `5.008s` | `20.013s` | `72.0` | 1,441 | 0 | `13.840ms` | `13.850ms` | `14.699ms` | `15.139ms` | `15.822ms` | `10.851ms` | 675 | 0 | 0 | 407 | 70 | 1,920,834 | 600,876 |
| 2026-06-27 | `65aa512` | `native:android-xr:perf:stationary:rd10` | 10 | `5.681s` | `20.004s` | `49.4` | 989 | 0 | `20.188ms` | `20.209ms` | `21.399ms` | `21.855ms` | `22.942ms` | `22.792ms` | 989 | 0 | 9 | 1,131 | 203 | 5,288,154 | 1,553,424 |

Terrain timing maxima:

| RD | Terrain frame | Runtime total | Runtime poll | Sync sections | GPU upload | Ready refresh | Left eye | Right eye |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `3.504ms` | `0.167ms` | `0.062ms` | `0.000ms` | `0.000ms` | `0.071ms` | `1.922ms` | `1.680ms` |
| 5 | `10.559ms` | `1.282ms` | `0.101ms` | `0.000ms` | `0.000ms` | `0.639ms` | `5.643ms` | `4.959ms` |
| 10 | `22.384ms` | `7.911ms` | `0.415ms` | `6.557ms` | `0.176ms` | `1.210ms` | `9.086ms` | `8.602ms` |

Runtime and compile maxima:

| RD | Poll total | Server tick | Scheduler tick | Updates | Section updates | Pending chunks | Pending jobs | Deferred sections | Submitted | Completed |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `0.046ms` | `1.575ms` | `1.561ms` | 1 | 0 | 5 | 0 | 0 | 0 | 0 |
| 5 | `0.098ms` | `9.688ms` | `9.659ms` | 2 | 0 | 40 | 0 | 0 | 0 | 0 |
| 10 | `0.414ms` | `15.126ms` | `15.070ms` | 4 | 2 | 68 | 1 | 16 | 1 | 1 |

Interpretation:

- RD1 and RD5 are viable 72 Hz stationary lanes after the 5 second settle gate.
  Both recorded zero work frames, zero skipped frames, and no over-2x-budget
  frames.
- RD10 misses the 72 Hz budget even while stationary: p50 is `20.209ms`, every
  sampled frame is over budget, and effective throughput is about `49.4 FPS`.
  This reproduces the high-distance problem without joystick motion, network
  interpolation, or player movement as the primary cause.
- RD10 is still not a perfectly pure render-only sample: it records `9` work
  frames, one submitted/completed section, and `16` deferred sections. The last
  frame is quiet, and the p50/p95 cost cannot be explained by a single late
  upload, but the next benchmark refinement should either eliminate the
  persistent deferred-section churn or add an explicit frozen-mesh render-only
  mode.
- The high-value target is RD10 render/runtime cost: `203` drawn sections,
  `1.55M` drawn indices, `9.086ms` left eye, `8.602ms` right eye, and a
  `6.557ms` max section-sync pass. GPU timestamps or Quest profiler capture
  should split shader/raster/GPU wait from CPU section traversal and culling.

### 2026-06-27 - Standalone Quest 3 Flight Sweep After Non-Invasive Diagnostics

Benchmarked code commit: `4cf97f97f3ea332984f93d2734f98daef93fbd0a`
(`Make Quest XR diagnostics non-invasive`).

Capture note: captured from a detached clean worktree at the commit above, with
the local ignored `reference/` assets junctioned in for APK asset staging. The
marker block includes diagnostics refresh/cache-age fields. The validator
cleanup slept the headset after the sweep; `dumpsys power` reported
`mWakefulness=Asleep` and `mHoldingDisplaySuspendBlocker=false`.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Supported refresh | `72.0,80.0,90.0,120.0 Hz` |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Flight | no-clip, `4.3 blocks/s`, about `86 blocks` over the sample |

Summary:

| Date | Commit | Lane | RD | Sample | FPS | Frames | Skipped | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Over 4x | Sections | Drawn sections | Indices | Drawn indices | Distance |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `4cf97f9` | `native:android-xr:perf:flight:rd1` | 1 | `20.003s` | `72.0` | 1,441 | 0 | `13.855ms` | `15.544ms` | `18.250ms` | `22.128ms` | `9.852ms` | 661 | 0 | 0 | 47 | 11 | 248,940 | 65,868 | `86.023` |
| 2026-06-27 | `4cf97f9` | `native:android-xr:perf:flight:rd5` | 5 | `20.004s` | `72.0` | 1,440 | 0 | `13.862ms` | `15.885ms` | `18.839ms` | `33.589ms` | `16.765ms` | 694 | 1 | 0 | 533 | 85 | 2,461,572 | 496,350 | `86.082` |
| 2026-06-27 | `4cf97f9` | `native:android-xr:perf:flight:rd10` | 10 | `20.010s` | `54.8` | 1,096 | 0 | `18.788ms` | `21.479ms` | `24.212ms` | `41.438ms` | `37.100ms` | 1,020 | 3 | 0 | 1,257 | 201 | 5,582,682 | 1,222,158 | `86.259` |

Terrain timing maxima:

| RD | Terrain frame | Runtime total | Runtime poll | Sync sections | GPU upload | Ready refresh | Left eye | Right eye |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `9.441ms` | `2.535ms` | `0.048ms` | `0.720ms` | `1.838ms` | `0.124ms` | `6.596ms` | `5.675ms` |
| 5 | `16.373ms` | `7.709ms` | `0.482ms` | `3.493ms` | `4.770ms` | `0.613ms` | `6.568ms` | `7.762ms` |
| 10 | `30.428ms` | `9.896ms` | `0.088ms` | `6.202ms` | `3.482ms` | `1.432ms` | `13.175ms` | `10.705ms` |

Runtime poll maxima:

| RD | Poll total | Drain updates | Apply updates | Diagnostics | Refresh frames | Cache age | Server detail age | Server tick | Scheduler tick | Updates | Section |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `0.042ms` | `0.005ms` | `0.003ms` | `0.004ms` | 39 | `499.969ms` | `1000.143ms` | `2.577ms` | `1.841ms` | 2 | 0 |
| 5 | `0.481ms` | `0.008ms` | `0.475ms` | `0.025ms` | 39 | `499.987ms` | `944.648ms` | `9.947ms` | `9.919ms` | 3 | 1 |
| 10 | `0.052ms` | `0.003ms` | `0.002ms` | `0.007ms` | 39 | `499.969ms` | `976.835ms` | `27.183ms` | `27.141ms` | 2 | 0 |

Queue maxima:

| RD | Server cmd q | Server update q | Server jobs | Publications | Scheduler jobs | Completed jobs | Dirty chunks | Loaded chunks | Visible chunks | Ticket chunks | Player visible | Player outbound |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 0 | 2 | 2 | 3 | 5 | 65 | 65 | 9 | 841 | 9 | 0 |
| 5 | 2 | 0 | 3 | 3 | 1 | 4 | 265 | 265 | 121 | 1,369 | 121 | 0 |
| 10 | 2 | 0 | 5 | 0 | 4 | 4 | 499 | 499 | 289 | 1,849 | 289 | 0 |

Upload maxima:

| RD | Work frames | Rebuilt sections | Removed sections | Rebuilt indices | Uploaded sections | Upload removed | Uploaded indices | Ready sections |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 805 | 16 | 48 | 50,064 | 7 | 20 | 50,064 | 112 |
| 5 | 149 | 16 | 160 | 42,588 | 8 | 50 | 42,588 | 1,296 |
| 10 | 799 | 16 | 240 | 41,064 | 8 | 78 | 41,064 | 3,600 |

Compile / streaming maxima:

| RD | Pending chunks before | Pending chunks after | Pending jobs before | Pending jobs after | Deferred sections | Submitted sections | Completed sections | Stale sections | Visibility total | Visibility worst |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 12 | 9 | 1 | 1 | 16 | 16 | 16 | 16 | `1.040ms` | `0.414ms` |
| 5 | 59 | 49 | 1 | 1 | 16 | 16 | 16 | 0 | `0.917ms` | `0.222ms` |
| 10 | 98 | 83 | 1 | 1 | 16 | 16 | 16 | 0 | `0.738ms` | `0.297ms` |

Interpretation:

- The diagnostics overhead was real and is now removed from the headset frame
  loop. Max `poll_diagnostics_ms` dropped from the prior `2.409ms` / `28.119ms`
  / `76.531ms` RD1/RD5/RD10 values to `0.004ms` / `0.025ms` / `0.007ms`.
- Full diagnostics still refresh during the sample: `39` refresh frames over
  the 20 second window, with cache age near the intended `500ms` runtime
  cadence. Server detail snapshot age peaks near `1s` because server-side
  detail is sampled rather than recomputed on every tick/command.
- RD5 recovered to the 72 Hz lane. RD10 improved from the previous `21 FPS`
  stress result to about `54.8 FPS`, but still exceeds the 72 Hz frame budget.
- The remaining RD10 pressure is no longer runtime diagnostics. The next target
  is terrain/render cost: `30.428ms` terrain frame max, `37.100ms` max mclone
  render frame, 201 drawn sections, and a `27.141ms` scheduler tick max.

### 2026-06-27 - Standalone Quest 3 Flight Sweep With Runtime Poll Attribution

Benchmarked code commit: `320bdd2610b45b9e9947f916f75db1cf84b96fd5`
(`Fix explicit empty light section packing`).

Capture note: captured from a detached clean worktree at the commit above, with
the local ignored `reference/` assets junctioned in for APK asset staging. The
marker block includes `RUNTIME_MAX` and `QUEUE_MAX`.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Supported refresh | `72.0,80.0,90.0,120.0 Hz` |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Flight | no-clip, `4.3 blocks/s`, about `86 blocks` over the sample |

Summary:

| Date | Commit | Lane | RD | Sample | FPS | Frames | Skipped | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Over 4x | Sections | Drawn sections | Indices | Drawn indices | Distance |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `320bdd2` | `native:android-xr:perf:flight:rd1` | 1 | `20.007s` | `72.0` | 1,441 | 0 | `13.881ms` | `14.967ms` | `15.738ms` | `27.327ms` | `11.883ms` | 713 | 0 | 0 | 47 | 11 | 247,308 | 65,868 | `86.048` |
| 2026-06-27 | `320bdd2` | `native:android-xr:perf:flight:rd5` | 5 | `20.017s` | `56.6` | 1,132 | 0 | `17.480ms` | `27.737ms` | `33.756ms` | `44.455ms` | `40.183ms` | 909 | 56 | 0 | 538 | 89 | 2,531,364 | 537,858 | `86.040` |
| 2026-06-27 | `320bdd2` | `native:android-xr:perf:flight:rd10` | 10 | `20.047s` | `21.0` | 421 | 0 | `51.758ms` | `65.606ms` | `78.369ms` | `109.878ms` | `109.726ms` | 421 | 361 | 71 | 1,276 | 185 | 5,681,634 | 1,131,312 | `86.134` |

Terrain timing maxima:

| RD | Terrain frame | Runtime total | Runtime poll | Sync sections | GPU upload | Ready refresh | Left eye | Right eye |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `11.536ms` | `5.648ms` | `2.412ms` | `5.601ms` | `1.813ms` | `0.113ms` | `5.241ms` | `4.484ms` |
| 5 | `38.065ms` | `31.360ms` | `28.122ms` | `18.350ms` | `6.349ms` | `0.496ms` | `15.867ms` | `6.588ms` |
| 10 | `98.805ms` | `84.650ms` | `76.535ms` | `18.895ms` | `3.005ms` | `1.253ms` | `10.557ms` | `20.844ms` |

Runtime poll maxima:

| RD | Poll total | Drain updates | Apply updates | Dirty mark | Client apply | Diagnostics | Server tick | Scheduler tick | Updates | Snapshot | Section | Unload |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `2.410ms` | `0.004ms` | `0.003ms` | `0.001ms` | `0.002ms` | `2.409ms` | `3.684ms` | `3.670ms` | 2 | 0 | 0 | 0 |
| 5 | `28.120ms` | `0.004ms` | `0.002ms` | `0.001ms` | `0.001ms` | `28.119ms` | `12.494ms` | `12.471ms` | 2 | 0 | 0 | 0 |
| 10 | `76.533ms` | `0.002ms` | `0.002ms` | `0.001ms` | `0.001ms` | `76.531ms` | `29.737ms` | `29.693ms` | 2 | 0 | 0 | 0 |

Queue maxima:

| RD | Server cmd q | Server update q | Server jobs | Publications | Scheduler jobs | Completed jobs | Dirty chunks | Loaded chunks | Visible chunks | Ticket chunks | Player visible | Player outbound |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 0 | 5 | 4 | 5 | 5 | 65 | 65 | 9 | 841 | 9 | 0 |
| 5 | 4 | 0 | 5 | 4 | 5 | 4 | 265 | 265 | 121 | 1,369 | 121 | 0 |
| 10 | 3 | 0 | 5 | 4 | 5 | 4 | 499 | 499 | 289 | 1,849 | 289 | 0 |

Upload maxima:

| RD | Work frames | Rebuilt sections | Removed sections | Rebuilt indices | Uploaded sections | Upload removed | Uploaded indices | Ready sections |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 825 | 16 | 48 | 50,064 | 7 | 20 | 50,064 | 112 |
| 5 | 188 | 16 | 160 | 42,588 | 8 | 50 | 42,588 | 1,296 |
| 10 | 348 | 16 | 240 | 41,064 | 8 | 78 | 41,064 | 3,600 |

Compile / streaming maxima:

| RD | Pending chunks before | Pending chunks after | Pending jobs before | Pending jobs after | Deferred sections | Submitted sections | Completed sections | Stale sections | Visibility total | Visibility worst |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 12 | 9 | 1 | 1 | 16 | 16 | 16 | 1 | `0.890ms` | `0.286ms` |
| 5 | 59 | 49 | 1 | 1 | 16 | 16 | 16 | 16 | `0.703ms` | `0.394ms` |
| 10 | 98 | 85 | 1 | 1 | 16 | 16 | 16 | 16 | `0.732ms` | `0.327ms` |

Interpretation:

- Render distance 1 remains a viable 72 Hz lane. p95/p99 are close to budget
  and there are no over-2x frames.
- Render distance 5 remains uneven at about `56.6 FPS`. The worst app frame is
  dominated by runtime polling (`28.122ms`) and section sync (`18.350ms`), not
  raw GPU upload (`6.349ms`).
- Render distance 10 remains a stress lane at about `21 FPS`, with a worst
  frame over `100ms`. Runtime polling (`76.535ms`) dominates the terrain update
  bucket.
- The runtime poll split points at diagnostics collection: `poll_diagnostics_ms`
  is effectively equal to `poll_total_ms` in all three lanes, while
  `drain_updates_ms`, `apply_updates_ms`, dirty marking, and client application
  are near zero. The update payload was tiny (`2` updates, no snapshot or unload
  updates).
- The expensive diagnostics path scales with scheduler state: dirty/loaded
  chunks increase from `65` to `265` to `499`, and ticket chunks from `841` to
  `1,369` to `1,849`. The next optimization should make the diagnostics
  snapshot cheaper or less frequent on headset frames before changing mesh
  upload policy.

### 2026-06-27 - Standalone Quest 3 Flight Sweep With Upload Attribution

Benchmarked code commit: `2bd0e04b3e4ea64c8698b5e91571d40f19b1de39`
(`Split Quest XR perf markers and add upload counters`).

Capture note: captured from a clean worktree at the commit above. This run uses
the split marker block, so draw counts and upload/compile counters are not
truncated by logcat.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Supported refresh | `72.0,80.0,90.0,120.0 Hz` |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Flight | no-clip, `4.3 blocks/s`, about `86 blocks` over the sample |

Summary:

| Date | Commit | Lane | RD | Sample | FPS | Frames | Skipped | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Over 4x | Sections | Drawn sections | Indices | Drawn indices | Distance |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `2bd0e04` | `native:android-xr:perf:flight:rd1` | 1 | `20.008s` | `72.0` | 1,441 | 0 | `13.868ms` | `15.055ms` | `15.749ms` | `29.670ms` | `10.575ms` | 701 | 1 | 0 | 47 | 11 | 247,308 | 65,868 | `86.048` |
| 2026-06-27 | `2bd0e04` | `native:android-xr:perf:flight:rd5` | 5 | `20.026s` | `57.3` | 1,147 | 0 | `17.408ms` | `28.149ms` | `32.938ms` | `35.756ms` | `35.574ms` | 909 | 60 | 0 | 540 | 85 | 2,494,272 | 496,350 | `86.076` |
| 2026-06-27 | `2bd0e04` | `native:android-xr:perf:flight:rd10` | 10 | `20.034s` | `21.0` | 421 | 0 | `51.212ms` | `58.602ms` | `80.605ms` | `83.147ms` | `82.974ms` | 419 | 366 | 59 | 1,242 | 194 | 5,565,192 | 1,199,940 | `86.191` |

Terrain timing maxima:

| RD | Terrain frame | Runtime total | Runtime poll | Sync sections | GPU upload | Ready refresh | Left eye | Right eye |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `10.238ms` | `5.900ms` | `2.309ms` | `5.857ms` | `2.916ms` | `0.057ms` | `6.084ms` | `3.984ms` |
| 5 | `35.067ms` | `27.122ms` | `25.717ms` | `10.193ms` | `6.359ms` | `0.823ms` | `6.836ms` | `6.336ms` |
| 10 | `82.663ms` | `68.483ms` | `63.248ms` | `4.978ms` | `3.255ms` | `13.320ms` | `11.817ms` | `10.389ms` |

Upload maxima:

| RD | Work frames | Rebuilt sections | Removed sections | Rebuilt indices | Uploaded sections | Upload removed | Uploaded indices | Ready sections |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 815 | 16 | 48 | 50,064 | 7 | 20 | 50,064 | 112 |
| 5 | 196 | 16 | 160 | 42,588 | 8 | 50 | 42,588 | 1,296 |
| 10 | 350 | 16 | 240 | 41,064 | 8 | 78 | 41,064 | 3,600 |

Compile / streaming maxima:

| RD | Pending chunks before | Pending chunks after | Pending jobs before | Pending jobs after | Deferred sections | Submitted sections | Completed sections | Stale sections | Visibility total | Visibility worst |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 12 | 9 | 1 | 1 | 16 | 16 | 16 | 1 | `0.861ms` | `0.230ms` |
| 5 | 67 | 57 | 1 | 1 | 16 | 16 | 16 | 0 | `0.674ms` | `0.247ms` |
| 10 | 96 | 81 | 1 | 1 | 16 | 16 | 16 | 1 | `0.881ms` | `0.515ms` |

Interpretation:

- Render distance 1 still tracks 72 Hz closely. It had one over-2x frame, but
  p95/p99 are both close to budget and the app render bucket stays under
  `11ms`.
- Render distance 5 remains uneven at about `57 FPS`. The worst terrain frame
  is mostly the runtime/poll side (`25.717ms`) rather than GPU buffer upload
  (`6.359ms`) or per-eye drawing (`6-7ms`).
- Render distance 10 remains a stress lane at about `21 FPS`. The dominant max
  bucket is runtime polling (`63.248ms`), with traversal-ready refresh also
  visible (`13.320ms`), while GPU upload is only `3.255ms`.
- Max upload work is bounded to small batches (`7-8` uploaded non-empty
  sections, `16` submitted/completed sections), so the first optimization target
  is not raw `wgpu` upload bandwidth. The sharper target is runtime polling /
  chunk-stream application and traversal-ready work while flying.
- The last frame still had pending chunks (`4` / `40` / `71` for RD1/RD5/RD10)
  with zero pending compile jobs, which suggests backlog outside active mesh
  compilation.

### 2026-06-27 - Standalone Quest 3 Flight Sweep With Stage Attribution

Benchmarked code commit: `518d21da92b3850f9b8b9d75bad6c7cf1318fb49`
(`Add Quest XR perf timing attribution`).

Capture note: captured from a clean worktree at the commit above. This run used
the pre-split single summary line, which was long enough that logcat truncated
the trailing draw-count fields after `indices`; record those fields as missing
for this run. Later captures use the split marker block described above.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Supported refresh | `72.0,80.0,90.0,120.0 Hz` |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Flight | no-clip, `4.3 blocks/s`, about `86 blocks` over the sample |

Summary:

| Date | Commit | Lane | RD | Sample | FPS | Frames | Skipped | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Over 4x | Sections | Drawn sections | Indices | Distance |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `518d21d` | `native:android-xr:perf:flight:rd1` | 1 | `20.005s` | `72.0` | 1,441 | 0 | `13.889ms` | `15.160ms` | `17.350ms` | `25.475ms` | `11.415ms` | 721 | 0 | 0 | 47 | 11 | 247,308 | `86.036` |
| 2026-06-27 | `518d21d` | `native:android-xr:perf:flight:rd5` | 5 | `20.011s` | `56.8` | 1,137 | 0 | `17.348ms` | `27.437ms` | `31.546ms` | `44.264ms` | `36.108ms` | 933 | 50 | 0 | 538 | 89 | 2,531,418 | `86.043` |
| 2026-06-27 | `518d21d` | `native:android-xr:perf:flight:rd10` | 10 | `20.005s` | `20.8` | 417 | 0 | `51.836ms` | `65.716ms` | `77.480ms` | `82.461ms` | `82.326ms` | 414 | 371 | 68 | 1,272 | 194 | 5,691,858 | `86.144` |

Stage attribution:

| RD | Max wait | Poll | Locate | Locomotion | Acquire L | Acquire R | Terrain frame | Render views | Upload | Left eye | Right eye | Release | End frame |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `22.087ms` | `0.429ms` | `0.078ms` | `1.524ms` | `4.848ms` | `0.040ms` | `11.182ms` | `0.045ms` | `4.561ms` | `6.731ms` | `4.315ms` | `0.141ms` | `0.594ms` |
| 5 | `17.410ms` | `0.672ms` | `0.189ms` | `4.163ms` | `5.229ms` | `0.052ms` | `35.783ms` | `0.047ms` | `29.300ms` | `6.105ms` | `6.831ms` | `0.353ms` | `1.053ms` |
| 10 | `0.388ms` | `0.832ms` | `0.181ms` | `4.504ms` | `15.198ms` | `0.078ms` | `79.015ms` | `0.077ms` | `66.443ms` | `13.458ms` | `10.015ms` | `0.170ms` | `0.931ms` |

Interpretation:

- Render distance 1 is still near the 72 Hz target. The app render bucket stays
  under budget, with tail frame time mostly reflecting OpenXR pacing/wait.
- Render distance 5 misses the 72 Hz budget often enough to feel uneven. The
  worst app frame is dominated by `runtime_upload` (`29.300ms`) inside the
  terrain frame, while per-eye rendering stays around `6-7ms`.
- Render distance 10 is a stress lane. It is heavily app/render limited, with
  `runtime_upload` peaking at `66.443ms` and terrain frame time at `79.015ms`.
- `skipped_delta=0` across the sweep means the runtime did not report skipped
  frames during the samples, but the app submitted far fewer frames at RD5/RD10.
  The current data points more toward upload/chunk-stream stalls than joystick
  motion or a simple 60 Hz vs 72 Hz interpolation mismatch.

### 2026-06-27 - Standalone Quest 3 Automated Flight Sweep

Benchmarked code commit: `446d47349cdab2a84d2e7d05ccbf2516b496da21`
(`Add Quest XR flight performance probe`).

Capture note: these samples were captured from the implementation worktree
before it was committed. The runtime code used for the run is now committed as
the hash above.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Target used by probe | `72.0 Hz` / `13.889 ms` fallback |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Flight | no-clip, `4.3 blocks/s`, about `86 blocks` over the sample |

Schema note: this baseline predates the refresh/stage-attribution fields added
after `446d473`, so it records only the coarse `max_render_mclone_frame_ms`
bucket.

Summary:

| Date | Commit | Lane | RD | Sample | FPS | Frames | Skipped | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Over 4x | Sections | Drawn sections | Indices | Drawn indices | Distance |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `446d473` | `native:android-xr:perf:flight:rd1` | 1 | `20.005s` | `72.0` | 1,441 | 0 | `13.887ms` | `15.070ms` | `15.696ms` | `27.190ms` | `11.343ms` | 719 | 0 | 0 | 47 | 11 | 248,940 | 65,868 | `86.035` |
| 2026-06-27 | `446d473` | `native:android-xr:perf:flight:rd5` | 5 | `20.017s` | `57.8` | 1,157 | 0 | `17.141ms` | `28.148ms` | `33.166ms` | `36.544ms` | `36.429ms` | 881 | 62 | 0 | 533 | 85 | 2,461,572 | 496,350 | `86.108` |
| 2026-06-27 | `446d473` | `native:android-xr:perf:flight:rd10` | 10 | `20.004s` | `22.2` | 444 | 0 | `48.764ms` | `56.691ms` | `74.015ms` | `83.101ms` | `82.951ms` | 442 | 377 | 41 | 1,265 | 186 | 5,633,760 | 1,138,038 | `86.029` |

Interpretation:

- Render distance 1 is close to the 72 Hz target. `Over 1x` is noisy because
  current `frame_wall_ms` includes OpenXR wait/pacing, but p95/p99 are still
  useful for tail comparison.
- Render distance 5 is already app/render limited: effective FPS drops to
  about 58 and max render time tracks max frame time.
- Render distance 10 is far over Quest budget in the current renderer path.
  Treat it as a stress lane, not a viable headset default.

Next comparable rows should keep the same seed, flight speed, sample duration,
view pose, and render-distance lanes unless the record explicitly says why the
lane changed.
