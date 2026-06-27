# 101: Create-World Chunk Progress Screen

Status: in progress. Investigation on 2026-06-27 found the Java 1.17.1
chunk-status loading screen, the native scheduler events we can reuse, and the
desktop startup blocking point that currently makes Create World feel like a
beachball. Slice 0 landed on 2026-06-27: local integrated-server status events
now feed compact loading-progress diagnostics, but UI drawing and non-blocking
startup are not implemented yet.

## Purpose

Pressing Create World should keep the app responsive and show useful progress
while the initial local world is being prepared. Minecraft Java already has a
good visual model here: a small color-coded chunk-status grid updates while the
integrated server prepares the spawn area. Native should not copy Java's full
blocking startup policy, though. The preferred mclone behavior is to enter the
world as soon as the chunk under the player's feet is ready, then keep warming
the broader spawn/view-distance region in the background.

Native should implement the same experience behind shared runtime/UI contracts
instead of adding a desktop-only workaround:

1. expose chunk generation/loading progress from the server scheduler,
2. retain a shared UI-readable loading-progress model,
3. draw a Java-style chunk grid on every relevant client lane,
4. stop draining initial world startup synchronously on the desktop event loop.

## Java Shape

Use the Java client/server source first if this screen is revisited or made more
vanilla-accurate:

- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/LevelLoadingScreen.java`
  - hardcoded `ChunkStatus -> RGB` color table at `:20`
  - progress text from `StoringChunkProgressListener.getProgress()` at `:63`
  - normal render call `renderChunks(..., cellSize = 2, gap = 0)` at `:79`
  - grid drawing loop at `:83`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/progress/StoringChunkProgressListener.java`
  - stores latest status by chunk position at `:34`
  - display radius is `spawnRadius + ChunkStatus.maxDistance()` at `:17`
  - status lookup is relative to spawn position at `:70`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/progress/LoggerChunkProgressListener.java`
  - percentage counts `ChunkStatus.FULL` events against `(radius * 2 + 1)^2`
    at `:19` and `:31`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/progress/ProcessorChunkProgressListener.java`
  - marshals progress callbacks through a mailbox at `:18`
- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java`
  - creates `StoringChunkProgressListener` for integrated-server startup at
    `:1885`
  - installs `LevelLoadingScreen` at `:1905`
  - keeps the client ticking/rendering with `runTick(false)` while waiting for
    server readiness at `:1909`
- `reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java`
  - creates the progress listener with spawn radius `11` at `:344`
  - updates spawn position and waits for start-region readiness at `:499`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
  - emits `progressListener.onStatusChange(pos, status)` while loading or
    generating statuses at `:450` and `:512`
  - clears status on unload at `:410`
  - tracks ticking-generated readiness at `:599`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java`
  - canonical Java status order and generation tasks at `:39`
  - `getStatusList`, `getStatusAroundFullChunk`, and `maxDistance` at `:232`

Java's hardcoded colors:

| Status | Hex |
|---|---:|
| no stored status | `#000000` |
| `EMPTY` | `#545454` |
| `STRUCTURE_STARTS` | `#999999` |
| `STRUCTURE_REFERENCES` | `#5F6191` |
| `BIOMES` | `#80B252` |
| `NOISE` | `#D1D1D1` |
| `SURFACE` | `#726809` |
| `CARVERS` | `#6D665C` |
| `LIQUID_CARVERS` | `#303572` |
| `FEATURES` | `#21C600` |
| `LIGHT` | `#CCCCCC` |
| `SPAWN` | `#F26060` |
| `HEIGHTMAPS` | `#EEEEEE` |
| `FULL` | `#FFFFFF` |

Java 1.17.1 displays a 45x45 grid for the normal startup radius because
`StoringChunkProgressListener(11)` adds `ChunkStatus.maxDistance()` to the
stored/displayed radius. The percentage listener uses a 23x23 `FULL` target.
Server readiness is related but not identical; `MinecraftServer.prepareLevels`
waits for 441 ticking chunks.

## Native Starting State

- Shared native statuses are currently coarse:
  `Terrain`, `Surface`, `Features`, `Light`, `Full` in
  `native/crates/mclone-core/src/chunk.rs:52`.
- The native scheduler already emits status events:
  `ChunkSchedulerEvent::StatusChanged { pos, status, step }` in
  `native/crates/mclone-server/src/scheduler.rs:289`.
- Status scheduling/ready events are produced during runtime chunk enqueue and
  publication:
  `native/crates/mclone-server/src/scheduler.rs:1560`,
  `:1740`, and `:1820`.
- The integrated server currently discards those status events after routing
  snapshot/unload/block/fluid events:
  `native/crates/mclone-server/src/integrated.rs:603`.
- Desktop Create World queues a start and presents at least one loading/status
  frame through the session coordinator:
  `native/apps/mclone-native-client/src/app.rs:528`, `:754`, and `:1785`.
- The freeze point is after that visible frame: `start_world_from_scene` builds
  the runtime, calls `poll_window_runtime_until_idle`, uploads all initial
  sections, and only then returns to the event loop:
  `native/apps/mclone-native-client/src/app.rs:853`.
- `poll_window_runtime_until_idle` delegates to app-runtime `poll_until_idle`:
  `native/apps/mclone-native-client/src/scene_runtime.rs:392`.
- `poll_until_idle` loops until runner diagnostics are idle or a 120 second
  timeout fires:
  `native/crates/mclone-app-runtime/src/local_single_view.rs:242` and `:911`.
- Native runtime target status is `Light` when lighting is enabled and
  `Features` when lighting is disabled:
  `native/crates/mclone-server/src/scheduler.rs:1934`.

## Target Shape

- `mclone-server` remains the producer of authoritative chunk status progress.
- `mclone-app-runtime` owns the platform-neutral loading-progress snapshot and
  startup polling contract.
- `mclone-ui` owns the platform-neutral chunk-grid render model and Java-style
  palette constants.
- App crates own platform glue only: desktop `winit` scheduling, browser async
  worker startup, Android activity/surface timing, and XR scene/session
  replacement.
- Startup has two thresholds:
  - **playable threshold:** the spawn/feet chunk reaches the current native
    gameplay target, player position/surface placement is resolved, collision
    queries for that chunk are valid, and the first renderable mesh for that
    immediate area is ready enough to avoid a blank frame;
  - **warm-region threshold:** the broader spawn/view-distance region continues
    loading/generating after gameplay starts.
- Desktop startup must advance in frame-budgeted steps or an explicit async
  startup task so `winit` can keep presenting frames and processing OS events.
- Reaching the playable threshold should close the blocking loading flow and
  enter gameplay. Reaching the warm-region threshold should only affect progress
  UI/debug state; it should not be required before the player can move.
- First implementation may be Minecraft-style, not exact Java-status parity:
  map native `Terrain`, `Surface`, `Features`, `Light`, and target-ready states
  to Java-inspired colors. Keep the Java source references above so a later
  parity slice can expose finer statuses.

## Proposed Native Palette

Initial native mapping:

| Native status | Suggested color | Java source inspiration |
|---|---:|---|
| none | `#000000` | no stored Java status |
| `Terrain` | `#D1D1D1` | Java `NOISE` |
| `Surface` | `#726809` | Java `SURFACE` |
| `Features` | `#21C600` | Java `FEATURES` |
| `Light` | `#CCCCCC` | Java `LIGHT` |
| target complete / playable | `#FFFFFF` | Java `FULL` |

If native later splits the status path, add direct mappings for Java
`EMPTY`, `STRUCTURE_STARTS`, `STRUCTURE_REFERENCES`, `BIOMES`, `CARVERS`,
`LIQUID_CARVERS`, `SPAWN`, and `HEIGHTMAPS` instead of changing the UI surface.

## Implementation Slices

### Slice 0 - First Bounded Work Chunk

- [x] Add a shared loading-progress accumulator fed by existing scheduler
  `StatusChanged` events in the local integrated server path.
- [x] Expose that progress through `mclone-app-runtime` diagnostics/state without
  changing startup behavior yet.
- [x] Add tests for status accumulation, target-status completion counts, and
  underfoot/playable chunk readiness derivation.
- [x] Keep UI drawing and non-blocking desktop startup out of this first chunk.

Slice 0 result:

- `native/crates/mclone-server/src/loading_progress.rs` owns
  `ChunkLoadingProgress` and compact `ChunkLoadingProgressStats`.
- `IntegratedServer` records the local chunk view, treats `Ready` status events
  as progress, clears unloaded chunks, and derives the current native target
  from lighting mode (`Light` with lighting, `Features` without).
- `ServerRunnerDiagnostics`, the web worker runner, and
  `mclone-app-runtime` stats expose the compact snapshot for UI/debug consumers.
- No UI drawing or desktop startup-loop behavior changed in this slice.

### Slice 1 - Shared Progress Model

- [ ] Extend the compact diagnostics into a UI-ready snapshot with percent
  complete and latest status per relative or absolute chunk coordinate.
- [ ] Keep the server-owned compact accumulator as the source of truth; add
  richer per-cell data only where the Java-style grid needs it.
- [ ] Decide whether `Scheduled` should become visible as a dim/pending state.
  Slice 0 records only `Ready`, matching the conservative first pass.
- [ ] Keep the native target status (`Light` with lighting, `Features` without)
  as the warm-region progress target for current native gameplay.
- [ ] Preserve the separate playable target from the spawn/feet chunk; do not
  regress to waiting for the whole warm region before entering gameplay.
- [ ] Unit-test radius indexing, percent calculation, and any per-cell palette
  mapping added for the overlay.

### Slice 2 - Server/Event Routing

- [ ] Stop discarding `StatusChanged` in `IntegratedServer::route_scheduler_events`.
- [ ] Route progress updates to local single-view runtime diagnostics/state
  without sending them as normal chunk snapshots.
- [ ] Preserve dedicated-server compatibility. For remote clients, either expose
  optional protocol progress events later or show only local connection/loading
  status until real chunk snapshots arrive.
- [ ] Keep status progress separate from `ServerUpdate::ChunkSnapshot` so visual
  loading progress cannot accidentally publish not-ready chunks.

### Slice 3 - Shared UI Rendering

- [ ] Add a `LoadingProgressOverlay` or equivalent to `mclone-ui`.
- [ ] Render the Java-style centered percent text and 2 px cell grid using
  `GuiDrawList` rectangles.
- [ ] Keep dimensions stable across desktop, web, Android, and XR menu surfaces.
- [ ] Add draw-list tests that verify expected cell count, placement, and palette
  colors without relying on platform GPU output.

### Slice 4 - Non-Blocking Desktop Startup

- [ ] Replace desktop `poll_window_runtime_until_idle` during Create World with a
  startup state that advances work over frames.
- [ ] Keep the previous world/session alive until the replacement has either
  succeeded or intentionally crossed the teardown point. Preserve the current
  failure behavior where practical.
- [ ] Apply the same progress model to initial boot-to-world, not only menu
  Create World.
- [ ] Close the New World screen / arm mouse lock when the playable threshold is
  reached, even if broader warm-region progress is still incomplete.
- [ ] Keep warm-region loading active after gameplay starts. The grid can either
  fade into a small status overlay or remain available through debug UI.
- [ ] Treat missing neighboring chunks conservatively for collision and
  interaction until they arrive, so early entry never allows movement through
  unknown solid terrain.

### Slice 5 - Platform Adoption

- [ ] Wire desktop flat first because it exposes the beachball most clearly.
- [ ] Wire native web/WASM local-world startup through the same progress model.
- [ ] Wire flat Android and XR scene replacement overlays through the shared UI
  model while keeping activity/OpenXR ownership in app crates.
- [ ] Leave remote dedicated joins with a simple connection/loading overlay until
  protocol-level remote progress is explicitly designed.

### Slice 6 - Finer Java Parity Follow-Up

- [ ] Revisit `ChunkStatus.java` and decide whether native should expose finer
  stage events for structure starts/references, biomes, noise, carvers, spawn,
  and heightmaps.
- [ ] If implemented, keep the existing UI model and only enrich producer status
  values and palette mapping.
- [ ] Do not port disabled Caves & Cliffs Part 1 systems for the 1.17.1 vanilla
  overworld target. This screen can display status names; it should not change
  worldgen scope.

## Validation

- `cargo test --manifest-path native/Cargo.toml`
- Desktop manual/automated Create World smoke: click New World -> Create World
  and verify the app remains responsive while the grid updates.
- Desktop screenshot/capture while startup is in progress, saved under `/tmp`.
- Existing replacement smokes for web, flat Android, desktop XR, and Android XR
  after platform adoption.
- For rendered-output changes, inspect screenshots before moving on, following
  the native validation policy.

## Open Questions

- Should percentage count target-ready chunks only, or weight each native status
  stage? Java counts `FULL` events only, but native may feel better with target
  readiness until finer statuses exist.
- Should `Scheduled` be visible as a darker shade, or should the grid only show
  `Ready` statuses like Java's stored current status?
- Should local-world creation keep the old world visible behind the overlay until
  the new world's playable threshold is reached, or switch to a full loading
  screen immediately after the user confirms Create World?
- After early entry, should warm-region progress remain visible as a compact
  overlay, fade out automatically, or move entirely into debug UI?
- How much remote dedicated progress belongs in protocol, and how much should
  stay as local connection/status UI?
