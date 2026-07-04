# 139: Vanilla Chunk Startup Scheduling

Status: active priority checkpoint before broader render-pipeline and
streaming-budget follow-ups. Slice A1 landed on 2026-07-04: feature-job size
diagnostics and high-radius batch-shape regression coverage.
Slice C1 landed on 2026-07-04: high-radius startup feature scheduling now emits
a center-first `3x3` job before streaming the rest of the view in bounded
background batches.
Slice B1 landed on 2026-07-04: local render distance now gets Java's
`requested + 1` tracking halo for normal values, while startup readiness waits
for the explicit center `3x3` publication gate.
Workstream: native Rust, server scheduling, startup readiness, loading UI;
desktop validation first

## Purpose

Make local basic play follow Minecraft Java's chunk scheduling model closely
enough that render distance changes, startup readiness, and loading progress are
coherent at both small and large view distances.

The immediate bug is visible with high render distance on desktop native:
`--render-distance 20` can take several minutes before entering the world, and
`--render-distance 30` can appear stuck on "Creating world..." at `0%`. The UI
allows the value, but pre-C1 startup put the playable center behind a large
whole-view generation batch. Separately, the local tracking radius still does
not use Java's `requested + 1` view-distance halo, so the outer render boundary
can lack neighbor snapshots and look one ring short.

This tactical should be treated as a correctness checkpoint before continuing
large workstreams such as `120` render-compile backpressure and `128` terrain
render-pipeline coordination. Those workstreams still matter, but they should
measure and optimize on top of the correct chunk publication/startup shape.

## Policy Decision

Follow the Java 1.17.1 model for chunk status dependencies, player-ticket
streaming, and normal chunk publication, with one deliberate startup divergence:

- Do **not** require Java's fixed spawn bootstrap of radius `11` / `441`
  ticking chunks before entering a local world.
- Do require the spawn/under-foot chunk to be publishable by the normal ticking
  rule: the center chunk's `3x3 FULL` gate must be satisfied before gameplay
  starts.
- After that gate, enter the world and stream the wider requested view outward
  in priority order.

This supersedes the looser "under-foot chunk only" language in
`101-create-world-chunk-progress-screen.md`. The desired startup policy is
small-view friendly, but still Java-shaped enough that local ticking,
collision, and first publication do not start from an isolated single chunk.

## Java Baseline

Always read the reference source before changing this area:

- `reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java`
  - `START_CHUNK_RADIUS = 11`
  - `START_TICKING_CHUNK_COUNT = 441`
  - `prepareLevels(...)` adds the `START` region ticket and waits for the fixed
    start region.
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
  - `setViewDistance(...)` clamps `requested + 1` to `3..=33`.
  - `prepareTickingChunk(...)` waits for a radius-1 `ChunkStatus.FULL` future
    before `playerLoadedChunk(...)`.
  - `scheduleChunkGeneration(...)` schedules one chunk/status future and asks
    `getChunkRangeFuture(...)` for that status's dependency futures.
  - `getDependencyStatus(...)` maps dependency radius to the required status.
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java`
  - `FEATURES` has range `8`.
  - `STATUS_BY_RANGE` means the outer feature dependencies are mostly
    `STRUCTURE_STARTS`, not fully carved terrain.
  - `LIGHT`, `SPAWN`, `HEIGHTMAPS`, and `FULL` form the publication tail.
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java`
  - `ensureCanWrite(...)` limits feature writes to the center plus immediate
    neighbors for `FEATURES`.
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/progress/`
  - `StoringChunkProgressListener` stores the latest status per chunk.
  - `LoggerChunkProgressListener` derives the loading percent from `FULL`
    progress.
  - `ProcessorChunkProgressListener` marshals progress events through a mailbox.
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/LevelLoadingScreen.java`
  - renders the color-coded status grid from latest per-cell status.

The key distinction: Java's radius-8 feature dependency is per chunk/status
future and mixed-status. It is not a request to fully generate the entire render
distance plus eight chunks before publishing the player.

## Original Native Problem

The native behavior at the start of this tactical differed in ways that
compounded at high render distance:

- `chunk_tracking_radius_for_render_distance(...)` returns the requested render
  distance for normal values instead of Java's `requested + 1` halo.
- `PlayerChunkTracking` converts the accepted view directly into a full square
  of player-visible chunks.
- `reconcile_ticketed_holders(...)` collects all runtime targets for that view
  and nearby ticket halo before scheduling feature work.
- `enqueue_runtime_chunks(...)` collects all missing feature targets into one
  `to_generate` list and calls `create_feature_job(&to_generate, ...)`.
- The native worldgen mailbox processes that `GenerateFeatures` request as one
  synchronous worker job and only reports completion when the whole batch is
  done.
- Startup `playable_ready` waits for the playable chunk to be target-ready and
  renderable, but that center chunk can be blocked behind the same huge
  whole-view feature/light job as the far edge.
- `ChunkLoadingProgress` records only `Ready` statuses. Scheduled or active
  status transitions are ignored, so the colored progress grid can remain black
  and percent can remain `0%` while real work is happening.

For approximate scale, render distance `30` means `3721` visible chunks. Current
ticket/status expansion can produce roughly `4489` feature/light runtime targets
and a feature dependency surface on the order of `7225` chunks. That scale is
not a valid startup gate.

## Target Shape

### View Distance

- Local render distance should map to a Java-shaped tracking radius:
  `clamp(render_distance + 1, 3, 33)` for normal values.
- Preserve existing debug/offscreen support for radius `0` and `1` where tests
  intentionally use tiny chunk windows.
- The renderer should still draw only the requested render distance. The extra
  tracking ring exists to satisfy publication, neighbor, and mesh-boundary
  needs.
- Dedicated-server defaults may remain capped by the dedicated policy unless a
  separate server view-distance UI/CLI is added.

### Startup Readiness

- Local startup readiness is the center `3x3 FULL` publication/ticking gate, not
  Java's fixed radius-11 start region.
- If native `ChunkStatus::Full` is not currently a faithful publication gate,
  make the gate explicit instead of treating `Light` alone as enough.
- The startup pump should enter the world once:
  - the center chunk has passed the `3x3 FULL` gate,
  - the client has the center chunk snapshot and light data required for the
    active lighting mode,
  - the renderer has accepted enough mesh data to avoid a blank first frame.
- The wider render distance should continue warming after entry.

### Scheduler

- Replace whole-view feature jobs with bounded, center-first,
  status-shaped scheduling.
- A high render distance must not require a single `GenerateFeatures` completion
  for the whole view before the center can publish.
- Preserve Java's dependency status semantics:
  - feature writes are center plus immediate neighbors,
  - outer radius-8 feature dependencies are lower-status metadata where Java
    requires only metadata,
  - lower-status dependency holders remain explicit rather than hidden inside a
    larger terrain window.
- Keep scheduling host-neutral. The shared server/runtime owns policy; desktop,
  web, Android, and XR adapters should only host the pump and presentation.

### Progress UI

- Track latest announced status per chunk separately from target-ready status.
- Use latest status for the colored Java-style grid.
- Use a clearly defined percent:
  - preferably Java-shaped `FULL` progress for the startup target, or
  - a deliberately conservative target-ready percent, as long as the grid still
    shows phase movement.
- The progress grid should not stay all black during a long feature or light
  phase. If the scheduler has announced work, the UI should show that phase.
- Keep the compact post-join debug view based on live current-view readiness,
  not stale startup events.

## Non-Goals

- Do not port Java's fixed radius-11 spawn wait as the local startup gate.
- Do not make render distance `30` fully generated or fully compiled before
  entering the world.
- Do not solve far LOD, greedy meshing, Quest frame pacing, or render compile
  backpressure in this tactical except where validation needs to avoid
  conflating those systems with startup scheduling.
- Do not weaken normal chunk publication silently. If a temporary early-preview
  state is introduced, label it explicitly and keep it out of the default local
  gameplay path.

## Implementation Slices

### Slice A: Lock The Contract With Tests And Diagnostics

- [x] Add tests for local render-distance to tracking-radius conversion,
      including Java-shaped `requested + 1`, max clamp, and debug tiny radii.
- [ ] Add scheduler tests proving the center `3x3 FULL` gate is the local
      startup threshold.
- [x] Add a high-radius regression test and diagnostic assertion that documents
      raw whole-view feature-job expansion shape.
- [x] Enable a high-radius scheduling test that proves the first feature request
      is bounded and center-prioritized, not the whole view.
- [x] Add diagnostics for feature job target count, feature-center count,
      dependency count, latest job id, and first target so RD20/RD30 failures
      are obvious.
- [ ] Add diagnostics tying feature-job sizes back to requested render distance,
      accepted tracking radius, and startup-critical versus background work.

Slice A1 result:

- `ChunkSchedulerMetrics` now reports max/latest feature job target counts,
  feature-center counts, dependency counts, latest feature job id, and first
  target.
- `scheduler_movement_smoke` prints those fields in its JSON metrics block.
- A passing scheduler regression records raw high-radius feature expansion: a
  radius-33 target set would produce one `67x67` target job, `69x69`
  feature-center set, and `85x85` dependency set if submitted as a single job.
- The Java-shaped render-distance halo contract started as ignored
  future-coverage and was enabled by Slice B1.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-server` passed on
  2026-07-04 (`357` passed, `1` ignored).
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime` passed
  on 2026-07-04 (`78` passed, `1` ignored).

### Slice B: Fix View-Distance Tracking And Startup Gate

- [x] Change local tracking radius to Java's `requested + 1` halo for normal
      render distances.
- [x] Keep render filtering at the requested render distance.
- [x] Replace startup `playable_ready` semantics with the center `3x3 FULL`
      gate plus client snapshot/renderable first-frame requirements.
- [x] Make the native publication gate explicit because current native `Full`
      status is not yet a faithful Java publication/ticking status.
- [x] Update tests that currently assume `Light` or `Features` alone makes the
      center playable.

Slice B1 result:

- `chunk_tracking_radius_for_render_distance(...)` now preserves tiny debug
  radii `0` and `1`, then maps normal render distances through Java's
  `clamp(requested + 1, 3, 33)` shape.
- `SingleViewRuntime::render_section_within_render_distance(...)` remains keyed
  to the requested render distance, and an app-runtime test now proves the extra
  tracking halo is not rendered.
- `ChunkLoadingProgressStats` now carries explicit playable-gate diagnostics:
  radius, total gate chunks, and ready gate chunks.
- `playable_chunk_ready` now means the center `3x3` gate has reached the
  runtime target status (`Light` with lighting, `Features` without lighting),
  not merely that the center chunk is ready.
- The startup pump still also requires the client center snapshot and at least
  one accepted render section before entering play.
- The scheduler's live view-readiness snapshot uses the same center `3x3` gate
  as loading progress.

Completed validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-server` passed on
  2026-07-04 (`359` passed, `0` ignored).
- `cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime` passed
  on 2026-07-04 (`80` passed, `0` ignored).

Planned validation:

- desktop local startup at small render distance

### Slice C: Replace Whole-View Feature Batches

- [x] Refactor `enqueue_runtime_chunks(...)` so missing feature work is emitted
      in bounded center-first jobs.
- [ ] Preserve Java-shaped per-status dependencies instead of expanding every
      target into full terrain dependencies.
- [x] Publish completed center chunks as soon as their own dependencies finish;
      do not wait for far-edge jobs.
- [x] Keep light-status scheduling center-first and able to complete the startup
      `3x3` before the full requested view.
- [x] Ensure view-distance changes and player movement stream outward instead
      of synchronously generating the whole view.

Slice C1 result:

- `enqueue_runtime_chunks(...)` now preserves center-first priority order and
  submits missing feature work through a bounded helper instead of one
  whole-view feature job.
- The first startup-sized candidate set larger than the background limit is
  capped to `9` target chunks, matching the local center `3x3 FULL` startup
  gate.
- Follow-up feature jobs are limited to `128` target chunks and are enqueued
  only after the previous feature job has fully published, so high view
  distances stream outward instead of blocking startup behind one large
  synchronous `GenerateFeatures` request.
- The high-radius scheduler contract test is enabled and proves that a
  radius-33 candidate set starts with `9` target chunks, `5x5` feature centers,
  and `21x21` dependencies, with the center chunk first.

Planned validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- movement/perf smoke at render distances `8`, `12`, and `20`
- diagnostic check that RD30 no longer creates one startup feature job for the
  entire view

Completed validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-server` passed on
  2026-07-04 (`359` passed, `0` ignored).

### Slice D: Make Progress Java-Shaped Enough To Be Honest

- [ ] Store latest announced status per chunk in `ChunkLoadingProgress`, not
      only ready statuses.
- [ ] Keep target-ready/full-ready counts separate from latest-status colors.
- [ ] Map native statuses to the existing Java-inspired palette.
- [ ] Decide and document whether native percent follows Java `FULL` progress or
      conservative target-ready progress.
- [ ] Ensure the loading grid shows scheduled/active phase colors while the
      current job is running.

Validation:

- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo test --manifest-path native/Cargo.toml -p mclone-ui`
- visual check of Create World progress at RD20/RD30 before first entry

### Slice E: Desktop High-Radius Validation

- [ ] Run local desktop/offscreen startup at render distances `2`, `8`, `12`,
      `20`, and `30`.
- [ ] Confirm RD30 enters after the center `3x3 FULL` gate instead of waiting
      for the whole view.
- [ ] Confirm the loading grid changes colors before the world is entered.
- [ ] Confirm chunks stream outward after entry.
- [ ] Capture and inspect a drawable screenshot under `/tmp` for at least one
      high-radius lane.
- [ ] Record startup timings and first feature/light job sizes in this doc or a
      durable performance record.

Validation commands should use the current defaults from
`docs/platforms.md#validation-policy`.

## Open Questions

- Should native expose Java's fixed radius-11 spawn bootstrap as an optional
  parity mode for oracle/debug runs, while keeping small-view local startup as
  the default?
- Should the loading percent match Java's `FULL` status announcement count
  exactly, or remain target-ready based while the grid shows in-progress
  statuses?
- How much of Java's `ChunkTaskPriorityQueueSorter` should be ported directly
  versus represented by a smaller native priority/budget scheduler?
- Should dedicated-server view distance get its own CLI/config in the same
  workstream, or stay separate from local startup correctness?

## Cross-Links

- `docs/worldgen-deterministic-order.md` owns the Java status/dependency model.
- `docs/tactical/101-create-world-chunk-progress-screen.md` owns the current
  progress screen history; this tactical supersedes its startup readiness
  target.
- `docs/tactical/120-vanilla-render-compile-backpressure.md` should resume after
  startup scheduling no longer blocks the center behind whole-view generation.
- `docs/tactical/128-terrain-render-pipeline-coordination.md` should measure
  streaming on top of the center-first publication model from this tactical.
