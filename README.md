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

The current architecture target is a shared client-experience core with
profile-specific presentation/adapters, so flat, XR, web, Android, offscreen,
and emulated validation lanes keep one product behavior surface instead of
separate platform clients. See
[`docs/client-experience-architecture.md`](docs/client-experience-architecture.md).

## Project Map

Current project posture and work indexes:

- [`docs/platforms.md`](docs/platforms.md) — current platform matrix
- [`docs/native-engine-architecture.md`](docs/native-engine-architecture.md) — durable native architecture
- [`docs/tactical/`](docs/tactical/README.md) — native Rust workstream tacticals
- [`docs/topics/`](docs/topics/README.md) — durable subsystem progress indexes
- [`docs/native-web.md`](docs/native-web.md) — Rust/WASM web build, smoke, and deploy notes
- [`docs/reference-minecraft.md`](docs/reference-minecraft.md) — Minecraft 1.17.1 reference tree, bootstrap scripts, and mapping notes

Core architecture docs:

- [`docs/client-experience-architecture.md`](docs/client-experience-architecture.md) — draft shared client-experience core, profiles, adapters, and platform parity guardrails
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

## Code Layout

- [`native/Cargo.toml`](native/Cargo.toml) - primary Rust workspace
- [`native/apps/mclone-native-client/`](native/apps/mclone-native-client/) - desktop flat app shell, desktop OpenXR entrypoint, CLI diagnostics, and offscreen capture entrypoints
- [`native/apps/mclone-web-client/`](native/apps/mclone-web-client/) - Rust/WASM browser client
- [`native/apps/mclone-android-client/`](native/apps/mclone-android-client/) - flat Android client
- [`native/apps/mclone-android-xr-client/`](native/apps/mclone-android-xr-client/) - Quest / Android XR client
- [`native/apps/mclone-dedicated-server/`](native/apps/mclone-dedicated-server/) - headless dedicated server
- [`native/crates/`](native/crates/) - shared engine, protocol, runtime, renderer, UI, worldgen, asset, mesh, lighting, and XR crates
- [`oracle/`](oracle/) - Java and TypeScript fixture-generation helpers
- [`test/fixtures/`](test/fixtures/) - shared oracle fixture data consumed by native Rust tests
- [`reference/minecraft-1.17.1/`](reference/minecraft-1.17.1/) - generated, gitignored Minecraft reference tree

## Common Validation

Recommended default gates:

```bash
cargo test --manifest-path native/Cargo.toml
pnpm native:desktop-offscreen:smoke
pnpm native:web:build
```

Use [`docs/platforms.md`](docs/platforms.md) for the full platform validation matrix. Rendered-output work should use the native headless/offscreen capture paths where available and keep screenshots outside the repo, for example under `/tmp`.
