# mclone native workspace

This workspace is the clean Rust track for the native-first engine. Native desktop is the current bring-up priority, web/WASM is kept alive as an early compatibility gate, and future Android XR / Quest standalone support is an explicit native target once the desktop/runtime/renderer path is mature enough. It lives inside the existing repository so it can reuse the project oracle fixtures, reference notes, asset extraction scripts, and the TypeScript engine as legacy prior art.

The durable direction is documented in [`../docs/native-rewrite-roadmap.md`](../docs/native-rewrite-roadmap.md), and current platform posture is documented in [`../docs/platforms.md`](../docs/platforms.md). The short version: native desktop is the primary development target, web/WASM is an early compatibility gate, flat Android is the next platform frontload target, Android XR / Quest is a later native target, and the TypeScript implementation is now legacy/reference rather than the main engine direction.

Native Rust tactical docs live in [`../docs/tactical/`](../docs/tactical/README.md) and use zero-padded numeric filenames such as `000-native-render-bringup.md`. Legacy TypeScript/browser tacticals are archived under `../docs/tactical/legacy/`.

Use `~/code/playbox` as the reference Rust engine for native app/render/XR patterns. In particular, its `winit`/`wgpu` setup, frame pacing, headless capture, render-target, camera, diagnostics, Android NativeActivity, and OpenXR code are useful references. Do not depend on Playbox directly, and do not copy its PhysX/VaM-specific runtime shape.

Playbox reference entry points:

- `~/code/playbox/Cargo.toml`: debug-profile optimization policy for meaningful `cargo run` perf numbers.
- `~/code/playbox/docs/architecture/rendering.md`: render view/target boundaries and `wgpu` escape-hatch policy.
- `~/code/playbox/docs/architecture/platforms.md`: desktop, flat Android, desktop OpenXR, and Android XR host boundaries.
- `~/code/playbox/android/README.md`: flat Android NativeActivity build/validation notes.
- `~/code/playbox/android-xr/README.md`: Quest/OpenXR package, runtime, and validation notes.
- `~/code/playbox/docs/tactical/106-desktop-xr-companion-window.md`: desktop OpenXR companion window/mirror/input design notes.

This workspace intentionally keeps native `profile.dev` optimized at `opt-level = 2`, following Playbox's policy. Debug assertions and incremental rebuild behavior remain enabled, but movement/render/worldgen perf smokes should not be interpreted as fully unoptimized Rust numbers.

The Rust workspace is quarantined from the TypeScript implementation:

- Rust crates may read shared docs, fixtures, and extracted assets.
- Rust crates should not import, execute, or depend on TypeScript runtime code.
- Minecraft Java `1.17.1` and the existing oracle fixtures remain the correctness target.
- The browser target should be kept alive early, but native desktop is the main development loop.
- Android and Android XR / Quest should stay visible as future platform constraints; do not introduce their packaging/runtime scaffolding until the renderer and app boundaries are explicit enough to validate them.
- `winit`, Android activity glue, and OpenXR session/swapchain code belong in app/platform adapters, not in shared client/server/mesh/asset crates.

## Initial crate boundaries

- `mclone-core`: pure data/model/math primitives.
- `mclone-protocol`: versioned host/client messages and encodings, with no sockets.
- `mclone-net`: transport adapters and local/native/web channel boundaries, with no game rules.
- `mclone-server`: authoritative runtime, chunk scheduling, ticks, sessions, and persistence hooks.
- `mclone-client`: client replica, prediction/interpolation, and render-facing presentation state.
- `mclone-worldgen`: vanilla 1.17.1 terrain, biome, surface, carver, feature, and structure generation.
- `mclone-light`: packed sky/block lighting.
- `mclone-mesh`: chunk meshing into renderer-ready buffers.
- `mclone-assets`: blockstate/model/texture/NBT asset loading.
- `mclone-render`: `wgpu` renderer.

Applications:

- `mclone-native-client`: desktop client.
- `mclone-dedicated-server`: headless native server.
- `mclone-web-client`: browser/WASM client shell.

Future applications, not scaffolded yet:

- flat Android client: next platform target, expected to reuse the single-view client/render path with Android lifecycle/input adapters; see [`../docs/tactical/074-flat-android-build-smoke.md`](../docs/tactical/074-flat-android-build-smoke.md).
- Android XR / Quest client: expected to own a separate XR host loop over shared client/runtime/render data because stereo views, runtime swapchains, and controller/hand input do not fit the desktop single-window loop.

Useful gates:

- `cargo test --workspace`
- `pnpm native:scheduler:smoke`
- `pnpm native:dedicated:smoke`
- `pnpm native:perf:smoke`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- `pnpm native:web:smoke`

Native benchmark baselines are recorded in [`../docs/performance-records.md`](../docs/performance-records.md). Use `pnpm native:worldgen:smoke`, `pnpm native:movement:smoke`, and `pnpm native:timedemo:smoke` for the standard optimized-dev smoke lanes; use the matching `:perf` scripts for release-oriented runs.

Start narrow: prefer oracle-backed engine slices and validation-backed platform work over broad scaffolding. Flat Android should start as a focused non-XR APK/screenshot smoke, borrowing Playbox's package and validation shape while keeping Android lifecycle, input, and asset paths in the platform adapter.
