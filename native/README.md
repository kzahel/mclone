# mclone native workspace

This workspace is the Rust track for the native-first engine. It currently has five validated client/platform lanes: desktop flat, desktop OpenXR, Android XR / Quest standalone, flat Android, and web/WASM. Native desktop flat remains the fastest daily loop, but shared client/server/runtime/mesh/asset/UI/renderer contracts must stay platform-neutral. The workspace lives inside the existing repository so it can reuse the project oracle fixtures, reference notes, and asset extraction scripts.

The durable direction is documented in [`../docs/native-rewrite-roadmap.md`](../docs/native-rewrite-roadmap.md), current platform posture is documented in [`../docs/platforms.md`](../docs/platforms.md), flat Android build/validation commands are documented in [`../android/README.md`](../android/README.md), and Quest/OpenXR commands are documented in [`../android-xr/README.md`](../android-xr/README.md). The short version: desktop flat is the primary development loop; desktop XR, Android XR, flat Android, and web/WASM are active validation lanes over shared engine contracts.

Native Rust tactical docs live in [`../docs/tactical/`](../docs/tactical/README.md) and use zero-padded numeric filenames such as `000-native-render-bringup.md`.

Use `~/code/playbox` as the reference Rust engine for native app/render/XR patterns. In particular, its `winit`/`wgpu` setup, frame pacing, headless capture, render-target, camera, diagnostics, Android NativeActivity, and OpenXR code are useful references. Do not depend on Playbox directly, and do not copy its PhysX/VaM-specific runtime shape.

Use `../reference/minecraft-1.17.1/src/` as the reference for vanilla behavior and visual correctness. For renderer work that has a Java client counterpart, inspect the Minecraft source before borrowing behavior from Playbox. This includes block/entity model baking, texture atlas stitching, mipmap generation/filtering, UV shrink/bleed behavior, light texture math, fog, sky, render layers, transparency/cutout state, particles, and render-section traversal.

Playbox reference entry points:

- `~/code/playbox/Cargo.toml`: debug-profile optimization policy for meaningful `cargo run` perf numbers.
- `~/code/playbox/docs/architecture/rendering.md`: render view/target boundaries and `wgpu` escape-hatch policy.
- `~/code/playbox/docs/architecture/platforms.md`: desktop, flat Android, desktop OpenXR, and Android XR host boundaries.
- `~/code/playbox/android/README.md`: flat Android NativeActivity build/validation notes.
- `~/code/playbox/android-xr/README.md`: Quest/OpenXR package, runtime, and validation notes.
- `~/code/playbox/docs/tactical/106-desktop-xr-companion-window.md`: desktop OpenXR companion window/mirror/input design notes.

This workspace intentionally keeps native `profile.dev` optimized at `opt-level = 2`, following Playbox's policy. Debug assertions and incremental rebuild behavior remain enabled, but movement/render/worldgen perf smokes should not be interpreted as fully unoptimized Rust numbers.

The Rust workspace owns live engine implementation:

- Rust crates may read shared docs, fixtures, and extracted assets.
- Rust crates should not import, execute, or depend on retired browser-engine code from Git history.
- Minecraft Java `1.17.1` and the existing oracle fixtures remain the correctness target.
- The browser target should be kept alive early, but native desktop is the main development loop.
- Flat Android should remain a focused non-XR validation lane; do not fold Quest/OpenXR assumptions into it.
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
- `mclone-xr-scene`: shared XR terrain runtime, startup-pose alignment, and locomotion mapping.

Applications:

- `mclone-native-client`: desktop client.
- `mclone-dedicated-server`: headless native server.
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

Native benchmark baselines are recorded in [`../docs/performance-records.md`](../docs/performance-records.md). Use `pnpm native:worldgen:smoke`, `pnpm native:movement:smoke`, and `pnpm native:timedemo:smoke` for the standard optimized-dev smoke lanes; use the matching `:perf` scripts for release-oriented runs.

Start narrow: prefer oracle-backed engine slices, shared contract tests, and targeted platform smokes over broad scaffolding. Use the full Android, desktop XR, and Quest lanes when a change touches the platform boundary they uniquely exercise; otherwise keep shared logic covered by shared tests and representative desktop/web gates.
