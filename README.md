# mclone

Mclone is a Minecraft-inspired voxel sandbox built on a shared Rust engine.

The project is internal and unreleased today, but it is being built for an
eventual public release rather than only personal or home/LAN use. Release
readiness requires a complete distributable first-party asset set, sufficiently
complete world generation and gameplay, and the remaining product and release
work. Local Minecraft 1.17.1 reference code and assets are optional comparative
and legacy-test inputs only; they are not the product direction or public
release content.

## Project Status

Mclone has five first-class client targets:

- flat Android
- Android XR / Quest standalone
- desktop flat
- desktop OpenXR
- web/WASM

All five are equally valid product targets. None is the preferred product,
implementation home, or measure of completeness for the others. Target-specific
adapters own operating-system, browser, activity, surface, input, and OpenXR
integration while the product behavior stays in shared contracts.

The established shared product path includes local integrated and remote
dedicated-server sessions, persistence, locomotion and interaction, chunk
generation and streaming, and world, actor, HUD, and menu rendering. Offscreen
flat and headset-free synthetic-stereo hosts exercise the same scene and
renderer contracts as additional validation hosts; they are not separate
product targets.

Gameplay, runtime, asset, mesh, UI, renderer, and XR contracts stay
host-neutral. Client platform and server host mode are separate axes: every
client target supports the shared local/remote session model, with future
P2P/session topologies fitting behind the same command/update contracts.

The architecture centers on a shared client-experience core with
profile-specific presentation and adapters, so flat, XR, web, Android,
offscreen, and emulated validation paths keep one product behavior surface
instead of becoming separate platform clients. See
[`docs/client-experience-architecture.md`](docs/client-experience-architecture.md).

## Project Map

Current project posture and work indexes:

- [`docs/platforms.md`](docs/platforms.md) — current platform matrix
- [`docs/topics/platform-parity.md`](docs/topics/platform-parity.md) — feature and shared-contract parity matrix
- [`docs/native-engine-architecture.md`](docs/native-engine-architecture.md) — durable engine architecture
- [`docs/tactical/`](docs/tactical/README.md) — implementation tacticals and execution records
- [`docs/topics/`](docs/topics/README.md) — durable subsystem progress indexes
- [`docs/native-web.md`](docs/native-web.md) — Rust/WASM web build, smoke, and deploy notes
- [`docs/linux-setup.md`](docs/linux-setup.md) — Linux toolchain, assets, GPU access, and no-window smoke setup
- [`docs/reference-minecraft.md`](docs/reference-minecraft.md) — Minecraft 1.17.1 reference tree, bootstrap scripts, and mapping notes

Core architecture docs:

- [`docs/client-experience-architecture.md`](docs/client-experience-architecture.md) — accepted shared client-experience core, profiles, adapters, and platform parity guardrails
- [`docs/offscreen-flat-client.md`](docs/offscreen-flat-client.md) — offscreen flat-client target
- [`docs/architecture.md`](docs/architecture.md) — runtime/host split
- [`docs/runtime-data-model.md`](docs/runtime-data-model.md) — shared chunk/block-state data contracts
- [`docs/protocol.md`](docs/protocol.md) — host/client message model
- [`docs/frame-pipeline-accounting.md`](docs/frame-pipeline-accounting.md) — frame/terrain/server accounting model for performance work
- [`docs/loading-persistence.md`](docs/loading-persistence.md) — world loading and save policy
- [`docs/persistence-architecture.md`](docs/persistence-architecture.md) — broader shared persistence target

Platform-specific entry points:

- [`android/README.md`](android/README.md) — flat Android
- [`android-xr/README.md`](android-xr/README.md) — Quest/OpenXR

Worldgen, rendering, and subsystem docs:

- [`docs/strategy.md`](docs/strategy.md) — original-product/reference boundary
- [`docs/worldgen-deterministic-order.md`](docs/worldgen-deterministic-order.md) — retained Java-order research and shared scheduling lessons
- [`docs/topics/lod.md`](docs/topics/lod.md) — current LOD terminology, ownership, and documentation routes
- [`docs/topics/procedural-horizon-clipmap.md`](docs/topics/procedural-horizon-clipmap.md) — current shared terrain-horizon LOD implementation
- [`docs/lod-architecture.md`](docs/lod-architecture.md) — retired chunk-based Far LOD architecture archive
- [`docs/carver-status.md`](docs/carver-status.md) — legacy Java-shaped carver/oracle record
- [`docs/structures.md`](docs/structures.md) — original structure foundation and retained Java architecture reference
- [`docs/liquids.md`](docs/liquids.md) — liquid simulation architecture
- [`docs/entity-architecture.md`](docs/entity-architecture.md) — entity/mob runtime boundaries
- [`docs/creatures.md`](docs/creatures.md) — overworld creature spawning architecture
- [`docs/topics/habitat-driven-creature-ecology.md`](docs/topics/habitat-driven-creature-ecology.md) — terrain/creature co-design, habitats, persistence, and mechanics-led content promotion
- [`docs/topics/playable-showcases.md`](docs/topics/playable-showcases.md) — bounded tiny-save screenshot/play links and live-instantiation guardrails
- [`docs/topics/performance.md`](docs/topics/performance.md) — high-priority known performance issues and current pickup queue
- [`docs/topics/actor-rendering-performance.md`](docs/topics/actor-rendering-performance.md) — actor rendering baselines, memory tradeoffs, and future optimization queue
- [`docs/topics/xr-render-path-switching.md`](docs/topics/xr-render-path-switching.md) — live dual-eye, array per-eye, and multiview selection with one resident XR target family
- [`docs/performance-records.md`](docs/performance-records.md) — native benchmark baselines
- [`docs/assets-plan.md`](docs/assets-plan.md) — local Minecraft reference-asset extraction
- [`docs/topics/asset-pack-profiles.md`](docs/topics/asset-pack-profiles.md) — first-party asset packs, provenance, and remaining distribution boundary

Reference and oracle material:

- `reference/minecraft-1.17.1/src/` — generated, gitignored local source tree for comparative behavior and visual research
- [`oracle/`](oracle/) — retained Java and TypeScript reference tooling
- [`test/fixtures/`](test/fixtures/) — shared oracle fixture data consumed by Rust tests

The generated reference tree and extracted Minecraft assets are not part of the
live engine or distributable content. **Mclone Overworld**
(`mclone-overworld-v1`) is the normal new-world default and the active product
world-generation direction. The retained Java-1.17-shaped `overworld` profile
and oracle fixtures are legacy development/reference surfaces, not an active
seed-parity target.

## Code Layout

- [`native/Cargo.toml`](native/Cargo.toml) - primary Rust workspace
- [`native/apps/mclone-native-client/`](native/apps/mclone-native-client/) - desktop flat app shell, desktop OpenXR entrypoint, CLI diagnostics, and offscreen capture entrypoints
- [`native/apps/mclone-web-client/`](native/apps/mclone-web-client/) - Rust/WASM browser client
- [`native/apps/mclone-android-client/`](native/apps/mclone-android-client/) - flat Android client
- [`native/apps/mclone-android-xr-client/`](native/apps/mclone-android-xr-client/) - Quest / Android XR client
- [`native/apps/mclone-world-explorer/`](native/apps/mclone-world-explorer/) - standalone native/browser host for the shared terrain-horizon LOD system
- [`native/apps/mclone-dedicated-server/`](native/apps/mclone-dedicated-server/) - headless dedicated server
- [`native/crates/mclone-terrain-view/`](native/crates/mclone-terrain-view/) - shared procedural-horizon LOD, terrain composition, and detached exact-view owner
- [`native/crates/`](native/crates/) - shared engine, protocol, runtime, renderer, UI, worldgen, asset, mesh, lighting, and XR crates
- [`oracle/`](oracle/) - Java and TypeScript fixture-generation helpers
- [`test/fixtures/`](test/fixtures/) - shared oracle fixture data consumed by Rust tests
- `reference/minecraft-1.17.1/` - generated, gitignored Minecraft reference tree

## Previewing First-Party Assets

Build the current first-party packs and launch the desktop client with the
proprietary-free **Mclone Original** selection forced for this process:

```bash
pnpm native:original-assets
```

This selection enables authored Mclone assets over the generated fallback and
does not make the local Minecraft reference pack eligible. Missing authored
art therefore remains conspicuous instead of silently falling back to
Minecraft content.

For the ordinary persisted selection, first run
`pnpm assets:pack:first-party`, launch the client, then open **Options → Asset
Packs**, enable **Mclone Original Assets**, disable **Minecraft 1.17.1
Reference**, and choose **Apply**. `pnpm assets:validate:first-party` is the
strict provenance check for the same authored-plus-generated selection.

## Validation

The shared Rust workspace gate is:

```bash
cargo test --manifest-path native/Cargo.toml
```

Each client target has appropriate build, smoke, device, and rendered-output
gates. Use [`docs/platforms.md`](docs/platforms.md) for the complete validation
matrix and choose gates based on the contracts changed, not a ranking of
targets. Keep generated screenshots outside the repo, for example under
`/tmp`.
