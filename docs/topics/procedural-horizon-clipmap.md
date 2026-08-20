# Current LOD: Procedural Horizon Clipmap

Topic: `procedural-horizon-clipmap`

Status: current shared product architecture as of 2026-08-20. The regular
geometry clipmap, focus-connected exact composition, and bounded hybrid
frontier run through `mclone-terrain-view` on native, WebGPU, flat Android,
and XR. Tactical
[`321`](../tactical/321-exact-frontier-support-architecture.md) completed the
high-render-distance frontier implementation. Tactical
[`323`](../tactical/323-quest-frontier-performance-recovery.md) then removed
the dominant fragment-time support lookup cost and measured the remaining
full-belt cost. Final human pixel reviews remain the acceptance gates; this
topic is the canonical system description.

## System Contract

The system combines immutable procedural heightfield tiles with the ready
connected component of exact chunks. Neither subsystem expands merely to hide
the other's readiness. Instead, every submitted exact perimeter receives a
complete frontier certificate:

```text
worldgen source + camera focus
              |
       regular clipmap request
              |
    staged -> committed levels -----------------+
                                                 |
ready exact columns -> focus-connected coverage  |
              |                                  |
     mask + appearance + boundary profile        |
              |                                  |
              +----------> frontier plan <-------+
                                |
                 complete coarse fallback now
                                |
                    preferred fine belt later
                                |
        one immutable terrain/water/tree certificate
                                |
                    mono / per-eye / multiview
```

`mclone-terrain-view` owns clipmap planning, procedural evaluation,
composition formats, frontier planning and topology, certificate admission,
GPU terrain resources, and proxy vegetation. `mclone-scene` supplies live
ready exact columns, chooses the focus-connected component, prepares exact
boundary facts, and consumes one frame summary. Apps choose platform defaults,
create surfaces or swapchains, translate input, and present; they do not own
LOD geometry or seam policy.

The only supported product source is a reconstructible local
`mclone-overworld-v1` world with matching profile, seed, topology, exact
generation, and procedural source identity. Remote sessions and incompatible
generation profiles project the effective setting to Off while retaining the
user's desired preference.

## Regular Clipmap And Presets

Every enabled level is a four-by-four toroidal grid of 64-cell terrain tiles.
Level zero samples every block. Each following level doubles sample spacing
and cuts a rectangular hole matching the committed finer level. Smooth
triangles weld adjacent clipmap levels at their shared footprint; no level
changes the one-cell render stride to implement a cheaper preset.

Sub-cell motion changes only the camera. When a level crosses a tile boundary,
the admission owner reuses retained physical slots, prepares the entering
strip, and commits that complete level atomically. Seven guard resources per
level permit the prior committed presentation to remain drawable during axial
or diagonal movement. A teleport drops obsolete staged coverage and reuses the
same bounded pool. Independently ready levels may commit separately; their
combined origins and generations form the immutable procedural presentation
identity used by the frontier certificate.

| Preset | Levels | Logical tiles | Guard tiles | Proxy vegetation spacing |
|---|---:|---:|---:|---:|
| Off | 0 | 0 | 0 | none |
| Low | 6 | 96 | 42 | 1 |
| Medium | 8 | 128 | 56 | 2 |
| High | 10 | 160 | 70 | 4 |

Off constructs no terrain-view engine, clipmap, frontier, or proxy-vegetation
resources. Low, Medium, and High share identical near geometry and differ only
in outer reach and vegetation reach. Platform profiles choose defaults without
changing those meanings; Android XR remains Off by default because its full-
resolution Low workload does not reliably meet 72 Hz.

## Exact Readiness And Formats

The scene admits only the four-connected ready exact component containing the
focus, or the still-valid component connected to it during transition.
Disconnected ready islands stay procedural. Growth, eviction, source reset,
negative coordinates, and periodic-coordinate lifting produce a new immutable
exact generation rather than mutating the generation being drawn.

One generation supplies all exact-side composition facts:

- a sparse chunk set and packed discard mask;
- a 32-block appearance-distance field, with water limited to its nearest
  eight-block procedural-side halo;
- a typed exposed boundary profile containing height, side material, and
  water ownership;
- complete exact/proxy tree ownership; and
- the source and topology identity used by every consumer.

The shared legal maximum is 65 by 65 chunks, covering desktop render distance
32. The GPU mask uses 133 words and a 65-chunk row stride. Boundary storage is
bounded to 1,040 blocks per axis and transition storage to 276 texels per axis.
Oversized, disconnected, source-mismatched, or incompletely profiled inputs
are rejected explicitly rather than truncated.

## Frontier Plan And Hybrid Closure

`TerrainFrontierPlan` compares every directed exposed exact block edge with
one committed procedural presentation. It records the adjacent level and
sample spacing, unique procedural owner, solid/water/world-boundary class,
existing spacing-one closure, desired fine tile, topology lift, and bounded
cost. `TerrainFrontierTopology` then assigns every edge exactly one closure:

- a preferred solid connector or water curtain owned by spacing-one terrain;
- a resolution-aware solid connector or water curtain evaluated against the
  actual bordering coarse triangle; or
- an explicit finite-world boundary.

The preferred layer may allocate at most 32 additional spacing-one tiles.
Tiles already present in the regular finest ring are reused semantically and
do not consume that pool. Each added tile is the normal 64-by-64 heightfield
resource. Its outer block-segmented skirt hands back to the regular clipmap,
and a suppression key removes the coarser base surface beneath it. Support is
compiled at no more than four tiles per frame.

Pool exhaustion is deterministic. A topology prepared with zero additional
fine tiles is a complete synchronous fallback for all solid and water edges;
it allocates no support terrain and suppresses no base tile. The fallback is
not an error state or a hidden overlap. The developer-only
`frontier-fallback` selector reduces the pool to one so those pixels can be
reviewed at an ordinary site. Natural is always the product path; the former
hybrid-proof selector and diagnostic-only lifecycle are deleted.

## One-Frame Admission And Ownership

Every new exact generation or procedural presentation first commits a
resolution-aware fallback certificate in the same render preparation. A
preferred 32-tile generation may compile behind it. Support terrain,
coarse-base suppression, and connector ownership become active together only
after every selected resource is ready. Newer exact or clipmap epochs coalesce
obsolete pending work. The public admission state is one of `disabled`,
`synchronous-fallback`, `preparing-preferred`, `preferred`, or `rejected`;
the three middle states are complete drawable certificates.

Horizontal ownership is binary:

- exact coverage discards every procedural horizontal surface, including
  water;
- selected support tiles own only the procedural side of the exact boundary;
- the regular coarse base is discarded wherever committed support owns the
  same horizontal tile; and
- connectors and outer skirts are vertical closure, never a coincident
  horizontal collar.

The ordinary procedural vegetation presentation remains the sole proxy-tree
owner over both base and support terrain. Support therefore creates no second
tree population. Exact-owned complete tree records are removed from that
population through the same exact generation. Land, water, materials, grass
and water tint, sky darkening, fog, time of day, and geometric shading all use
the same per-frame render inputs on both sides of the boundary.

Missing exact boundary facts or missing/ambiguous procedural coverage leave
the topology incomplete and the certificate rejected. Normal rendering never
publishes a partial preferred generation. Source reset and device recreation
drop all GPU frontier epochs and rebuild from shared source facts; no GPU
identity is persisted.

## View And Platform Consumption

Mono, multi-camera flat frames, synthetic stereo, ordinary per-eye XR, and
full-frame multiview prepare one immutable frontier and vary only view
uniforms, culling masks, and targets. Multiview does not plan or compile a
second support belt. Terrain and proxy-tree shaders have separate single-view
and multiview modules so browser WGSL never sees unsupported `view_index`
builtins.

The final Tactical 321 validation inspected crack-free native RD2, native
RD8 preferred, native forced-fallback, headed WebGPU RD8, flat-Android RD8,
and synthetic stereo output. A physical Quest 3 accepted both ordinary
per-eye and full-frame multiview Low/RD8 submissions. The full-resolution
Quest steady sample measured 14.52 ms p50 and 18.48 ms p95 app work against a
13.89 ms 72 Hz budget. Settled full-frame multiview measured 16.87 ms p50 and
20.15 ms p95 while retaining all 96 Low slots and 289 exact columns. Enabled
LOD on that platform is therefore an explicit quality choice rather than its
default.

Tactical 323 replaces the old per-fragment scan through 32 support records
with a collision-free 18-by-18 row-mask lookup. The fixed suppression binding
shrinks from 512 to 96 bytes and lookup becomes constant time for every legal
65-chunk exact span, including negative coordinates. The retained preferred
Quest Low/RD8 path improves to 13.27 ms p50 and 14.31 ms p95 app work with
7.48 ms reported app GPU and 17.2% over-period frames. It reaches 72
submissions per second but still misses the strict p95 budget. A zero-weight
appearance branch is pixel-safe but measured within variance. A compact
support-cell draw removed only 3.1% of visible support vertices, did not
improve Quest timing, and was removed rather than retained as complexity.

The now-cheap forced-fallback control measures 12.12 ms p50 and 13.12 ms p95,
6.73 ms app GPU, and 0.1% over-period frames. This isolates the remaining
stationary preferred cost to the full spacing-one support belt. Fixed
foveation Low and Medium do not clear p95 and remain unpromoted. Preferred
settled orbit remains over budget at 13.71/18.98 ms p50/p95, so Low remains
Off by default on Quest.

The bounded RD8 preferred case uses 20 added tiles, 11,212,240 terrain bytes,
and 31,488 active connector bytes. It certifies all 1,088 perimeter segments.
The forced one-tile case assigns 1,024 of those segments to the fallback.
RD2 needs no added support tile. A 240-frame native traversal crossed three
clipmap presentations and 138 exact generations over 130.68 blocks with zero
incomplete certificates: 188 frames used a complete fallback during
preparation and 52 used the preferred certificate.

## Diagnostics And Recovery

The native seam receipt exposes four complementary views:

- `frontier`: raw boundary classification and candidate cost;
- `frontierTopology`: closure assignments, support/suppression counts, and
  bounded byte/vertex estimates;
- `frontierAdmission`: active/pending identity, completeness, and lifecycle
  counters; and
- `frontierGpu`: allocated/ready support, dispatch, connector, and draw facts.

`frontier-support` colorizes the unmodified classifier. `frontier-fallback`
forces bounded exhaustion. Window-frame reports record a certificate on every
presented frame, which is the objective motion gate. Unit fixtures cover legal
render distances through 32, every finest-tile phase, irregular and adversarial
connected shapes, water, finite and periodic worlds, negative coordinates,
growth, eviction, source reset, level transitions, and teleport.

## Accepted Limits

The procedural horizon is a surface heightfield. It does not represent caves,
arches, overhangs, arbitrary structures, distant block edits, or other sparse
volumetric silhouettes. An unsupported exact volumetric boundary must remain
outside this contract rather than being hidden by a curtain. The preferred
support unit is still a complete 64-by-64 tile, so large high-distance exact
frontiers have a meaningful vertex and GPU cost even when only a narrow belt
is needed. Exact-owned cell compaction is not a useful next step: the measured
RD8 view removed only 1,024 of 32,768 visible support cells. The credible next
design is sub-tile or strip-owned spacing-one support with a matching bounded
coarse-suppression representation. That is a certificate topology change, not
an encoding optimization; it requires a new tactical, adversarial shape
proofs, and human pixel acceptance while preserving the same single-owner and
complete-fallback rules.

## Historical Development Record

The dated material below records how the current system was reached. It is not
the current contract above.

The standalone cross-platform proof, transition hardening, shared
vegetation service, Terrain Lab composition, and live-game adoption are
complete. Tactical
[`313`](../tactical/313-direct-exact-to-smooth-horizon-transition.md) now has
one direct exact-to-smooth implementation awaiting final Human Review 2:
ordinary sessions use smooth spacing-one geometry, a generation-cached
32-block appearance field, focus-connected irregular exact admission, and a
measured exact-profile perimeter connector. Human Review 1 accepted that
direction; the former spacing-one voxel shell, shader branches, diagnostics,
selector, and A/B harness are deleted rather than retained as a fallback.
Tactical
[`320`](../tactical/320-cross-platform-lod-quality-presets.md) is complete as
of 2026-08-20. The shared in-game Graphics screen exposes
`Distant Terrain: Off / Low / Medium / High`; the same typed action,
preference, settings reducer, and scene effect serve desktop, browser, flat
Android, and ordinary per-eye XR. All non-Off qualities use the same
stride-one four-by-four geometry clipmap with six, eight, or ten levels and a
separately bounded proxy-vegetation reach. Live changes reconfigure that
engine in place and reuse common level, admission, GPU-pool, and vegetation
state. Full-frame multiview terrain and vegetation share the same preset
contract; multiview remains an opt-in diagnostic path because its accepted
Quest comparison regressed on GPU. The control is available only for a local
`mclone-overworld-v1` source; remote and incompatible-profile sessions retain
the desired choice, show it as unavailable, and project the effective state
to Off. Native, browser-Wasm, flat-Android APK, Android-XR APK, synthetic
stereo, AVD, and physical Quest gates pass.

Review at higher exact render distance has exposed one unresolved composition
contract. The exact-to-smooth connector is emitted only by spacing-one tiles,
while focus-connected exact coverage can extend beyond the fixed four-by-four
finest clipmap bounds and meet spacing-two or coarser terrain directly. The
same audit found that legal desktop render distance 32 describes a 65-by-65
chunk exact window while the exact mask and boundary formats currently permit
only a 64-chunk span. Tactical
[`321`](../tactical/321-exact-frontier-support-architecture.md) now owns the
architecture-first response. Its Phase 0 audit and Human Review A1 are
accepted. Phase 1 diagnostic planning is complete, and Human Review A2
accepted bounded Hybrid D on 2026-08-20. Phase 2's opt-in topology proof is
complete and awaits Human Review B; no support candidate changes ordinary
geometry yet. The audit establishes
that radius 4 is the largest square always
contained by the current finest ring, radius 3 is the largest with a direct
spacing-one neighbor in every tile phase, and radius 2 is the largest with the
complete 32-block finest-level halo. It also finds that periodic-coordinate
lifting, exact eviction, independently committed clipmap levels, water, and
vegetation must participate in the same per-edge composition certificate;
connector geometry alone cannot establish the missing invariant. No candidate
is yet part of the product geometry contract.

The shared diagnostic `TerrainFrontierPlan` now classifies every directed
exact boundary block edge against one complete committed clipmap presentation,
reports solid, water, missing-profile, spacing, current closure, format
capacity, and hypothetical candidate costs, and preserves a topology-aware
observer-local lift. It is cached by exact generation and presentation
identity, remains off the readiness/admission path, and adds no candidate
resources. A matched seed-12345 campaign classifies all 320 RD2 edges beside
spacing one, with 277 solid connectors and 43 uncertified water edges. The
same site at RD8 classifies all 1,088 edges beside spacing two, with 896
unsupported solid edges, 192 uncertified water edges, and no connector. The
inspected `frontier-support` diagnostic shows the corresponding green and
magenta frontiers.

Candidate projections selected bounded Hybrid D: prefer a sparse spacing-one
belt, but require an explicit
resolution-aware and water-aware fallback when its pool is exhausted. At RD8,
a complete 6-by-6 finest extent costs 13.45 MB added while a 32-tile sparse
belt needs 20 new tiles / 11.21 MB. At compact legal RD31, complete expansion
costs 188.37 MB and the sparse belt needs all 128 hypothetical slots / 71.76
MB. A legal 63-by-63 comb requests 308 new sparse tiles and rejects 180 at that
cap, proving that sparse support alone is not a worst-case policy. Completion
must consolidate the selected topology, budgets, admission lifecycle, and
accepted limitations back into this topic after the remaining human gates.

The Phase 2 proof makes that selected topology drawable without adopting it.
`frontier-hybrid-proof` lazily compiles at most 32 spacing-one tiles, at four
dispatches per frame, then atomically publishes the identical support and
coarse-suppression sets. Each exact edge is owned by one preferred fine or
resolution-aware coarse solid/water curtain. Every selected support outer edge
has a block-segmented skirt back to the regular clipmap. The natural path
allocates no dynamic proof resources and uses its prior geometry; a 512-byte
fixed suppression-key buffer is the only common plumbing cost.

At the matched RD8 review site the full proof selects 20 tiles, adds 11.24 MB
resident work, and submits 202,848 additional visible vertices. A developer-
only one-tile selector forces 832 solid and 192 water fallbacks at the same
site, adds 0.57 MB and 3,168 visible vertices, and provides exhaustion pixels
without changing the accepted 32-tile capacity. The inspected full and forced-
fallback frames close the sky gap without a conspicuous wall. This proves the
topology and bounded degradation only. Exact/support/base epoch retention,
streaming coalescence, eviction, vegetation participation, and real frame-time
cost remain Phase 3 work and are not authorized before Human Review B.

The 2026-08-13 phone-browser black-frame regression is resolved. XR
multiview support had added `@builtin(view_index)` entry points to the same
WGSL modules used by ordinary single-view clients. Chrome validates the whole
module even when the device has no multiview feature, so selecting Composed
invalidated both terrain pipelines. Once that was unmasked, Chrome also
rejected one river-coverage derivative inside non-uniform fragment control
flow. Single-view terrain and tree modules now contain no multiview builtin;
XR-capable devices construct separate multiview modules, and all derivatives
run in the uniform fragment prelude. `pnpm native:web:terrain-horizon-smoke`
now exercises the real phone-sized Graphics row, captures Composed, reloads
without a URL override, verifies the persisted `composed` choice, and captures
again. The inspected 780-by-1688 toggle and reload images contain 46,737 and
47,422 colors respectively instead of a black frame.

A separate 2026-08-15 browser black-frame regression from the new surface
diagnostics is also resolved. The terrain and proxy-tree WGSL used
`diagnostic` as a local identifier; native Naga validation accepted it, but
Chrome treats that WGSL diagnostic directive word as reserved and invalidated
both render pipelines. The shared shaders now use `horizon_diagnostic`, and a
browser reserved-word lint now rejects `diagnostic`, `enable`, or `requires`
in every generated Terrain View shader variant before Naga validation.
Terrain Lab also captures uncaught WebGPU device errors and scopes runtime
pipeline creation so an invalid shader becomes an initialization failure
instead of a ready black canvas. Inspected desktop and Pixel 7 Terrain Lab
runtime captures reach 25/25 exact chunks and all 160 horizon slots with
composed pixels. The full phone game Graphics toggle and persisted-reload
smoke reaches 49 exact columns, all ten drawn levels, 23 drawn horizon tiles,
and 37,750/38,735 distinct interior colors in its 780-by-1688 captures.

This is the current system meant by unqualified **LOD** or **LOD system** in
project discussion. **Distant Terrain** is its player-facing settings name.
World Explorer was its first proof host and remains a consumer, but the shared
implementation owner is `mclone-terrain-view`. The canonical terminology and
document routes live in [`lod.md`](lod.md). The deleted chunk-granular system
is always called the **retired chunk-based Far LOD**.

The exact-water ownership defect reported on 2026-08-20 is corrected. The
procedural shader previously exempted water from exact-coverage discard, so an
opaque LOD water surface could replace translucent exact water throughout the
admitted footprint. Exact coverage now discards every procedural surface
class. Exact chunks own water inside; only an eight-block procedural-side
appearance halo approaches the active pack's exact water response. Land keeps
the existing 32-block appearance band. Neither path overlaps horizontal
geometry inside exact coverage.

Same-day composed review exposed one exact/procedural surface-contract defect:
ordinary continental terrain may bottom out at Y62 and canonical columns then
receive the profile's Y63 sea fill, but both LOD evaluators raised the display
surface to Y63 while retaining a grass material. Final-stage water now selects
the water material in the CPU reference sample, GPU evaluator, and shared
Basic/Inferred renderer without changing terrain height, canonical blocks, or
persisted worlds. The reference-grid schema is
`mclone-terrain-preview-reference-grid-v10` and the GPU evaluator is
`mclone-overworld-v1-gpu-preview-a10`. Focused worldgen and terrain-view tests,
WGSL validation, and an inspected 1,024-by-576 native composed capture at seed
`8675309`, center `(-1536, 2032)`, pass; the capture shows blue procedural
inland water rather than a sea-level grass sheet.

Tactical
[`276`](../tactical/276-lod-water-surface-presentation-unification.md) closes
the related procedural water presentation defect found in that same composed
capture. The analytic signed-distance river overlay had its own RGB palette,
darkened from fragment depth, and continued across the continental boundary
above valid submerged-outlet bathymetry. The shared shader now uses one
bed-depth water color and material-texture path for ocean, pond, sampled river,
and coarse river coverage; it admits the analytic coverage only with physical
continental channel ownership. A matched 1,024-by-576 native composed capture
at seed `8675309`, center `(-1536, 2032)`, retains the inland rivers and removes
their dark ocean-surface continuations. Focused terrain-view/Naga tests and the
World Explorer `wasm32-unknown-unknown` library check pass. Terrain evaluation,
sea level, submerged floor shape, exact columns, and persisted worlds remain
unchanged; any residual pond-junction silhouette is a separately deferred
hydrology geometry question.

Coordinating parent Tactical
[`261`](../tactical/261-procedural-horizon-product-integration-roadmap.md)
owns the global path into `mclone-scene`, exact/procedural arbitration,
flat-platform promotion, and XR/multiview acceptance, but further execution is
paused behind the playable intro homestead in Tactical
[`274`](../tactical/274-playable-intro-homestead.md) as of 2026-08-10. The first
cross-platform proof was completed and deployed on 2026-07-25 by Tactical
[`249`](../tactical/249-cross-platform-procedural-horizon-proof.md). Shared
toroidal planning, a ten-level fixed-budget renderer, native tree proxies, and
one Rust terrain-view engine now run through Explorer, Terrain Lab, and the
opt-in live game on native and browser hosts. Android and XR promotion are now
implemented; Tactical 313 includes physical flat-Android acceptance for the
direct frontier. Product scope and platform hosting are independent: the small
Explorer and full game may both run in the browser, while the same terrain
system remains usable on desktop, Android, and XR. Tactical
[`245`](../tactical/245-retire-chunk-far-lod-runtime.md) remains the completed
removal boundary for the rejected chunk-based Far LOD system. Tactical
[`274`](../tactical/274-all-client-distant-terrain-control.md) now owns the
all-client Graphics control, runtime preference, and default
per-eye XR projection reach. Full-frame multiview terrain and vegetation are
now implemented and physically accepted on Quest 3 standalone plus Linux
Vulkan/WiVRn. Distant terrain is now a normal quality setting with bounded
platform defaults; only the full-frame multiview render path remains an
experimental diagnostic choice because its matched Quest workload regresses
on GPU.
Focused coordinating Tactical
[`280`](../tactical/280-xr-multiview-render-path-workstream.md) now owns
procedural-horizon multiview, safe live XR path selection, interactive
regression coverage, and the final disposition. Tactical
[`278`](../tactical/278-quest-procedural-horizon-multiview.md) remains its
renderer and Quest A/B child.
Tactical
[`247`](../tactical/247-standalone-world-explorer-foundation.md) supplied the
small native proof host and shared navigation boundary for the first ring; it
deliberately implemented no clipmap residency.
Tactical
[`262`](../tactical/262-world-explorer-exact-procedural-composition.md) has
now reached Human Review 1 with a shared exact-painted snapshot, bounded GPU
mask, caller-owned color/depth target, extracted canonical compiler/codec,
and native `Horizon`, `Exact`, `Composed`, and `Coverage` modes. The 5-by-5
exact footprint stays coherent through delayed movement, negative
coordinates, and teleport. Its then-current 1.5-block procedural collar
removed the earlier full-height footprint wall. Human review accepted that
terrain behavior overall but found a blocking natural-tree ownership defect: an exact
tree can be depth-occluded by the retained collar while fragment masking
leaves the outside part of the same stable record's LOD proxy visible.
Tactical 262 Slice 3A now separates natural-tree admission from terrain and
selects one complete exact-or-proxy representation from stable ID, complete
bounds, exact-safe interior, and drawable readiness. Delayed native movement
and inspected forest captures report zero missing, dual, or unowned records.
Human Review 1A was nevertheless retracted on 2026-07-27 when low pitch made
the whole focus-centered exact patch appear behind procedural terrain.
Deterministic exact/composed/coverage captures and a temporary full-footprint
discard showed that the broad effect was not collar overlap: the orbit eye
was outside the patch, with real procedural foreground between it and the
focus. Slice 3B now uses one viewer-forward anchor for exact terrain,
procedural residency, and vegetation, plus a terrain-safe composed target
height when sea-level targeting would place the low orbit eye in resolved
ground. Four-sided captures and delayed native window/offscreen movement pass
with no missing exact or proxy trees. Slice 4 now runs the same exact renderer
through an isolated browser Worker and accepts deterministic
`composition`/`exactRadius` URLs. Desktop and Pixel 7 semantic receipts match
all 25 painted chunks, coverage generation 27, and the 8 exact/7 proxy split
across 15 whole-tree records. The content-versioned production build now also
passes hosted desktop `Composed`/`Coverage` and Pixel 7 `Composed` gates with
zero missing representations. Human Review 1B rejected those pixels:
procedural terrain uses linear reversed depth while exact chunks use the
engine's nonlinear perspective reversed-Z projection, so the shared depth
target contains incomparable values and procedural surfaces cut nearer exact
trees and hills. Tactical 262 Slice 4B now owns the shared-projection
correction and source-colored silhouette evidence. Its correction landed at
`91fe9302`: terrain and proxy vegetation now consume the same
`ChunkRenderView` matrix as exact chunks, and a magenta exact-source
diagnostic is available natively and through `sourceColors=1`. Matched native
silhouette and hill captures preserve nearer exact geometry and allow nearer
procedural geometry to occlude it. Desktop and phone browser semantic gates
pass, and hosted interactive Human Review 1B accepted the corrected
composition on 2026-07-27. Minor z-fighting limited to the outermost exact
blocks remained a known near-coincident frontier-overlap issue at that
checkpoint; it did not reopen the shared-depth correction. Historical Tactical
[`304`](../tactical/304-lod-frontier-and-near-field-voxel-convergence.md)
implemented the first correction. Solid procedural tops and ordinary risers
were discarded over the complete exact-painted footprint; a bounded
procedural-side curtain covers height disagreement without a horizontal
collar. The spacing-one level is a flat-top/cardinal-riser voxel shell, while
coarser levels remain smooth. Active-pack face materials, worldgen-owned side
strata, grass tint, exact face shade, and shared sky-darken/lightmap inputs
converge the near shell on exact terrain. At that checkpoint, opaque
procedural water deliberately remained the visible owner through exact-painted
chunks. The 2026-08-20 correction supersedes that exception because it leaked
LOD presentation into the exact domain. Native,
desktop/mobile WebGPU, stereo, flat-Android build, and Android-XR build
boundaries passed, but later product review rejected the intermediate topology.
Tactical
[`309`](../tactical/309-procedural-horizon-lighting-and-seam-convergence.md)
accepted the fixed diagnostic packet and Phase 1's shared environmental
illumination on 2026-08-16. Human Review 2 then rejected two corrections for
the voxel-to-smooth seam. The inward-winding correction and expanded
steep-seam packet remain implemented evidence, but user review redirected the
product before exact-to-voxel polishing: the several-chunk-deep blocky
intermediate adds an unconvincing topology pop and a second transition before
exact terrain.
Tactical
[`313`](../tactical/313-direct-exact-to-smooth-horizon-transition.md) now owns
the visual gate and has implemented its direct-only deletion candidate. Smooth
spacing-one geometry carries the successful near/far parameter interpolation
through a 32-block world-space field prepared once per admitted generation.
The exact draw, procedural discard, connector, field, and vegetation ownership
share one focus-connected non-rectangular exact set, so disconnected ready
chunks remain procedural. Canonical exact surface columns feed a compact
two-sided vertical perimeter connector over measured height differences.

The revision-`3f0ff7ee` Human Review 1 packet contains 28 matched and
state-validated captures. Its ordinary 5-by-5 field is `1,296` bytes and took
`18-37` microseconds to prepare natively; direct terrain submits `1,633,494`
vertices versus `2,614,872` for the temporary voxel comparison. Human Review 1
accepted the direct result. Revisions `22ee5622` and `cb06c20f` deleted the
voxel geometry and 498-line A/B harness. The finest procedural level now has
one six-vertex topology, fixed residency is `133,209,624` bytes, and source
search plus generated WGSL validation find no dormant voxel mode. Revision
`44b5c9be` also corrected the surviving connector's grass-side material
orientation so the grass strip follows its upper exact edge.

The direct-only native window/offscreen, headed desktop and phone-sized WebGPU,
live-game WebGPU, Terrain Lab WebGPU, synthetic-stereo, Android build, desktop
XR, physical Pixel 7a, and physical Quest 3 gates pass. The Quest proof covers
ordinary OpenXR and full-frame terrain multiview. Exact Only remains
horizon-allocation-free. Human Review 2 is the remaining Tactical 313 stop.
The exact renderer's full-sky/zero-block-light RGB now applies once to every
procedural terrain level, sampled and analytic water, and proxy tree after
albedo and geometric shade composition. A fixed 32-image, two-seed native
packet covers every accepted scene at dawn, noon, dusk, and midnight plus
Exact Only and eight diagnostic controls. Its gate requires all 25 RD2 exact
columns, all 160 clipmap slots, drained failure-free vegetation, and identical
settled presentation state within each comparison group; Exact Only reports no
horizon allocation.

Inspected midnight pixels no longer contain daytime-green smooth land,
bright-cyan procedural water, or daylight proxy crowns. The environmental
diagnostic is constant across procedural topology and surface class, while
noon remains normally illuminated. That topology-independent lighting result
is retained by Tactical 313. Tactical 304's material and bounded-connector
lessons remain foundations, while its water exception and voxel-shell topology
survive only in history.
The first Phase 2 review packet improved the appearance terms but was rejected
for the geometric crack above. Voxel face shade
approaches the smooth slope response over the existing committed presentation
band, both procedural representations share active-pack grass tint, and
analytic river texture strength uses that same continuous weight. A strict
48-image packet covers all prior scenes plus multi-scene term diagnostics and
camera stability endpoints. A clean 18-second headed traversal crosses a
spacing-one tile boundary with every frame presented and final view interest
fully ready. No residency, ownership, geometry, sample-record, or fixed-byte
contract changed. Commit `d2e81187` keeps those constraints: the reserved outer
voxel cardinal face now spans to the actual stitched parent profile at both
segment endpoints, without horizontal overlap, a second geometry owner, or a
crossfade. The strict replacement packet and headed traversal passed, but a
later steep-snow interactive view exposed that the connector was culled from
the only side the camera-centered fine clipmap normally observes. That Human
Review evidence overrides the packet's false acceptance. Commit `fcc0edfe`
reverses only the outer connector's horizontal endpoint order, while commit
`4079d5ba` adds matched natural/topology captures from all four cardinal
directions at the steep site. The 56-image packet and clean moving-camera
receipt pass; inspected pixels close the large sky wedge. Retain a bounded
two-sided connector or dedicated zipper ring only as fallback work if Human
Review rejects the resulting material wall.
Post-review Explorer evidence then showed that Slice 3B's viewer-forward
anchor was useful for foreground diagnosis but confusing as the product
default: it moves exact residency when yaw changes and can place exact terrain
below the low-angle viewport. Commit `c75b488b` restores the orbit focus as
the native/browser default composition anchor and retains the old placement
through explicit `viewer-forward` options. This changes proof-host placement,
not the accepted shared-depth or whole-record ownership contracts.
Tactical
[`266`](../tactical/266-terrain-lab-runtime-composition-adoption.md) now
extracts the exact renderer, composition session, and browser executors from
World Explorer into `mclone-terrain-view`. Terrain Lab consumes them through
the `runtime` pane while retaining its four opt-in research panes. Following
the accepted review, Terrain Lab now defaults to only that composed pane and
World Explorer defaults to composed exact-plus-horizon presentation; explicit
LOD-only controls remain available. The hosted desktop and Pixel 7 gates reach 25/25 exact chunks,
non-empty whole-tree exact/proxy ownership, and all 160 horizon slots at the
fixed low-angle review site. This is the PH-3 reusable-consumer checkpoint;
hosted Human Review 1 accepted it on 2026-07-27. PH-3 is complete, PH-4
shared terrain-view engine extraction plus full-game scene adoption is active
in Tactical
[`269`](../tactical/269-shared-terrain-engine-scene-adoption.md). Tactical
[`274`](../tactical/274-all-client-distant-terrain-control.md) now owns
Android/XR test promotion and remaining real-device evidence. PH-4 has an
implemented ordinary scene path: a source-qualified live adapter derives exact
coverage from the active draw store's traversal-ready columns, exact
opaque/cutout terrain establishes the ordinary reversed-Z depth, and the
shared procedural backdrop
loads and extends the same target before actors and translucent terrain. The
current player-facing control is `Graphics -> Distant Terrain -> Off / Low /
Medium / High`, with `terrainLodQuality` / `--terrain-lod-quality` as the
canonical launch-only inputs. The former `terrainPresentation` /
`--terrain-presentation` inputs remain deprecated Off/High compatibility
aliases. Composition is restricted to local `mclone-overworld-v1`; Off is
allocation-free. A stored non-Off choice reads, for example,
`Medium (Unavailable)` while an incompatible world is active instead of
silently pretending that composition is live. Legacy Exact Only migrates to
explicit Off, while legacy Composed or Experimental migrates to explicit
High.
Native low-angle and elevated captures show the exact foreground silhouette
correctly occluding the surrounding procedural terrain. Native thread and
browser Worker executors now feed the same vegetation coordinator; exact
readiness selects whole authoritative tree records, and ordinary mono plus
preliminary per-eye stereo consume one committed presentation. The browser
tier uses six levels and render stride eight to retain an approximately
eight-kilometre horizon without constructing unused viewport pipeline
families. Headed full-game WebGPU pixels pass. The PH-4 full-game checkpoint
is deployed at
`https://mclone.kzahel.com/app?startInWorld=1&generationProfile=mclone-overworld-v1&terrainPresentation=composed`;
hosted subjective review rejected the overall result on 2026-07-27. The
browser tier removed `63/64` of every tile's ground cells, invalidated the
stride-one normal-halo assumptions, amplified unsown fine/coarse boundaries
into sky-colored wedges, and remained clipped by the near-field game
projection. Native stride-one captures contain smaller blue ring cracks and
are not an accepted quality baseline either. Corrective Tactical
[`271`](../tactical/271-procedural-horizon-quality-baseline.md) now requires
matched ten-level/stride-one Explorer, desktop-game, and browser-game
evidence; shared seam closure and horizon reach precede any performance tier.
Its implementation passed Human Review on 2026-07-27: odd fine outer-edge
vertices meet the adjacent coarse interpolation, normal derivatives consume
that stitched surface, and composed flat views derive a roughly 140-kiloblock
far plane from the resident clipmap while exact-only remains unchanged.
Inspected native
elevated, native low, Explorer, and headed-browser captures show no
sky-colored ring crack or stable tile-lighting grid. The browser reaches the
same ten-level/stride-one pixels but required roughly two minutes to settle on
the validation host, so startup and frame cost remain a separate, now
measurable follow-up rather than justification for an implicit quality fork.
Tactical 274 promotes the control to every shared client menu, persists it
through the native/Android/browser graphics adapters, extends the default
per-eye XR projection to the same clipmap-derived reach, and adds a
target-ready synthetic stereo capture. Flat Android and Android XR APKs,
desktop XR compilation, native flat pixels, synthetic stereo pixels, and
headed browser pixels pass. A physical Quest 3 per-eye session then confirmed
that the full-reach horizon is unusually compelling at altitude and remains
usable through extended 8x-speed travel. The same session exposed a new P0:
after returning to ground level exact terrain stopped catching up, and Horizon
OS killed the process at a `6,621,968 kB` footprint (about `3.86 GB` RSS plus
`2.02 GB` swap). The clipmap remains fixed at 160 resident tiles; the strongest
source-level candidate is unbounded stale load/full-record cache-save ownership
in the persistent exact-world mailbox under continuous movement. Tactical 274
records the device evidence, causal audit, required owned-byte diagnostics,
and persistent-world Quest soak. Physical flat-Android evidence remains open.
Tactical
[`275`](../tactical/275-bounded-persistence-streaming.md) now removes those
known unbounded mailbox mechanisms through lane bounds, cancellation, fair
write progress, residency-linked browser cache eviction, and owned-byte
diagnostics. At that point the device incident remained open pending the same
persistent Quest travel pattern with exact-center reacquisition and
memory/queue plateaus; completed scheduler job-history and physical world-file
growth remained separate audit items.
Tactical
[`277`](../tactical/277-quest-procedural-horizon-performance.md) closes that
device incident and the first headset performance pass. Consecutive
composed-horizon 8x flights over `6,191` and `10,319` blocks retained an
exact-ready current center in every sampled frame, held transient persistence
work below 30 foreground requests / 15 cache requests / about `0.61MiB`, and
kept Quest RSS in a non-monotonic `1.03–1.22GiB` band.

That closeout remains valid for bounded persistence and the RD5 five-minute
lane, but it did not prove every shared exact-world owner under a longer RD7
pressure test. A later planned 20-minute RD7 flight was killed after about
12 minutes and 25 kiloblocks. The procedural horizon remained fixed-residency;
a two-minute diagnostic isolated an unbounded copied-input initial-light
backlog. The incident and vanilla 1.17.1 comparison live in
[`chunk-lighting-admission-and-backpressure.md`](chunk-lighting-admission-and-backpressure.md),
and Tactical
[`279`](../tactical/279-chunk-lighting-admission-and-backpressure.md) now
closes the shared scheduler/light failure and an adjacent unbounded client
publication owner found during device acceptance. The final Quest 3 RD7 8x
flight completed 20 minutes and 41,277 blocks normally, bounded Light
ownership below 10.7 MiB, held the transient client payload backlog below 549
items, and reclaimed memory throughout the run. The complete 225-chunk exact
view converged in `17.772s` after movement stopped, and the pulled 31,960-chunk
SQLite world passed integrity checking. This confirms that the procedural
horizon was the feature that exposed the shared exact-world lifecycle defects,
not their owner, and does not invalidate Tactical 277's horizon render
measurements or Tactical 278's optional multiview experiment.

The same tactical found a concrete horizon render defect: the per-eye path
submitted all 160 resident tiles, including tiles covered by finer levels and
outside the eye frustum. At full quality it ran at `51.91 FPS`, with
`19.157ms` average app work and `11.906ms` Meta app GPU time. Shared
covered/frustum culling reduced the stationary draw to 27–28 terrain tiles,
preserved the accepted synthetic stereo capture byte-for-byte, and restored
`72.01 FPS` with `3.1–3.2ms` average headroom.

Moving tails remain slightly outside the target: composed RD5 orbit repeats
run at `71.81 FPS` with `2.2–2.8%` over-period frames, while an 8x five-minute
flight runs at `71.70 FPS` with `5.6%`. Meta app GPU time is only
`4.75–5.63ms`; duplicate per-eye horizon encoding was therefore a plausible
CPU-side target. The completed experiment now gives the procedural horizon
and proxy vegetation immutable two-eye uniforms, multiview terrain/tree
pipelines, stereo-union admission, and per-layer visibility masks. In the
matched ten-actor Quest RD5 orbit, multiview saves about `2.75ms` average
thread CPU versus dual per-eye but increases app GPU from `7.052ms` to
`9.244ms` and app work from `14.384ms` to `17.053ms`. It remains available
for live diagnostics but is not the default XR renderer. Tactical
[`280`](../tactical/280-xr-multiview-render-path-workstream.md) coordinates
the complete renderer/live-switch/regression workstream. Child Tactical
[`278`](../tactical/278-quest-procedural-horizon-multiview.md) owns the true
two-layer renderer and its alternating Quest comparison.

A later matched Quest 3 RD5 stationary diagnostic at revision `20ec052d`
measured the current direct-smooth full-quality path at `68.75 FPS`,
`14.386 / 15.828ms` average/p95 app work, and `8.058ms` Meta app GPU against
an exact control at `72.01 FPS`, `5.007 / 5.425ms`, and `2.538ms`. The settled
composed frame drew 55 of 160 terrain tiles and 612 proxy trees from 35 tiles;
93 terrain tiles were frustum-culled, 12 were inner-hole-culled, and zero were
fog/far-culled. Average 72-Hz budget is only `0.497ms` away, while p95 needs
`1.939ms`.

Tactical
[`320`](../tactical/320-cross-platform-lod-quality-presets.md) therefore keeps
this shared clipmap and completes player-facing Off/Low/Medium/High bounds
rather than another terrain system. Preset semantics remain identical across
native, Web, Android, and XR clients; only the unset default varies by shared
platform profile. Fog remains an independent optional setting. Low, Medium,
and High use six, eight, and ten terrain levels plus one, two, and four proxy
vegetation levels respectively, all at stride one. Their fixed residency is
96, 128, and 160 tiles.

Matched forward/reverse physical Quest 3 fog-off orbit runs establish Low as
the standalone Android-XR default. Low submitted `71.83 / 71.87 FPS` with
`12.671 / 12.649ms` average app work and `4.8% / 2.0%` over-period frames;
Medium submitted `70.52 / 70.45 FPS` and spent `45.6% / 46.8%` of frames over
budget; High submitted `63.98 / 63.78 FPS` and spent more than 93% over
budget. A no-override rebuilt APK resolved to Low and measured `71.86 FPS`,
`1.246ms` average headroom, and `6.786ms` Meta app GPU against High's
`64.07 FPS`, `-1.561ms`, and `8.185ms`. Native desktop defaults High;
SteamOS, Web, and desktop OpenXR default Medium; flat Android and Android XR
default Low. An explicit stored or launch choice still wins.
Canonical exact generation and live authoritative render sections remain
different truth-source adapters.

That corrective review accepted one additional live-source limitation. The
detached canonical renderer separates exact natural-tree meshes and enforces
whole-record XOR. The game retains natural tree blocks inside ordinary
authoritative chunk meshes, so a proxy-owned frontier record can overlap the
same exact tree and z-fight locally. No renderer workaround landed at
acceptance. The cheap natural LOD is already allowed to ignore persisted edits
outside exact range; later work may compare a slightly inset proxy,
generated-feature mesh partitioning, and sparse record/region invalidation or
storage.
Interactive review on 2026-07-26 diagnosed two remaining proof defects and
activated Tactical
[`252`](../tactical/252-procedural-horizon-seams-and-transition-admission.md):
tile-edge normal calculations clamp to local samples and visibly split
lighting, while aligned multi-level movement can expose requested origins
before all entering strips are ready and show coarse fallback for one frame.
Slice 1 now provides fixed requested/staged/committed admission with seven
guard resources per level. Terrain and asynchronous vegetation retain
separate complete presentations and commit atomically without an uncovered
frame. Slice 2 stores a fixed two-sample normal halo while retaining the
`65x65` drawn grid. Same-LOD borders use identical absolute neighbor samples;
a two-cell fine-ring collar converges to the adjacent coarse normal footprint
at their shared edge. A dedicated scalar `69x69` normal-height field keeps the
semantic sample grid at `65x65`; the combined fixed allocation is
`128,837,720` bytes and does not rely on a larger per-frame dispatch budget.
The later shared-projection matrix adds `29,440` bytes across fixed terrain and
tree uniforms. The two-eye multiview suffix adds another `44,160` bytes across
the same fixed terrain and tree uniform set; the current allocation is
`128,911,864` fixed bytes including the exact-coverage resources.
Native/offscreen plus headed desktop and Pixel 7 browser closeout passed.
Side-by-side native/browser review also found three proof-host parity gaps.
Completed parent Tactical
[`253`](../tactical/253-world-explorer-cross-host-parity.md) sequences their
independent corrections. Tactical
[`255`](../tactical/255-world-explorer-color-output-parity.md) completed the
shared display-space color contract on 2026-07-26. Terrain, material/river
overrides, tree proxies, and the background now use one generated target
transform, preserving the selected dark appearance on UNORM and sRGB targets.
Tactical
[`256`](../tactical/256-shared-horizon-vegetation-worker-topology.md) replaced
native synchronous/browser-disabled tree compilation with one shared
engine terrain-vegetation coordinator over native-thread and browser-Worker
executors. It completed on 2026-07-26 and explicitly defines World Explorer as
the first proof host, not the final owner. Native uses one named
bounded-channel thread; browser uses an isolated Rust actor, the shared opaque
Worker transport, and a persistent external-SAB result arena. The
coordinator, compiler session, job identity, cache policy, and prepared result
handoff belong in shared crates so a later `mclone-scene` tactical can consume
the same service. A pinned native/offscreen/desktop-browser/phone-browser
receipt agreed exactly on the semantic source, `2,609` records/instances,
family counts, record hash, and `281,772` proxy vertices; failure/restart,
overflow, large coordinates, and shutdown also passed. UI-less host cleanup
completed in Tactical
[`254`](../tactical/254-ui-less-world-explorer-host.md) and remains owned by
the platform-host topic rather than terrain rendering.

Tactical 261 is the macro source of truth for past and future work. Its
remaining children are paused behind Tactical 274. Tactical
[`262`](../tactical/262-world-explorer-exact-procedural-composition.md)
defines one renderer-neutral exact-painted snapshot, caller-owned shared
target, coverage-mask lifecycle, and true World Explorer composition before
the first full-game pixels. Later children own `mclone-scene` adoption,
exact/proxy vegetation XOR, world and device lifecycle, browser/Android
promotion, synthetic stereo, desktop OpenXR, Quest, and full-frame multiview.
Completed proof tacticals remain historical records and are not reopened for
that integration.

## Scope

This topic owns the current runtime composition for presenting untouched
natural terrain beyond exact chunks:

- fixed-budget multiscale terrain around a moving observer;
- toroidal geometry-clipmap residency and incremental updates;
- seams between procedural levels;
- exact-chunk replacement of procedural terrain;
- procedural vegetation handoff;
- XR-safe scheduling, coordinate precision, and frame admission; and
- the shared ownership boundary between Terrain Lab and the game.

The terrain field, structured worldgen semantics, and current GPU evidence
remain owned by
[`gpu-procedural-terrain.md`](gpu-procedural-terrain.md). Player-facing map,
orbit, tabletop, touch, and enter-world behavior belong to
[`world-view-navigation.md`](world-view-navigation.md). This topic does not
revive any type or lifecycle from the removed chunk Far LOD.

## Recorded Direction

Use a **toroidal geometry clipmap** as the first in-game procedural-horizon
proof.

Each level has:

- a fixed grid resolution;
- a power-of-two world-space sample spacing;
- a snapped world origin;
- a fixed central hole occupied by the next finer level; and
- fixed GPU storage addressed as a two-dimensional ring buffer.

This gives the runtime a fixed upper bound on resident samples, geometry, draw
count, and per-boundary-crossing updates. Those properties are more important
to the first XR proof than adapting every patch independently to the camera.

A quadtree remains valuable as:

- an adaptive Terrain Lab comparison mode;
- a possible map or tabletop presentation where variable work is acceptable;
- a reference for future terrain-edit summary roll-ups; and
- a later hybrid candidate if measurement identifies waste in fixed rings.

This is a preferred first proof, not a claim that a quadtree can never be used.
The two representations must share terrain evaluation and summary semantics;
they must not become unrelated game and Lab generators.

## Why This Is Not The Removed System

The rejected system retained one identity and lifecycle per 16×16 chunk at
every distance. Reducing vertices inside each chunk did not reduce the number
of covered chunk identities, requests, arbitration decisions, or resident
objects.

A clipmap instead keeps roughly the same number of samples at every level.
Each coarser level increases sample spacing and therefore world-space
footprint:

```text
level 0: N x N samples at spacing S
level 1: N x N samples at spacing 2S
level 2: N x N samples at spacing 4S
...
```

The maximum horizon is consequently bounded by ring count, grid size, and
sample spacing rather than by allocating every chunk beneath the view. A
100–200 km target is a budget and visual-quality question, not a requirement
to create millions of chunk-granular LOD residents.

## Shared Terrain Source

The current reuse is already substantial:

- [`mclone-terrain-view`](../../native/crates/mclone-terrain-view/Cargo.toml)
  depends on `mclone-core`, `mclone-worldgen`, and `wgpu`, and owns terrain
  evaluation, viewport planning, sample buffers, canonical comparison, grid
  rendering, and tree summaries;
- [`mclone-terrain-lab`](../../native/apps/mclone-terrain-lab/Cargo.toml)
  combines that crate with production assets, block catalogue, exact
  generator, mesher, and shared renderer; and
- the Lab's exact pane is therefore a small production render host, not a
  TypeScript imitation of terrain generation.

The Lab is not a complete game engine. It deliberately lacks authoritative
client/server simulation, persistence, propagated world light, collision,
entities, ticks, and protocol lifecycle. React, URLs, Worker orchestration,
diagnostic readbacks, and cheap preview lighting remain host-specific.

The implementation direction is one shared Rust terrain-view service with
multiple consumers:

```text
worldgen semantic source
        |
        v
shared terrain-view / clipmap service
        |
        +-- Terrain Lab and World Explorer
        +-- minimal native renderer diagnostic
        +-- game scene and render session
```

Lab-first means using the Lab as the fastest visual and instrumentation host.
It does not mean landing a second Lab-only clipmap implementation.

Terrain Lab's exact generated-chunk pane has its own
`CanonicalTerrainWorkerCoordinator`. That coordinator is a useful precedent
for isolated Rust actors, cache/session ownership, epochs, bounded admission,
and external-SAB result publication. It is not the procedural LOD service to
move into the game. The shared procedural CPU/GPU products and viewport
contracts are the reusable system.

The full web game's `WebRenderWorkerCoordinator` is likewise a specialized
exact render-section owner, not a universal worker framework. It contributes
generic browser transport and lifecycle evidence, but procedural terrain must
not become an artificial render-section job to reuse it.

Tactical 247 proved the current bounded renderer through both a real native
surface and an offscreen target in the standalone World Explorer. Its
continuous-movement closeout peaked at 171,994,752 resident bytes and 159
tiles, then settled to zero pending work. That is useful portability and
control evidence, but it is not the fixed-memory moving-horizon contract.
The first ring should replace this growing bounded-preview residency in both
proof hosts rather than expand its tile budget.

## Reusable Terrain-LOD Streaming Service

World Explorer proves host portability and remains useful on its own, but it
is not the ownership boundary. Procedural terrain streaming should compose as:

```text
Terrain Lab / World Explorer / Universe preview / game scene
                    |
     shared terrain-LOD coordinator and products
                mclone-terrain-view
                    |
     semantic evaluator and compiler sessions
                 mclone-worldgen
                    |
          native thread | browser Worker
```

The shared service accepts semantic desired tiles and explicit admission
tokens. It owns priority, source identity, budgets, stale rejection, cache
lifecycle, failure state, and prepared products without depending on a WGPU
device, browser API, `winit`, or one app crate. A presentation owner uploads
admitted products afterward.

The crate dependency remains one-way: terrain-view owns
`TerrainViewportTileId`, slot generations, and admission, while worldgen owns
validated compiler requests, semantic source/product revisions, compiler
sessions, and product encoding. Browser correlation fields are fixed-width
opaque scalars at the worldgen codec boundary, not a reason for worldgen to
import terrain-view.

The service API must not bake in World Explorer's current ten-level,
four-by-four default. That configuration is the first measured consumer. The
coordinator derives bounded work from a validated desired set so Terrain Lab,
the Explorer, a later Universe detached-preview presentation, and the game
scene can use different measured ring budgets without different scheduling
semantics.

Platform adapters choose execution mechanics:

- native moves typed jobs and results through bounded channels to one worker
  thread in the first implementation; and
- browser keeps isolated Wasm heaps and publishes encoded results through an
  explicit external `SharedArrayBuffer`.

The same logical service does not require every domain to share one physical
thread or Worker. Exact chunk meshing and procedural vegetation have different
resident state and failure lifecycles. A later measured pool may host multiple
services behind unchanged domain coordinators, but Tactical 256 does not
create a universal job enum or Worker framework.

### One engine with multiple truth sources

The reusable boundary is broader than sharing a clipmap scheduler or an
exact-painted snapshot. Explorer, Terrain Lab runtime composition, the live
game scene, and a future Universe overview should consume one logical terrain
representation engine in `mclone-terrain-view`. That engine owns procedural
residency and admission, exact/procedural coverage and frontier policy,
bounded representation ownership, stale rejection, and immutable prepared
terrain-frame products.

The engine receives exact facts through narrow adapters:

- Explorer and Terrain Lab use a detached canonical source reconstructed from
  a qualified generator recipe and explicitly carry no edit authority.
- `mclone-scene` adapts authoritative client-replica render sections,
  readiness, edits, revisions, and topology while retaining live session and
  frame orchestration.
- A later bounded observer source may consume server-published facts without
  assuming that a seed or generator recipe is available.

The current `TerrainRuntimeExactRenderer` is a transitional proof aggregate.
PH-4 should separate its canonical producer/residency responsibilities from
the generally reusable coordination and draw-preparation path. It must not
become a permanent Explorer-only terrain implementation beside a separate
scene compositor. Equally, the shared-engine requirement does not justify
running `McloneSceneHost`, an integrated server, persistence, or networking in
the lightweight Explorer.

This is logical runtime unification, not a requirement for one physical
thread, Worker, or WGPU allocation. Hosts may choose different view policies,
budgets, and platform executors while consuming the same source-qualified
composition contract. Whether detached and live hosts can safely retain or
transfer GPU resources is a later measured optimization.

The first shared off-thread slice is individual vegetation record planning.
Coarse continuous forest summaries remain part of terrain evaluation and do
not enumerate trees. Terrain evaluation, summary filtering, vegetation
records, and later exact/procedural arbitration remain separable layers behind
one source identity.

## Geometry And Residency

### Fixed nested rings

Every level uses reusable grid topology. The inner level covers the observer;
each outer level draws only the annulus outside the finer level's fixed
footprint. The central holes are part of the mesh/index pattern, not
per-frame Boolean cuts.

Rings should:

- use power-of-two spacing;
- snap origins to their own spacing;
- align coarse and fine sample coordinates where they meet;
- keep a small guard region beyond the visible ring; and
- evaluate terrain from absolute world coordinates, never by stretching
  already-sampled fine data.

The exact grid dimensions and number of levels remain measured settings. The
current Terrain Lab maximum is evidence for broad coverage, not an automatic
100–200 km runtime configuration.

### Two-dimensional toroidal addressing

The GPU allocation stays fixed while its logical world origin advances. A
logical world sample coordinate maps to a physical slot modulo the grid
dimensions:

```text
physical_x = world_sample_x.rem_euclid(grid_width)
physical_z = world_sample_z.rem_euclid(grid_height)
```

The implementation must use Euclidean modulo so negative world coordinates
wrap consistently. A separate snapped logical origin determines which
world-space interval the fixed slots currently represent.

Moving the camera inside a level's sample cell changes no sample residency.
Crossing a snapped boundary exposes only:

- one entering column for movement in X;
- one entering row for movement in Z; or
- one row and one column, with their shared corner generated once, for
  diagonal movement.

The opposite edge is not copied or shifted. Its physical slots are simply
reinterpreted and overwritten as the new entering strip. This is the
two-dimensional extension of an ordinary circular buffer.

### Requested and committed origins

Do not expose a new origin before its entering strips are drawable. Each level
needs explicit state resembling:

```text
resident origin
requested origin
pending strip work
committed origin
source identity and revision
```

Movement requests work against the next snapped origin. Compute and uploads
populate the wrapped entering slots under a bounded budget. Once all required
strips and summaries for that step are ready, the level atomically exposes the
new origin. A guard margin lets ordinary movement remain within already
prepared coverage.

Tactical 252 implements this as 16 logical and seven guard resources
per level. The guard bound is the non-duplicated entering set for a one-tile
diagonal move. Requested origins converge one tile at a time under the
existing dispatch budget, while the committed terrain presentation retains
all 16 old resources until the replacement set is complete. Vegetation owns a
separate committed presentation so Worker latency can retain the prior valid
forest without delaying terrain or exposing a forest-free frame.

A teleport or world switch is different from a one-cell move. Cancel stale
work, establish source identity, refill coarse levels first for immediate
coverage, and progressively admit finer levels. Never let old-source slots
become valid merely because their toroidal indices match.

This still needs a small scheduler or admission queue. It does not need the
old system's area-proportional chunk request graph.

## Level Fidelity

Coarser rings must sample lower-frequency terrain meaningfully. Merely calling
the point evaluator less often can alias coastlines, peaks, water, and
vegetation.

The shared source therefore needs footprint-aware, band-limited summaries
appropriate to each spacing, including eventually:

- representative or conservative terrain height;
- surface material or biome mixture;
- water presence and water surface facts;
- silhouette-preserving extrema where useful; and
- stable vegetation density and instance summaries.

Level selection and ring count should be tied to projected screen-space error,
field bandwidth, and target device budget. They should not be described as
fake chunk render distances.

Large coordinates also require an explicit precision policy. CPU decisions
should retain integer or double-precision world positions. Shaders should use
camera-relative coordinates or a high/low origin decomposition so distant
terrain does not jitter as the observer moves far from world zero.

Tactical
[`250`](../tactical/250-continuous-explorer-presentation-and-cadence.md)
completed the first explicit presentation/residency split on 2026-07-26.
Fractional camera focus and scale update every frame, while toroidal origins
change only at aligned 64-block finest-tile boundaries. Shaders receive a
nearby integer anchor plus a small fractional remainder so camera motion
stays continuous without converting a large absolute `f64` coordinate
directly to `f32`. Terrain and tree vertices share that transform.

Uniform slots grew from 144 to 160 bytes, making the unchanged 160-slot fixed
allocation `86,553,600` bytes. Same-tile two-contact motion changed exact
fractional focus without changing revision 1, 160 initial refills, 10 rebases,
or allocation. Held movement then crossed tile boundaries and produced
bounded entering-strip refills before returning to full readiness. Native,
offscreen, desktop-browser, phone-browser, negative-coordinate,
million-block, and hosted production captures show no unpainted hole,
fine/coarse seam, or terrain/tree separation.

## Seams And Transitions

Skirts are the accepted first seam treatment. Vertical faces are already a
natural part of block terrain, they tolerate imperfect neighboring summaries,
and they avoid making the first proof depend on a complex stitch topology.

The first implementation should combine:

- power-of-two sample alignment;
- a consistent ownership rule at fine/coarse boundaries;
- skirts deep enough to hide expected height disagreement; and
- optional fog or material blending when it improves the horizon.

Crack stitching and geomorphing remain later refinements if measured captures
show skirts are insufficient. Avoid alpha-crossfading two opaque heightfields
at the same depth; it invites overdraw, z-fighting, and double silhouettes.

In a quadtree comparison mode, one active leaf set covers each extent exactly
once. A parent remains visible until all replacing children are ready, then
the parent is removed. The parent is not drawn behind children with several
dynamic holes. Enforce a 2:1 neighboring-level constraint and use the same
skirt policy.

## Exact Terrain Handoff

The game combines two different spatial structures:

- clipmap holes remove portions covered by **finer procedural levels**; and
- one dynamic exact-coverage mask removes portions covered by **real chunks**.

Only the second is scene-dependent.

### One frame snapshot

The exact draw list and procedural coverage mask must be derived from the same
immutable frame snapshot. A chunk becomes `exact-painted` only when all
resources needed to draw it this frame are ready. Generated, loaded, resident,
or compiling is not enough.

The scene publishes a small world-space coverage texture, bitset, or
`R8Uint`-like mask around the exact frontier. Procedural fragments convert
world XZ to the same chunk coordinates and discard when the mask says exact
terrain owns that position. CPU planning may omit whole procedural patches
that are completely covered; the mask handles the ragged boundary.

This is cheap relative to terrain shading because it is a small indexed lookup
and branch, not mesh/mesh intersection or polygon clipping. It must still be
measured on mobile and XR GPUs.

Depth testing alone is not a correct arbitration mechanism. Approximate
terrain can be above the exact surface, can z-fight where close, and can
incorrectly hide caves or edited silhouettes.

### Draw order

The intended opaque composition is:

1. sky and background;
2. masked procedural terrain;
3. exact opaque and cutout terrain using the same depth convention;
4. actors and vegetation; and
5. translucent terrain and water.

Exact terrain and water use full-footprint procedural discard. A bounded
connector on the procedural side covers solid boundary-height disagreement;
ordinary procedural fragments inside exact-owned land are discarded. Exact
translucent water is the only water owner inside painted chunks. Procedural
water begins outside that footprint and approaches the exact active-pack water
response only through a bounded eight-block appearance halo. The halo changes
neither geometry nor depth ownership.

### Vegetation

Far trees and other terrain-native proxies use the same procedural source and
coverage snapshot. A proxy is removed only when all exact chunks intersecting
its footprint are drawable in the replacing exact representation. An
owner-chunk shortcut is incorrect for trees that cross chunk boundaries.

Tree proxy removal and exact tree admission should be atomic from the
observer's perspective. Later density or clustered representations may change
by level, but their placement identity must remain stable enough to avoid
sparkling during movement.

Detached canonical composition implements that atomic handoff by separating
exact tree meshes. The accepted PH-4 live adapter currently suppresses only
the proxy side: its ordinary chunk mesh may retain exact tree blocks while the
record remains proxy-owned. This localized exception is recorded rather than
mistaken for completed live-tree XOR.

## Natural Terrain Only In The First System

The first procedural horizon intentionally represents untouched natural
terrain only.

When an edited exact chunk is drawable, its real blocks appear. When it leaves
the exact range, the procedural natural surface returns. A distant tower may
therefore pop out or disappear. That limitation is acceptable because the
first product purpose is the wow factor of exploring broad natural terrain.

Do not add speculative edit listeners, dirty ancestor propagation, persistent
LOD databases, distant build silhouettes, or structure-proxy APIs to the first
implementation.

A future edit system can be modeled separately as:

```text
procedural natural base
        +
sparse authoritative edit / structure overlay
```

A quadtree summary roll-up is a plausible future mechanism: mark an edited
leaf dirty, recompute its compact summary, and propagate changed summaries to
parents. Tall buildings may be better represented by explicit sparse
structure silhouettes than by raising a terrain heightfield. Those are future
research questions and must not block natural-terrain proof.

Tactical
[`305`](../tactical/305-fine-homestead-lod-overlay.md) now records the first
explicitly accepted, bounded exception to the implemented natural-only state:
a block-resolution presentation patch derived from the persisted starter
homestead plan, plus stable full-detail landmark records, admitted only through
spacing-`1`, spacing-`2`, and spacing-`4` levels. It keeps the rings unchanged
and resolves the patch while intersecting tiles refill. It is planned, not yet
implemented.

That tactical intentionally does not begin the generic edit system described
above. It accepts the initial plan as its distant baseline, lets exact-painted
chunks show authoritative edits, and allows the baseline to return after an
edited chunk leaves exact range. Per-edit invalidation, dirty summary
propagation, persistence, world-scale indexing, and remote publication remain
future generic-overlay concerns rather than homestead-specific machinery.

## XR And Frame Predictability

The procedural horizon is view-independent world geometry and is generated
once per frame state, not once per eye. Both XR eyes consume the same committed
ring set with their own view/projection data. Every renderer addition must
support normal mono/per-eye rendering and full-frame multiview, or explicitly
document an unavailable mode.

The live player setting currently supports normal mono and per-eye XR. The
alternative full-frame XR multiview topology explicitly advertises Terrain
Horizon as unavailable and projects the scene to exact-only; it does not
accept a composed choice that would silently omit the horizon.

Residency should be centered on a locomotion/body anchor or stabilized
head-space anchor. Raw per-eye positions and normal head wobble must not
request entering strips.

Frame admission needs explicit budgets for:

- compute evaluation;
- summary and vegetation work;
- GPU upload or buffer writes;
- origin commits; and
- device-loss rebuild.

The system should prefer holding the previous valid coverage over exposing a
partially updated ring. Coarse-first recovery is more useful than isolated fine
patches.

## Shared Ownership

The intended ownership split is:

- `mclone-worldgen`: semantic terrain source, profiles, source identity, and
  footprint-aware evaluators, plus stateful vegetation compiler sessions and
  bounded semantic caches;
- `mclone-terrain-view`: clipmap geometry math, snapped/toroidal addressing,
  sample planning, procedural summaries, WGPU-independent terrain-product
  coordination, stale/admission policy, and a renderer-neutral prepared draw
  service;
- `mclone-render`: reversed-Z-compatible terrain, fog, material, vegetation,
  mono/per-eye, and multiview pipelines;
- `mclone-render-session`: resident GPU lifecycle, pending/committed origins,
  device rebuild, and bounded upload/compute admission;
- `mclone-scene`: exact/procedural arbitration, frame snapshot, world
  switching, locomotion anchor, and cross-feature budgets; and
- app hosts: surface/session cadence, platform events, diagnostics, and
  presentation only.

Exact ownership may move as shared contracts become concrete. The invariant is
that no desktop, browser, Android, or XR app owns terrain semantics or a
private LOD policy.

## Work Streams

The work should proceed as independently reviewable slices:

Tactical
[`247`](../tactical/247-standalone-world-explorer-foundation.md) proved
that `mclone-terrain-view` and platform-neutral navigation compose into a small
native application. Tactical
[`249`](../tactical/249-cross-platform-procedural-horizon-proof.md) owns Steps
2–4 as a cross-platform Explorer proof. It keeps the system in shared Rust and
requires native and browser evidence rather than making either platform the
semantic owner.

1. **Protect the old-system removal boundary.** Finish any branch-local
   cleanup before starting the proof, keep Tactical 245 closed, and do not
   retain compatibility types for an unimplemented replacement.
2. **Prove toroidal addressing — complete in Tactical 249.** Shared tests
   cover negative coordinates, rows, columns, diagonal movement, long wrapping
   walks, retained slots, and teleports.
3. **Draw nested rings in the native and browser Explorer — complete.** Reuse
   the current evaluator and shared renderer while leaving Terrain Lab
   unchanged. The browser adapter must remain suitable for either this small
   product or the full web game.
4. **Promote the renderer contract — first proof complete.** Reversed-Z,
   caller-owned targets, and fixed-capacity diagnostics are proven across
   native and browser. Device rebuild, synthetic stereo, and multiview remain
   hardening work; the Explorer is a cross-platform proof host, not another
   disposable diagnostic.
5. **Harden nested levels.** Fixed aligned holes, coarse-first refill, and the
   first large-coordinate camera-relative precision path are proven. Add
   explicit skirts where independent surfaces require them, retained
   committed origins, footprint summaries, and device-specific budgets.
6. **Build the reusable vegetation streaming service — complete.** Tactical
   256 moved
   semantic tree-record compilation behind one shared coordinator, one native
   thread executor, and one isolated browser Rust actor. World Explorer proves
   identical source identity, records, failure behavior, and presentation
   without becoming the service owner.
7. **Prove exact/procedural composition, then integrate the game scene.**
   Tactical 262 first combines a reusable canonical exact view and the horizon
   on one World Explorer target. Tactical 261 then sequences `mclone-scene`
   adoption, proxy/exact vegetation XOR, edit invalidation, lifecycle
   recovery, and all-target frame admission.
8. **Measure an adaptive comparator only if useful.** A quadtree Lab mode
   should answer a specific waste or quality question, not fork the content
   system.

Shared navigation and the World Explorer product can advance alongside these
slices through [`world-view-navigation.md`](world-view-navigation.md), but
neither should force clipmap ownership into UI code.

## Validation

Each rendered slice requires an inspected capture at its first drawable
milestone. The eventual acceptance set includes:

- stationary, single-axis, diagonal, high-speed, and teleport movement;
- negative coordinates and large distances from world zero;
- no unpainted holes during delayed strip generation;
- source/profile/world switches with stale completion rejection;
- ring seams across coast, mountain, water, and forest cases;
- exact-chunk admission, eviction, edits, and vegetation crossing the mask;
- persistent-world high-speed travel where stale load/save work is cancelled
  or bounded, current-center exact terrain reacquires, and RSS plateaus;
- device loss and surface rebuild;
- desktop and headed-Wayland browser evidence;
- flat Android and Android XR scripted build/validation lanes;
- mono, synthetic stereo, per-eye XR, and full-frame multiview;
- bounded work and stable frame-time evidence under continuous locomotion; and
- a protected feature-off ordinary exact-terrain path.

Queue emptiness, requested origins, or a color-only screenshot are not
sufficient evidence. Coverage, depth, work counts, and committed-source
identity must be observable.

## Flight-Simulator Reference Posture

Microsoft Flight Simulator is a useful experience and progressive-delivery
reference, but not the intended Mclone storage architecture. Microsoft's
official material describes cloud-streamed world data and a rolling cache;
Mclone can reconstruct the untouched natural base from a compact world seed
and generator revision.

Reuse the product lessons—coarse-first coverage, progressive refinement,
predictive movement, and optional caching—without assuming a planet-scale
stored imagery pipeline. See the official
[Microsoft Flight Simulator 2024 FAQ](https://www.flightsimulator.com/microsoft-flight-simulator-2024-faq/)
and
[release/preorder description](https://www.flightsimulator.com/msfs2024-preorder-now-available/).

## Open Questions

- What grid dimension, level count, guard size, and update budget give useful
  horizons on desktop, browser, phone, and Quest?
- Which footprint summaries best preserve coastlines, mountain silhouettes,
  water, and forest character at each level?
- Should samples live as height/material textures, structured buffers, or
  generated vertex data on each backend?
- Does the first ring renderer belong directly in `mclone-render`, or should
  `mclone-terrain-view` initially own a narrowly reusable WGPU presentation
  while the contract settles?
- How should water surfaces transition without slopes or shoreline flicker?
- When are skirts insufficient enough to justify stitching or geomorphing?
- What is the smallest exact-coverage representation that remains cheap and
  correct at the frontier?
- How much compute actually overlaps rendering on each `wgpu` backend?
- Which source descriptor can a remote server expose when its seed or
  generator details are private?

## Related Documents

- [`lod.md`](lod.md) — canonical LOD terminology, ownership, and routing.
- [`universe-product-shell.md`](universe-product-shell.md) — detached-preview
  product role, authority transitions, and preview-truth requirements.
- [`gpu-procedural-terrain.md`](gpu-procedural-terrain.md) — terrain
  evaluation, Terrain Lab evidence, exact comparison, and GPU research.
- [`far-lod.md`](far-lod.md) — rejected chunk system and removal boundary.
- [`lod-native-vegetation.md`](lod-native-vegetation.md) — forest intent,
  stable tree records, exact realization, and procedural summaries.
- [`procedural-horizon-surface-appearance.md`](procedural-horizon-surface-appearance.md)
  — material classification, block-atlas texture, biome color, approximate
  lighting, inland-water presentation, and measured quality boundaries.
- [`world-view-navigation.md`](world-view-navigation.md) — shared map/orbit
  controls and Explorer-to-play product path.
- [`tabletop-overview-mode.md`](tabletop-overview-mode.md) — active-world scale
  model and scene-owned overview policy.
- [`performance.md`](performance.md) — budgets, profiling, and regression
  evidence.
- [`platform-parity.md`](platform-parity.md) — all-target ownership and
  validation.
- [`../native-engine-architecture.md`](../native-engine-architecture.md) —
  shared renderer, scene, and host boundaries.
- [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md) — frame
  work admission and accounting.
