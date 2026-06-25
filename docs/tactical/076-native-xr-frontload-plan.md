# 076: Native XR Frontload Plan

Status: proposed parent. This is a sequencing checklist for XR-enabling
cleanup and smoke work after the flat Android baseline.

## Purpose

Capture the next platform/frontload work after flat Android without jumping
straight to Android XR scaffolding.

The goal is to make future XR integration cheap to add and hard to accidentally
fork. We should first remove the remaining single-window assumptions from the
shared render and input boundaries, then prove desktop OpenXR, then treat
Android XR / Quest standalone as its own package/runtime target.

## Current State

Landed platform work:

- Desktop native is the main interactive and validation loop.
- Native web/WASM remains an active compatibility and deploy target.
- Flat Android now has a `NativeActivity` APK, AVD screenshot/logcat smokes,
  staged packed assets, integrated-runtime terrain rendering, and touch orbit.
- `mclone-app-runtime` owns shared single-view runtime state and render-section
  streaming orchestration for host adapters.
- Renderer-facing target and view facts are already partly explicit through
  `RenderFrameContext`, `RenderFrameTarget`, `ChunkRenderView`, and
  `ChunkRenderTarget`.

Remaining XR pressure points:

- `mclone_app_runtime::frame_render::render_full_frame` still takes a
  `ChunkCamera`, derives one `ChunkRenderView`, and renders one target.
- Desktop, Android, and web input adapters do not yet share one small
  platform-neutral player/action intent surface.
- Flat Android validation exists, but suspend/resume, asset-missing, and
  startup-configuration coverage are still light compared with what a Quest
  OpenXR app will need.
- No mclone OpenXR loader/session/swapchain code exists yet.

## Direction

Do not start with Android XR. The expected sequence is:

1. Finish multi-view renderer contract cleanup without OpenXR.
2. Harden the shared platform input/startup seams while the code is still
   single-view and easy to validate.
3. Bring up desktop OpenXR as the first real XR runtime path.
4. Render the mclone runtime through desktop OpenXR per-eye targets.
5. Add Android XR / Quest standalone only after desktop XR has proven the
   renderer/runtime shape.

This mirrors the Playbox lesson: flat desktop, flat Android, and headless can
share a single-view runtime shell, while XR owns the session/frame loop and
feeds shared renderer/world data with per-eye views and targets.

## Tactical Children

- [`077-multiview-render-contract.md`](077-multiview-render-contract.md):
  make shared full-frame rendering accept explicit render views and targets, and
  validate with deterministic dual-view headless screenshots.
- [`078-desktop-openxr-clear-smoke.md`](078-desktop-openxr-clear-smoke.md):
  add the first desktop OpenXR loader/session/frame/swapchain smoke, with no
  mclone runtime dependency.
- [`079-desktop-openxr-mclone-frame.md`](079-desktop-openxr-mclone-frame.md):
  render real mclone terrain through the desktop OpenXR path using the
  multi-view renderer contract.

Likely follow-up docs, after the three above:

- Platform-neutral input/action intent cleanup for desktop, web, Android, and
  future XR action sets.
- Flat Android lifecycle/startup validation hardening.
- Quest-flat hardware capture on a connected headset.
- Android XR / Quest standalone package and OpenXR runtime smoke.

## Guardrails

- Shared crates must not depend on `winit`, `android-activity`, DOM/canvas
  objects, browser workers, OpenXR sessions, OpenXR action sets, or OpenXR
  swapchains.
- Do not move Android XR into the flat Android `NativeActivity` workstream.
- Do not create inert Android XR or OpenXR packages before there is a validation
  lane that can launch and capture/log the result.
- Do not fork the renderer, mesh, asset, protocol, client, or server contracts
  for XR.
- Keep screenshots and captures under `/tmp`.
- For any native slice that produces pixels, capture and inspect the image
  before moving on.

## Playbox References

Use `~/code/playbox` as a pattern library only:

- `docs/architecture/rendering.md` for explicit render view/target boundaries.
- `docs/architecture/platforms.md` for flat desktop, flat Android, desktop
  OpenXR, and Android XR sequencing.
- `src/render/mod.rs` and `src/render/targets.rs` for renderer-facing view and
  target facts.
- `src/xr/` for OpenXR loader/session/swapchain ownership and per-eye rendering
  shape.
- `android-xr/README.md` for Quest packaging and validation lessons after the
  desktop OpenXR path exists.

## Completion Criteria

This parent doc is complete when:

- Multi-view full-frame rendering is validated without OpenXR.
- Desktop OpenXR can render at least a clear/non-runtime frame through the real
  OpenXR frame loop.
- Desktop OpenXR can render a real mclone frame through shared runtime and
  renderer code.
- The follow-up Android XR tactical can be written against proven renderer and
  host boundaries rather than guesses.
