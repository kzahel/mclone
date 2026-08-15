# Tactical 305: Fine Homestead LOD Overlay

Status: planned 2026-08-15

Topic: `procedural-horizon-clipmap`

Topic: `starter-farmstead-settlement`

## Instruction Synthesis

Give the realized starter homestead a real presentation overlay in the current
procedural-horizon LOD system. The overlay must carry the homestead's terrain
grading, paths, pond, ordinary-tree clearing, authored tree, and buildings so
the natural procedural source does not visibly restore the pre-homestead site
just outside exact chunk coverage.

Keep the existing annular geometry clipmap. A rectangular homestead is sparse
sample data consumed while intersecting clipmap tiles refill, not a second
rectangular terrain mesh competing with the rings. Admit the overlay only in
the fine spacing-`1`, spacing-`2`, and spacing-`4` levels and omit it at
spacing `8` and beyond, so it is a nearby landmark rather than a promise
visible from kilometres away.

Treat buildings and the focal oak as stable bounded records, similarly to the
current tree-record ownership model, but allow the first implementation to
reuse their full-detail authored meshes. There are few enough objects in one
or two homesteads that a speculative model simplifier is not justified.

Define the semantic payload as a small generic terrain-overlay contract so the
renderer does not depend on homestead plan types. Do not turn this tactical
into the future generic distant-edit system. In particular, do not build
per-block edit listeners, dirty hierarchies, persistent LOD databases, or a
homestead-specific invalidation architecture that would be discarded when the
generic overlay system arrives.

## Starting Point And Scale

The current homestead is bounded enough for a deliberately simple first
overlay:

- the compact designed core has half-extent `32`, or `65x65` blocks and at
  most `5x5` intersected chunks;
- the unused full-tier core has half-extent `48`, or `97x97` blocks and at
  most `7x7` intersected chunks;
- the actual terrain and decoration reservation has half-extent `80`, or
  `161x161` blocks and exactly `11x11` intersected chunks for the accepted
  seed-`0` placement; and
- the `513x513` scenic survey is read-only site-selection input. It is not
  overlay extent and must not be uploaded or retained.

The accepted compact instance is anchored at `(-200, -1384)`. Its reservation
is X `-280..=-120`, Z `-1464..=-1304`, covering chunk X `-18..=-8` and chunk Z
`-92..=-82`. These coordinates are a fixture, not renderer policy. Runtime
bounds must come from the persisted realized plan.

At one cell per block, the reservation contains only `25,921` cells. An
illustrative four-byte packed cell is about `101 KiB`; a padded `256x256`
four-byte texture is `256 KiB`, and even an eight-byte texture is `512 KiB`.
The implementation must select and measure its actual packing, but this scale
does not justify a world overlay database or spatial hierarchy.

The current horizon has ten four-by-four toroidal levels. Each tile contains
`64x64` cells, and the first three levels sample at spacings `1`, `2`, and `4`.
Those are already the only levels that produce detailed tree records. Their
combined geometric ownership is a filled fine region despite each individual
coarser level being annular. That makes them the natural bounded admission
domain for the homestead.

The server already persists an `IntroHomesteadPlanRecord` with the realized
source identity, reservation, pieces and bounds, grade regions, path, pond,
decoration reservation, focal oak, markers, content fingerprints, and
checksum. Ordinary chunk materialization applies that plan after base terrain
features, clears conflicting decoration, grades and surfaces the site, carves
the pond, and clips authored structure pieces. The LOD source currently sees
only terrain profile plus seed, so it reconstructs the untouched natural site.

Tactical
[`304`](304-lod-frontier-and-near-field-voxel-convergence.md) separately owns
the exact/procedural frontier and the spacing-one surface voxel shell. This
tactical changes the semantic sample selected under the current or future
surface topology. It must not create a second frontier, retain the old
horizontal collar, or fork the block-shell work.

## Objective

When a local composed-terrain session approaches an admitted homestead:

- the procedural ground agrees with the realized grade, path, pond, and
  decoration-clearing intent throughout the fine LOD domain;
- natural proxy trees do not regrow through the settlement;
- authored buildings and the focal oak remain visible between exact chunk
  coverage and the spacing-`4` horizon boundary;
- exact-painted chunks remain the sole owner wherever they are drawable;
- the overlay appears and disappears as one coherent site, rather than as a
  rectangular patch fighting annular ring geometry; and
- CPU work, GPU storage, upload/refill work, and draw cost remain measured and
  bounded for the current one-homestead case and a two-homestead canary.

The result represents the persisted plan's initial realized state. It does not
promise distant fidelity for later player edits.

## Binding Decisions

### Keep the rings; overlay their sample source

The spatial composition is:

```text
absolute world X/Z
        |
        v
natural procedural sample ----+
                               +--> resolved fine-LOD sample --> resident tile
admitted sparse overlay -------+

realized building records -------------------------------> bounded mesh draw
exact-painted coverage -------------> masks both procedural presentations
```

The clipmap planner continues to choose exactly the same toroidal tiles and
annular ownership. For every requested or refilled tile, intersect its
world-space bounds with the small admitted overlay list. A non-intersecting
tile follows the existing path. An intersecting fine tile evaluates the
natural sample and then applies overlay cell semantics at the same absolute
sample coordinate.

Bake the result into the tile's staged terrain sample or derived geometry.
Do not scan homestead cells in every terrain fragment, tessellate a permanent
`161x161` rectangle, cut rectangular holes in rings, or create a camera-local
patch grid with separate seam rules. Normal halos, material derivation, water
coverage, vegetation extraction, and Tactical 304's future surface shell must
all consume the same resolved sample function.

Power-of-two levels share absolute lattice points. Spacing-`2` and spacing-`4`
therefore directly sample the block-resolution patch at their coordinates;
they do not require resampled rectangular meshes. Pin agreement at shared
points and tile halos so the overlay adds no new fine/coarse seam.

### Add only a narrow generic semantic contract

Create a small WGPU-independent shared owner, preferably a focused
`mclone-terrain-overlay` crate, for the minimum semantic records needed now:

- stable overlay id, revision, and producing-source identity;
- base terrain identity and active world/source binding;
- inclusive block bounds and block-resolution layout;
- independently inheritable or replacing ground height, visible surface or
  surface-recipe reference, water surface/depth, and vegetation policy;
- stable bounded landmark records with world bounds and mesh source identity;
- maximum admitted sample spacing; and
- a deterministic content fingerprint.

The first codec may be dense within each small rectangular patch. It must
allow multiple patches in a deterministic ordered snapshot, but the runtime
may linearly test the current one or two patches. Do not add an R-tree,
quadtree, region database, paging format, network protocol, edit journal, or
arbitrary overlay manager in anticipation of unknown scale.

`mclone-server` owns compilation from the realized homestead plan through
ordinary homestead producers. `mclone-scene` publishes the immutable snapshot
for the active local source. `mclone-terrain-view` owns fine-level admission,
tile intersection, GPU representation, committed-generation checks, and
rendering. No renderer or app crate may import `IntroHomesteadPlanRecord` or
reimplement grade/path/pond rules.

### Compile one presentation artifact from the realized plan

The overlay is derived, presentation-only data. It is not another world state
authority and must never write chunks, advance structure status, persist
entities, or trigger site selection.

Extract or reuse pure homestead evaluators so exact chunk materialization and
overlay compilation consume the same persisted plan facts. Do not transcribe
foundation rectangles, path curves, pond depths, reservation clearing, or
template transforms into scene or WGSL conditionals. Compilation must not load
or pin all `121` reservation chunks.

For ground cells, preserve independent intent:

- untouched land may inherit the natural procedural height and material;
- graded, path, shore, pond, crop, or other planned cells may replace the
  relevant resolved surface fields;
- decoration suppression may apply even where ground fields inherit; and
- authored vertical content is emitted as landmark mesh records rather than
  collapsed into one height value.

The exact materializer uses canonical terrain while the horizon uses its
procedural preview. Where their pre-homestead surfaces differ, derive a
presentation-only transition collar inside the reservation if needed so a
height replacement converges to inherited procedural terrain without a hard
edge. The collar may affect only derived LOD pixels. It must not alter the
persisted plan, exact chunks, or accepted site grading.

### Admit only a complete site in the fine domain

An overlay is eligible only when its complete declared bounds fit inside the
geometric union of clipmap levels whose effective sample spacing is at most
`4`. Exact-painted holes do not make the domain incomplete; exact rendering
supersedes the procedural result there.

When the site first becomes eligible, stage refills only for resident fine
tiles intersecting its bounds and stage its landmark meshes. Continue
presenting the prior natural generation until the complete site generation is
ready, then commit terrain samples, vegetation policy, and landmark records
together. Use the inverse transition when leaving the fine domain. Reject
late work after movement, source switch, plan replacement, mode change, or
device reset through the existing source/generation rules.

This whole-site gate prevents half a farm from appearing merely because an
annular tile edge crossed it. A bounded distance pop is accepted in the first
slice. Fades, hysteresis beyond what is needed to prevent boundary chatter,
and coarser kilometre-scale silhouettes require separate visual evidence and
are deferred.

Do not increase the exact render distance or keep reservation chunks loaded to
make the gate easier.

### Carry terrain, water, and vegetation ownership together

The resolved cell schema must distinguish solid display surface from optional
water surface and depth. The path, pond, shore, garden surface, and foundation
grades must use the same material or recipe vocabulary consumed by the current
horizon and Tactical 304. A flat blue color rectangle is not a pond overlay.

Natural vegetation records whose declared ownership or bounds conflict with
the plan's decoration reservation must be suppressed before the terrain and
vegetation generation commits. Pin this filter to the materializer's actual
reservation rule; do not guess from camera distance or from a building's
visual bounds. Natural vegetation outside the declared suppression region
continues through the existing deterministic source.

The focal oak and any other deliberately authored vertical planting are
landmark records from the plan. They are not regenerated as natural
procedural-tree guesses.

### Use stable full-detail landmark meshes first

Compile buildings, fences, crops, the focal oak, and other non-heightfield
authored blocks needed for the initial silhouette into stable bounded landmark
records. Their identity includes overlay id/revision, piece or feature id,
template/content fingerprint, transform, and world bounds.

The first renderer may cache and draw the same full-detail block mesh used by
the authored templates. This is intentionally simpler and more faithful than
inventing cottage, barn, coop, garden, and oak LOD models for a handful of
records. Meshes are compiled once per overlay revision, reused across frames
and views, frustum culled by bounds, and drawn only while the overlay is
admitted. Record triangle counts, bytes, draw calls, and GPU time before
deciding whether a later simplified representation is necessary.

This is "tree-like" in record lifetime, bounds, stable identity, culling, and
exact/proxy ownership. It does not require using the procedural tree geometry
or reducing a building to a billboard. Residents and moving animals remain in
their ordinary authoritative entity/tracking path and are not part of this
overlay.

### Reuse exact-painted coverage for handoff

Terrain overlay samples are still procedural terrain for ownership purposes.
The immutable exact-painted coverage snapshot masks them through the same
complete-footprint rule as the natural horizon. Tactical 304's explicit
frontier connector consumes the resolved procedural surface; this tactical
must not add its own collar or connector.

The landmark pass must use the same source-qualified exact coverage snapshot.
At minimum, fragment-level world X/Z masking removes proxy building or oak
pixels inside exact-painted chunks, while the ordinary exact chunk mesh shows
the real blocks. Once exact coverage leaves, the cached landmark pixels may
return. Do not depend on depth testing to arbitrate coincident full-detail
meshes.

If fragment masking exposes an unacceptable chunk-edge cut for a template,
partition the cached mesh deterministically at chunk boundaries. Do not hold
an entire multi-chunk building proxy until every touched exact chunk is ready,
and do not invent per-building chunk residency or edit invalidation before
pixels demonstrate that the bounded partition is necessary.

### Accept baseline-only edit fidelity

This tactical deliberately represents the initial realized plan, not the
subsequent edit history:

- an edit in a drawable exact chunk wins because exact coverage masks the
  terrain and landmark proxy there;
- when that chunk leaves exact range, the original plan-derived ground or
  landmark may reappear; and
- the overlay never restamps or overwrites the authoritative edit.

That pop is the same class of known limitation currently accepted when an
edited exact chunk falls back to untouched natural LOD. It must be stated in
diagnostics and acceptance notes rather than hidden behind a building-specific
dirty bit system.

Do not subscribe to block edits, rebuild mesh fragments, maintain per-overlay
dirty regions, roll summaries up a hierarchy, or persist a replacement
overlay in this tactical. A later generic distant-edit overlay should replace
or extend this baseline artifact instead of reverse-engineering a special
homestead invalidator.

The only required rebuild events are whole-artifact events: initial plan
availability, an explicit persisted plan/content revision change, active
world/source change, composed-mode lifecycle, and renderer/device recovery.
Ordinary player edits are not overlay revisions.

### Preserve platform and frame topology neutrality

The semantic overlay, mesh compilation, and scene admission are shared. Apps
may only pass their existing local source and frame targets. Exact-only mode
must allocate, compile, upload, and draw no overlay resources.

The landmark renderer must work in mono, stereo/per-eye, and full-frame XR
multiview with each view's own projection data. Reuse an existing
multiview-capable textured world renderer where practical. A new landmark
pipeline must provide both ordinary and multiview shader modules and must not
reintroduce `@builtin(view_index)` into single-view WebGPU modules.

## Non-Goals

- A generic player-edit or Distant Horizons-style overlay system.
- Per-block invalidation, dirty propagation, edit summary roll-up, or an
  overlay persistence database.
- World-scale structure discovery, indexing, streaming, or arbitrary counts.
- Remote overlay protocol publication or remote-server trust policy.
- A building simplifier, billboard system, multi-distance impostors, or
  kilometre-scale settlement silhouettes.
- Distant residents, moving animals, particles, sounds, lights, inventories,
  crop growth, doors, or other live structure behavior.
- Full voxels, caves, overhangs, or arbitrary stacked edit representation in
  the terrain-cell payload.
- Pinning or generating exact chunks outside ordinary interest.
- Changing homestead selection, grading, materialization, persistence, or the
  accepted seed-`0` composition.
- Changing the natural procedural base, promoting Composed to the default, or
  reviving the retired `LodTileKey` chunk-based Far LOD.
- Solving Tactical 304's frontier, block shell, or material/light convergence
  work inside a private overlay renderer.

## Slice 0: Contract And Baseline Fixtures

Status: planned.

- Record the accepted plan bounds, `11x11` chunk reservation, changed surface
  footprint, cleared-decoration footprint, piece/landmark bounds, and content
  checksum without hard-coding them into runtime policy.
- Capture exact, composed, coverage, and elevated homestead views showing
  natural ground/tree restoration and missing landmarks outside exact
  coverage.
- Define the minimal neutral overlay records, deterministic ordering, source
  binding, fingerprint, byte limits, and validation errors.
- Add pure bounds, negative-coordinate, cell-index, overlap-priority, and
  maximum-spacing tests, including a two-patch canary.
- Record current tile refill, allocation, draw, CPU-frame, and GPU-frame
  baselines at the accepted homestead.

Gate: the fixture proves the defect and the shared contract represents only
the bounded facts required by this tactical.

## Slice 1: Plan-Derived Ground Overlay

Status: planned.

- Refactor the existing pure homestead terrain decisions as needed so the
  exact materializer and overlay compiler share grade, path, pond, garden,
  material, water, and reservation semantics.
- Compile the accepted plan into a block-resolution derived patch without
  loading, generating, or pinning all reservation chunks.
- Compare representative overlay cells with final authoritative initial chunk
  surfaces across foundations, feathers, path, pond, shore, garden, unchanged
  ground, and reservation boundaries.
- Add the bounded presentation collar only if the procedural-base mismatch
  produces a measured edge discontinuity.
- Publish the immutable local-source snapshot through `mclone-scene` without
  exposing homestead plan types to `mclone-terrain-view`.

Gate: compilation is deterministic across native and Wasm, matches the
planned initial surface facts, performs no world writes, and changes nothing
outside declared overlay bounds.

## Slice 2: Fine-Clipmap Admission And Vegetation

Status: planned.

- Intersect patches with requested tile bounds and apply cells during terrain
  refill only at effective sample spacing `1`, `2`, or `4`.
- Feed the same resolved sampler to normal halos, water/material derivation,
  vegetation selection, and Tactical 304's surface path when present.
- Stage and commit every intersecting terrain tile plus vegetation policy as
  one overlay generation; reject stale work through existing source epochs.
- Suppress conflicting natural vegetation records and retain ordinary trees
  immediately outside the plan's declared reservation behavior.
- Exercise approach, retreat, boundary chatter, teleport, world switch,
  exact/composed toggle, delayed refill, and device recovery.
- Capture and inspect native ground/pond/tree pixels as soon as the first
  complete overlay generation draws.

Gate: the whole site appears only when completely fine-domain eligible, no
rectangular mesh seam or annular half-patch appears, shared lattice samples and
halos agree, and work is proportional to intersecting resident tiles.

## Slice 3: Full-Detail Landmark Records And Exact Handoff

Status: planned.

- Compile stable cached landmark meshes through the same authored content and
  transforms used by initial materialization.
- Include all elevated authored content needed to avoid an empty or misleading
  homestead silhouette; keep residents excluded.
- Add bounds/frustum culling, source/revision validation, and the shared exact
  coverage mask in ordinary and multiview render paths.
- Prove partial exact coverage, delayed chunk readiness, eviction, edit while
  exact, source switch, and full overlay admission/removal.
- Measure full-detail mesh bytes, triangles, draw calls, CPU preparation, and
  mono/stereo/multiview GPU cost before proposing any simplifier.
- Capture and inspect walking-height and elevated composed pixels with terrain,
  natural vegetation, focal oak, buildings, and exact chunks together.

Gate: landmarks are stable and singular, exact chunks win without coincident
proxy pixels, and leaving exact coverage exhibits only the explicitly accepted
initial-state edit limitation.

## Slice 4: Cross-Platform And Human Acceptance

Status: planned.

- Run focused Rust, source-identity, codec, CPU/GPU sample, Naga/WGSL, tile
  admission, tree ownership, landmark ownership, and renderer tests.
- Inspect native pixels after every drawable expansion rather than waiting for
  the final slice.
- Prove headed WebGPU, phone-size browser, synthetic stereo, per-eye XR, and
  full-frame multiview pixels through the shared renderer.
- Run the current flat Android and Android XR build/validation lanes, followed
  by physical evidence where the platform policy requires it.
- Report patch bytes, admitted/intersecting/refilled tiles, upload bytes,
  landmark records/triangles/draws, frame CPU/GPU time, and movement tails for
  no overlay, one accepted homestead, and a synthetic two-patch canary.
- Present approach, just-outside-exact, partial-exact, elevated, pond/path,
  departure, and edited-building limitation captures for Human Review.

Gate: Human Review accepts the nearby landmark continuity and its bounded
distance transition, supported render topologies show the same feature,
exact-only remains allocation-free and pixel-stable, and no generic edit or
world-scale overlay subsystem was introduced.

## Acceptance

- The accepted homestead overlay is derived from its persisted realized plan,
  not renderer-side seed/coordinate special cases.
- The overlay covers only its `161x161` reservation; the scenic survey does not
  become resident data.
- Fine clipmap tiles resolve natural base plus overlay during refill without a
  second rectangular terrain draw or new annular ownership rule.
- The complete site is admitted only through spacing `1`, `2`, and `4`, and is
  absent at spacing `8` and beyond.
- Grade, path, pond, shore, garden, and surface materials match the planned
  initial-state semantics at pinned cells and converge cleanly to inherited
  terrain at the edge.
- Natural proxy trees respect the decoration reservation; the focal oak and
  buildings use stable bounded authored records.
- Full-detail landmark meshes are accepted unless measured cost or pixels
  justify a later, separately scoped simplifier.
- Exact-painted chunks mask both overlaid terrain and landmark pixels from the
  same immutable source-qualified coverage snapshot.
- Ordinary edits remain authoritative and durable. Their possible reversion to
  the original proxy after leaving exact range is explicitly accepted and
  does not trigger homestead-specific invalidation work.
- Overlay state commits atomically per whole site and never combines world,
  source, plan, or content generations.
- Exact-only mode allocates and executes no overlay work.
- One- and two-patch costs are bounded and reported; no world-scale performance
  claim is made.
- Native, browser, flat Android, per-eye XR, and full-frame multiview share one
  semantic and rendering contract.

## Code And Documentation Map

- `native/crates/mclone-server/src/homestead_plan.rs` — persisted realized plan
  identity and semantic inputs.
- `native/crates/mclone-server/src/homestead_terrain.rs` — current exact grade,
  path, pond, surface, and decoration application to share rather than copy.
- `native/crates/mclone-server/src/homestead_structure.rs` — authored piece and
  focal-oak realization used to derive landmark records.
- `native/crates/mclone-worldgen/src/homestead_site.rs` — reservation/core
  dimensions and site-plan geometry.
- `native/crates/mclone-worldgen/src/terrain_preview.rs` — natural preview
  sample and detailed-vegetation spacing contract.
- `native/crates/mclone-terrain-view/src/clipmap.rs` — unchanged toroidal tile
  planning and fine-level bounds.
- `native/crates/mclone-terrain-view/src/composition.rs` — source-qualified
  exact-painted coverage and handoff.
- `native/crates/mclone-terrain-view/src/terrain_vegetation_coordinator.rs` —
  staged terrain/vegetation admission.
- `native/crates/mclone-terrain-view/src/viewport_renderer.rs` — tile refill,
  resident GPU resources, landmark rendering, coverage, and multiview.
- `native/crates/mclone-scene/src/terrain_view.rs` — local source publication
  and frame orchestration.
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  — parent residency, ownership, and future sparse-overlay direction.
- [`../topics/starter-farmstead-settlement.md`](../topics/starter-farmstead-settlement.md)
  — realized plan, scale, persistence, and settlement presentation contract.
- [Tactical 304](304-lod-frontier-and-near-field-voxel-convergence.md) —
  separate exact frontier and finest-level surface topology work.
