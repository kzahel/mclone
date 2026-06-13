# Tactical 64 - Chunk lifecycle observatory

Build a testable chunk-lifecycle report before adding more runtime complexity or relying on manual playtesting.

Status: host-side snapshot, coordinate-bearing route delta slices, protocol transport, loading-screen diagnostics, and debug HUD v0 are the current slice. Renderer mesh/render-state correlation remains follow-up work.

## Goal

When a chunk is missing, half-loaded, non-ticking, stale, or not rendered, the host should be able to say why in a structured way:

- which chunks are in the current publish, full, and authority windows
- which chunks have generated statuses, block sections, holder access records, and full statuses
- which tickets apply to a chunk
- whether the chunk is published, dirty, queued for unload, pending unload, or blocked by storage/generation work
- for visible but unpublished chunks, the concrete blocker: missing materialized chunk, waiting on a specific `FULL` neighbor, waiting on light, dirty-published, or ready to publish

This is intentionally host-first. The browser minimap consumes the same facts instead of inventing a separate UI interpretation.

## Landed first slice

- `GeneratedWorldHost.getDebugChunkLifecycleSnapshot()` returns per-chunk lifecycle records and aggregate counts.
- `GeneratedChunkTicketSet` exposes covered chunks and per-chunk ticket-source debug facts without leaking private ticket state.
- `pnpm observe:chunks` runs a headless generated-world warp and prints the lifecycle report.
- Focused tests cover both a settled flat-grass view and a deliberately blocked preload where visible chunks are unpublished with explicit blockers.
- Route mode computes coordinate-bearing deltas between snapshots. Counts are always backed by actual chunk coordinates so unit tests, JSON artifacts, ASCII maps, and the future browser minimap can all consume the same facts.

Route delta categories:

- `newlyPublished`: chunks published in the current step that were not published in the previous step
- `unpublished`: chunks that were published in the previous step and are no longer published
- `newlyFull`: chunks that newly reached `FULL`
- `newlyFeatures`: chunks that newly reached `FEATURES`
- `newlyMaterialized`: chunks that newly gained block sections
- `stillBlocked`: visible chunks that are still unpublished, including the blocker kind and blocker coordinate when applicable
- `readyToPublish`: visible chunks that are ready but not yet published

Example:

```bash
pnpm observe:chunks -- --preset flat_grass --radius 0 --wait-for-publish
pnpm observe:chunks -- --preset flat_grass --radius 0 --route "0,0 -> 1,0" --wait-for-publish
pnpm observe:chunks -- --preset default --center-chunk-x 5 --center-chunk-z 115 --radius 0 --json --output /tmp/mclone-chunk-lifecycle.json
```

## HUD v0 plan

The in-game HUD should make the same report visible while playing, without turning normal polling into a debug data stream.

Implementation shape:

- `set_chunk_view` and `poll_world_updates` accept an optional debug request: `{ debug: { chunkLifecycle: true } }`.
- The generated host appends a `chunk_lifecycle` message only when that flag is present.
- The client replica stores the latest lifecycle snapshot alongside the existing performance snapshot.
- The browser runtime requests the snapshot while loading and while `showDebugInfo` is enabled.
- The loading screen and debug overlay draw a compact minimap from the protocol snapshot:
  - cell color is the actionable lifecycle state: published, blocked, ready, materialized, generated, dirty, or unloading
  - the panel shows chunk coordinate bounds, view center/radius, publication counts, loaded count, and blocked count
  - the current view square and center chunk are marked directly on the grid
  - the legend uses color swatches with state labels, not unrelated letter codes
  - the loading screen shows a placeholder lifecycle panel before the first chunk-view snapshot arrives
  - during loading, the normal progress text/bar shifts left when the lifecycle panel is visible

This HUD is deliberately a host lifecycle view. It does not yet answer whether a published chunk has been hydrated into the render worker, meshed, uploaded, or submitted by the renderer.

## Follow-up

1. Add a browser/client/render-world layer to distinguish host publication, client cache hydration, mesh queue, and rendered chunk state.
2. Add HUD hover/selection details once mouse capture and pause/debug-screen interactions are settled.
3. Use the lifecycle report in browser performance fly-by artifacts so manual reports can include a small JSON file instead of screenshots alone.
4. Add a route shorthand for block-space paths if scripted movement needs finer control than chunk-center warps.

## Validation

Required for this slice:

- `pnpm vitest run test/runtime/generated-world-host-observatory.test.ts`
- `pnpm observe:chunks -- --preset flat_grass --radius 0 --wait-for-publish`
- `pnpm observe:chunks -- --preset flat_grass --radius 0 --route "0,0 -> 1,0" --wait-for-publish`
- `pnpm typecheck`
- browser screenshot with `showDebugInfo=1` showing the lifecycle HUD
