# 029: Native Frame Pacing And Streaming Hitches

Status: active.

## Purpose

Make desktop frame pacing and streaming hitches visible enough to validate what a
player sees on a high-refresh display, then use those measurements to drive the
chunk publish, remesh, and upload fixes.

The immediate player symptom is skipped frames while walking in the native
desktop client. At 120 Hz the frame budget is 8.33 ms. Current movement/runtime
perf lanes already show work far above that budget, but they do not report live
desktop frame intervals, missed refresh counts, or per-frame attribution.

## Findings

Initial native behavior when this tactical was opened:

- `mclone-worldgen` runs feature generation on a background thread.
- completed chunk publication still runs on the main scheduler/presentation
  path and includes snapshot packing plus provisional lighting.
- CPU render-section remeshing runs synchronously before the frame render.
- GPU section buffer creation/upload runs synchronously on the render thread.
- desktop present mode is fixed to FIFO, with no app-facing VSync/capped/uncapped
  controls.

That meant background worldgen did not prevent frame hitches: the frame could
still block on completed-job publication, dirty-section rebuilds, and GPU buffer
churn.

Later performance slices reduced the main-thread engine work substantially:
section block deltas avoid full snapshot publication for fluid/block updates,
section-precise dirtying reduced rebuild scope, and async render-section
compilation moved CPU mesh generation off the frame path. The current status and
priority order live in [`../topics/performance.md`](../topics/performance.md).

## Playbox Reference

Use Playbox as a pattern library only:

- `~/code/playbox/src/desktop/frame_pacing.rs` owns VSync, capped, and uncapped
  pacing policy, monitor refresh discovery, and capped redraw deadlines.
- `~/code/playbox/src/app.rs` applies present-mode changes before surface
  acquire, records per-frame timing stages, and schedules capped redraws with
  `ControlFlow::WaitUntil`.
- `~/code/playbox/src/ui/performance.rs` exposes monitor refresh, frame budget,
  pacing mode, FPS cap, active CPU time, and acquire/present waits.

Do not copy Playbox's egui UI or runtime shape. Mclone should keep the
first-party `mclone-ui` menu/debug path and keep desktop pacing in the app
adapter.

## First Implementation Slice

1. Add native desktop frame pacing state:
   - VSync mode uses `wgpu::PresentMode::Fifo`.
   - capped mode uses a no-vsync present mode and `ControlFlow::WaitUntil`.
   - uncapped mode uses a no-vsync present mode and continuous redraw.
   - default remains VSync with a 120 FPS cap ready for capped mode.
2. Add options-screen controls for VSync, capped, uncapped, and common FPS caps.
3. Add live diagnostics:
   - monitor refresh and frame budget
   - last frame delta
   - frames over 1x, 2x, and 4x budget
   - last runtime poll, remesh, upload, render, surface acquire, and present
     timings
4. Add a separate headless frame-budget probe:
   - fixed seed, camera/chunk-interest path, frame count, and target Hz
   - no sleep, no swapchain, no VSync, and no monitor present-mode simulation
   - count offscreen work frames over 1x, 2x, and 4x the requested budget
   - report per-frame poll, remesh, upload, render, submit, and device-wait
     timings
5. Add a movement-shaped headless frame-budget probe:
   - speed-based spectator movement at the requested target Hz
   - same live-frame poll/render-work/upload/render path as the stress probe
   - no full render-work drain per movement step
6. Keep this first slice diagnostic and pacing-focused. Do not yet change chunk
   scheduling, mesh threading, or GPU upload budgeting.

The headless probe is deterministic in workload shape, not in measured wall
clock. CPU/GPU throttling, other processes, backend driver behavior, and power
state can still move timings. Treat it as a repeatable regression probe, not as
a substitute for live desktop/XR present-pacing validation.

## Follow-Up Performance Slices

See [`../topics/performance.md`](../topics/performance.md) for the live priority
queue. As of the movement-frame probe slice, the next investigation is desktop
present/wait attribution because the headless walking probe is green while the
reported symptom is visible desktop walking hitching.

## Validation

First-slice gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-render -p mclone-native-client -- --nocapture
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:movement:smoke
pnpm native:runtime:smoke
pnpm native:frame-budget:smoke
pnpm native:movement-frame:smoke
git diff --check
```

Manual validation:

- open the native desktop client on a 120 Hz display
- enable the debug pane
- walk across chunk boundaries
- confirm budget overruns and timing stage spikes correspond to visible skipped
  frames
- switch VSync / capped / uncapped in Options and confirm the debug pane updates
  monitor budget and pacing mode

## Done For First Slice

- player can choose VSync, capped FPS, or uncapped from native Options.
- debug pane reports frame budget overruns and stage timings.
- surface present mode changes without restarting the client.
- `--frame-budget-probe --target-hz 120` reports deterministic offscreen budget
  misses and stage attribution.
- `--movement-frame-probe --target-hz 120` reports speed-based walking budget
  misses without fully draining render work per movement step.
- existing movement/runtime perf lanes still pass.
