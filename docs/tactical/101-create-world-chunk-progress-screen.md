# 101: Create-World Chunk Progress Screen

Status: in progress. Investigation on 2026-06-27 found the Java 1.17.1
chunk-status loading screen, the native scheduler events we can reuse, and the
desktop startup blocking point that currently makes Create World feel like a
beachball. Slice 0 landed on 2026-06-27: local integrated-server status events
now feed compact loading-progress diagnostics. Slice 3's shared `mclone-ui`
render model also landed on 2026-06-27. Slice 1 landed on 2026-06-27: server
diagnostics now carry real per-cell progress snapshots and app-runtime can
convert them into the shared overlay. Slice 4's first landing on 2026-06-27
added the shared local startup pump and wired desktop flat Create World and
boot-to-world startup through it. Platform adoption and conservative
unknown-neighbor gameplay hardening remain open. The first XR prerequisite also
landed on 2026-06-27: shared XR scenes now start with the title/menu panel open
so the loader can render on that surface. XR local Create World replacement then
adopted the shared startup pump on 2026-06-27: the previous XR runtime stays
frozen while the menu panel shows the chunk-status grid, and the scene swaps to
the new local world as soon as the playable threshold is reached. A follow-up on
2026-06-27 moved initial local XR boot-to-world onto the same pump: local XR
startup can now render the menu/loader before terrain exists, then install the
runtime/draw resources at the playable threshold. A center-prioritized scheduler
follow-up landed on 2026-06-28: player view centers now shape runtime chunk
ordering, feature publication starts at the underfoot area, and light-status
batches are released in center-first 3x3 groups so the playable target can turn
white before the full warm region finishes. A debug-visibility follow-up also
landed on 2026-06-28: the desktop tilde debug pane can show the latest
post-join loading-progress grid as a compact panel instead of hiding the grid
once gameplay starts. Slice 4C landed on 2026-06-28: the post-join compact
debug panel now uses an authoritative current-view readiness snapshot from
scheduler holder state and is labeled `VIEW`, while startup keeps using the
event-sourced loading overlay.

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
4. stop draining initial world startup synchronously through a shared runtime
   startup pump that each platform adapter can host.

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
  - constructs `ChunkTaskPriorityQueueSorter` and wraps the worldgen, main
    thread, and light-engine mailboxes at `:155`
  - emits `progressListener.onStatusChange(pos, status)` while loading or
    generating statuses at `:450` and `:512`
  - schedules generation work through the priority sorter-backed worldgen
    mailbox at `:516`
  - clears status on unload at `:410`
  - tracks ticking-generated readiness at `:599`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java`
  - priority sorter messages carry chunk position plus a ticket/queue level
    supplier at `:37` and `:48`
  - level changes resort queued chunk tasks at `:80`
  - submitted tasks are bucketed by current level at `:103`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
  - owns ticket levels and the player-ticket throttler at `:35` and `:46`
  - derives active ticket level from the first sorted ticket at `:78`
  - updates chunk scheduling from propagated ticket levels through
    `runAllUpdates(...)` at `:83`
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
- Startup must advance in frame-budgeted steps or an explicit async startup task
  owned by shared runtime state. Desktop flat is the first validation lane
  because it exposes the OS beachball most clearly, but the readiness policy is
  not desktop-specific.
- Reaching the playable threshold should close the blocking loading flow and
  enter gameplay. Reaching the warm-region threshold should only affect progress
  UI/debug state; it should not be required before the player can move.
- First implementation may be Minecraft-style, not exact Java-status parity:
  map native `Terrain`, `Surface`, `Features`, `Light`, and target-ready states
  to Java-inspired colors. Keep the Java source references above so a later
  parity slice can expose finer statuses.
- Native startup scheduling should be center-prioritized by the accepted player
  view center. This approximates Java's ticket/queue-level shaping without
  requiring a full `ChunkTaskPriorityQueueSorter` port in the first pass.
- After gameplay starts, the compact debug grid should stop using the startup
  event accumulator as its authority. It should use a live current-view
  readiness snapshot produced by the server/scheduler and mapped through
  `mclone-app-runtime` into the same UI model.

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

- [x] Extend the compact diagnostics into a UI-ready snapshot with percent
  complete and latest status per relative or absolute chunk coordinate.
- [x] Keep the server-owned compact accumulator as the source of truth; add
  richer per-cell data only where the Java-style grid needs it.
- [x] Decide whether `Scheduled` should become visible as a dim/pending state.
  Slice 0 records only `Ready`, matching the conservative first pass.
- [x] Keep the native target status (`Light` with lighting, `Features` without)
  as the warm-region progress target for current native gameplay.
- [x] Preserve the separate playable target from the spawn/feet chunk; do not
  regress to waiting for the whole warm region before entering gameplay.
- [x] Unit-test radius indexing, percent calculation, and any per-cell palette
  mapping added for the overlay.

Slice 1 result:

- `ChunkLoadingProgressSnapshot` carries compact aggregate stats plus relative
  `ChunkLoadingProgressCell` records for status-bearing chunks.
- The playable cell is included even before its first ready status arrives, so
  the shared UI can outline the underfoot chunk without inventing state.
- `ServerRunnerDiagnostics` and the web worker runner expose the snapshot while
  preserving existing `Copy` aggregate runtime stats.
- `mclone-app-runtime` converts server snapshots into `mclone-ui`
  `LoadingProgressOverlay` values with the native-to-Java-inspired palette.

### Slice 2 - Server/Event Routing

- [x] Stop discarding `StatusChanged` in `IntegratedServer::route_scheduler_events`.
- [x] Route progress updates to local single-view runtime diagnostics/state
  without sending them as normal chunk snapshots.
- [x] Preserve dedicated-server compatibility. For remote clients, either expose
  optional protocol progress events later or show only local connection/loading
  status until real chunk snapshots arrive.
- [x] Keep status progress separate from `ServerUpdate::ChunkSnapshot` so visual
  loading progress cannot accidentally publish not-ready chunks.

### Slice 3 - Shared UI Rendering

- [x] Add a `LoadingProgressOverlay` or equivalent to `mclone-ui`.
- [x] Render the Java-style centered percent text and 2 px cell grid using
  `GuiDrawList` rectangles.
- [x] Keep dimensions stable across desktop, web, Android, and XR menu surfaces.
- [x] Add draw-list tests that verify expected cell count, placement, and palette
  colors without relying on platform GPU output.

Slice 3 result:

- `native/crates/mclone-ui/src/lib.rs` now defines `LoadingProgressOverlay`,
  `LoadingProgressCell`, and `LoadingProgressCellStatus` as a platform-neutral
  render input for the Java-style grid.
- The palette maps native coarse statuses to the Java-inspired colors recorded
  above: none, terrain/noise, surface, features, light, and target-ready.
- `render_loading_progress_overlay` draws the centered percentage, status grid,
  and playable-cell outline using `GuiDrawList` commands.
- This slice intentionally does not invent per-cell data from aggregate runtime
  diagnostics. The next producer slice should feed real status cells into the
  shared overlay.

### Slice 4 - Shared Non-Blocking Startup Pump

- [x] Replace blocking Create World startup drains with shared app-runtime
  startup state that advances work over frames.
- [ ] Keep the previous world/session alive until the replacement has either
  succeeded or intentionally crossed the teardown point. Preserve the current
  failure behavior where practical.
- [x] Apply the same progress model to initial boot-to-world, not only menu
  Create World.
- [x] Close the New World screen / arm mouse lock when the playable threshold is
  reached, even if broader warm-region progress is still incomplete.
- [x] Keep warm-region loading active after gameplay starts. The grid can either
  fade into a small status overlay or remain available through debug UI.
- [ ] Treat missing neighboring chunks conservatively for collision and
  interaction until they arrive, so early entry never allows movement through
  unknown solid terrain.

Slice 4 result:

- `LocalSingleViewStartupPump` in
  `native/crates/mclone-app-runtime/src/local_single_view.rs` owns the shared
  per-step local startup contract. Each step drains server updates, refreshes
  progress diagnostics, incrementally syncs render sections, and reports when
  the playable chunk plus first cached mesh are ready.
- `WindowSceneStartupPump` in
  `native/apps/mclone-native-client/src/scene_runtime.rs` adapts that shared
  pump to desktop assets without changing the underlying readiness policy.
- Desktop flat Create World and initial boot-to-world now create the pump after
  the first status/loading frame, render the shared Java-style progress overlay
  while startup advances, and complete the session as soon as the playable
  threshold is reached. The previous `poll_until_idle` full-startup path remains
  for existing synchronous/headless helper paths.
- The first desktop landing intentionally tears down the old world at confirmed
  local-start time instead of preserving it behind the overlay. Revisit this if
  replacement failure UX becomes a priority.
- Warm-region loading continues through the normal runtime streaming path after
  gameplay starts. The next correctness slice should harden collision and
  interaction behavior around not-yet-loaded neighboring chunks before relying
  heavily on very early entry at larger render distances.

### Slice 4A - Center-Prioritized Startup Scheduling

- [x] Carry accepted player view centers from player chunk tracking into the
  shared scheduler's distance manager.
- [x] Sort runtime chunk targets, feature job target lists, and dependency
  metadata from the nearest accepted view center outward, with z-major order as
  the fallback for non-player/manual tickets.
- [x] Publish feature-ready chunks in that center-first order so the grid no
  longer fills left-to-right/top-to-bottom for player-startup work.
- [x] Release light-status work in center-first 3x3 batches. Per-chunk light
  batches were rejected because they changed existing scheduler light oracle
  output; 3x3 preserves current lighting correctness while letting the
  underfoot area become target-ready before the whole warm region.
- [ ] If large render-distance startup still spends too long before the first
  green cell, split or stream the worldgen feature job itself. This should be a
  measured follow-up because the current feature worker still computes a full
  requested batch before any feature publication can start.

Slice 4A result:

- `ChunkDistanceManager` stores aggregate player ticket priority centers
  alongside the aggregate player-ticket positions.
- `PlayerChunkTracking` reports priority-center changes so integrated local and
  dedicated player views can update scheduler ordering without regenerating
  duplicate chunks for unchanged views.
- `ChunkScheduler` now uses center-first ordering for runtime generation and
  publishes `Features` / `Light` readiness for the underfoot 3x3 before the
  rest of the warm region.
- `pnpm native:movement:smoke` passed after this change, including the prior
  `client_visible_chunks=46 expected 49` smoke lane.

### Slice 4B - Post-Join Progress Visibility

- [x] Keep the full-screen loading grid for startup only.
- [x] Reuse the existing tilde debug-pane toggle after join to render a compact
  loading-progress grid panel beside the normal debug pane.
- [x] Expose the detailed progress overlay through shared native app-runtime
  accessors without adding the cell grid to the hot-path `Copy` diagnostics
  structs.
- [x] Mirror the compact panel in headless debug screenshots for rendered-output
  validation.

### Slice 4C - Authoritative In-World View Readiness

- [x] Keep `ChunkLoadingProgress` as the startup/event producer for the
  full-screen loading flow. It answers "which status events has startup heard?"
  and should not be treated as authoritative after gameplay starts.
- [x] Add a server-owned current-view readiness snapshot for the local accepted
  view. It should answer "for the current desired local view, what is each
  chunk's actual highest ready status right now?"
- [x] Compute the snapshot from scheduler holder/snapshot state, not from the
  loading-progress event map. The target-ready test should use the same runtime
  target as startup (`Light` with lighting, `Features` without).
- [x] Preserve ownership boundaries: `mclone-server` produces the authoritative
  diagnostic, `mclone-app-runtime` maps it into `LoadingProgressOverlay`, and
  app/UI crates render it without peeking into holders, tickets, or scheduler
  internals.
- [x] Use the startup event overlay while `LocalSingleViewStartupPump` is active.
  After the runtime is active, use the current-view readiness overlay for the
  tilde/debug panel.
- [x] Rename the compact post-join label from `LOAD` to `VIEW` or `STREAM` so
  it does not imply world-startup loading.
- [x] Backfill already-ready chunks through the authoritative snapshot instead
  of special-casing `SnapshotReady` or existing `client_visible_snapshot` event
  paths in the event accumulator.
- [x] Unit-test scheduler/current-view counts against already-ready chunks,
  moved views, and target-status changes with lighting enabled/disabled.
- [x] Unit-test app-runtime mapping and compact-panel rendering, then capture a
  headless debug screenshot to verify the in-world compact panel label and
  placement.

Implementation notes:

- Existing event-progress undercounts after join because it only observes
  `ChunkSchedulerEvent::StatusChanged { step: Ready, ... }`. Chunks that are
  already ready and later re-enter the accepted view can be visible to the
  client without producing a fresh ready-status event.
- The current accepted local view is already tracked by the local
  `SetChunkView` path; the new diagnostic should use that same accepted
  center/radius rather than the initial spawn center.
- The denominator should be the current accepted local view's tracking square.
  The numerator should be chunks in that square whose authoritative status is at
  or past the runtime target.
- Remote dedicated play should keep the simple connection/status behavior until
  protocol-level remote progress or diagnostics are designed.

Slice 4C result:

- `ChunkHolder::highest_ready_status()` and
  `ChunkScheduler::view_readiness_snapshot()` expose a full current accepted-view
  square from authoritative holder status slots, including chunks with no ready
  status yet.
- `IntegratedServer::view_readiness_snapshot()` and
  `ServerRunnerDiagnostics::view_readiness_snapshot` carry the server diagnostic
  through native and web-worker runner paths.
- `mclone-app-runtime` now keeps startup `loading_progress` overlays separate
  from post-join `view_readiness` overlays. Desktop/headless debug rendering uses
  the post-join source and labels the compact panel `VIEW`.
- Tests cover moved accepted views, already-ready chunks, and lit/unlit runtime
  target status selection. The rendered debug screenshot at
  `/tmp/mclone-view-readiness-debug.png` showed `VIEW 100%` beside the debug
  pane.

### Slice 4D - Java-Style Initial Spawn Surface

- [x] Keep early entry: gameplay still starts when the playable spawn chunk is
  ready, not when the whole warm region is ready.
- [x] Use Java's player-spawn-friendly biome selection to choose the local-world
  startup center before the startup pump requests chunks.
- [x] Keep generic server `SetChunkView` semantics literal. Low-level tests,
  debug scene captures, and remote/dedicated clients still get the chunk view
  they explicitly requested.
- [x] Move surface placement closer to `PlayerRespawnLogic`: require the biome
  surface top material to be Java `valid_spawn` (`grass_block` / `podzol`),
  derive motion/world/ocean-floor heightmap floors from loaded block state, and
  scan down from the motion-blocking surface to the biome top material instead
  of accepting cave floors below rejected surfaces.
- [x] Gate spawn scanning to chunks that already have a client-visible snapshot,
  so unloaded spiral chunks do not create a repeated full-height scan while the
  startup pump is still generating.

Java references for future parity:

- Initial spawn biome search and spawn chunk spiral:
  `reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java:427`.
- `BiomeSource.findBiomeHorizontal(...)` seeded quart-biome search:
  `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/BiomeSource.java:70`.
- Player-spawn-friendly biome flags:
  `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/biome/VanillaBiomes.java:171`,
  `:353`, `:888`, and `:900`.
- Valid spawn block tag:
  `reference/minecraft-1.17.1/src/net/minecraft/data/tags/BlockTagsProvider.java:405`.
- Heightmap/top-material respawn column logic:
  `reference/minecraft-1.17.1/src/net/minecraft/server/level/PlayerRespawnLogic.java:15`.

Slice 4D result:

- `mclone-worldgen` exposes player-spawn-friendly biome classification and a
  Java-shaped seeded search for the initial spawn chunk.
- `mclone-server::initial_spawn_center_for_seed` is the narrow shared API used
  by startup code; the server itself does not rewrite arbitrary first
  `SetChunkView` commands.
- `mclone-server` spawn placement now uses Java-shaped chunk spiral order,
  biome top material, valid-spawn top block filtering, and heightmap-derived
  surface rejection.
- `mclone-app-runtime` adds `LocalSingleViewSceneOptions::with_initial_spawn_center`.
  Desktop Create World startup, flat Android local startup, desktop XR local
  startup, and Android XR local startup opt into it; generic desktop scene/debug
  runtime remains centered on the requested chunk.

### Slice 5 - Platform Adoption

- [x] Validate desktop flat first because it exposes the beachball most clearly
  and is the fastest local feedback lane.
- [ ] Wire native web/WASM local-world startup through the same progress model.
- [ ] Wire flat Android scene startup/replacement overlays through the shared UI
  model while keeping activity ownership in app crates.
- [x] Wire XR scene replacement and initial local boot overlays through the
  shared UI model while keeping OpenXR ownership in app crates.
- [ ] Leave remote dedicated joins with a simple connection/loading overlay until
  protocol-level remote progress is explicitly designed.

Slice 5 partial result:

- Shared XR scene initialization now uses the title/menu UI instead of closed
  in-game UI, so both desktop OpenXR and Android XR have a flat menu panel on
  their first rendered frame.
- XR local Create World replacement now queues `LocalSingleViewStartupPump`,
  freezes old-runtime streaming while startup is active, renders the shared
  `LoadingProgressOverlay` on the XR menu panel, and swaps runtime/draw resources
  once the playable threshold is ready.
- Android XR session smoke readiness now waits for `local_startup_active ==
  false` so the replacement smoke does not report ready while the async local
  startup overlay is still advancing.
- Initial local XR boot-to-world now creates an empty terrain draw resource and
  starts `LocalSingleViewStartupPump` immediately, so the first submitted XR
  frames can show the menu panel plus progress overlay before active terrain
  exists.
- Android XR now separates first-frame logging from `MCLONE_ANDROID_XR_READY`;
  replacement and perf smokes wait until `local_startup_active == false`.
- Remote dedicated XR startup still uses the synchronous runtime path. Keep it
  there until protocol-level remote progress is designed.

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
- Slice 4C validation on 2026-06-28:
  `cargo test --manifest-path native/Cargo.toml -p mclone-server`,
  `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`,
  `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime loading_progress_snapshot_maps_to_shared_overlay`,
  `cargo test --manifest-path native/Cargo.toml -p mclone-ui loading_progress_panel_draws_compact_grid_without_fullscreen_scrim`,
  `pnpm native:web:build`, `pnpm native:movement:smoke`, and headless screenshot
  capture/inspection at `/tmp/mclone-view-readiness-debug.png`.
- Slice 4D validation on 2026-06-28:
  `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen`,
  `cargo test --manifest-path native/Cargo.toml -p mclone-server`,
  `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime`,
  `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`,
  `pnpm native:movement:smoke`, `pnpm native:web:build`, and headless screenshot
  capture/inspection at `/tmp/mclone-seed-789-spawn-parity.png`.
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
- Should the older synchronous desktop boot-to-world path also opt into the Java
  spawn center, or should it remain a literal scene/debug bootstrap until it is
  replaced by the non-blocking startup pump?
- How much remote dedicated progress belongs in protocol, and how much should
  stay as local connection/status UI?
