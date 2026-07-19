# mclone Rust workspace

This is the live engine workspace. It contains shared Rust crates plus five
first-class client targets: flat Android, Android XR / Quest standalone,
desktop flat, desktop OpenXR, and web/WASM. All five are equally valid product
targets over shared client, server, runtime, mesh, asset, UI, and renderer
contracts. The `native/` directory name is repository history, not a platform
priority.

Durable crate and app ownership is documented in
[`../docs/native-engine-architecture.md`](../docs/native-engine-architecture.md),
and current platform posture and validation live in
[`../docs/platforms.md`](../docs/platforms.md). Flat Android build and validation
commands are documented in [`../android/README.md`](../android/README.md), and
Quest/OpenXR commands are documented in
[`../android-xr/README.md`](../android-xr/README.md).

Implementation tacticals live in
[`../docs/tactical/`](../docs/tactical/README.md) and use zero-padded numeric
filenames such as `000-native-render-bringup.md`.

Use `~/code/playbox` as the reference Rust engine for app/render/XR patterns. In particular, its `winit`/`wgpu` setup, frame pacing, headless capture, render-target, camera, diagnostics, Android NativeActivity, and OpenXR code are useful references. Do not depend on Playbox directly, and do not copy its PhysX/VaM-specific runtime shape.

Use `../reference/minecraft-1.17.1/src/` as the reference for vanilla behavior and visual correctness. For renderer work that has a Java client counterpart, inspect the Minecraft source before borrowing behavior from Playbox. This includes block/entity model baking, texture atlas stitching, mipmap generation/filtering, UV shrink/bleed behavior, light texture math, fog, sky, render layers, transparency/cutout state, particles, and render-section traversal.

Playbox reference entry points:

- `~/code/playbox/Cargo.toml`: debug-profile optimization policy for meaningful `cargo run` perf numbers.
- `~/code/playbox/docs/architecture/rendering.md`: render view/target boundaries and `wgpu` escape-hatch policy.
- `~/code/playbox/docs/architecture/platforms.md`: desktop, flat Android, desktop OpenXR, and Android XR host boundaries.
- `~/code/playbox/android/README.md`: flat Android NativeActivity build/validation notes.
- `~/code/playbox/android-xr/README.md`: Quest/OpenXR package, runtime, and validation notes.
- `~/code/playbox/docs/tactical/106-desktop-xr-companion-window.md`: desktop OpenXR companion window/mirror/input design notes.

This workspace intentionally keeps Rust `profile.dev` optimized at
`opt-level = 2`, following Playbox's policy. Debug assertions and incremental
rebuild behavior remain enabled, but movement/render/worldgen perf smokes
should not be interpreted as fully unoptimized Rust numbers.

The Rust workspace owns live engine implementation:

- Rust crates may read shared docs, fixtures, and extracted assets.
- Minecraft Java `1.17.1` and the existing oracle fixtures remain the correctness target.
- Shared changes must preserve every affected client target rather than treating one target as the implementation baseline.
- Flat Android remains independent from Quest/OpenXR; do not fold XR assumptions into it.
- Desktop XR and Android XR should share reusable OpenXR host/graphics/scene contracts, while keeping desktop runtime launch behavior and Android activity/JNI/Horizon behavior in app/platform adapters.
- `winit`, Android activity glue, browser glue, and OpenXR session/swapchain code belong in app/platform adapters or dedicated app/platform XR crates, not in shared client/server/mesh/asset/worldgen/light crates.

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
- `mclone-render-session`: render-section dirty/cache/compile and camera-controller contracts.
- `mclone-app-runtime`: shared client runtime/render helpers and native asset loading.
- `mclone-ui`: shared Rust/WebGPU UI model.
- `mclone-xr-host`: shared OpenXR host/session/frame/action/view helpers.
- `mclone-xr-graphics`: shared Vulkan OpenXR/`wgpu` graphics bridge.
- `mclone-scene`: shared scene host for mono, stereo, and multiview
  runtime/render/UI orchestration.

Applications:

- `mclone-native-client`: desktop client.
- `mclone-dedicated-server`: headless Rust server.
- `mclone-web-client`: browser/WASM client shell.
- `mclone-android-client`: flat Android `NativeActivity` client shell.
- `mclone-android-xr-client`: standalone Quest/OpenXR client shell.

Useful gates:

- `cargo test --workspace`
- `pnpm native:scheduler:smoke`
- `pnpm native:dedicated:smoke`
- `pnpm native:perf:smoke`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- `pnpm native:web:smoke`
- `pnpm native:android:avd-smoke`
- `pnpm native:xr:windows:mclone:connected`
- `pnpm native:android-xr:validate`

Benchmark baselines are recorded in
[`../docs/performance-records.md`](../docs/performance-records.md). Use
`pnpm native:worldgen:smoke`, `pnpm native:movement:smoke`, and
`pnpm native:timedemo:smoke` for the standard optimized-dev smoke lanes; use
the matching `:perf` scripts for release-oriented runs.

Start narrow: prefer oracle-backed engine slices, shared contract tests, and
targeted platform smokes over broad scaffolding. Choose validation from the
contracts and platform boundaries changed, not from a preferred target.
