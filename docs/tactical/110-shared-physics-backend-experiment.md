# 110: Shared Physics Backend Experiment

Status: active; Slices 1-4c landed with the shared `mclone-physics` facade,
no-op backend, optional Rapier backend, synthetic cuboid/static-patch
simulation test, section terrain collider probe, feature-gated live
chunk-to-physics terrain conversion, server-owned debug cube physics runtime,
debug cube protocol/entity publication, desktop `F7` diagnostic launch wiring,
fresh debug cube entity IDs on repeated throws, one-way debug player AABB
collision, and `mclone-server` / native-client intent feature wiring. App
binary-size measurement is deferred until the physics path is promoted beyond
diagnostics.

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

- [x] Add optional `rapier3d` dependency and backend implementation.
- [x] Keep Rapier handles internal to `mclone-physics`.
- [x] Map mclone core math and block units into Rapier's f32/glam-backed
  coordinate types.
- [x] Add synthetic tests for a dynamic cuboid falling onto a static cuboid.
- [ ] Record app binary-size deltas for default vs. `physics-rapier` once an
  app/server path actually links physics.

Recorded Slice 2 result:

- Added `rapier3d = "0.33"` as an optional `mclone-physics` dependency.
- Added `rapier`, `rapier-deterministic`, `rapier-parallel`, `rapier-serde`,
  and `rapier-simd` feature gates in `mclone-physics`.
- Replaced the private `PhysicsWorld` storage with a backend enum so the public
  facade still exposes only mclone-owned ids, poses, shapes, terrain patches,
  and reports.
- Added a private Rapier backend with `RigidBodySet`, `ColliderSet`, pipeline,
  island/broad/narrow phase, joint sets, CCD solver, and body/terrain handle
  maps hidden from callers.
- Added mclone-to-Rapier conversions for cuboid, ball, capsule-Y, body pose,
  body velocity, and static AABB terrain patches.
- Added a `rapier_world_simulates_dynamic_cube_against_static_patch` test that
  drops a dynamic cube onto a static terrain patch and verifies pose updates
  and settling.
- Added `mclone-server` manifest-only `physics` / `physics-rapier` features so
  higher-level intent-feature wiring can be validated without server behavior
  integration yet.
- Noted implementation detail: Rapier 0.33's public math aliases are
  glam/glamx-backed (`Vector`, `Rotation`, `Pose`), not the older
  nalgebra-shaped example API.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-physics
cargo test --manifest-path native/Cargo.toml -p mclone-physics --features rapier
cargo check --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml
```

### Slice 3 - Terrain Patch Benchmark

- [x] Define a section-sized static terrain occupancy input for collider
  building.
- [x] Benchmark merged cuboids, Rapier voxel colliders, and triangle meshes
  against the same section fixtures.
- [x] Measure build time, collider count, primitive/mesh shape, and
  step/query cost with one dynamic cube and one capsule.
- [x] Choose the first MVP terrain representation based on measurement, not
  aesthetics.
- [x] Wire section terrain input to real loaded chunk/block facts in the
  server-authoritative slice.

Recorded Slice 3 result:

- Added `PhysicsTerrainSection`, a fixed 16x16x16 section occupancy container
  with mclone-owned coordinates and no Rapier dependency in the default build.
- Added `run_rapier_section_terrain_probe`, available only behind the `rapier`
  feature, to build the same terrain section three ways and run one dynamic cube
  plus one capsule against each candidate for 240 simulation steps.
- The synthetic fixture is a 16x16 floor plus a three-block pillar: 259 solid
  cells total.
- Stable fixture shape counts:
  - merged X-runs: 19 cuboid primitives
  - Rapier voxel collider: 259 voxel cells
  - visible-face trimesh: 1,176 triangles and 2,352 vertices
- All three candidates settle the cube and capsule against the fixture. For the
  first runtime MVP, prefer merged X-run compound cuboids until real chunk
  fixtures show that voxel colliders or triangle meshes win. This keeps flat
  terrain compact while still representing vertical section details.
- Added a feature-gated server terrain-source follow-up:
  - `mclone-server/src/physics_terrain.rs` converts `MutableChunkBlockBuffer`
    live chunk sections into `PhysicsTerrainSection`.
  - `ChunkScheduler::physics_terrain_section(...)` and
    `ChunkScheduler::physics_terrain_section_at_block(...)` expose that source
    without leaking holders, live block buffers, or Rapier.
  - The MVP solidity classifier uses the existing
    `mclone_worldgen::block::material_blocks_motion` taxonomy, so air, fluids,
    snow, plants, glow lichen, and pointed dripstone do not become full physics
    terrain cells, while ordinary full-cube terrain and leaves do.
  - Default server builds still do not compile `mclone-physics`; the terrain
    source exists only behind the `physics` / `physics-rapier` intent features.
- Still pending: retaining/invalidation of built colliders around active bodies,
  dynamic body ownership in the server tick, entity publication, rendering, and
  physics diagnostics.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-physics
cargo test --manifest-path native/Cargo.toml -p mclone-physics --features rapier
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml
```

Add a focused benchmark/smoke command if normal tests are too noisy.

### Slice 4 - Server-Authoritative Throwable Cube

- [x] Add an explicit physics-enabled server/runtime path.
- [x] Add a temporary debug/test spawn API for one cube.
- [x] Step the debug cube against a live scheduler terrain section.
- [x] Add physics diagnostics to simulation tick reports.
- [x] Add a temporary debug spawn command for one throwable cube.
- [x] Publish cube pose through existing entity/update concepts or a small
  shared entity extension.
- [x] Render the test cube without adding renderer-side physics knowledge.
- [ ] Add physics diagnostics to the native smoke output.

Recorded Slice 4a result:

- Added `PhysicsWorld::add_terrain_section` / `update_terrain_section` /
  `remove_terrain_section` / `terrain_section` facade APIs.
- The no-op backend stores terrain sections without simulating; the Rapier
  backend converts terrain sections to merged X-run compound cuboids, matching
  the Slice 3 MVP recommendation.
- Added `mclone-server/src/physics_runtime.rs`, feature-gated behind
  `physics-rapier`, with one server-owned debug cube, one retained terrain
  section collider, and no Rapier handles exposed outside `mclone-physics`.
- Added `IntegratedServer::spawn_debug_physics_cube(...)`,
  `debug_physics_cube_pose(...)`, and `physics_diagnostics()` for tests and the
  next debug command slice.
- Added `ServerPhysicsTickDiagnostics` to simulation and runner tick reports.
  Default builds report disabled/zero diagnostics; `physics-rapier` builds fill
  body, collider, active-body, terrain-collider, pose-update, and test-cube
  position facts.
- Added an integrated server test that loads a real chunk, forces one live
  section into a 16x16 floor fixture, spawns the debug cube, and verifies it
  falls and settles through the authoritative simulation tick.
- Still pending: a protocol/debug command or input hook, entity publication,
  app feature wiring, client replica/rendering, smoke output, and broader
  terrain collider retention/invalidation around moving bodies.

Slice 4a validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-physics
cargo test --manifest-path native/Cargo.toml -p mclone-physics --features rapier
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml
```

If the slice produces pixels, capture and inspect a screenshot before moving
on.

Recorded Slice 4b result:

- Added `ClientCommand::ShootDebugPhysicsCube` and bumped
  `PROTOCOL_VERSION` to 13.
- Added `EntityKind::DebugCube`; the server publishes the physics body pose as
  ordinary entity snapshots/updates, with the Rapier center pose converted to
  the entity bottom/feet position used by actor rendering.
- Added a server command handler that launches the cube from the authoritative
  player pose using the current yaw/pitch, so local, dedicated, and future web
  client lanes use the same command.
- Expanded debug terrain collision from one current section to a capped 3x3x3
  loaded-section island around the launch section, skipping empty sections.
- Added `mclone-native-client` feature forwarding for `physics-rapier` and a
  desktop `F7` diagnostic hotkey. With physics disabled, the command is a
  validated no-op; with `physics-rapier`, it spawns/publishes the cube.
- Added a simple colored block-sized actor for `DebugCube` in the shared render
  path without exposing physics handles or Rapier types to render crates.
- Added tests for the command codec, debug cube entity publication/update, the
  bounded terrain island behavior, and the cube actor mesh.

Slice 4b validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-protocol
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-client
cargo test --manifest-path native/Cargo.toml -p mclone-physics --features rapier
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features physics-rapier
cargo check --manifest-path native/Cargo.toml
pnpm native:web:build
pnpm native:desktop-offscreen:smoke
```

Screenshot inspected:

```text
/tmp/mclone-desktop-offscreen.png
```

The current offscreen smoke validates the composed native frame and actor path,
but it does not yet press `F7` or include physics diagnostics in its printed
report. A dedicated scripted physics screenshot/smoke remains pending.

Recorded Slice 4c result:

- Repeated `ShootDebugPhysicsCube` commands now allocate a fresh debug cube
  entity ID and publish an `EntityRemove` for the previous debug cube entity.
  This keeps client actor interpolation from treating a new throw as a smooth
  continuation of the old cube.
- Per-tick physics synchronization still updates the current debug cube entity
  in place, so ordinary movement interpolation remains available after spawn.
- The feature-gated server physics runtime now owns a fixed player AABB
  collider using the current player dimensions, updated before physics steps
  for the player that launched the cube.
- The player collider is intentionally one-way for this diagnostic slice: the
  cube collides with the player body, but physics does not push, correct, or
  otherwise author player movement.
- Added `physics-rapier` server tests covering fresh debug cube entity IDs,
  previous-entity removal, and cube collision against the player AABB.

Slice 4c validation:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-server -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier debug_physics_cube -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-server --features physics-rapier
cargo test --manifest-path native/Cargo.toml -p mclone-physics --features rapier
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features physics-rapier
cargo check --manifest-path native/Cargo.toml
```

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
