# 078: Desktop OpenXR Clear Smoke

Status: active; Slice 1 dependency/feature gate complete, loader/session work next.

## Purpose

Add the first desktop OpenXR runtime path for mclone without pulling in the full
client/server/render-session stack.

This slice should prove loader discovery, instance/session creation, graphics
binding, swapchain acquisition, frame wait/begin/end, projection layer
submission, and validation/logging. It should not yet render the mclone world.

## Why Desktop First

Desktop OpenXR is the lowest-risk way to prove stereo runtime ownership before
Android XR:

- It avoids Quest packaging, Android loader initialization, and headset storage
  policy.
- It lets the renderer/device/swapchain boundary be debugged with normal native
  logs and desktop tools.
- It creates the host shape that Android XR should reuse later instead of
  growing out of the flat Android `NativeActivity` path.

## Target Shape

Add desktop OpenXR behind an explicit opt-in feature and app mode, for example:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke
```

Expected ownership:

- `mclone-native-client` owns desktop OpenXR startup and validation mode.
- OpenXR loader, instance, session, spaces, frame loop, and swapchains stay in
  the app/platform layer.
- `mclone-render` may keep shared `wgpu` resources and helpers, but must not
  own OpenXR sessions or action sets.
- Shared engine crates remain OpenXR-free.

The first image can be a per-eye clear color or simple renderer-independent
test pattern. It should still acquire real XR swapchain images and submit a
projection layer through the runtime.

## Playbox References

Use Playbox as a pattern library only:

- `~/code/playbox/docs/architecture/platforms.md` for desktop OpenXR platform
  ownership.
- `~/code/playbox/src/xr/` for loader/session/swapchain/frame-loop shape.
- `~/code/playbox/Cargo.toml` for the opt-in `xr` feature shape:
  `default = []`, `xr = ["dep:ash", "dep:openxr", "dep:libloading"]`.
- `~/code/playbox/src/main.rs` for the CLI pattern where non-XR builds accept
  the flag shape but fail with a clear "rebuild with `--features xr`" message.
- `~/code/playbox/docs/tactical/106-desktop-xr-companion-window.md` if a
  companion window is needed later. Do not add companion-window complexity in
  this first smoke unless required by the local runtime.

Do not copy Playbox's PhysX, scene, egui, controller, body, passthrough, or
spatial-room systems.

## Implementation Slices

### Slice 1 - Dependency And Feature Gate

- [x] Add the minimum OpenXR dependency behind an `xr` feature.
- [x] Keep default desktop, web, Android, and dedicated-server builds unchanged.
- [x] Add a CLI mode for the clear smoke.
- [x] Document that no runtime prerequisites or environment variables are used
  before the loader/session slice.

Landed in Slice 1:

- `mclone-native-client` now has an opt-in `xr` feature modeled after
  Playbox's feature gate: optional `ash`, `openxr`, and `libloading`
  dependencies with `default = []`.
- `--xr-clear-smoke [--frames N]` is parsed as a separate run mode and rejects
  combinations with headless or perf modes.
- Non-XR builds retain the CLI mode but return
  `rebuild with --features xr to use --xr-clear-smoke`.
- XR-enabled builds compile a small placeholder module that proves the feature
  dependency path and leaves loader/session/swapchain work to Slice 2.
- No OpenXR types were added to shared engine crates.

Runtime prerequisites:

- Slice 1 does not load the OpenXR runtime and does not require runtime
  environment variables.
- Slice 2 should document the exact desktop runtime path it uses. Based on
  Playbox, likely candidates are loader/runtime discovery variables such as
  `XR_RUNTIME_JSON` or Monado/WiVRn-specific runtime paths, but mclone should
  record only variables it actually consumes.

Validation:

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:web:build
```

Slice 1 validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
# Expected failure with a feature hint:
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --xr-clear-smoke --frames 2
# Expected success:
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 2
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
pnpm native:web:build
git diff --check
```

Expected Slice 1 CLI results:

- Without `--features xr`: exits with
  `rebuild with --features xr to use --xr-clear-smoke`.
- With `--features xr`: exits successfully after printing that loader/session/
  swapchain bring-up is next.

### Slice 2 - Runtime And Graphics Binding

- [ ] Load the active OpenXR runtime.
- [ ] Create an OpenXR instance with the required graphics extension for the
  local desktop backend.
- [ ] Create the runtime-selected graphics device or validate that the current
  `wgpu` device is compatible, depending on backend requirements.
- [ ] Create a session and reference space.
- [ ] Create color swapchains for both eyes and host-owned depth targets.

Implementation should follow the measured platform path. On macOS this likely
means Metal-specific runtime/device matching. On Windows/Linux this likely
means Vulkan instance/device negotiation. Keep backend-specific code isolated
inside the app/platform XR module.

### Slice 3 - Clear Frame Submission

- [ ] Wait/begin/end OpenXR frames.
- [ ] Acquire and release per-eye swapchain images.
- [ ] Clear each eye to a visible diagnostic color or pattern.
- [ ] Submit a projection layer using runtime-provided views/projections.
- [ ] Run for a bounded number of frames in smoke mode and exit cleanly.

Suggested validation:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr -- --xr-clear-smoke --frames 120
```

Record the exact runtime, platform backend, and observed result in this doc
when the smoke lands.

## Out Of Scope

- Android XR / Quest packaging.
- Controller action bindings.
- Hand tracking.
- Passthrough.
- Spatial room APIs.
- In-world UI.
- Mclone world/client/server rendering.
- Desktop companion window, unless needed only to keep the runtime alive.

## Review Rejection Criteria

- OpenXR types leaking into shared client, server, protocol, mesh, asset,
  worldgen, light, or app-runtime crates.
- Android XR manifest/package changes in this desktop smoke.
- A fake stereo path that does not acquire real OpenXR swapchain images.
- A permanent dependency on one local runtime path without documented fallback
  or clear error reporting.

## Completion Criteria

- The native client has an opt-in desktop OpenXR clear-smoke mode.
- The smoke creates a real OpenXR session, renders/submits bounded frames, and
  exits cleanly.
- Non-XR desktop, native web, and flat Android builds remain unaffected.
- The follow-up mclone-frame tactical can reuse the XR frame loop and swapchain
  ownership without reopening loader/session work.
