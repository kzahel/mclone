# 110: Shared Physics Backend Experiment

Status: active; Slice 1 landed with the shared `mclone-physics` facade crate
and no-op backend. Rapier is not wired yet.

## Purpose

Investigate a real rigid-body physics backend without making physics a default
engine dependency, without growing desktop-only behavior, and without committing
the project to Rapier if the experiment does not pay for itself.

The initial target is not "physics everywhere." The target is a narrow,
measured, shared experiment:

- a compile-time optional physics backend
- server-authoritative dynamic test bodies
- static terrain collision generated from nearby chunk/section facts
- desktop validation first
- native web/WASM viability behind the same shared contract

## Current State

- Player movement and collision are custom engine behavior, currently driven
  through shared client/render-session/server contracts rather than a general
  rigid-body world.
- `mclone-server` already has basic scheduled falling-block ticks for
  sand/red-sand/gravel. That path deliberately stops short of a
  `FallingBlockEntity` port and should stay Minecraft-rule-driven.
- The web client already uses browser workers and `SharedArrayBuffer` for
  render/server job lanes, but the stable web build should not be forced onto a
  nightly threaded-WASM toolchain just to test physics.
- Runtime boundaries should stay chunk/section-sized. Do not create a design
  that requires per-voxel calls across app, worker, protocol, or WASM adapter
  boundaries.

## Backend Choice

Use Rapier as the first backend candidate, behind a feature gate.

Reasons:

- It is Rust-native and engine-agnostic.
- It has official Rust and JavaScript/WASM surfaces.
- It supports the primitives the experiment needs: cuboids, balls, capsules,
  compound shapes, sensors, collision groups, scene queries, collision events,
  static triangle/heightfield terrain, and voxel colliders.
- It has explicit optional features for SIMD, Rayon parallelism, serde, WASM
  bindgen integration, and enhanced determinism.

Constraints:

- Do not let Rapier types leak into app crates, renderer crates, protocol
  crates, or gameplay logic.
- Do not enable Rapier by default in normal workspace builds.
- Do not depend on Rapier parallelism for the MVP. Rapier's `parallel` feature
  is Rayon-based and only helps on sufficiently large scenes; web Rayon also
  brings `wasm-bindgen-rayon`, cross-origin isolation, and threaded-WASM build
  constraints.
- Treat `enhanced-determinism`, `parallel`, and SIMD as mutually exclusive
  policy choices until measured otherwise. Rapier documents that
  `enhanced-determinism` cannot be enabled together with `parallel` or SIMD.

Reference docs to re-check before landing the dependency:

- <https://rapier.rs/docs/user_guides/rust/getting_started/>
- <https://rapier.rs/docs/user_guides/rust/colliders/>
- <https://rapier.rs/docs/user_guides/rust/determinism/>
- <https://github.com/RReverser/wasm-bindgen-rayon>

## Target Shape

Add a shared crate:

```text
native/crates/mclone-physics
```

The crate owns the physics facade and backend selection. App crates and server
code use mclone-owned types such as:

- `PhysicsWorld`
- `PhysicsBackendKind`
- `PhysicsBodyId`
- `PhysicsColliderId`
- `PhysicsTerrainPatch`
- `PhysicsBodyPose`
- `PhysicsBodySpawn`
- `PhysicsStepReport`

The default crate build must be cheap:

- no Rapier dependency
- no Rayon dependency
- no web threading glue
- deterministic no-op or tiny test backend only

Feature shape:

```toml
[features]
default = []
rapier = ["dep:rapier3d"]
rapier-serde = ["rapier", "rapier3d/serde-serialize"]
rapier-deterministic = ["rapier", "rapier3d/enhanced-determinism"]
rapier-parallel = ["rapier", "rapier3d/parallel"]
rapier-simd = ["rapier", "rapier3d/simd-stable"]
```

Higher-level crates should expose intent features instead of naming Rapier at
every callsite:

```toml
[features]
physics = ["dep:mclone-physics"]
physics-rapier = ["physics", "mclone-physics/rapier"]
```

Exact feature names may change during implementation, but the policy should
hold: default builds do not compile Rapier, and all Rapier use is isolated to
the shared physics crate.

## Authority Model

The authoritative simulation owns physics.

- Local integrated server: physics runs inside the server/runtime simulation
  tick.
- Dedicated server: physics runs on the dedicated server, and clients receive
  entity pose updates.
- Browser local integrated: physics should eventually run inside the integrated
  server worker, not on the canvas/main-thread path.
- Client prediction is a later optional layer. Do not require it for the MVP.

The renderer should receive ordinary entity snapshots and draw debug/test
physics bodies like any other simple entity. The renderer should not own
Rapier, rigid-body handles, or collision queries.

## Terrain Collision Policy

Do not create or destroy a collider per block.

The physics backend receives terrain as coarse patches derived from already
loaded chunk/section facts. Patches should be built and retained at
chunk-section or small region granularity, invalidated when relevant blocks
mutate, and updated when active physics bodies move far enough to need more
nearby terrain.

Initial terrain collider candidates, in benchmark order:

1. merged cuboid runs or compound cuboids for solid block spans
2. Rapier voxel colliders for section-shaped occupied-cell volumes
3. triangle meshes generated from visible collision faces

Heightfields are not an MVP terrain answer because vanilla terrain has caves,
overhangs, trees, and vertical structures. They can be benchmarked later for
surface-only approximations if useful.

Use local coordinates for physics islands. Large absolute world coordinates
should be rebased around the active patch/body region before entering the
backend.

## MVP Demonstration

The first user-visible demonstration should be a throwable cube, not a falling
tree.

Required behavior:

- Spawn one test cube near the local player through a debug command, menu
  action, or temporary keybind.
- Give it an initial velocity based on camera/player facing.
- Build a small static terrain patch around the spawn point.
- Step the physics body on the authoritative simulation path.
- Publish the cube pose as a normal server entity update.
- Render the cube as a simple block-sized test entity.
- Let it bounce, slide, settle, and sleep against terrain.
- Report physics diagnostics: body count, collider count, active body count,
  terrain patch count, terrain build time, physics step time, and body pose
  update count.

Acceptance:

- Desktop flat can throw a cube into nearby generated terrain.
- The default workspace build still does not compile Rapier.
- `physics-rapier` builds and runs behind an explicit app/smoke flag.
- The cube path does not require app crates to import Rapier.
- Terrain patch generation is section/region-sized, not per-frame per-block
  collider churn.
- A headless/offscreen or smoke path records that the body moves, collides,
  and settles without requiring visual inspection only.

## Why Not Falling Trees First

Falling trees are attractive but they mix several problems:

- connected-component extraction from block terrain
- support rules for logs, leaves, vines, fluids, and adjacent trees
- chunk-boundary ownership
- converting blocks into a temporary dynamic body
- deciding whether the body re-voxelizes, drops items, or remains an entity
- multiplayer authority and update routing

Those are game-policy problems. Rapier can simulate a detached temporary body,
but it will not decide the Minecraft-like extraction and settling policy.

A later tree slice should start with an intentionally tiny case: a small
artificial trunk/canopy cluster converted into one compound rigid body made of
4-12 cuboids. Only after that works should real tree detection be considered.

## Implementation Slices

### Slice 1 - Facade Crate And No-Op Backend

- [x] Add `mclone-physics` to the workspace.
- [x] Define facade types and a no-op backend with no external physics
  dependency.
- [x] Add unit tests for feature-independent facade constructors, body IDs,
  terrain patch lifecycle, and step reports.
- [x] Keep default workspace builds unchanged in dependency weight.

Recorded Slice 1 result:

- Added `native/crates/mclone-physics` with no dependency beyond
  `mclone-core`.
- Added mclone-owned facade types for backend kind, body/collider ids, body
  kind, shapes, pose, velocity, body spawn, terrain patch, and step reports.
- Added `PhysicsWorld` backed by a no-op implementation that accepts body
  spawns, body pose updates, terrain patch add/update/remove calls, and returns
  stable diagnostics without simulating.
- Kept Rapier, Rayon, SIMD, serde, and web-threading glue out of the default
  build.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-physics
cargo check --manifest-path native/Cargo.toml
```

### Slice 2 - Rapier Backend Behind Feature

- [ ] Add optional `rapier3d` dependency and backend implementation.
- [ ] Keep Rapier handles internal to `mclone-physics`.
- [ ] Map mclone `glam`/core math and block units into Rapier local-island
  coordinates.
- [ ] Add synthetic tests for a dynamic cuboid falling onto a static cuboid.
- [ ] Record compile-time and binary-size deltas for default vs.
  `physics-rapier`.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-physics
cargo test --manifest-path native/Cargo.toml -p mclone-physics --features rapier
cargo check --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
```

### Slice 3 - Terrain Patch Benchmark

- [ ] Build one section-sized static terrain patch from loaded block facts.
- [ ] Benchmark merged cuboids, Rapier voxel colliders, and triangle meshes
  against the same section fixtures.
- [ ] Measure build time, collider count, memory shape where available, and
  step/query cost with one dynamic cube and one capsule.
- [ ] Choose the first MVP terrain representation based on measurement, not
  aesthetics.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-physics --features rapier
```

Add a focused benchmark/smoke command if normal tests are too noisy.

### Slice 4 - Server-Authoritative Throwable Cube

- [ ] Add an explicit physics-enabled server/runtime path.
- [ ] Add a temporary debug spawn command for one throwable cube.
- [ ] Publish cube pose through existing entity/update concepts or a small
  shared entity extension.
- [ ] Render the test cube without adding renderer-side physics knowledge.
- [ ] Add physics diagnostics to the native smoke output.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features physics-rapier
pnpm native:movement:smoke
```

If the slice produces pixels, capture and inspect a screenshot before moving
on.

### Slice 5 - Web Worker Viability

- [ ] Compile the no-threaded Rapier backend for `wasm32-unknown-unknown`.
- [ ] Run the same throwable-cube authority path inside the browser integrated
  server worker.
- [ ] Keep physics single-threaded for the first web pass.
- [ ] Add browser diagnostics for physics step time and terrain patch churn.
- [ ] Only after this passes, decide whether threaded WASM/Rayon is worth a
  separate tactical.

Validation:

```bash
pnpm native:web:build
pnpm native:web:smoke
```

### Slice 6 - Small Compound Body Probe

- [ ] Spawn a fixed test compound body made of several cuboids.
- [ ] Validate that it falls, rotates, collides, and sleeps acceptably.
- [ ] Use this as the first proxy for detached tree/block clusters.
- [ ] Do not implement real tree extraction in this slice.

## Threading And WASM Policy

The MVP should assume single-threaded physics.

Native desktop may later enable Rapier `parallel` if scenes are large enough to
benefit. Browser parallelism should be treated as a separate investigation
because it requires a shared-memory/threaded-WASM toolchain shape and browser
deployment constraints beyond the current stable baseline.

For web:

- run physics in a worker, not the browser main thread
- prefer stable `wasm32-unknown-unknown` for the first pass
- do not introduce `wasm-bindgen-rayon` unless a measured scene needs it
- keep all worker setup in web app/platform code, not in shared physics policy

## Determinism Policy

Do not require cross-platform deterministic physics for the first experiment.

The server is authoritative. Native and web clients can render received entity
poses. If future prediction, replay, or lockstep behavior needs deterministic
physics, evaluate Rapier `enhanced-determinism` as a separate policy choice and
accept that it conflicts with SIMD/parallel acceleration.

## Guardrails

- Do not put Rapier in `mclone-native-client`, `mclone-web-client`, renderer
  crates, protocol crates, or app-runtime platform adapters.
- Do not route gameplay policy through physics handles.
- Do not replace Minecraft-specific block ticks with a rigid-body engine.
- Do not generate per-block terrain colliders for loaded chunks.
- Do not make normal default builds compile Rapier.
- Do not make threaded WASM a prerequisite for the first web physics path.
- Do not fork physics behavior per platform. Platform adapters may own worker
  setup and timing integration, but the physics contract stays shared.

## Open Questions

- Should `mclone-server` always depend on cheap `mclone-physics`, or should
  even the facade dependency be behind a `physics` feature?
- Which terrain representation wins for one section: merged cuboids, voxel
  collider, or triangle mesh?
- Should a sleeping dynamic block entity re-enter the terrain as a block, stay
  an entity, or become an item/drop?
- How much entity-update bandwidth is acceptable for dynamic physics bodies in
  dedicated-server play?
- Should physics coordinates be rebased per player-active island, per chunk
  region, or per body cluster?
