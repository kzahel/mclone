# Lighting Topic

Status: active living index.

This document is the current map for Java 1.17.1-style lighting work in the
Rust engine.
The broad architecture reference remains [`../lighting.md`](../lighting.md);
the first concrete Rust slice is
[`../tactical/026-lighting-pipeline.md`](../tactical/026-lighting-pipeline.md).
Presentation-side point lights and dynamic shadows are a separate concern in
[`dynamic-point-lights.md`](dynamic-point-lights.md); they must not distort this
stored-light parity contract.
The sustained-travel admission, cancellation, Light-ticket, and memory-bound
campaign is now a focused sibling topic in
[`chunk-lighting-admission-and-backpressure.md`](chunk-lighting-admission-and-backpressure.md);
Tactical
[`279`](../tactical/279-chunk-lighting-admission-and-backpressure.md) owns its
implemented P0 lifecycle and physical Quest acceptance.
This topic records where the subsystem stands now, how the next slices should
compose, and which Java/Rust boundaries to preserve.

Update this file whenever a lighting tactical lands, a validation result changes
the next recommendation, or the implementation shape needs correction.

## Current Baseline

The first storage/render handoff has landed, and generated chunk sky/block light
now routes through native graph-driven light engines behind a Java-shaped
`LevelLightEngine` coordinator. Light is a real chunk fact that can cross the
native server/client/render boundary, and generated runtime chunks now advance
through a first native `ChunkStatus::Light` scheduler step before client
publication.

Landed pieces:

- `mclone_light` has Java-shaped `DataLayer`, `LightLayer`, packed-light
  helpers, and tests for nibble/index/constant parity.
- `mclone_light` is split into Java-shaped modules for data layers, layers,
  packed-light helpers, packed positions, storage maps, and section storage.
- `mclone_light` has Java-compatible packed `BlockPos` / `SectionPos` helpers
  for future graph solver keys.
- `mclone_light` has `DataLayerStorageMap`, `BlockDataLayerStorageMap`,
  `SkyDataLayerStorageMap`, and the non-scheduling storage/lifecycle foundation
  for `LayerLightSectionStorage`.
- `mclone_light` has a reusable `DynamicGraphMinFixedPoint` port with
  Java-style per-level queues, pending computed levels, and brighten/darken
  repair behavior.
- `DynamicGraphMinFixedPoint` now reports per-run queue and processed-node
  metrics and uses mixed integer-key hash maps/sets for the packed long graph
  keys, matching Java fastutil's primitive-map performance intent more closely
  than tree-backed collections.
- `mclone_light` has a first `BlockLightEngine` foundation over
  `LayerLightSectionStorage`, using Java's inverted internal levels, the source
  node shape, six-direction propagation, opacity attenuation, and synthetic
  fixtures for brightening, darkening/repair, opacity boundaries, and active
  section boundaries.
- `mclone_light` has a `BlockLightSectionStorage` wrapper so block-light storage
  is split from generic layer storage along the Java class boundary.
- `mclone_light` has a first `SkyLightSectionStorage` and `SkyLightEngine`
  foundation with source-column/top-section tracking, direct-down transparent
  sky propagation, and side-spread decay through the fixed-point graph.
- `mclone_light` has a first `LevelLightEngine` coordinator over block and sky
  engines, including Java-style `checkBlock`, `onBlockEmissionIncrease`,
  `hasLightWork`, `runUpdates` budget splitting, and `getRawBrightness` shape.
- `mclone_light` exposes first-pass Java-shaped `queueSectionData` and
  `retainData` hooks through block, sky, and level engine boundaries.
- `ChunkSnapshot` carries `light_correct` plus optional sky/block
  `PackedLightSection` bytes.
- Native protocol and filesystem persistence roundtrip light payloads.
- The server marks generated chunks `Features` ready first, then runs a
  scheduler-owned `Light` status and publishes client snapshots only with
  `light_correct=true`.
- Native desktop computes `ChunkStatus::Light` payloads through a
  `LightStatusMailbox` worker so foreground scheduler polling no longer runs
  initial light propagation directly. WASM still uses an inline mailbox backend
  until worker plumbing exists there.
- The light-status worker batches initial light inputs at the completed
  feature-job boundary and runs one shared `LevelLightEngine` solve for the
  target chunks plus their sky-light halo, instead of independently solving one
  temporary 3x3 world per light status.
- The light-status worker now owns a retained initial light world and persistent
  `LevelLightEngine` in `mclone-server/src/light_world.rs`, so subsequent light
  batches reuse retained block facts and light storage instead of rebuilding the
  whole raw light world.
- Cold Player promotion is now keyed and limited to four active positions.
  Compact pending Light demand follows current ticket priority, every request
  carries an exact generation/revision token and temporary Light ticket, and
  cancellation/unload control is observed before later target compute.
- Overlapping targets share immutable raw inputs. The Light mailbox covers
  request, active, and undrained completion ownership with hard limits of 18
  statuses and 64 MiB, while full completed scheduler jobs are pruned behind a
  fixed 64-entry diagnostic ring.
- Loaded `ChunkStatus::Light` snapshots hydrate persisted sky/block light bytes
  through `LevelLightEngine` before scheduler publication.
- Native client and integrated-server startup now expose a diagnostic lighting
  mode: `--disable-lighting` bypasses server-side `ChunkStatus::Light`
  promotion and publishes generated `Features` snapshots directly. Unless
  explicitly overridden, that mode also forces shader fullbright for visual
  comparison.
- Block light is generated by adapting loaded raw chunk blocks into
  `BlockLightWorld` and running `BlockLightEngine`; existing synthetic Java
  block-light fixtures still match exactly.
- Sky light is generated by adapting loaded raw chunk blocks into
  `SkyLightWorld` and running `SkyLightEngine`; existing synthetic Java sky
  fixtures still match exactly.
- Runtime initial lighting now uses a combined server `level_light_bridge.rs`
  so sky/block section payloads are produced through `LevelLightEngine` instead
  of merged by the provisional seed module.
- Current generated-block emission facts cover lava, magma block, and glow
  lichen.
- Current generated-block opacity facts now special-case generated oak, birch,
  and spruce leaves to match `LeavesBlock.getLightBlock(...) == 1`.
- Textured mesh vertices carry Java-packed light.
- Textured meshing samples the face-adjacent block for flat solid faces, matching
  the shape of `ModelBlockRenderer.tesselateWithoutAO`.
- Textured meshing now reads packed sky light across omitted all-air sky
  sections using Java `SkyLightSectionStorage.getLightValue(...)` semantics:
  exact layer first, then the next sky layer above, then full sky above data.
- The chunk shader decodes packed sky/block values through a first Java
  `LightTexture` lightmap port, with a native/headless fullbright toggle.
- Textured meshing has a first Java-shaped
  `ModelBlockRenderer.AmbientOcclusionFace` port: side and corner neighbor
  sampling, Java `calculateShape(...)` flags, non-cubic `SizeInfo` weighting,
  per-vertex brightness, packed-light blending, model `ambientocclusion`
  metadata, and flat fallback for non-AO cases.
- Textured mesh catalog render facts are now split into
  `mclone-mesh/src/render_facts.rs` and follow Java
  `BlockStateBase.Cache` semantics for the current terrain-MVP block set:
  `canOcclude`, `solidRender`, `getLightBlock`, `isViewBlocking`,
  `isCollisionShapeFullBlock`, shade brightness, and light emission. Full-cube
  leaves/glass-style blocks no longer act as opaque culling neighbors.

Known gaps:

- The native `LightStatusMailbox` now has vanilla-shaped upstream admission,
  keyed priority, cancellation, Light-ticket lifetime, unload control, shared
  inputs, and stronger explicit memory bounds. It is still not a complete
  `ThreadedLevelLightEngine` port: the worker has no general live light-update
  task queue, and the retained Rust state is not yet Java's shared
  `ChunkAccess`/`ChunkHolder` graph. The 20-minute RD7 8x Quest lane is
  memory-clean, so solver parity and measured sky-graph throughput again take
  priority over generic queue-capacity work.
- Desktop startup still waits for the initial light-ready view before opening
  the window in the current native path. This avoids a sky-only first frame but
  leaves launch latency high when lighting is enabled.
- There is no full Java-shaped `LayerLightEngine` abstraction yet, and loaded
  light hydration uses a temporary engine rather than long-lived world solver
  state.
- Runtime block light feeds generated block facts into `BlockLightEngine`, but
  it still uses reduced generated-block metadata beyond the current leaf
  opacity special case rather than full vanilla `BlockState.getLightBlock(...)`
  tables.
- Scheduler `LIGHT` strict block-light parity for seed `12345`, chunk `(0,0)`
  is guarded at one known edge nibble delta: section `1`, byte `1784`,
  expected `0x10`, native `0x00`. Current evidence points to missing/inexact
  neighbor decoration parity, likely glow-lichen source placement around the
  southern edge, rather than block-light solver math or render gamma.
- Light sections are attached to chunk snapshots, but native does not yet model
  Java's padded light-section lifecycle as solver-owned storage.
- Provisional opacity is coarse (`material_blocks_motion`) and does not use
  full `BlockState.getLightBlock(...)`, emission tables, or face shape
  occlusion.
- Live block changes do not call `checkBlock`, do not publish light deltas, and
  do not dirty render sections by changed light sections.
- Rendering now uses the stable Java `LightTexture` brightness ramp and
  clear-weather sky-darken curve, but does not yet allocate the exact 16x16 GPU
  lightmap texture or port torch flicker, gamma, night vision, conduit power,
  boss-world darkening, rain, or thunder effects.
- Model AO now has Java's non-cubic shape-weight branch and current-block
  Java-shaped render facts, but native still lacks a full generated vanilla
  block-state facts source and exact voxel-shape tables for future block
  families.
- Liquid light sampling is not ported.

## Current Throughput Handoff

Tactical [`153`](../tactical/153-vanilla-shaped-chunk-pipeline-capacity.md)
closed the broad chunk-pipeline capacity pass on 2026-07-08 and handed the
remaining fresh-startup ceiling back to lighting. The pipeline valves that could
hide light cost have been split or fixed: retained-light replacement rechecks
now only enqueue opacity/emission changes, publication grants are cost-derived,
completed light publication backlog drains under the elapsed grant, and
Tactical [`279`](../tactical/279-chunk-lighting-admission-and-backpressure.md)
now prevents obsolete continuous-travel demand from becoming an unbounded
copied-input queue. Its physical 20-minute RD7 flight sustained matched
Features/Light publication at about `26.46/s`, bounded Light ownership below
10.7 MiB, and converged the complete view after stopping. The
current server-only boundary profile still shows about `7.7ms` light compute per
status, with `run_updates` around `6.1ms` and sky updates around `5.8ms` per
status (`73-74%` of light compute). Batch `5` versus `9` does not materially
change that per-status shape. The next throughput slice should therefore target
sky graph/storage hot paths with parity proofs, not scheduler publication knobs
or parallel lighting by default.

## Latest Visual Probe

On 2026-06-19, after adding graph-drain instrumentation and switching the graph
queues/maps to mixed integer-key hash collections, the retained radius-5 cold
startup benchmark showed the remaining bottleneck is still sky graph work:

| Metric | Instrumented baseline | Mixed hash kept |
|---|---:|---:|
| radius-5 total elapsed | `10,614.161 ms` | `6,733.051 ms` |
| light-status compute | `9,173.467 ms` | `5,236.809 ms` |
| `LevelLightEngine.run_all_updates` | `9,115.460 ms` | `5,185.626 ms` |
| block graph drain | `48.058 ms` | `32.612 ms` |
| sky graph drain | `9,067.052 ms` | `5,152.656 ms` |
| block processed nodes | `52,348` | `52,348` |
| sky processed nodes | `10,002,274` | `10,002,274` |

The capture for the kept version is:

```text
/tmp/mclone-light-graph-drain.png
```

It was inspected and rendered nonblank terrain with `166` cached sections and
`26` drawn sections.

Interpretation: the collection change is a safe first throughput win because
node counts are unchanged and existing solver tests still pass. The next
throughput work should reduce the sky graph's duplicate processed-node count,
not tune block light or scheduler publication.

On 2026-06-19, after changing retained initial light setup to pass real
section-empty flags and seed sky from the highest non-empty section instead of
the build-limit top row, the radius-5 cold startup benchmark improved again:

| Metric | P6.8 mixed hash | P6.9 empty-section setup |
|---|---:|---:|
| radius-5 total elapsed | `6,733.051 ms` | `1,931.110 ms` |
| light-status compute | `5,236.809 ms` | `485.407 ms` |
| `LevelLightEngine.run_all_updates` | `5,185.626 ms` | `415.744 ms` |
| block graph drain | `32.612 ms` | `29.855 ms` |
| sky graph drain | `5,152.656 ms` | `385.856 ms` |
| block processed nodes | `52,348` | `52,348` |
| sky processed nodes | `10,002,274` | `794,466` |
| run-update iterations | `614` | `52` |

The capture for this slice is:

```text
/tmp/mclone-light-sky-empty-sections.png
```

It was inspected and rendered nonblank terrain with `166` cached sections and
`26` drawn sections. Tree canopy shadows are visibly stronger than the prior
all-sections-active setup.

Interpretation: the large startup regression is now mostly removed. The next
lighting parity work should move manual sky-source seeding into a Java-shaped
`SkyLightSectionStorage` source-section queue and port the remaining
skip-through-empty-section sky propagation behavior.

On 2026-06-19, after moving sky source-section ownership into
`SkyLightSectionStorage`, the radius-5 performance result stayed stable while
manual retained-world sky source scanning disappeared:

| Metric | P6.9 empty-section setup | P6.10 source storage |
|---|---:|---:|
| radius-5 total elapsed | `1,931.110 ms` | `1,927.655 ms` |
| light-status compute | `485.407 ms` | `455.894 ms` |
| `LevelLightEngine.run_all_updates` | `415.744 ms` | `402.035 ms` |
| sky source scan | `14.736 ms` | `0.000 ms` |
| sky source enqueue | `3.872 ms` | `0.000 ms` |
| sky graph drain | `385.856 ms` | `371.212 ms` |
| sky processed nodes | `794,466` | `794,466` |

The capture for this slice is:

```text
/tmp/mclone-light-sky-source-storage.png
```

It was inspected and rendered nonblank terrain with `166` cached sections and
`26` drawn sections.

Interpretation: this was primarily a Java-shaped ownership improvement. The
next sky parity gap is `SkyLightEngine.checkNeighborsAfterUpdate(...)`
skip-through behavior across missing vertical light-storage sections.

On 2026-06-19, after matching Java leaf opacity and teaching the mesh packed
sky-light sampler to read across omitted all-air sky sections, the dark foliage
top visual bug was fixed without materially changing startup perf:

| Metric | P6.10 source storage | P6.11 leaf/sky render parity |
|---|---:|---:|
| radius-5 total elapsed | `1,927.655 ms` | `1,945.648 ms` |
| light-status compute | `455.894 ms` | `460.263 ms` |
| `LevelLightEngine.run_all_updates` | `402.035 ms` | `405.848 ms` |
| block graph drain | `30.788 ms` | `30.681 ms` |
| sky graph drain | `371.212 ms` | `375.135 ms` |
| block processed nodes | `52,348` | `52,348` |
| sky processed nodes | `794,466` | `794,963` |

The validated captures for this slice are:

```text
/tmp/mclone-light-leaf-sky-sampler-lit.png
/tmp/mclone-light-leaf-sky-sampler-fullbright.png
```

Both were inspected and rendered nonblank terrain with `166` cached sections
and `26` drawn sections. The lit image no longer has near-black canopy-top
patches; open-sky surfaces still look close to fullbright because block-model
ambient occlusion is not ported yet.

On 2026-06-19, after porting the stable Java `LightTexture` lightmap curve into
`mclone-render`, native full-frame captures for seed `12345`, chunk `(0,0)`,
render distance `2`, `960x540`, server lighting enabled, and shader fullbright
disabled produced:

```text
/tmp/mclone-light-lightmap-day.png
/tmp/mclone-light-lightmap-night.png
```

Both were inspected and rendered nonblank terrain with `166` cached sections
and `26` drawn sections. The daytime image is intentionally close to the prior
fixed daylight capture because Java full sky is nearly white. The night capture
visibly darkens terrain through the Java sky-darken/lightmap path while keeping
the same packed light payloads.

On 2026-06-19, after adding the first Java `AmbientOcclusionFace` mesh-side port
for full cube faces, a native daylight full-frame capture for seed `12345`,
chunk `(0,0)`, render distance `2`, `960x540`, server lighting enabled, and
shader fullbright disabled produced:

```text
/tmp/mclone-light-ao-day.png
```

It was inspected and rendered nonblank terrain with `166` cached sections and
`26` drawn sections. Daylight terrain now has visible per-face/corner shading
from mesh-side AO while still using the Java `LightTexture` shader curve.

On 2026-06-19, after porting Java `calculateShape(...)` and the non-cubic
`SizeInfo` weight branch, a native daylight full-frame capture for seed
`12345`, chunk `(0,0)`, render distance `2`, `960x540`, server lighting
enabled, and shader fullbright disabled produced:

```text
/tmp/mclone-light-ao-shape-day.png
```

It was inspected and rendered nonblank terrain with `166` cached sections and
`26` drawn sections. The scene remains visually stable after switching AO to
the shape-aware path.

On 2026-06-19, after moving AO/culling facts into Java
`BlockStateBase.Cache`-style `mclone-mesh/src/render_facts.rs`, a native
daylight full-frame capture for seed `12345`, chunk `(0,0)`, render distance
`2`, `960x540`, server lighting enabled, and shader fullbright disabled
produced:

```text
/tmp/mclone-render-facts-day.png
```

It was inspected and rendered nonblank terrain with `166` cached sections and
`26` drawn sections. Full-cube leaf models now use Java `noOcclusion()`
behavior for culling, so render face pressure rises, but the scene remains
stable and canopy tops remain visibly lit.

Performance from the same slice:

| Metric | Value |
|---|---:|
| radius-5 lighting-enabled total | `1,947.392 ms` |
| radius-5 light-status compute | `456.666 ms` |
| radius-5 `LevelLightEngine.run_all_updates` | `403.441 ms` |
| release movement-frame over budget | `0 / 240` |
| release movement-frame p95 / p99 / max | `4.128 / 5.889 / 8.250 ms` |
| release timedemo average / max frame | `2.739 / 13.450 ms` |
| release timedemo face count | `405,407` |

On 2026-06-19, after moving initial lighting to the retained worker-owned light
world, a native full-frame capture for seed `12345`, chunk `(0,0)`, render
distance `2`, `960x540`, with server lighting enabled and shader fullbright
disabled produced:

```text
/tmp/mclone-light-retained-world.png
```

The capture was inspected and rendered nonblank terrain with `166` cached
sections and `26` drawn sections. It did not reproduce the blue-sky-only
interactive failure.

Scheduler perf from the same slice:

| Metric | Value |
|---|---:|
| radius-5 lighting-enabled total | `11,292.102 ms` |
| radius-5 light-status compute | `9,341.649 ms` |
| radius-5 `LevelLightEngine.run_all_updates` | `9,301.529 ms` |
| radius-3 step 0 light compute | `4,652.194 ms` |
| radius-3 step 1 incremental light compute | `423.869 ms` |

Interpretation: retained state does not change cold startup materially because
the first view is still one large graph drain. It does make subsequent movement
batches much cheaper by reusing retained block/light state.

On 2026-06-18, after batching initial light work per completed feature job, a
native full-frame capture for seed `12345`, chunk `(0,0)`, render distance `2`,
`960x540`, with server lighting enabled and shader fullbright disabled
produced:

```text
/tmp/mclone-light-batch-shared.png
```

The capture was inspected and rendered nonblank terrain with `166` cached
sections and `26` drawn sections. It did not reproduce the blue-sky-only
interactive failure.

Radius-5 scheduler perf from the same slice:

| Metric | Value |
|---|---:|
| lighting disabled total | `1,107.779 ms` |
| lighting enabled total before batch | `47,782.677 ms` |
| lighting enabled total after batch | `10,745.617 ms` |
| completed light statuses | `169` |
| completed light batches | `1` |
| light-status batch compute | `9,323.850 ms` |
| `LevelLightEngine.run_all_updates` | `9,281.490 ms` |

Interpretation: the duplicate per-target graph drain was the largest immediate
startup regression. The remaining throughput blocker is now the single large
graph propagation drain and the lack of Java-shaped long-lived world light
state.

On 2026-06-18, after moving native `ChunkStatus::Light` computation to the
light-status worker, frozen daytime full-frame captures for seed `12345`, chunk
`(0,0)`, render distance `2`, `960x540`, day time `1000` produced:

```text
/tmp/mclone-light-worker-enabled-day.png
/tmp/mclone-light-worker-disabled-day.png
```

Both captures were inspected and rendered nonblank terrain with `166` cached
sections and `26` drawn sections. The enabled capture uses stored light; the
disabled capture bypasses server-side light status and uses the diagnostic
fullbright fallback.

Movement-frame probe after the same change:

| Metric | Value |
|---|---:|
| over-budget frames | `0` |
| over 2x budget frames | `0` |
| over 4x budget frames | `0` |
| p95 frame time | `7.824 ms` |
| p99 frame time | `9.280 ms` |
| max frame time | `9.280 ms` |
| runtime setup | `28,497.187 ms` |
| initial poll count | `14,279` |
| initial poll time | `10,544.738 ms` |
| initial sections | `400` |

Interpretation: the severe in-frame stutter from synchronous light propagation
is addressed for the probed path. Startup remains slow because the native window
path currently waits for the initial light-ready view before first presentation.

On 2026-06-18, after loaded light hydration landed, a native
headless capture for seed `12345`, chunk `(0,0)`, render distance `2`,
`960x640` produced:

```text
/tmp/mclone-light-loaded-hydration.png
```

The capture was inspected and rendered a nonblank, correctly framed terrain
cube with visible blocks and no obvious missing light payload or
section-boundary artifact. It did not reproduce the blue-sky-only failure seen
in one interactive launch.

The previous `ChunkStatus::Light` scheduler capture was:

```text
/tmp/mclone-light-status-scheduler.png
```

The latest forced-fullbright comparison from the prior storage slice produced:

```text
/tmp/mclone-light-current.png
/tmp/mclone-light-fullbright.png
/tmp/mclone-light-diff.png
```

The current-light and forced-fullbright captures are not identical, but the
difference is small:

| Metric | Value |
|---|---:|
| differing pixels | `9,105 / 614,400` |
| differing pixels percent | `1.48%` |
| current mean brightness | `0.307135` |
| fullbright mean brightness | `0.308349` |

Interpretation: the render handoff works and occluded/cave pixels darken, but
most visible open terrain still reads close to fullbright. That is expected
while stored light is provisional and while the renderer lacks Java model AO.

Validation run from the same check:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -p mclone-mesh
```

Result: pass.

## Java Reference Anchors

Read the relevant Java source before porting each piece:

| Concern | Java source |
|---|---|
| 4-bit storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/DataLayer.java` |
| layer enum | `reference/minecraft-1.17.1/src/net/minecraft/world/level/LightLayer.java` |
| storage map | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DataLayerStorageMap.java` |
| shared storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightSectionStorage.java` |
| block storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightSectionStorage.java` |
| sky storage | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightSectionStorage.java` |
| fixed-point graph | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DynamicGraphMinFixedPoint.java` |
| shared engine | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightEngine.java` |
| block engine | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightEngine.java` |
| sky engine | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightEngine.java` |
| coordinator | `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java` |
| threaded wrapper | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java` |
| light status | `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java` |
| status scheduling | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`, `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java` |
| client packed light | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java` |
| lightmap | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LightTexture.java` |
| solid block lighting/AO | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/ModelBlockRenderer.java` |
| liquid lighting | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/LiquidBlockRenderer.java` |

## Rust Code Map

Current native entry points:

| Concern | Native path |
|---|---|
| light data helpers | `native/crates/mclone-light/src/lib.rs` |
| 4-bit data layer | `native/crates/mclone-light/src/data_layer.rs` |
| layer enum | `native/crates/mclone-light/src/layer.rs` |
| packed-light helpers | `native/crates/mclone-light/src/packed.rs` |
| packed block/section positions | `native/crates/mclone-light/src/pos.rs` |
| storage maps | `native/crates/mclone-light/src/storage_map.rs` |
| section storage foundation | `native/crates/mclone-light/src/section_storage.rs` |
| block storage wrapper | `native/crates/mclone-light/src/block_storage.rs` |
| fixed-point graph | `native/crates/mclone-light/src/dynamic_graph.rs` |
| block light engine foundation | `native/crates/mclone-light/src/block_engine.rs` |
| generated block light facts | `native/crates/mclone-worldgen/src/block.rs` |
| snapshot payload | `native/crates/mclone-core/src/chunk.rs` |
| sky storage wrapper | `native/crates/mclone-light/src/sky_storage.rs` |
| sky light engine foundation | `native/crates/mclone-light/src/sky_engine.rs` |
| level light coordinator | `native/crates/mclone-light/src/level_engine.rs` |
| light section merge/orchestration | `native/crates/mclone-server/src/lighting_seed.rs` |
| graph-backed level light bridge | `native/crates/mclone-server/src/level_light_bridge.rs` |
| pending/hydrated light status inputs | `native/crates/mclone-server/src/light_status.rs` |
| native light status worker | `native/crates/mclone-server/src/light_mailbox.rs` |
| retained initial light world | `native/crates/mclone-server/src/light_world.rs` |
| graph-backed block light bridge tests | `native/crates/mclone-server/src/block_light_bridge.rs` |
| graph-backed sky light bridge tests | `native/crates/mclone-server/src/sky_light_bridge.rs` |
| scheduler publication | `native/crates/mclone-server/src/scheduler.rs` |
| worldgen mailbox feature jobs | `native/crates/mclone-server/src/worldgen_mailbox.rs` |
| lighting diagnostic CLI | `native/apps/mclone-native-client/src/cli.rs` |
| protocol roundtrip | `native/crates/mclone-protocol/src/lib.rs` |
| persistence roundtrip | `native/crates/mclone-server/src/persistence.rs` |
| mesh light sampling | `native/crates/mclone-mesh/src/builder.rs` |
| textured vertex payload | `native/crates/mclone-mesh/src/data.rs` |
| shader consumption | `native/crates/mclone-render/src/shaders/chunk_textured.wgsl` |
| fullbright toggle | `native/crates/mclone-render/src/chunk.rs`, `native/apps/mclone-native-client/src/cli.rs` |

## Module Shape Policy

Lighting should follow the reference Java separation closely. Do not grow
`mclone_light/src/lib.rs` or `mclone-server/src/lighting_seed.rs` into large
catch-all modules.

Preferred native shape as the real solver lands:

| Java class | Preferred native home |
|---|---|
| `DataLayer` | `mclone-light/src/data_layer.rs` |
| `LightLayer`, packed constants/helpers | `mclone-light/src/layer.rs`, `mclone-light/src/packed.rs` |
| section/block position helpers | `mclone-light/src/pos.rs` or shared core helpers when already generic |
| `DataLayerStorageMap` | `mclone-light/src/storage_map.rs` |
| `LayerLightSectionStorage` | `mclone-light/src/section_storage.rs` |
| `BlockLightSectionStorage` | `mclone-light/src/block_storage.rs` |
| `SkyLightSectionStorage` | `mclone-light/src/sky_storage.rs` |
| `DynamicGraphMinFixedPoint` | `mclone-light/src/dynamic_graph.rs` |
| `LayerLightEngine` | `mclone-light/src/layer_engine.rs` |
| `BlockLightEngine` | `mclone-light/src/block_engine.rs` |
| `SkyLightEngine` | `mclone-light/src/sky_engine.rs` |
| `LevelLightEngine` | `mclone-light/src/level_engine.rs` |
| `ThreadedLevelLightEngine`-like scheduling | `mclone-server`, as a scheduler/mailbox wrapper around `mclone_light` |
| render `LightTexture` | `mclone-render`, consuming packed light/lightmap inputs only |
| `ModelBlockRenderer` AO sampling | `mclone-mesh`, as renderer-facing mesh vertex/light data, not solver state |

Rules:

- Port parity-critical data and solver code directly, with the same field and
  method names where practical.
- Keep runtime scheduling in `mclone-server`; keep propagation/data ownership in
  `mclone_light`.
- Keep `mclone-mesh` and `mclone-render` consumers of stored light, not owners of
  light propagation.
- Keep provisional lighting isolated and removable. It should not become the
  permanent home for solver logic.
- Split modules before they become difficult to review. A direct Java class
  counterpart is enough reason to create a small Rust module.

## Recommended Next Slices

### P0: Solver Storage Foundation

Status: completed first pass in
[`037-native-light-solver-storage-foundation.md`](../tactical/037-native-light-solver-storage-foundation.md).

Landed:

- Split `mclone_light` into small Java-shaped modules.
- Move existing `DataLayer`, `LightLayer`, and packed helpers behind those
  modules without changing behavior.
- Port `DataLayerStorageMap`.
- Port the non-scheduling parts of `LayerLightSectionStorage`.
- Add focused tests for queued sections, visible/updating map separation,
  copy-on-write data layers, changed sections, and padded light-section ranges.

### P1: Fixed-Point Graph And Block Light

Status: completed first pass in
[`038-native-light-graph-and-block-engine.md`](../tactical/038-native-light-graph-and-block-engine.md).

Landed:

- Ported `DynamicGraphMinFixedPoint` as a reusable native module.
- Preserved the Java graph shape: level count, one queue per internal level,
  pending computed levels, `firstQueuedLevel`, and brighten/darken repair.
- Added `BlockLightEngine` over `LayerLightSectionStorage`.
- Added synthetic block input through `BlockLightWorld`.
- Added source emission, six-direction propagation, attenuation by
  `max(1, opacity)`, missing-position opacity, and active-section checks.
- Added focused fixtures for source brightening, opaque section-wall blocking,
  source removal repair from an alternate source, and cross-section
  propagation.

Remaining parity gaps in this slice: the block engine does not yet use real
block states, `BlockState.getLightBlock(...)`, emission tables, or
`Shapes.faceShapeOccludes(...)`-style face occlusion.

### P2: Block Light Integration Bridge

Status: completed first pass in
[`039-native-block-light-integration-bridge.md`](../tactical/039-native-block-light-integration-bridge.md).

Landed:

- Added `BlockLightSectionStorage` as the block-specific storage boundary.
- Added a server-side chunk adapter for `BlockLightWorld`.
- Runtime block-light section generation now uses `BlockLightEngine`.
- Existing synthetic Java block-light fixtures, including cross-chunk light,
  still match exactly through the graph path.
- Generated-block emission now includes lava, magma block, and glow lichen.

Remaining parity gaps: full vanilla block-state opacity/emission tables and
face shape occlusion are still pending.

### P3: Sky Light Storage And Engine

Status: completed first pass in
[`040-native-sky-light-engine-bridge.md`](../tactical/040-native-sky-light-engine-bridge.md).

Landed:

- Added `SkyLightSectionStorage` and `SkyLightEngine`.
- Added a server-side chunk adapter for `SkyLightWorld`.
- Runtime sky-light section generation now uses `SkyLightEngine`.
- Existing synthetic Java sky fixtures, including roofed, overhang, and
  cross-chunk cases, still match exactly through the graph path.
- Removed the old provisional sky flood-fill from `lighting_seed.rs`.

Remaining parity gaps: full Java `LevelLightEngine` scheduling, live section
status changes, face shape occlusion, and full block-state opacity tables are
still pending.

### P4: LevelLightEngine Coordinator

Status: completed first pass in
[`041-native-level-light-engine-bridge.md`](../tactical/041-native-level-light-engine-bridge.md).

Landed:

- Added `mclone-light/src/level_engine.rs`.
- Routed generated chunk initial lighting through one combined
  `LevelLightEngine` bridge.
- Preserved the Java coordinator shape for `checkBlock`,
  `onBlockEmissionIncrease`, `hasLightWork`, `runUpdates`, and
  `getRawBrightness`.
- Kept layer-specific bridges as test-only coverage.

Remaining parity gaps from that slice: `queueSectionData`, `retainData`, full
padded section lifecycle, and live block-change updates were still pending.
The real `ChunkStatus::Light` scheduler boundary landed in P5.

### P5: ChunkStatus::Light Scheduling

Status: completed first pass in
[`042-native-light-status-scheduling.md`](../tactical/042-native-light-status-scheduling.md).

Make `ChunkStatus::Light` real, using `LevelLightEngine` as the propagation
boundary and `ThreadedLevelLightEngine.lightChunk(...)` as the ordering model.

Landed:

- Runtime chunks target `ChunkStatus::Light`.
- Feature publication marks `Features` ready, then queues pending light status
  work owned by `mclone-server`.
- `worldgen_mailbox.rs` no longer precomputes light sections.
- Client snapshots are sent only at `ChunkStatus::Light` with
  `light_correct=true`.
- Scheduler tests cover `Light` scheduled/ready status slots and light-correct
  client snapshots.

At the end of this slice, light propagation was still synchronous inside
scheduler polling, Java `queueSectionData` / `retainData` were pending, and
loaded light data hydration did not yet use solver-owned storage. The loaded
hydration gap landed in P6; the foreground propagation stall was addressed by
the P6.5 light-status worker.

### P6: Loaded Light Hydration And Retained Data

Status: completed first pass in
[`043-native-loaded-light-hydration.md`](../tactical/043-native-loaded-light-hydration.md).

Teach the native light engine how to accept persisted light section data through
Java-shaped hooks.

Landed:

- Added `queue_section_data` / `retain_data` hooks to native block, sky, and
  level light engines.
- Loaded `ChunkStatus::Light` snapshots hydrate persisted light sections through
  a temporary `LevelLightEngine`.
- The scheduler's stored-snapshot branch now publishes hydrated light payloads.

Remaining parity gaps: the server still does not retain a long-lived world light
engine, and loaded neighbor light data stitching is not modeled yet.

### P6.5: Light Status Worker And Diagnostic Bypass

Status: completed first pass in
[`044-native-light-status-worker-and-disable-flag.md`](../tactical/044-native-light-status-worker-and-disable-flag.md).

Move the expensive initial light-status payload computation off the foreground
scheduler poll path and add a runtime switch for isolating lighting cost.

Landed:

- Added `mclone-server/src/light_mailbox.rs`.
- Native desktop light status work runs on a `mclone-light-status` worker
  thread; WASM keeps an inline backend for now.
- `ChunkScheduler` drains completed light results and publishes them under the
  existing completed-light publication budget.
- `--disable-lighting` makes runtime chunks target `ChunkStatus::Features`
  instead of `ChunkStatus::Light` and publishes client snapshots without light
  sections.
- Native startup/headless screenshots place the spectator above the loaded
  surface column so captures do not start inside or below terrain.

Remaining parity gaps: this is still a first mailbox boundary, not full Java
`ThreadedLevelLightEngine` parity. Startup still blocks on the initial
light-ready view.

Immediate operational follow-up: make desktop launch responsive while initial
chunks/light are still warming, either by showing a real loading/progress screen
or by presenting a partial first view without waiting for the whole
light-ready radius. The first throughput follow-up, replacing per-target
temporary 3x3 light-world recomputation, landed in P6.6.

Measured radius-5 result on 2026-06-18:

- native lighting disabled: `1,106.492 ms`
- native lighting enabled: `47,782.677 ms`
- light-status compute: `46,983.813 ms` across `169` statuses
- `LevelLightEngine.run_all_updates`: `46,753.960 ms`
- Java oracle to `FEATURES`: `4,718.254 ms` for `121` target chunks

Interpretation: feature generation is not the active regression. Initial light
propagation is slow because native drains a fresh isolated graph per light
status instead of using a long-lived `ThreadedLevelLightEngine`-like world
engine.

### P6.6: Shared Initial Light Batch

Status: completed first pass in
[`045-native-shared-initial-light-batch.md`](../tactical/045-native-shared-initial-light-batch.md).

Batch initial `ChunkStatus::Light` inputs for all publishable targets in one
completed feature job.

Landed:

- `PendingLightStatusBatch` merges target chunks and dependency halo chunks into
  one raw light-world input set.
- `level_light_bridge.rs` can collect packed light sections for multiple target
  chunks after one shared `LevelLightEngine.run_all_updates` drain.
- `LightStatusMailbox` accepts batch requests while preserving individual
  completed-light publication for scheduler budgets and holder status changes.
- `ChunkScheduler` stages light inputs during sliced feature publication and
  enqueues the batch only once the feature job has fully published.
- Scheduler perf JSON now reports `completed_light_batches` separately from
  `completed_light_statuses`.

Measured radius-5 result on 2026-06-18:

- native lighting disabled: `1,107.779 ms`
- native lighting enabled after batch: `10,745.617 ms`
- light-status batch compute: `9,323.850 ms` across `169` statuses in `1` batch
- `LevelLightEngine.run_all_updates`: `9,281.490 ms`

Remaining parity gaps: the worker still rebuilds a temporary raw light world per
feature-job batch. It is not yet Java's long-lived threaded world light state,
does not own light tickets, and does not support live `checkBlock` updates.

### P6.7: Long-Lived Threaded Light State

Status: completed first pass in
[`046-native-retained-initial-light-world.md`](../tactical/046-native-retained-initial-light-world.md).

Replace the temporary per-batch raw light world with a server-owned light state
that more closely follows Java `ThreadedLevelLightEngine`.

Landed:

- Added `mclone-server/src/light_world.rs` as the worker-owned retained initial
  light world.
- `LightStatusMailbox` now keeps a persistent `LevelLightEngine` on the light
  worker side.
- Newly retained or changed chunks get section-status/source-enable/emission
  pre-update work; repeated chunks reuse retained state.
- `scheduler.rs` still owns chunk status publication and does not grow light
  engine logic.

Measured result on 2026-06-19:

- radius-5 cold startup remains dominated by one graph drain:
  `9,341.649 ms` light-status compute, `9,301.529 ms` in
  `LevelLightEngine.run_all_updates`
- radius-3 two-step movement shows retained-state reuse: step `0` light compute
  `4,652.194 ms`, step `1` incremental light compute `423.869 ms`

Remaining parity gaps: no Java-shaped task prioritization, cancellation, light
ticket release, retained-state unload policy, loaded-neighbor stitching, or live
`checkBlock` queue yet.

### P6.8: Graph Drain Instrumentation And Optimization

Status: completed first pass in
[`047-native-light-graph-drain-instrumentation.md`](../tactical/047-native-light-graph-drain-instrumentation.md).

Make the remaining `LevelLightEngine.run_all_updates` cost concrete and apply a
first safe graph-drain optimization.

Landed:

- `DynamicGraphRunReport`, `LightLayerRunReport`, and `LevelLightRunReport`
  expose block/sky queue sizes, processed nodes, call counts, and drain time.
- `scheduler_movement_smoke` JSON reports `light_status_graph` and per-layer
  `run_updates` timing.
- Graph pending-level maps and queue-members sets now use mixed integer-key
  hash collections instead of tree collections.

Measured result on 2026-06-19:

- radius-5 `LevelLightEngine.run_all_updates` dropped from `9,115.460 ms` to
  `5,185.626 ms`
- sky graph drain dropped from `9,067.052 ms` to `5,152.656 ms`
- graph work count stayed unchanged: `52,348` block nodes and `10,002,274` sky
  nodes

Out of scope for the first pass: live block-change propagation, light ticket
release policy, and render light-delta packets. Those belong after initial
startup light is affordable.

### P6.9: Sky Graph Duplicate-Work Reduction

Status: completed first pass in
[`048-native-sky-empty-section-light-setup.md`](../tactical/048-native-sky-empty-section-light-setup.md).

The latest metrics showed sky propagation was still the cold-start limiter:
roughly `10M` sky graph nodes were processed for a max sky queue size of
`57,600`. This slice compared native sky source/section activation against Java
and removed the largest redundant queue source.

Landed:

- Retained light setup now computes section emptiness from raw blocks and calls
  `LevelLightEngine.update_section_status(section, is_empty)`.
- Sky sources are enabled once per chunk column.
- Manual sky source seeds now use the top block row of the highest non-empty
  section instead of the build-limit top row.
- The test-only light bridge mirrors the retained setup and focused tests cover
  empty-section classification/source height.

Measured result on 2026-06-19:

- radius-5 `LevelLightEngine.run_all_updates` dropped from `5,185.626 ms` to
  `415.744 ms`
- sky graph drain dropped from `5,152.656 ms` to `385.856 ms`
- sky graph processed nodes dropped from `10,002,274` to `794,466`
- block graph processed nodes stayed unchanged at `52,348`

### P6.10: Java Sky Source-Section Ownership

Status: completed first pass in
[`049-native-sky-source-section-ownership.md`](../tactical/049-native-sky-source-section-ownership.md).

The retained setup still manually scans top non-empty section rows and calls
`check_sky_source`. Java owns this inside `SkyLightSectionStorage` through
source-section add/remove queues, and `SkyLightEngine.checkNeighborsAfterUpdate`
has special skip-through-empty-section behavior. Porting those pieces should
improve parity and make future live section/block updates less ad hoc.

Landed:

- `SkyLightSectionStorage` owns source-section sets and add/remove queues.
- `SkyLightEngine` drains source-section updates into graph source edges before
  propagation and treats source queues as light work.
- Retained light setup and the test-only light bridge no longer manually scan
  sky source rows.
- Radius-5 sky processed nodes stayed at `794,466`, and manual sky source scan
  timing went to `0.000 ms`.

Remaining parity gap: native still uses the current reduced source-row edge
behavior rather than Java's full source-section fill/horizontal-boundary path
for `LIGHT_ONLY` source sections.

### P6.12: Sky Neighbor Skip-Through Propagation

Status: pending solver parity.

Port the remaining Java `SkyLightEngine.checkNeighborsAfterUpdate(...)`
behavior for vertical gaps in light-storage sections.

Initial scope:

- When a sky update reaches local Y `0`, skip downward through missing storage
  sections while `SkyLightSectionStorage.hasSectionsBelow(...)` is true.
- Check the skipped-down block and horizontal side neighbors using Java's source
  node choice.
- Add synthetic fixtures for a vertical empty-section gap and side spread across
  that gap.
- Preserve the current radius-5 graph metrics and screenshot benchmark as
  acceptance checks.

### P7: Live Deltas And Render Dirtying

Status: pending after the remaining initial-light throughput work.

Connect runtime mutations to the light engine.

Scope:

- Block changes call `checkBlock` when opacity, emission, or light-occlusion
  shape changes.
- Light deltas are revisioned and published to clients.
- Changed light sections dirty render sections and neighbor sections as needed.
- Section block deltas and light deltas are batched when practical.

### P8: Rendering Parity

Status: active, first terrain-MVP block render facts pass complete.

Improve visual parity after stored light is correct.

Scope:

- Read Java `BlockModelRenderer`, `ModelBlockRenderer`, and `LightTexture`
  before porting.
- First Java `LightTexture` lightmap behavior is in `mclone-render`; next,
  decide whether exact dynamic 16x16 GPU texture allocation is needed or whether
  the procedural shader curve remains sufficient.
- First Java `ModelBlockRenderer.AmbientOcclusionFace` sampling/blending is in
  `mclone-mesh`, including `calculateShape(...)`, the non-cubic `SizeInfo`
  weighting branch for partial boxes, and current terrain-MVP
  `BlockStateBase.Cache`-style render facts.
- Replace the hand-maintained current-block render-facts bridge with full
  generated vanilla block-state facts when the native registry grows beyond the
  terrain-MVP surface.
- Port liquid light sampling from `LiquidBlockRenderer`.
- Keep lightmap, model-face AO, and mesh data plumbing split into separate
  modules instead of growing `mclone-mesh/src/builder.rs` into a renderer
  catch-all.

## Tactical Index

Primary native lighting docs:

- [`../lighting.md`](../lighting.md)
- [`../tactical/026-lighting-pipeline.md`](../tactical/026-lighting-pipeline.md)
- [`../tactical/031-native-section-block-delta-updates.md`](../tactical/031-native-section-block-delta-updates.md)
- [`../tactical/036-native-sky-and-day-night-cycle.md`](../tactical/036-native-sky-and-day-night-cycle.md)
- [`../tactical/037-native-light-solver-storage-foundation.md`](../tactical/037-native-light-solver-storage-foundation.md)
- [`../tactical/038-native-light-graph-and-block-engine.md`](../tactical/038-native-light-graph-and-block-engine.md)
- [`../tactical/039-native-block-light-integration-bridge.md`](../tactical/039-native-block-light-integration-bridge.md)
- [`../tactical/040-native-sky-light-engine-bridge.md`](../tactical/040-native-sky-light-engine-bridge.md)
- [`../tactical/041-native-level-light-engine-bridge.md`](../tactical/041-native-level-light-engine-bridge.md)
- [`../tactical/042-native-light-status-scheduling.md`](../tactical/042-native-light-status-scheduling.md)
- [`../tactical/043-native-loaded-light-hydration.md`](../tactical/043-native-loaded-light-hydration.md)
- [`../tactical/044-native-light-status-worker-and-disable-flag.md`](../tactical/044-native-light-status-worker-and-disable-flag.md)
- [`../tactical/045-native-shared-initial-light-batch.md`](../tactical/045-native-shared-initial-light-batch.md)
- [`../tactical/046-native-retained-initial-light-world.md`](../tactical/046-native-retained-initial-light-world.md)
- [`../tactical/047-native-light-graph-drain-instrumentation.md`](../tactical/047-native-light-graph-drain-instrumentation.md)
- [`../tactical/048-native-sky-empty-section-light-setup.md`](../tactical/048-native-sky-empty-section-light-setup.md)
- [`../tactical/049-native-sky-source-section-ownership.md`](../tactical/049-native-sky-source-section-ownership.md)
- [`../tactical/050-native-leaf-sky-render-parity.md`](../tactical/050-native-leaf-sky-render-parity.md)
- [`../tactical/051-native-light-texture-render-parity.md`](../tactical/051-native-light-texture-render-parity.md)
- [`../tactical/052-native-model-ao-render-parity.md`](../tactical/052-native-model-ao-render-parity.md)
- [`../tactical/053-native-non-cubic-ao-render-parity.md`](../tactical/053-native-non-cubic-ao-render-parity.md)
- [`../tactical/054-native-block-render-facts-parity.md`](../tactical/054-native-block-render-facts-parity.md)

Reference-only prior art from the retired browser engine now lives only in Git
history. New implementation work belongs in the shared Rust crates and should
read the Java source plus committed oracle fixtures before porting parity logic.

## Validation Lanes

Fast focused gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -p mclone-mesh
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Native movement/render smoke:

```bash
pnpm --silent native:movement:smoke
pnpm --silent native:timedemo:smoke
```

Pixel-affecting lighting work must capture and inspect a native screenshot in
`/tmp` before the slice is considered complete. Keep a forced-fullbright
comparison around when changing render consumption:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- \
  --screenshot /tmp/mclone-light-current.png \
  --width 960 --height 640 --seed 12345 --chunk-x 0 --chunk-z 0 \
  --render-distance 2 --fullbright false

cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- \
  --screenshot /tmp/mclone-light-fullbright.png \
  --width 960 --height 640 --seed 12345 --chunk-x 0 --chunk-z 0 \
  --render-distance 2 --fullbright true
```

## Update Policy

- Keep the current recommendation in this file; keep detailed implementation
  notes in tacticals.
- When a solver/status/rendering slice lands, update Current Baseline, Known
  gaps, Rust Code Map, and Recommended Next Slices.
- If a native module starts collecting multiple Java-class responsibilities,
  split it before extending the subsystem further.
