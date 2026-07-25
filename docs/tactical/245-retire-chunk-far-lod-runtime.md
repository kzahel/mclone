# Tactical 245: Retire Chunk-Based In-Game Far LOD

Status: proposed 2026-07-25; dependency audit complete, implementation not
started.

Topic: `retire-chunk-far-lod`

Workstream: aggressive removal of the experimental chunk-granular synthetic
Far LOD product from every in-game runtime, while retaining only machinery
with a current non-LOD consumer.

## Originating Decision

The current in-game Far LOD was a useful experiment, but it is not the
multi-kilometer terrain architecture Mclone should continue refining. Remove
it instead of leaving a dormant implementation that future work may mistake
for the intended foundation.

The retired product has this binding identity:

```text
one LodTileKey per ChunkPos and detail level
  -> one 16 x 16-block world footprint
  -> 4/8/16-block samples inside that same footprint
  -> one CPU-built vertex/index payload
  -> one chunk-granular exact/LOD arbitration decision
```

Changing sample spacing reduces vertices inside a tile but does not reduce the
number of world-space tiles needed to cover a distant area. A 100 km radius is
about 6,250 chunks; area-proportional chunk footprints are therefore the wrong
unit even before mesh compilation, upload, draw, and transition costs.

Terrain Lab proves the relevant contrast: a bounded visible lattice and
resident-tile budget can span 65.5 km immediately by increasing world-space
sample spacing instead of enumerating every intervening chunk. The eventual
in-game solution needs that scale behavior. This tactical removes the
conflicting experiment; it does not select or implement the replacement.

## Audit Snapshot

Audit base: commit `0c44abc2` on 2026-07-25.

The direct symbol/path search found 68 non-document code, fixture, script, and
asset-tool files mentioning the old Far LOD feature. Native Rust contains more
than 1,800 `FarTerrainLod`, `FarLod`, `far_lod`, `LodTile`, budget-family, and
related occurrences.

Five feature-specific implementation and probe files alone contain 8,250
lines:

| File | Lines | Audit result |
|---|---:|---|
| `mclone-app-runtime/src/far_lod.rs` | 4,407 | delete |
| `mclone-app-runtime/src/lod_coverage.rs` | 713 | delete |
| `mclone-render/src/far_lod.rs` | 802 | delete |
| `mclone-scene/src/far_lod_settle.rs` | 200 | delete |
| `mclone-native-client/src/lod_settle_probe.rs` | 2,128 | delete |

The feature also cuts through shared files for workers, scene orchestration,
frame composition, configuration, UI, diagnostics, browser Workers, Android
XR receipts, asset preparation, and validation. Removing only the five large
files would leave a misleading dormant product surface.

## Binding Removal Policy

Use one test for retention:

> Does this API or mechanism have a current, useful non-chunk-Far-LOD consumer?

If yes, keep that consumer and remove the LOD specialization. If no, delete
the code. A possible future multiscale terrain system is not by itself a
current consumer.

Consequences:

- do not leave an always-off Far LOD flag;
- do not keep `LodTileKey` as a speculative future identity;
- do not rename chunk-specific code to a generic name and call it reusable;
- do not keep LOD-only diagnostics, budget rows, asset sidecars, or Worker
  message kinds;
- do not replace the feature in the same tactical; and
- keep completed tacticals as historical execution records, clearly
  superseded by this product decision.

## Removal Ledger

### 1. Core identity, producer, and coverage policy — delete

Delete:

- `mclone_core::LodTileKey`;
- `mclone-app-runtime::far_lod` in full;
- `mclone-app-runtime::lod_coverage` in full;
- `FarTerrainLodConfig`, `FarLodDetailMode`, startup prewarm, band/hysteresis,
  guard-ring, double-residency, and retained-patch policy;
- chunk-position desired/prefetch/visible maps and level replacement;
- surface-stage `GeneratedChunk` caches used only to synthesize Far LOD;
- CPU top/drop/skirt mesh construction and neighbor-spacing seams;
- chunk-clipped vegetation summary/proxy synthesis from commit `b1db968c`;
- `LodCoverageCoordinator`, source/quality/lifecycle enums, and
  `BTreeMap<ChunkPos, ...>` exact/LOD selection; and
- per-chunk real/LOD overlap assertions and settle snapshots.

The semantic rule “never leave a hole or co-render exact and approximate
terrain” remains useful design guidance. Its current chunk-keyed coordinator
is not reusable implementation for a hierarchy whose nodes cover different
world-space extents.

### 2. Native and browser compile lanes — strip the LOD branch

Keep the render-section worker pool and browser render-worker mechanics.
Delete:

- `FarTerrainLodCompiler`, build request/result/input, and worker caches;
- `RenderCompileWork::FarLod`;
- pending/queued Far LOD deques and result channels;
- LOD job accounting and completion-release methods;
- `ensure_far_lod_capacity` and the LOD-specific two-worker/eight-pending
  capacity floor;
- the browser `WorkKind::FarLod`, `compileFarLodTile`, packed mesh codec, and
  Far LOD completion/error queues; and
- tests asserting real-section priority over this particular LOD job type.

The valuable result is a simpler render-section compiler. A future procedural
terrain producer may use a general job service only after its payload,
priority, cancellation, and platform topology are known.

### 3. Renderer and frame composition — delete the pass

Delete:

- `mclone-render::far_lod`;
- the fixed 16-by-16-chunk region arenas and per-tile capacity assumptions;
- Far LOD mono and multiview shaders, pipelines, buffers, and draw stats;
- `FarTerrainLodRenderer` ownership in flat and shared scene resources;
- `FarTerrainLodFrameUpdate` parameters throughout frame composition;
- `render_*_with_far_lod*` naming and wrappers, collapsing them back to the
  ordinary full-frame entry points;
- Far LOD timing, vertex/index, region-draw, and upload fields from shared
  render summaries; and
- desktop, web, Android, and Android XR plumbing that only copies or asserts
  those fields.

Do not preserve this renderer merely because it supports per-eye and
multiview. Its data layout, region arena, draw granularity, and update contract
are all products of the rejected chunk tile.

### 4. Scene/runtime lifecycle — remove the vertical feature

Delete from local, remote-dedicated, browser, mono, stereo, and multiview
paths:

- Far LOD cache/coordinator fields;
- clear, prepare, prewarm, stats, and settle service methods;
- per-frame build/upload grants and pending-work contributions;
- asset-epoch and world-switch Far LOD invalidation;
- startup visual-coverage gates and prewarm reports;
- exact/LOD settlement ledgers; and
- host-specific proof requirements for a Far LOD draw.

Keep normal chunk authority, render-section readiness, full-height visibility,
camera-relative rendering, and ordinary device/asset rebuild behavior.

In particular, keep the real-section high-altitude traversal correction in
`mclone-render::chunk::traversal_start_keys`. It fixed an independently real
visibility-graph defect discovered by the LOD probe; it is not LOD machinery.

### 5. Product controls and persisted settings — delete

Delete:

- `--far-lod` and `--far-lod-detail`;
- any range, detail, prewarm, or settle CLI arguments;
- Graphics menu Far LOD enable/detail/range controls;
- `GameFarLodDetailMode`;
- `ToggleFarLod`, `CycleFarLodDetail`, and `SetFarLodRange`;
- client-experience capability, setting, action, and effect variants;
- scene/session option fields and host projections; and
- related stored-preference serialization and tests.

Old optional preference keys may be ignored on read. Do not keep live fields
solely to round-trip an unreleased experimental setting.

### 6. Diagnostics, fixtures, and platform lanes — delete

Delete:

- the native `lod_settle_probe` executable mode and CLI parser;
- `test/fixtures/far-lod/`;
- `native:lod-settle:smoke` and `native:lod-settle:probe`;
- browser Far LOD probes and the known-red C5 assertion;
- Quest/Android XR Far LOD requirements and receipt fields;
- debug panels and frame-pipeline labels specific to Far LOD;
- off/on canaries whose only product under test is the retired feature; and
- current-platform instructions that require those lanes.

Retain generic depth readback, offscreen capture, movement, world-switch,
device-rebuild, and frame-accounting tools when they have another active
consumer. Delete only their Far LOD modes and fields.

### 7. Asset sidecar and Texture Lab residue — remove or rename by actual use

Delete the runtime-only contract:

- `assets/mclone/lod/materials.v1.json`;
- `FarTerrainLodMaterialPalette`;
- `far_lod_materials` fields in prepared/render asset bundles;
- Far LOD color counts in asset coverage;
- the optional `FarLod` first-party inventory requirement; and
- automatic generation/export of the LOD material JSON sidecar.

The `tools/texture-lab/packs/mclone-default/block/far-lod-materials.ts` set is
also an old LOD-specific noise-placeholder family under
`assets/mclone/lod/textures/`. It is not a runtime replacement for ordinary
block textures. Remove that family and its lifecycle exceptions unless a
separate current Texture Lab consumer is identified during implementation.
Do not retain it as a promise about the replacement renderer's material
format.

### 8. Documentation — retire, do not erase history

- Keep Tacticals 121, 162, 166, 172, and 244 as historical records.
- Mark their active/resume directions superseded where an index currently
  sends new work into them.
- Mark `docs/lod-architecture.md` as the retired experiment's architecture,
  retaining its Distant Horizons/Voxy research as background only.
- Replace the living `docs/topics/far-lod.md` status with the removal decision,
  this audit, and a link to the future procedural-terrain concern.
- Update platform, native-web, client-experience, parity, asset, and topic
  documents so they no longer claim the product exists.
- Do not rewrite historical milestone evidence to pretend the experiment never
  happened.

## Machinery To Retain

### Retain unchanged in purpose

- `mclone-worldgen` absolute-coordinate terrain samplers;
- Terrain Lab preview/reference products, viewport planning, progressive
  coverage, and CPU/GPU comparison;
- Mclone forest intent, stable tree records, exact realization, and Terrain
  Lab vegetation products;
- normal chunk generation, publication, persistence, collision, and render
  sections;
- shared mono/per-eye/multiview frame composition; and
- generic offscreen/depth/frame-accounting facilities.

### Retain, but remove LOD specialization

| Machinery | Why it survives | Required cleanup |
|---|---|---|
| `RenderSectionCompileWorker` | active normal-section consumer | remove mixed Far LOD queues, result channels, cache, capacity floors, and priority tests |
| `BudgetController` / `EwmaCostEstimator` | active feature/light/render consumers | remove `LodBuildAdmission` and `LodUpload` families, grants, telemetry, labels, and tests |
| resident upload coordinator | active render-section upload consumer | remove `LodTileKey`/Far LOD payload implementations and LOD commentary |
| resident metadata cache | active render-section cache consumer | keep as section machinery; remove unused `lod_level()` and do not claim it handles multiscale footprints |
| real-section visibility and traversal | active exact renderer behavior | retain the high-altitude traversal fix and ordinary culling diagnostics |

The current `ResidentTileKey` abstraction maps every key to one `ChunkPos`.
That is acceptable for its remaining real-section consumer but is not the
future far-terrain hierarchy. A later system with multi-chunk extents must
define its own spatial identity instead of being forced through this trait.

## Replacement Boundary

This tactical intentionally leaves no in-game distant-terrain product. Normal
render distance remains the complete live terrain presentation until a
separate architecture and implementation are accepted.

The replacement study should begin from
[`../topics/gpu-procedural-terrain.md`](../topics/gpu-procedural-terrain.md)
and Terrain Lab evidence, not from the deleted runtime. Candidate clipmap,
quadtree, or hybrid shapes remain unselected.

Minimum questions for that later tactical:

1. Is work bounded by visible sample/patch budget rather than covered chunk
   count?
2. Can tile world-space extent grow exponentially with distance?
3. Can first coarse coverage appear before refinement without holes?
4. Can untouched Mclone terrain be sampled without materializing complete
   `GeneratedChunk` values?
5. Are summaries footprint-aware at extreme spacing?
6. Can exact chunks replace approximate coverage by spatial extent rather
   than one chunk-keyed LOD atom?
7. Does the same identity and lifecycle work on desktop, web, Android, and XR?
8. Are CPU-built per-tile vertex/index arrays avoided or justified by
   measurement?

## Implementation Slices

### Slice 0: record the retirement boundary

- [x] Audit direct and cross-cutting dependencies.
- [x] Classify delete versus retain by current non-LOD consumers.
- [x] Record the replacement boundary without selecting an implementation.
- [x] Stop indexes and living topics from recommending more work on the old
  system.

Gate: a maintainer can identify the complete removal surface without treating
the current runtime as future architecture.

### Slice 1: remove the product surface and diagnostic contract

- [ ] Remove CLI, UI, settings, capabilities, startup prewarm, and scene
  options.
- [ ] Remove settle-probe modes, fixtures, package scripts, browser assertions,
  and platform proof requirements.
- [ ] Remove Far LOD fields from public diagnostics and host receipts.

Gate: no user or validation surface can enable, configure, or require the
retired feature.

### Slice 2: delete the runtime, worker, and renderer vertical path

- [ ] Delete `LodTileKey`, both app-runtime modules, the renderer module, and
  the scene settle module.
- [ ] Remove native/browser compile request/result branches and packed codecs.
- [ ] Remove local/remote/web cache, coverage, preparation, and render paths.
- [ ] Collapse frame composition APIs and remove Far LOD timing/stats.

Gate: all clients render normal terrain with no dormant Far LOD runtime types,
jobs, buffers, or branches.

### Slice 3: simplify retained machinery and assets

- [ ] Remove LOD-specific budget families and render-admission grants.
- [ ] Remove LOD implementations/comments from resident cache/upload code.
- [ ] Remove the LOD material sidecar, palette preparation, inventory entries,
  and LOD-only Texture Lab placeholder family.
- [ ] Prove normal render-section workers, residency, uploads, asset
  replacement, and budgets retain their existing behavior.

Gate: retained machinery is justified by a current consumer and contains no
placeholder contract for the unknown replacement.

### Slice 4: platform and documentation closeout

- [ ] Run shared native tests and direct browser/WASM checks.
- [ ] Build desktop, web, flat Android, and Android XR boundaries selected by
  the removed cross-platform APIs.
- [ ] Inspect normal-terrain flat and synthetic-stereo captures.
- [ ] Update platform, web, architecture, parity, asset, and topic docs.
- [ ] Run an exhaustive old-symbol/path search and record intentional
  historical references only.

Gate: every first-class client builds without the feature, normal terrain
still renders, and living documentation names no chunk-based Far LOD follow-up.

## Validation

At minimum:

```text
cargo test --manifest-path native/Cargo.toml
cargo check --manifest-path native/Cargo.toml \
  -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:thin-adapters:check
pnpm native:web:build
pnpm native:android:apk
pnpm native:android-xr:apk
```

Also run the smallest current desktop flat, offscreen, synthetic-stereo, and
browser rendered lanes that exercise shared frame composition after the
parameter/pipeline deletion. Save and inspect captures under `/tmp`.

Record:

- normal section worker count, pending capacity, and queue health;
- section upload/resident counts and bytes;
- frame-budget decisions after the two LOD families disappear;
- startup/playable behavior with no prewarm gate;
- asset preparation/provenance after removing the sidecar; and
- zero active old-system symbols outside intentional historical docs.

## Stop Conditions

Stop and correct the cleanup if it:

- deletes or weakens Terrain Lab's bounded multiscale pipeline;
- deletes Mclone terrain samplers, forest intent, tree records, or exact tree
  realization;
- removes the generic budget controller, render-section worker pool, or
  resident upload queue instead of only their LOD specializations;
- regresses real-section high-altitude traversal or ordinary exact terrain;
- leaves an enabled, hidden, or partially configurable Far LOD path;
- preserves chunk-specific code under a misleading generic name; or
- begins the replacement architecture without a separate measured tactical.

## Commit Plan

1. Record the retirement audit and supersede active old-system directions.
2. Remove the product surface, probes, fixtures, and public diagnostics.
3. Delete the runtime/compiler/renderer vertical path.
4. Simplify retained budgets/residency/assets and close platform evidence.

Every implementation commit uses:

```text
Topic: retire-chunk-far-lod
```
