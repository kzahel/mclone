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

Current native behavior:

- `mclone-worldgen` runs feature generation on a background thread.
- completed chunk publication still runs on the main scheduler/presentation
  path and includes snapshot packing plus provisional lighting.
- CPU render-section remeshing runs synchronously before the frame render.
- GPU section buffer creation/upload runs synchronously on the render thread.
- desktop present mode is fixed to FIFO, with no app-facing VSync/capped/uncapped
  controls.

This means background worldgen does not prevent frame hitches. The frame can
still block on completed-job publication, dirty-section rebuilds, and GPU buffer
churn.

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
4. Keep this first slice diagnostic and pacing-focused. Do not yet change chunk
   scheduling, mesh threading, or GPU upload budgeting.

## Follow-Up Performance Slices

1. Move completed chunk publish work off the presentation path or slice it by a
   per-frame budget. Snapshot packing and provisional lighting are the main
   candidates.
2. Move CPU mesh builds to worker jobs that return section mesh data.
3. Budget GPU uploads on the render thread by section count or byte count per
   frame.
4. Reduce dirty rebuild scope from chunk-neighborhood-wide to section-aware once
   block/light deltas carry enough coordinates.
5. Add benchmark budgets for movement/runtime perf after release-mode baselines
   are refreshed with the new frame timing counters.

## Validation

First-slice gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-render -p mclone-native-client -- --nocapture
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:movement:smoke
pnpm native:runtime:smoke
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
- existing movement/runtime perf lanes still pass.
