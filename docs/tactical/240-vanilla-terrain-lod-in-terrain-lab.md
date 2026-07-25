# Vanilla Terrain LOD In Terrain Lab

Status: active.

Topic: `vanilla-terrain-lod`

## Objective

Add a global Mclone/Java-1.17.1 profile switch to Terrain Lab and implement the
first direct CPU vanilla terrain LOD sampler without generating complete
chunks.

The bounded product result must:

- keep exact and LOD panes on one selected profile;
- preserve recognizable vanilla macro density terrain, water, and biomes;
- compile vanilla LOD tiles in a replaceable Web Worker;
- disable unsupported vanilla GPU and Mclone-semantic controls explicitly;
- retain aligned tiles, progressive refinement, caching, cancellation,
  navigation, and exact canonical review; and
- record and test the first-pass inclusions and exclusions in
  [`vanilla-terrain-lod.md`](../topics/vanilla-terrain-lod.md).

## Implementation Slices

1. Record the topic, capability boundary, implementation plan, and commit
   topic.
2. Expose a reusable direct vanilla density-column query and add a bounded
   cached preview sampler in `mclone-worldgen`.
3. Pin point, region, negative-coordinate, cold/warm, and full-generator
   comparison tests.
4. Make exact Terrain Lab compilation profile-aware while preserving separate
   vanilla and Mclone dependency caches.
5. Carry profile identity through viewport requests, tile/cache identity,
   reports, focus/projection, and supported draw semantics.
6. Add a dedicated vanilla CPU LOD Worker and transferable packed-grid
   admission with stale-epoch rejection.
7. Add the global URL/UI profile switch and deterministic pane, checkpoint,
   and layer capability normalization.
8. Update point inspection and evidence labels so no Mclone semantic receipt
   is presented as vanilla.
9. Run Rust, TypeScript, Wasm, and browser validation; capture and inspect
   desktop and phone vanilla terrain.
10. Update the topic, tactical, and Terrain Lab product documentation with the
    implementation receipt and remaining gaps.

Commit each coherent slice with `Topic: vanilla-terrain-lod`.

## Acceptance

- `profile=overworld` survives URL round trips and selects only exact vanilla
  plus CPU vanilla LOD.
- `profile=mclone-overworld-v1` retains today's exact/CPU/GPU behavior.
- One workspace cannot display cross-profile exact and LOD products.
- Vanilla CPU tiles are generated outside the browser main thread.
- Direct point heights match the shared vanilla density generator at pinned
  sites without allocating a whole chunk.
- Vanilla Surface and Final canonical chunks match their production
  generators.
- Profile changes invalidate incompatible cache and Worker identity.
- Unsupported controls are absent or visibly unavailable, never silently
  interpreted as zero-valued vanilla facts.
- Desktop and phone captures show coherent, coordinate-locked vanilla exact
  and LOD terrain with no GPU pane.
