# 020: Native Movement Perf And Render Culling

Status: active.

## Purpose

Add an app-level native perf smoke that moves the spectator/chunk-interest center around a deterministic circular path, records chunk load/unload behavior, simulation tick timing, remesh cost, and render-section culling facts. This is the bridge between scheduler-only throughput tests and the real desktop runtime path.

This slice also starts the renderer split between resident/uploaded sections and sections submitted for the current camera. Loaded chunks should not imply every render section is drawn every frame.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/culling/Frustum.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`

Key Java facts for this slice:

- `LevelRenderer.setupRender(...)` updates the view area around the camera and rebuilds the visible render chunk list when camera position/rotation changes.
- `LevelRenderer.updateRenderChunks(...)` does not submit every loaded chunk. It traverses render chunks, checks `Frustum.isVisible(...)`, and optionally uses smart-cull face visibility between neighboring compiled chunks.
- `Frustum` extracts six planes from the view/projection matrices and tests render chunk AABBs.
- `ChunkRenderDispatcher.RenderChunk` owns compiled GPU buffers separately from the current visible render list.

## Landed In First Slice

- `TexturedSectionDrawResources` now computes a frustum for the current `ChunkCamera` and only draws render-section meshes whose section AABB intersects the camera frustum.
- The native window title reports drawn/loaded section and index counts as `sections drawn/loaded` and `idx drawn/loaded`.
- `mclone-native-client --movement-perf` runs a deterministic circular camera/chunk-interest path against the local integrated server, builds the same textured section meshes as the app, computes frustum-visible section stats, and prints JSON timing/count reports.
- Package scripts:
  - `pnpm native:movement:smoke`
  - `pnpm native:movement:perf`

## Current Limits

- This is frustum AABB culling only. It does not yet port Java's neighbor/occlusion traversal or compiled-chunk face visibility graph.
- Section GPU uploads are still full-set replacements when chunk snapshots change. Dirty-only section diffs remain a later renderer-runtime task.
- The movement perf smoke records timings but does not enforce fixed release-mode budgets yet.
- The smoke is local integrated-server only; remote/dedicated-server movement perf should reuse the same report shape later.

## Gate

Default smoke:

```bash
pnpm native:movement:smoke
```

Release comparison:

```bash
pnpm native:movement:perf
```

Focused small smoke:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --movement-perf --movement-steps 4 --path-radius 2 --chunk-radius 1
```

The smoke fails if:

- the loaded/client-visible chunk count does not match the interest square
- any movement step produces no render sections
- frustum culling removes every render section for a step
- no step culls any loaded render section

Important JSON fields:

- `set_interest_ms`: synchronous chunk-interest command path.
- `poll_ms`: app/runtime polling until native worldgen jobs are idle.
- `remesh_ms`: CPU textured section rebuild from client snapshots.
- `loaded_chunks` / `client_visible_chunks` / `active_ticket_chunks`: scheduler/runtime residency facts.
- `loaded_sections` / `visible_sections`: render-section residency vs frustum-submitted sections.
- `loaded_indices` / `visible_indices`: approximate GPU draw pressure before and after frustum culling.
- `simulation_*_ms`: current no-op simulation phase timings; these become important when real fluid/block/entity ticks land.

## Fluid Tick Follow-Up

The next simulation workload should be scheduled liquid ticks, not broad physics. Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/ServerTickList.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FlowingFluid.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/WaterFluid.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/LavaFluid.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java`

Key Java facts:

- `ServerLevel` owns separate `ServerTickList<Block>` and `ServerTickList<Fluid>`.
- `ServerTickList.tick()` caps work at `65536` scheduled ticks per server tick and only executes positions that are ticking with entities loaded; otherwise it reschedules.
- `ServerLevel.tick(...)` runs `blockTicks.tick()` and `liquidTicks.tick()` before chunk-source and entity phases.
- `FlowingFluid.tick(...)` computes a new fluid state, mutates the block state, reschedules if needed, updates neighbors, then spreads downward/sides.
- Water delay is `5`; lava delay is `10` in ultra-warm dimensions and `30` otherwise, with extra spread-delay rules.

The first native fluid slice should introduce the scheduled-fluid-tick queue and a small water-flow scenario that exercises the existing simulation tick report. It should not start with a broad physics abstraction.
