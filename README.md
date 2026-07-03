# mclone

Minecraft-inspired voxel sandbox. Private project — primary target is home/LAN use for my daughter to play with.

## Current Direction

The current direction is a native-first Rust engine with five validated client/platform lanes:

- desktop flat
- desktop OpenXR
- Android XR / Quest standalone
- flat Android
- web/WASM

The basic gameplay/rendering loop is proven across those lanes:

- local integrated runtime
- locomotion and input
- world rendering
- chunk loading and generation

Native desktop flat remains the fastest daily bring-up path. The headless/offscreen path is being promoted into a real no-window flat-client validation host rather than a screenshot-only helper.

Shared gameplay, runtime, asset, mesh, UI, renderer, and XR contracts must stay host-neutral. Client platform and server host mode are separate axes: every client lane should be able to play against a dedicated server, with future P2P/session topologies fitting behind the same shared command/update contracts.

## Project Map

Current project posture and work indexes:

- [`docs/platforms.md`](docs/platforms.md) — current platform matrix
- [`docs/native-rewrite-roadmap.md`](docs/native-rewrite-roadmap.md) — durable native architecture
- [`docs/tactical/`](docs/tactical/README.md) — native Rust workstream tacticals
- [`docs/topics/`](docs/topics/README.md) — durable subsystem progress indexes
- [`docs/native-web.md`](docs/native-web.md) — Rust/WASM web build, smoke, and deploy notes
- [`docs/reference-minecraft.md`](docs/reference-minecraft.md) — Minecraft 1.17.1 reference tree, bootstrap scripts, and mapping notes

Core architecture docs:

- [`docs/offscreen-flat-client.md`](docs/offscreen-flat-client.md) — offscreen flat-client target
- [`docs/architecture.md`](docs/architecture.md) — runtime/host split
- [`docs/runtime-data-model.md`](docs/runtime-data-model.md) — shared chunk/block-state data contracts
- [`docs/protocol.md`](docs/protocol.md) — host/client message model
- [`docs/loading-persistence.md`](docs/loading-persistence.md) — world loading and save policy
- [`docs/persistence-architecture.md`](docs/persistence-architecture.md) — broader shared persistence target

Platform-specific entry points:

- [`android/README.md`](android/README.md) — flat Android
- [`android-xr/README.md`](android-xr/README.md) — Quest/OpenXR

Worldgen, rendering, and subsystem docs:

- [`docs/strategy.md`](docs/strategy.md) — translation/oracle policy
- [`docs/worldgen-deterministic-order.md`](docs/worldgen-deterministic-order.md) — vanilla status order, decoration finality, lighting gates, and chunk publication gates
- [`docs/lod-architecture.md`](docs/lod-architecture.md) — far-terrain LOD plan
- [`docs/carver-status.md`](docs/carver-status.md) — carver-parity/oracle tracker
- [`docs/structures.md`](docs/structures.md) — vanilla overworld structure generation architecture
- [`docs/liquids.md`](docs/liquids.md) — liquid simulation architecture
- [`docs/entity-architecture.md`](docs/entity-architecture.md) — entity/mob runtime boundaries
- [`docs/creatures.md`](docs/creatures.md) — overworld creature spawning architecture
- [`docs/performance-records.md`](docs/performance-records.md) — native benchmark baselines
- [`docs/assets-plan.md`](docs/assets-plan.md) — asset extraction

Reference material:

- [`reference/minecraft-1.17.1/src/`](reference/minecraft-1.17.1/src/) — primary source for vanilla behavior and visual correctness
- `~/code/playbox` — local Rust `winit`/`wgpu`, headless capture, diagnostics, Android, and OpenXR pattern library
- [`oracle/`](oracle/) — retained Java and TypeScript reference tooling
- [`test/fixtures/`](test/fixtures/) — shared oracle fixture data consumed by native Rust tests

The retired browser engine has been removed from the live tree. Worldgen aims for **seed parity** with Minecraft Java 1.17.1 so we can oracle-test against real MC output.

## Stack

- **Primary language:** Rust, under [`native/`](native/).
- **Primary development loop:** desktop flat, with `mclone-native-client` as the fastest interactive loop and the offscreen flat-client host as the target no-window validation path.
- **Desktop XR:** opt-in OpenXR mode in `mclone-native-client` behind the `xr` feature; validated with real mclone stereo terrain and controller locomotion through shared XR crates.
- **Android XR / Quest:** standalone Quest OpenXR package under [`android-xr/`](android-xr/) using `mclone-android-xr-client`; validated with staged assets, real stereo terrain, controller setup, and basic locomotion.
- **Flat Android:** non-XR `NativeActivity` APK under [`android/`](android/) using `mclone-android-client`; validated with AVD screenshot and touch-orbit smokes.
- **Web target:** Rust/WASM browser client through `mclone-web-client`, WebGPU, browser workers, and deployment at `mclone.kzahel.com`.
- **Renderer:** `wgpu`, native first, web-compatible capability checks at renderer milestones. Vanilla visual behavior should be checked against the Java 1.17.1 client source before borrowing renderer policy from other engines.
- **Worldgen:** direct Rust port of MC Java 1.17.1's pipeline; bit-exact seed parity is the correctness bar.
- **Protocol/runtime:** `mclone_protocol`, `mclone_net`, `mclone_server`, `mclone_client`, `mclone_app_runtime`, and `mclone_render_session` keep singleplayer, remote, render-section, and platform app paths on shared contracts.
- **Host mode invariant:** local integrated, remote dedicated, and future P2P/session modes are runtime host choices, not platform identities. Desktop, web, flat Android, and XR clients should converge on the same client/server protocol and runtime shell wherever the display/input platform permits it.
- **XR sharing:** `mclone_xr_host`, `mclone_xr_graphics`, and `mclone_xr_scene` keep desktop XR and Android XR from growing private copies of session, swapchain, terrain, and controller-locomotion behavior.
- **Java reference client:** `reference/minecraft-1.17.1/src/` is the authority for vanilla block/entity rendering behavior, model baking, atlas stitching, mipmaps/filtering, render layers, lighting, fog, sky, particles, and client-visible state.
- **Sibling reference engine:** `~/code/playbox` is the local Rust `winit`/`wgpu`/headless/Android/OpenXR pattern library. For platform or XR work, start with its `Cargo.toml`, `docs/architecture/rendering.md`, `docs/architecture/platforms.md`, `android/README.md`, and `android-xr/README.md`.
- **Oracle tooling:** Java and TypeScript fixture-generation helpers live under [`oracle/`](oracle/); shared fixture JSON remains under [`test/fixtures/`](test/fixtures/) and is consumed by native Rust tests.

## Common Validation

Use the native Rust workspace for implementation work:

```bash
cargo test --manifest-path native/Cargo.toml
pnpm native:worldgen:smoke
pnpm native:movement:smoke
pnpm native:timedemo:smoke
pnpm native:web:build
pnpm native:web:smoke
```

Rendered-output work should use the native headless/offscreen capture paths where available and keep screenshots outside the repo, for example under `/tmp`.
