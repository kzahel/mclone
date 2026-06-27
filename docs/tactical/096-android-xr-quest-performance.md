# 096: Android XR Quest Performance

Status: active; Slice 1 logcat perf probe and automated flight sample landed
on 2026-06-27.

## Purpose

Make Quest standalone performance visible enough to debug headset-visible
jumpiness while walking. The symptom could come from several different layers:

1. missed OpenXR frames or uneven runtime frame pacing,
2. low/default headset refresh such as 72 Hz,
3. CPU stalls in session polling, chunk streaming, render-section upload, or
   per-eye rendering,
4. GPU time exceeding the headset frame budget,
5. movement/interpolation mismatch, such as a 60 Hz simulation sampled by a
   72/90/120 Hz display,
6. joystick axis jitter or locomotion-frame policy,
7. remote-host correction/network behavior.

The goal is to separate those causes with measured headset data instead of
tuning locomotion feel blind.

## Current Evidence

- Desktop/headless performance lanes exist:
  `native:frame-budget:*`, `native:movement-frame:*`, `native:timedemo:*`,
  worldgen, and scheduler perf. Their baselines live in
  [`../performance-records.md`](../performance-records.md).
- Android XR currently has build/install/ready/session smokes through
  `android-xr/validate-quest-openxr.sh`, but those prove startup and first
  submitted terrain frame. They do not report headset frame pacing, CPU/GPU
  timing, refresh, or walking-performance counters.
- `mclone-xr-host::XrFrameStats` tracks submitted/runtime/skipped frames, but
  no Quest perf summary or budget attribution is emitted.
- `mclone-xr-scene` currently feeds the shared menu a fixed XR frame-pacing
  state: `GameFramePacingMode::Vsync` and `XR_UI_FPS_CAP = 90`. That explains
  why the Quest menu can show incorrect refresh information.
- Tactical [`083`](083-android-xr-quest-standalone.md) already lists display
  refresh/foveation toggles and GPU/frame timing diagnostics as runtime
  hardening follow-ups. This tactical is the concrete performance plan for
  that work.

## Playbox Reference

Use Playbox as a pattern library only. Do not copy its egui/runtime shape.

- `~/code/playbox/android-xr/README.md`: Quest `--refresh`,
  `--gpu-timestamps`, post-ready validation, and render-mode timing sweeps.
- `~/code/playbox/src/xr/mod.rs`: `XR_FB_display_refresh_rate` extension
  enablement, supported/current refresh snapshots, refresh requests, XR frame
  timing samples, and minimal performance-overlay wiring.
- `~/code/playbox/src/xr/perf_overlay.rs`: headset-visible minimal performance
  overlay texture/layer and compact CPU/GPU breakdown.
- `~/code/playbox/src/ui/performance.rs`: display refresh, frame budget, render
  rate, active app CPU, and runtime/present wait presentation.
- `~/code/playbox/src/xr/locomotion.rs`: XR locomotion frame-dt clamping and
  smooth movement diagnostics shape.

## Target Shape

- `mclone-xr-host` owns reusable OpenXR display-refresh helpers and frame
  pacing/timing data that can be used by desktop XR and Android XR.
- Android XR collects headset runtime timing after the first ready frame:
  wait-frame wall time, predicted-display deltas where available, view locate,
  scene update, chunk sync/upload, per-eye acquire/render/release, queue submit,
  and end-frame cost.
- `mclone-xr-scene` receives actual XR display-refresh state and exposes it to
  the shared UI instead of hardcoding `90`.
- `mclone-ui` gets an XR-specific options/performance section instead of
  reusing desktop-only frame pacing labels.
- The Quest validator can run a repeatable performance sample, write a compact
  JSON summary under `/tmp`, and check for a logcat marker such as
  `MCLONE_ANDROID_XR_PERF_SUMMARY`.
- The first moving Quest sample should fly rather than walk. Terrain collision
  can block a walking probe and turn it into a physics/collision sample instead
  of a chunk-streaming sample.
- A headset-visible performance overlay can be toggled from the XR menu and
  can stay visible while walking.
- The first pass is diagnostic. Do not change chunk scheduling, render
  distance defaults, locomotion math, or network interpolation until the
  measured bottleneck is known.

## Implementation Slices

### Slice 1 - Quest Logcat Performance Probe

- [x] Add launch-scoped Android XR perf options:
  - `--perf-seconds N`
  - `--perf-flight`
  - `--perf-flight-speed N`
- [x] Extend `android-xr/validate-quest-openxr.sh` with a
  `--perf-seconds N` mode that waits for ready, samples post-ready frames, then
  keeps normal force-stop/restore cleanup behavior.
- [x] Emit a summary log marker with:
  - runtime frames, submitted frames, skipped frames,
  - frame wall p50/p95/p99/max,
  - frames over 1x/2x/4x budget,
  - stage maxima for wait/begin, controller poll, and full mclone render frame,
  - render-section counts, drawn indices, and actor draw counts.
- [x] Keep initial output logcat-first and write the final summary line to
  `/tmp/mclone-quest-openxr-perf-summary.txt` by default.
- [x] Add a deterministic automated flight path that starts after the ready
  frame, switches to no-clip, flies forward at walking-like speed, uses the
  same engine camera pose-sync/chunk-interest path as XR locomotion, and
  records actual flight distance in the summary.
- [x] Add render-distance flight scripts for radius 1, 5, and 10 plus a
  three-run sweep.
- [ ] Add current/requested/supported refresh state after Slice 2 lands.
- [ ] Split `render_mclone_frame` into narrower locate/locomotion/acquire/
  per-eye/end-frame timings after the coarse probe proves useful.
- [ ] Add pending compile/streaming counters to the summary once the shared XR
  scene exposes them directly.

Validation target:

```bash
pnpm native:android-xr:validate -- --perf-seconds 10
```

Package script:

```bash
pnpm native:android-xr:perf
```

Recorded first-pass implementation:

- Android XR startup argv accepts `--perf-seconds N`.
- The frame loop starts sampling after `MCLONE_ANDROID_XR_READY`, skipping the
  ready frame itself.
- The first budget uses a conservative `72 Hz` fallback until real OpenXR
  refresh state is plumbed in Slice 2.
- The validator waits for `MCLONE_ANDROID_XR_PERF_SUMMARY`, writes the last
  summary line to `/tmp/mclone-quest-openxr-perf-summary.txt`, and still runs
  the existing cleanup trap that force-stops the app, restores headset power
  settings, re-enables proximity, and sends `KEYCODE_SLEEP`.
- `package.json` includes `native:android-xr:perf` with a fixed seed, center
  chunk, render distance 2, noon, frozen time, and startup view pose.
- `package.json` includes `native:android-xr:perf:flight:rd1`,
  `native:android-xr:perf:flight:rd5`,
  `native:android-xr:perf:flight:rd10`, and
  `native:android-xr:perf:flight:sweep`. The flight summaries include
  `mode=flight`, `render_distance`, `flight_speed_blocks_per_second`, and
  `flight_distance_blocks`.

### Slice 2 - Real OpenXR Display Refresh State

- Enable and share `XR_FB_display_refresh_rate` when the runtime exposes it.
- Query and log:
  - extension support,
  - supported refresh rates,
  - current refresh rate,
  - requested refresh rate if one is pending.
- Add Android startup property/script support for
  `debug.mclone.xr_display_refresh`, matching the shape already reserved in
  [`083`](083-android-xr-quest-standalone.md).
- Add validator/install flags:
  - `--refresh HZ`
  - validation that the app logs the request and the current/applied rate, so a
    `72`/`90`/`120` comparison cannot silently run at the default rate.
- Feed actual XR refresh into `GameUiRenderState` or a new XR UI state so the
  menu no longer shows the fixed `XR_UI_FPS_CAP`.

### Slice 3 - XR Menu Performance Section

- Add an XR-specific menu/options section with:
  - current headset refresh,
  - supported refresh buttons when the extension is available,
  - performance overlay toggle,
  - compact overlay toggle,
  - locomotion frame policy display/toggle if the 092 path has landed,
  - render distance and fullbright/occlusion controls retained from the shared
    options screen.
- Keep desktop frame-pacing controls out of the Quest XR menu. On Quest the
  OpenXR runtime owns presentation; mclone can request supported display rates
  but should not present desktop VSync/capped/uncapped controls.
- Add UI tests for XR-state rendering and for hiding unsupported refresh
  controls when the runtime does not expose the extension.

### Slice 4 - Headset Performance Overlay

- Add a minimal headset-visible overlay that can remain visible while the menu
  is closed.
- First implementation may be an in-scene/view-space debug panel rendered by
  the existing XR world GUI path. A later pass can move it to a Playbox-style
  OpenXR composition-layer swapchain if the in-scene panel affects the timing
  being measured.
- Minimum visible metrics:
  - refresh and frame budget,
  - recent FPS / frame interval,
  - over-budget count,
  - app CPU active time,
  - OpenXR wait/runtime idle time,
  - per-eye render time,
  - chunk update/upload counters,
  - skipped frames.
- Compact mode should fit in a small always-on panel and avoid obscuring the
  center of the view.

### Slice 5 - GPU Timing And Render-Mode Attribution

- Add opt-in GPU timestamp support only when the Quest Vulkan/wgpu device
  exposes the required feature. Keep it off by default.
- Report unsupported GPU timing explicitly instead of leaving blanks.
- Attribute GPU time by at least:
  - left eye,
  - right eye,
  - GUI/menu/overlay pass if separate,
  - future expensive passes as they land.
- Use GPU data to decide whether Quest defaults need render-distance,
  foveation, render-scale, or shader work. Do not change defaults before the
  CPU/GPU split is measured.

### Slice 6 - Movement, Interpolation, And Network Diagnosis

- Add optional locomotion diagnostics:
  - raw and deadzoned stick axes,
  - locomotion dt before/after `XR_LOCOMOTION_MAX_FRAME_SECONDS` clamp,
  - HMD-yaw/body-yaw movement policy,
  - computed movement vector,
  - player/root pose delta per displayed frame.
- Compare local integrated and remote dedicated modes. Include correction IDs,
  correction count, and correction magnitude when the server corrects player
  position.
- Record whether player simulation/update cadence is independent from headset
  refresh and whether interpolation is happening at presentation time.
- Only after these counters exist, decide whether the visible jumpiness is a
  frame-pacing issue, a movement-sampling issue, an input filtering issue, or a
  network/client-prediction issue.

### Slice 7 - Baselines And Budgets

- Record Quest 3 release baselines in
  [`../performance-records.md`](../performance-records.md):
  - local integrated, render distance 2, default refresh,
  - local integrated at each supported requested refresh,
  - remote dedicated via LAN or `--adb-reverse`,
  - walking with overlay hidden and overlay visible.
- Keep thresholds advisory until at least two stable headset baselines exist.
  Host noise, thermal state, battery mode, Guardian/system overlays, and Quest
  runtime updates can move results.
- Once stable, add a validator budget that warns first, then later fails only
  on clear regressions such as sustained over-budget frames or missed submitted
  frame progress.

## Guardrails

- Keep OpenXR extension/session/swapchain types in app/platform XR crates or
  `mclone-xr-host`; do not leak them into shared client/server/runtime/mesh
  crates.
- Keep Quest Android properties for wrapper/runtime settings and
  launch-scoped argv for engine/session options.
- Do not fork mclone rendering, chunk streaming, movement, or session runtime
  solely for Quest. Add diagnostics and platform adapters around the shared
  contracts.
- Save logs, summaries, screenshots, and overlay layout dumps under `/tmp`.
- Do not make performance-overlay rendering so expensive that it hides the
  bottleneck. Validate overlay-hidden and overlay-visible samples separately.

## Validation Lanes

Initial diagnostic gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-xr-host -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
pnpm native:android-xr:apk
pnpm native:android-xr:validate
git diff --check
```

Planned headset perf lanes after Slice 1:

```bash
pnpm native:android-xr:perf
pnpm native:android-xr:perf:flight:rd1
pnpm native:android-xr:perf:flight:rd5
pnpm native:android-xr:perf:flight:rd10
pnpm native:android-xr:perf:flight:sweep
pnpm native:android-xr:perf -- --refresh 72
pnpm native:android-xr:perf -- --refresh 90
pnpm native:android-xr:perf -- --refresh 120
```

Only run refresh variants that the runtime reports as supported for the current
headset/session.

## Done Criteria

- Quest Android XR has a repeatable performance validator that captures a
  post-ready headset sample and writes a summary under `/tmp`.
- The XR menu reports actual OpenXR refresh state instead of a fixed `90`.
- A headset-visible performance overlay can be toggled from the XR menu.
- At least one Quest 3 release baseline is recorded.
- When walking feels jumpy, the overlay/logs can distinguish missed frames,
  CPU stalls, GPU over-budget frames, uneven movement sampling, and server
  correction/network causes.

## Open Questions

- Should the default Quest requested refresh remain runtime default/72 Hz, or
  should mclone request 90 Hz when available?
- Should the first overlay be an in-scene panel for simplicity or an OpenXR
  composition-layer overlay for lower measurement interference?
- Does player movement need presentation interpolation at headset refresh, or
  is the current engine camera/player sync already adequate once frame pacing
  is stable?
- Which render distance should become the Quest default after real headset
  GPU/CPU baselines exist?
