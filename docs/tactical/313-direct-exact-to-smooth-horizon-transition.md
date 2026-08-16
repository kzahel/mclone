# Tactical 313: Direct Exact-to-Smooth Horizon Transition

Status: planned 2026-08-16; direct-path implementation and phased Human
Review required

Topic: `procedural-horizon-clipmap`

Topic: `procedural-horizon-surface-appearance`

## Instruction Synthesis

Replace the current three-representation composed-terrain sequence:

```text
exact block terrain -> spacing-one procedural voxel shell -> smooth horizon
```

with one direct transition:

```text
exact block terrain -> smooth procedural horizon
```

The existing voxel-to-smooth transition is visually successful because it
interpolates material, texture, and geometric-shade parameters across one
opaque owner. It is not an alpha crossfade between coincident terrain meshes.
Preserve that appearance principle on the smooth spacing-one surface while
removing the intermediate blocky topology, its broad movement pop, and one
complete family of seams.

Exact and procedural geometry must retain single horizontal ownership. Close
their height disagreement with a narrow, explicit boundary connector derived
from the admitted exact perimeter. Do not alpha-fade, dither, depth-bias, or
otherwise overlap two opaque heightfields. A block-aligned connector may
remain because it is boundary geometry, not a third LOD representation.

Support connected, non-rectangular exact footprints as the ordinary case.
Derive appearance distance, connector edges, vegetation ownership, and exact
draw admission from one immutable snapshot. A ready chunk disconnected from
the player-anchored exact region remains procedural until it becomes
connected; loading and visible exact admission are separate decisions.

Use a temporary developer-only A/B path only long enough to compare the
current voxel shell with the direct smooth candidate. Once Human Review
accepts the direct path, delete the A/B selector, voxel topology, obsolete
shader branches, and unneeded support data before final validation. Do not
ship or retain a dormant voxel mode. Git history is the rollback mechanism.

This tactical redirects the unaccepted voxel-specific remainder of Tactical
[`309`](309-procedural-horizon-lighting-and-seam-convergence.md). Its accepted
shared environmental illumination and useful diagnostics remain foundations.
Tactical
[`304`](304-lod-frontier-and-near-field-voxel-convergence.md) remains the
historical implementation record for the voxel-shell experiment and the
single-owner lessons retained here.

## Product Decision

The voxel shell was a worthwhile experiment. It proved that the near horizon
can reuse active-pack materials, biome tint, exact-style illumination, and
explicit connector geometry. It also exposed a product-level weakness:
movement can present smooth mesh, then a several-chunk-deep blocky region,
then exact terrain. When multiple regions change together, the extra topology
change is more conspicuous than the material transition it was meant to
soften.

The selected product direction therefore removes the shell rather than
polishing its exact-facing edge. The final composed representation stack is:

```text
exact opaque/cutout terrain
        |
        | narrow exact-perimeter connector
        v
smooth spacing-one procedural surface
        |
        | existing stitched fine/coarse transitions
        v
progressively coarser smooth procedural levels
```

`Terrain Horizon: Exact Only / Composed` remains the player-facing control.
There is no new quality setting, compatibility mode, or saved preference for
the retired shell.

## Starting Point

The current implementation already supplies most of the required contracts:

- one fixed-budget ten-level, four-by-four-tile toroidal clipmap;
- a smooth six-vertex heightfield topology for spacing two and coarser levels;
- committed fine/coarse stitching, normal halos, and atomic terrain
  admission;
- a renderer-neutral exact-painted snapshot and fixed `64x64`-chunk GPU
  coverage mask;
- shared reversed-Z projection and depth ownership for exact and procedural
  terrain;
- active-pack material faces, biome tint, shared environmental illumination,
  water presentation, and vegetation ownership; and
- native, browser, flat-Android, per-eye XR, and full-frame multiview shader
  generation from the shared terrain-view owner.

The voxel shell changes only the spacing-one level. Each cell currently
submits 30 vertex invocations: six for a rounded flat top and six reserved for
each of four cardinal risers. Smooth cells submit six. Tactical 304 measured
worst-case terrain invocations increasing from `3,932,160` to `5,505,024`, a
`1,572,864`-invocation or 40-percent increase over the all-smooth topology.

The current exact coverage snapshot carries chunk readiness, but not an exact
surface-height and material profile along exposed chunk edges. The
procedural-side land curtain consequently extends a fixed 32 blocks downward,
while water deliberately emits no such fallback wall. This approximation is
the underlying boundary limitation; preserving a wider voxel shell does not
make the exact perimeter authoritative.

The attractive current transition is parameter interpolation, not geometry
alpha. Over the outer 32 cells of the spacing-one level, one owning surface
interpolates exact-like atlas detail and face response toward the coarser
smooth presentation. That mechanism should become a topology-neutral
near-presentation weight.

## Objective

Produce one composed terrain presentation in which:

- exact terrain meets the smooth spacing-one horizon directly;
- no procedural flat-top/cardinal-riser voxel shell remains;
- every horizontal location has one opaque terrain owner;
- connected non-rectangular exact footprints, concave corners, holes,
  negative coordinates, admission, eviction, and movement remain watertight;
- disconnected ready chunks do not appear as isolated exact islands;
- the smooth procedural surface approaches exact material, tint, texture,
  light, and water presentation near the admitted exact perimeter;
- appearance converges over a stable world-space band without changing the
  exact silhouette or making camera-relative squares breathe;
- the exact/procedural height mismatch is closed by perimeter-only connector
  geometry using the best available exact edge facts;
- exact admission can be paced without exposing holes or drawing duplicate
  terrain owners; and
- the final code contains no voxel compatibility path or user-facing toggle.

## Binding Decisions

### Keep geometry ownership binary

Exact geometry owns the complete footprint of every admitted exact chunk.
Procedural solid terrain is discarded over that footprint. The smooth horizon
owns every other horizontal position. The connector occupies only the shared
boundary plane and must not introduce a second horizontal surface.

Do not spatially alpha-fade eight blocks of an exact chunk. A chunk is only 16
blocks wide, so an eight-block inward fade would consume an isolated chunk and
leave no stable exact interior. More importantly, partial alpha cannot resolve
which opaque heightfield owns depth when their heights disagree.

The first candidate uses zero blocks of geometry overlap and no exact-side
geometry fade. A later exact-side material adjustment may use at most a narrow
measured band, but it must not make exact blocks translucent or change their
silhouette.

### Reuse a smooth-side appearance band

Start with a 32-block smooth-side appearance band, preserving the scale of the
existing successful parameter transition. At the exact perimeter, the smooth
owner uses the closest compatible active-pack material, biome tint,
environmental illumination, water response, and texture strength. Those
parameters approach the ordinary far-smooth response with a stable
`smoothstep`-like curve over the band.

The exact distance, not an individual chunk-local coordinate, owns this
weight. Adjacent exact chunks therefore form one union: internal chunk edges
have no blend, while the union's convex corners, concave bays, staircase
edges, and holes receive one continuous perimeter response.

The band width is a captured first candidate, not a user setting. Human Review
may select a narrower value if 32 blocks reads as a halo, but the final value
is one renderer constant with no preference or shader family split.

### Prepare distance once; do not search per fragment

Do not scan a multi-chunk neighborhood in every terrain fragment. Prepare a
bounded distance-to-admitted-exact field on the CPU when the immutable exact
coverage generation changes, then upload it with that generation.

The coverage mask spans at most `64x64` chunks, or `1024x1024` blocks. The
first candidate samples distance every four blocks in an `R8` field. Its base
`256x256` payload is 64 KiB; a fixed transition halo and upload alignment must
remain below an explicitly reported 80 KiB resident target. Saturate distance
at the selected appearance-band width.

Prefer one filtered vertex-stage lookup followed by an interpolated scalar,
one smooth curve, and bounded parameter mixes in the fragment stage. A
fragment lookup is a measured fallback only if vertex interpolation produces
visible corner or triangle artifacts. No implementation may replace the
field with an unbounded nearest-edge loop in WGSL.

Distance preparation is `O(field area)` only when coverage changes. Record its
CPU duration and upload bytes during bursty exact streaming; do not assume the
small allocation alone proves the update path cheap on phone or Quest.

### Admit one connected exact component

The visible exact set is the ready component connected to the current
player/focus anchor and intersected with the intended exact draw footprint.
Ready chunks outside that component remain represented by procedural terrain.
They become exact only after readiness connects them to the admitted set.

Do not clear a still-ready admitted region merely because movement reaches an
unready new center chunk. Retain the previous valid connected snapshot while
it intersects the intended footprint, then switch atomically when a new
focus-connected component is drawable. Empty coverage is valid only when no
retained ready component can satisfy that rule.

The admitted set may be non-rectangular. Concave edges and temporary holes are
valid and must render correctly. Admission should prefer compact growth and
avoid gratuitous one-chunk tendrils, but it must not require a rectangle or
misrepresent an unready chunk as exact merely to fill a hole.

If exact publication supplies many ready chunks in one frame, scene
orchestration may admit a bounded spatially coherent subset per frame while
the procedural owner remains visible underneath. Draw selection, procedural
discard, distance field, connector geometry, and whole-record vegetation
ownership must all consume that same admitted generation. Pacing only the
mask while ordinary exact rendering draws a withheld chunk is forbidden.

### Carry an exact boundary profile

Extend the immutable prepared exact contract with the minimum exposed-edge
facts needed to terminate the procedural owner:

- world-space block edge and orientation;
- exact visible solid surface height at each endpoint;
- exact-facing side material or a declared bounded fallback profile;
- procedural endpoint height and material from the committed spacing-one
  source; and
- water/solid ownership classification.

Generate connector segments only for edges separating admitted exact and
procedural ownership. Join straight edges and corners without cracks, choose
the camera-visible winding or a bounded two-sided connector where supported
views genuinely require both sides, and cover only the measured vertical
difference. Do not retain the unconditional 32-block land curtain after an
authoritative edge profile is available.

The connector may repeat block side materials or use worldgen-owned strata.
Those facts remain only if the direct connector consumes them. Delete
voxel-only top/riser material machinery that has no remaining owner.

### Preserve explicit water ownership

Retain one visible water owner through the transition. The first direct path
must preserve the current opaque procedural-water arbitration where the exact
snapshot lacks enough translucent water-column facts. Do not emit an opaque
solid curtain across open water or reintroduce coincident exact/procedural
water sheets.

Water classification, environmental light, depth response, analytic
river/pool coverage, and texture weight must remain continuous across the
appearance band. A future exact translucent-water handoff is separate work.

### Delete the voxel path after direct-path review

A temporary developer/capture selector may keep the current shell available
through the first matched A/B packet. It must not enter player settings,
preferences, URLs intended as product controls, or a permanent shader option.

After Human Review accepts the direct path, delete:

- 30-invocation spacing-one voxel topology;
- rounded voxel tops and ordinary cardinal risers;
- the voxel-to-spacing-two parent-profile connector;
- `voxel_shell`, `near_shell`, and `surface_kind` branches or fields with no
  surviving connector consumer;
- voxel-only diagnostics and topology assertions;
- obsolete side-strata/material entries; and
- the temporary A/B selector itself.

Generalize surviving terms by behavior rather than retaining voxel names. For
example, the current near-material scalar becomes an exact-proximity or
near-presentation weight.

The tactical cannot complete with the old renderer merely deactivated. If
Human Review rejects the direct candidate, correct it within this tactical or
stop for a new product decision; do not silently keep both systems.

### Keep ownership shared and multiview-correct

`mclone-terrain-view` owns distance preparation, smooth terrain presentation,
connector rendering, diagnostics, and the mono/per-eye/multiview shader
contract. `mclone-scene` owns admitted-exact selection and the immutable frame
snapshot shared by exact draw, terrain coverage, and vegetation. Worldgen and
mesh crates own semantic material/edge facts rather than app-local guesses.

No app crate may gain its own frontier policy. Every new resource, pipeline,
or shader field must serve mono, ordinary stereo, and full-frame multiview or
document an intentional absence before landing.

## Implementation Phases

### Phase 0: Baseline, shape fixtures, and temporary comparison

- Freeze current matched voxel-shell captures and performance receipts at
  walking height, grazing angles, elevated flight, coast, forest, exposed
  stone, steep snow, and open water.
- Add exact-coverage fixtures for a rectangle, L shape, staircase, concave
  bay, single-chunk hole, negative coordinates, disconnected ready island,
  connected growth, eviction, and source reset.
- Record the current spacing-one submitted invocations, fixed bytes, terrain
  CPU/GPU summaries, coverage-generation rate, and connector perimeter.
- Introduce at most one developer/capture-only direct-versus-voxel selector.
  Mark it for mandatory deletion in Phase 3.

Gate: the packet distinguishes topology pop, appearance change, exact
admission, missing connector geometry, and fine/coarse seams. A screenshot
without ownership and readiness receipts is insufficient.

### Phase 1: Connected admission and distance field

- Derive the player-anchored connected admitted subset from ready exact
  columns without changing the authoritative readiness set.
- Retain the prior valid connected component while a newly requested center
  is unready; prove the handoff never clears exact coverage prematurely.
- Route exact draw selection, procedural coverage, vegetation ownership, and
  diagnostics through the same admitted generation.
- Implement the bounded CPU distance field, generation matching, upload, and
  exact-only zero-allocation behavior.
- Prove non-rectangular boundaries, holes, corners, negative coordinates, and
  disconnected-island suppression in focused tests.
- Measure burst updates where several chunks become ready together.

Gate: no disconnected exact island or duplicate terrain owner appears, and
the distance field stays source/generation-coherent through delayed work.

### Phase 2: Direct smooth surface and exact connector

- Render the spacing-one level with the existing six-vertex smooth topology.
- Apply the exact-proximity presentation weight to material, tint, texture,
  geometric shade, environmental light, and water without a topology switch.
- Carry exact exposed-edge facts and generate adaptive perimeter connector
  segments over arbitrary admitted shapes.
- Remove the fixed curtain from the direct candidate once the authoritative
  connector covers the same cases.
- Exercise one-at-a-time and batched exact admission during walking, flight,
  orbit, clipmap rebase, teleport, and eviction.
- Capture direct/voxel matched natural, ownership, topology, albedo,
  environment, geometry, water, and texture diagnostics.

Gate -- Human Review 1: accept one direct exact-to-smooth frontier without a
blocky intermediate belt, exposed sky, conspicuous exact-region rectangle,
material halo, or distracting batch pop.

### Phase 3: Delete the retired shell

- Remove every voxel-shell topology path and the temporary A/B selector.
- Delete or generalize obsolete varyings, uniforms, tests, diagnostics,
  material facts, capture arguments, and documentation.
- Retain side profiles only where the exact connector has a tested consumer.
- Recapture the accepted review scenes from the deletion candidate; do not
  rely on pixels from the dual-path build.
- Confirm generated WGSL contains no dormant voxel mode and that fixed
  allocations exclude its resources.

Gate: source, shaders, tests, runtime controls, and docs expose one composed
terrain path. Git history is the only retained voxel implementation.

### Phase 4: Platform, motion, and performance acceptance

- Run focused terrain-view, composition, scene, shader-generation, source
  identity, and negative-coordinate tests.
- Capture and inspect native and headed desktop/phone WebGPU pixels.
- Prove synthetic stereo, ordinary per-eye XR, and full-frame multiview.
- Build flat Android and Android XR, then obtain physical evidence required by
  the current platform matrix.
- Compare current voxel baseline with the deletion candidate for submitted
  vertices, fixed/resident bytes, coverage-update CPU/upload work, frame CPU,
  and available GPU timing.
- Repeat delayed and batched exact admission with connected irregular shapes
  at walking and flight speeds.
- Verify Exact Only remains horizon-allocation-free and pixel-stable.

Gate -- Human Review 2: accept the final single-transition composed terrain
in still and moving evidence after voxel deletion. Supported platforms share
the result, direct composition remains bounded, and measured cost does not
regress the accepted baseline without an explicit product decision.

## Acceptance

- The composed frame has only exact terrain and smooth procedural terrain as
  horizontal geometry representations.
- No spacing-one flat-top/cardinal-riser shell remains in code or pixels.
- Exact and procedural ownership is binary and generation-coherent.
- Connected non-rectangular exact regions render with one external perimeter;
  internal exact chunk edges do not create blends or connectors.
- Disconnected ready chunks remain procedural until connected to the
  player-anchored admitted set.
- Movement into an unready center retains a still-valid prior connected set
  until the replacement component can be admitted atomically.
- Concave corners, holes, negative coordinates, admission, and eviction expose
  no sky crack, duplicate surface, or missing water owner.
- Smooth-side appearance converges over one stable world-space band without a
  square/annular material, light, texture, water, or vegetation discontinuity.
- Connector geometry uses exact edge facts and remains perimeter-bounded.
- Batched chunk readiness does not produce a several-chunk-deep blocky pop;
  any admission pacing retains the procedural owner until the exact snapshot
  changes atomically.
- The final implementation has no user-facing or dormant voxel switch.
- The smooth topology recovers the expected spacing-one vertex reduction;
  distance-field and connector costs are measured on native, WebGPU, and XR.
- Mono, per-eye, and multiview consume the same terrain and connector facts.
- Exact Only remains horizon-allocation-free.
- Human Review accepts the direct candidate before deletion and the final
  deletion candidate afterward.

## Non-Goals

- Literal alpha, dither, depth-bias, or polygon-offset blending of coincident
  opaque terrain.
- Retaining the voxel shell as a quality mode, fallback, diagnostic product
  option, or disabled compatibility system.
- Requiring exact coverage to be rectangular.
- Marking unready chunks exact to fill a hole.
- Increasing exact render distance to hide the frontier.
- Replacing the toroidal clipmap, its fixed level count, or its committed
  fine/coarse smooth stitching.
- General distant edits, structures, caves, arches, overhangs, or a volumetric
  LOD representation.
- Exact translucent-water parity, independent flat water geometry, or a
  hydrology rewrite.
- Shadow maps, TAA, screen-space effects, or a general terrain material
  rewrite.
- Changing canonical world generation or persisted world identity for LOD
  presentation.

## Code and Documentation Map

- `native/crates/mclone-terrain-view/src/composition.rs` -- immutable exact
  coverage, connected admission inputs, and distance-field representation.
- `native/crates/mclone-terrain-view/src/shaders/terrain_preview_render.wgsl`
  -- smooth geometry, exact-proximity appearance, water, and connector
  presentation.
- `native/crates/mclone-terrain-view/src/viewport_renderer.rs` -- shared GPU
  resources, pipelines, diagnostics, and mono/multiview generation.
- `native/crates/mclone-terrain-view/src/engine.rs` and
  `runtime_session.rs` -- prepared frame and generation-coherent handoff.
- `native/crates/mclone-scene/src/terrain_view.rs` -- player-anchored admitted
  exact set and scene-wide snapshot arbitration.
- `native/crates/mclone-mesh` -- exact boundary material/height facts where
  the existing prepared mesh contract owns them.
- `native/crates/mclone-worldgen` -- procedural endpoint material and retained
  side-profile semantics used by the connector.
- `scripts/capture-terrain-seam-review.mjs` -- temporary A/B evidence followed
  by final direct-only review evidence.
- [`309-procedural-horizon-lighting-and-seam-convergence.md`](309-procedural-horizon-lighting-and-seam-convergence.md)
  -- accepted environmental-light foundation and redirected voxel-specific
  closeout.
- [`304-lod-frontier-and-near-field-voxel-convergence.md`](304-lod-frontier-and-near-field-voxel-convergence.md)
  -- historical voxel-shell implementation and measured baseline.
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  -- living residency, ownership, exact-handoff, and platform status.
- [`../topics/procedural-horizon-surface-appearance.md`](../topics/procedural-horizon-surface-appearance.md)
  -- living material, lighting, water, and transition status.
