# Tactical 278: Quest Procedural-Horizon Multiview

Status: planned 2026-07-28; ready after Tactical 277. Focused renderer and
Quest A/B child of coordinating Tactical
[`280`](280-xr-multiview-render-path-workstream.md).

Topics: `procedural-horizon-clipmap`, `performance`

## Originating Evidence

Tactical 277 made the accepted full-quality procedural horizon stationary-safe
on Quest 3 by culling covered and off-frustum tiles. RD5 stationary composed
rendering now holds `72.01 FPS` with `3.1–3.2ms` average headroom.

Continuous orbit and 8x flight remain close to, but not fully inside, the
72-Hz frame budget: app-work p95 is `13.46–13.94ms`, with `2.2–5.6%`
over-period frames. Meta app GPU time is only `4.75–5.63ms`, while the current
per-eye frame-overlap path independently writes uniforms and encodes the
horizon draw set for each eye. This is the first evidence that a true
full-frame multiview horizon could improve the production workload. Older
exact-only multiview results are context, not an answer.

A 2026-07-28 follow-up found that dynamic actor population materially
confounded the orbit comparison. The earlier composed rows rendered six
actors. On final bounded-lighting runtime code, a 45-second composed orbit
with ten actors measured `12.252ms` average app work, `15.151ms` p95, and
`16.3%` over-period. The same lane with actors skipped measured
`10.723ms`, `12.889ms`, and `0.2%`; exact-only with actors skipped measured
`4.068ms` average and `4.858ms` p95. The horizon was active and target-ready
throughout the composed rows.

The actor-skipped lane is an attribution control, not a product workaround.
The multiview experiment must improve the normal actor+LOD workload, and its
analysis must not attribute actor-population variance to the horizon renderer.

## Objective

Add the procedural terrain and vegetation backdrop to the existing optional
full-frame multiview renderer without weakening the accepted per-eye path or
changing pixels, LOD density, fog, eye scale, foveation, or exact ownership.
Then run an alternating per-eye/multiview/per-eye Quest comparison.

This is an experiment until the device evidence supports making multiview the
default. Unsupported runtimes retain the current per-eye frame-overlap path.

This tactical does not own the live settings control or the render-target
topology needed to switch paths without relaunching. Tactical 280 owns that
cross-boundary work and the combined closeout. The existing startup flag is a
sufficient host for this child's first renderer and Quest comparison.

## Required Renderer Contract

- One committed clipmap/residency update is shared by both eyes.
- Terrain and vegetation pipelines target a two-layer color/depth view.
- Vertex projection selects distinct left/right view-projection facts by
  multiview layer.
- Culling conservatively admits the union of both physical-eye frusta.
- Both layers load the same full-frame reversed-Z depth contract as exact
  multiview terrain.
- Exact-painted discard, fog, biome/material appearance, water, tree ownership,
  and depth writes match the accepted per-eye renderer.
- The shared `mclone-terrain-view` owner exposes multiview encoding; the Quest
  app only chooses the existing OpenXR target/cadence path.
- Mono and per-eye pipelines remain available and pixel-protected.

## Validation

1. Focused shader/layout tests for two distinct eye matrices and array layers.
2. Synthetic stereo capture proving non-empty horizon pixels in both eyes,
   correct disparity, exact coverage, and shared depth.
3. A per-eye reference capture and inspected multiview capture of the same
   accepted low-angle tree/hill composition.
4. Release Quest 3 RD5 alternating stationary
   per-eye/multiview/per-eye samples.
5. Release Quest 3 RD5 alternating settled-orbit samples with normal actors.
6. Repeat the alternating settled-orbit comparison with actors skipped to
   isolate horizon/render-path cost.
7. Compare app-work p50/p95/p99, thread-CPU p50/p95/p99, over-period frames,
   app GPU, submission cadence, actor/drawn-actor counts, horizon
   active/ready/tile/tree counts, and render-stage CPU timings.

## Decision Gate

Promote multiview only if it materially improves moving 72-Hz tails without a
stationary regression, visual/depth discrepancy, or unsupported-runtime
product problem. A gain confined to the actor-skipped control is insufficient;
the normal-actor row remains the product gate. If it is neutral, keep the
implementation opt-in only if its maintenance cost is low and record the
result. If it regresses, retain the per-eye path and use Tactical 277's
remaining CPU attribution plus the actor counters in the performance topic to
choose the next bounded optimization.
