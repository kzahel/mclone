# Tactical 271: Procedural Horizon Quality Baseline

Status: complete; hosted Human Review accepted 2026-07-27 with the live-tree
overlap and other limitations below. Corrective child of PH-4 Tactical
[`269`](269-shared-terrain-engine-scene-adoption.md) under coordinating
Tactical [`261`](261-procedural-horizon-product-integration-roadmap.md).

Topic: `procedural-horizon-clipmap`

## Objective

Establish one platform-independent procedural-horizon visual baseline before
performing any browser performance optimization:

1. render the same seed, center, eye, target, viewport, clipmap, color profile,
   and settled content in World Explorer, the desktop game, and the browser
   game;
2. remove positional and lighting seams at the shared full-quality
   `10`-level, render-stride-`1` baseline;
3. make composed scene projection reach the resident horizon rather than
   clipping it at the exact-terrain camera distance; and
4. deliver matched native and headed-browser screenshots for human review
   without requiring the reviewer to run a desktop application.

Startup latency and steady-state frame cost are important but separate
acceptance dimensions. A slow correct browser frame is valid evidence for this
tactical. Later optimization must compare every proposed quality tier against
the accepted baseline rather than redefining correctness until it runs.

## Review Finding

The hosted PH-4 checkpoint was rejected on 2026-07-27.

The browser game used six clipmap levels and render stride eight while World
Explorer and native game paths used ten levels and stride one. That is not a
small device budget: it reduces each tile from `64x64` to `8x8` rendered cells
and removes `63/64` of its ground cells. It also invalidates assumptions in
the fixed two-sample normal halo and magnifies independent fine/coarse
heightfield disagreement into large sky-colored wedges.

Reinspection of the native PH-4 captures found the same positional defect as
thin blue slivers at several fine/coarse boundaries. Native quality is much
better, but it is not an accepted baseline. Browser stride eight amplified a
shared missing-stitch/skirt defect; it did not create the entire problem.

The prior evidence answered narrower questions:

- exact and procedural terrain used comparable reversed-Z depth;
- near exact trees and hills preserved their silhouettes;
- complete natural-tree records selected one exact-or-proxy owner; and
- preliminary stereo drew both eyes.

It did not compare overall visual quality under matched host inputs. The final
browser proof image itself contained visible sky gaps and should have blocked
the checkpoint.

## Binding Decisions

- The quality reference is ten clipmap levels and render stride one on every
  host. Browser performance does not authorize an implicit visual fork.
- A render stride is a topology and filtering contract, not merely a vertex
  budget. Any future stride above one must prove position stitching, normal
  footprints, material interpolation, and transition coverage explicitly.
- Fine/coarse boundaries must meet geometrically. The preferred first
  implementation welds each fine outer edge to the adjacent coarse edge
  interpolation. A bounded skirt may be added only where independent
  representation boundaries still need coverage.
- Same-LOD and fine/coarse normal calculations must be derived from the
  geometry actually drawn. Static tests must reject a halo/stride combination
  that cannot supply its selected footprint.
- Composed rendering continues to use one projection and one reversed-Z depth
  target. The scene raises that shared projection's far reach when the horizon
  is active; the horizon must not use an incomparable private projection.
- World Explorer remains a lightweight host and the game remains authoritative
  for exact chunks, lighting, edits, actors, and lifecycle. Matched horizon
  pixels test their shared renderer; production-composed pixels test the
  intended host differences.
- Human desktop review is screenshot-based. Interactive review remains hosted
  web because the reviewer is frequently away from a desktop.

## Evidence Matrix

Every comparison uses a recorded seed, absolute center, eye, target,
resolution, FOV, color profile, clipmap configuration, and settled-state
receipt.

| Comparison | Purpose |
| --- | --- |
| Native World Explorer horizon vs native game horizon diagnostic | Isolate shared geometry, ring coverage, projection, and lighting from exact composition. |
| Native game exact-only vs composed | Preserve ordinary exact pixels while reviewing the frontier and horizon. |
| Native game vs headed browser game at `10/1` | Prove platform parity before optimization. |
| Full-quality browser vs any later optimized tier | Measure visual degradation rather than hiding it in a platform default. |

Two image families are required:

1. a high-contrast diagnostic view that makes sky holes, ring boundaries, and
   normal discontinuities obvious; and
2. the production-lit composed view used for subjective believability.

The first camera is elevated enough to cross several ring boundaries. The
second is low enough that a foreground exact tree or hill, fine/coarse
silhouettes, and the distant horizon are simultaneously visible.

## Slices

### Slice 0: status and executable comparison contract

Status: complete 2026-07-27.

- Mark the hosted PH-4 visual checkpoint rejected.
- Add deterministic matched camera/configuration facts to the Explorer and
  game capture lanes.
- Capture the current `10/1` defect before changing seam behavior.
- Keep browser startup and frame timing observable without using them as a
  quality gate.

### Slice 1: shared geometric seam closure

Status: complete 2026-07-27.

- Make fine outer-edge vertices meet the adjacent coarse edge interpolation,
  including corners and negative coordinates.
- Preserve exact absolute sample identity at coincident coarse vertices.
- Add static geometry tests and an inspected multi-ring diagnostic capture.
- Retain bounded clipmap work, toroidal residency, and atomic level admission.

### Slice 2: lighting and production composition

Status: complete 2026-07-27.

- Derive normal footprints from the stitched geometry and enforce the
  halo/stride invariant.
- Remove stable tile and fine/coarse lighting grids at stride one.
- Preserve exact/procedural source ownership and the known narrow frontier
  overlap until a dedicated exact-frontier treatment replaces it.

### Slice 3: shared horizon reach

Status: complete 2026-07-27.

- Derive a composed-scene far distance from the resident clipmap bounds.
- Apply it to the one shared exact/procedural projection.
- Prove that exact-only retains its current camera reach and allocation-free
  horizon path.

### Slice 4: cross-host quality checkpoint

Status: complete; Human Review accepted 2026-07-27.

- Use ten levels and stride one in the headed browser game even if it is slow.
- Capture and inspect the complete evidence matrix.
- Publish desktop images for phone-accessible human review.
- Stop before performance tuning.

## Implementation And Evidence

The corrective series is:

- `5b26107c` — record the rejected PH-4 checkpoint and bind one shared
  quality tier;
- `9846f2e8` — weld odd fine outer-edge vertices to the adjacent coarse
  interpolation, restore every host to ten levels/stride one, and reject
  unproven larger strides;
- `ec8dfb66` — derive the composed flat-view far plane from the coarsest
  resident ring while leaving exact-only projection unchanged;
- `2aee0fef` — route narrow and wide normal derivative taps through the same
  stitched height function used by vertex positions; and
- `c3bbc0e9` — give only the explicit full-quality browser capture a longer
  warmup/observation window.

The binding game comparison uses profile `mclone-overworld-v1`, seed `12345`,
viewport `1280x720`, eye `(8,105,8)`, target `(8,72,-300)`, render distance
three, ten clipmap levels, render stride one, and settled vegetation. The
elevated diagnostic uses eye `(8,220,8)` and target `(8,70,-300)`; its receipt
reports zero drawn exact sections, so every visible surface defect belongs to
the procedural renderer.

Inspected native captures:

- `/tmp/mclone-t271-native-quality-final-elevated.png` — isolated procedural
  rings with the full resident reach, no sky-colored fine/coarse crack, and no
  stable rectangular lighting boundary;
- `/tmp/mclone-t271-native-quality-final-low.png` — 27 drawn exact sections
  correctly occluding and joining the procedural background; and
- `/tmp/mclone-t271-native-exact-only.png` — the same low camera with the
  ordinary exact-only foreground and no procedural allocation/draw path.

The comparable native Explorer capture is
`/tmp/mclone-t271-explorer-quality-final.png`: seed `12345`, center
`(8,-300)`, `1280x720`, ten levels, 160 ready slots, 48/48 vegetation
products, `128,867,704` fixed resident bytes, and target-ready in `510.47 ms`.
Its orbit inputs (`310` blocks, yaw `-pi/2`, pitch `0.453`) reproduce the game
camera line. Explorer deliberately retains its 58-degree orbit FOV,
sea-level target, and black clear color; those proof-host differences prevent
a literal whole-image diff but do not change the shared terrain geometry,
lighting, residency, or vegetation implementation.

The final headed-Wayland browser capture is
`/tmp/mclone-t271-browser-quality-final.png`. It uses the exact low game
camera above and reports 49 exact columns, ten drawn levels, 160 drawn tiles,
809 tree instances, target-ready true, and 48/48 completed vegetation jobs.
The matching native and browser terrain is visually the same; the browser
image retains the ordinary crosshair/HUD. The browser needed roughly two
minutes to produce a fully settled correctness frame on this host. That is
explicit evidence for the separate performance campaign, not an accepted
shipping startup time.

The phone-friendly four-image review sheet is
`/tmp/mclone-t271-quality-review-sheet.png`.

Known limitations at this checkpoint:

- the previously accepted narrow exact/procedural frontier can still
  z-fight on the outermost exact blocks; a later skirt/frontier treatment owns
  that issue;
- a proxy-owned natural-tree record can overlap the same tree already present
  in the live game's ordinary exact chunk mesh, producing localized
  leaf/crown z-fighting. Detached canonical composition separates exact tree
  meshes and does not have this gap. The live-game overlap is provisionally
  accepted and recorded in
  [`lod-native-vegetation.md`](../topics/lod-native-vegetation.md); slightly
  shrinking proxy crown width/depth is a possible rough mitigation, while
  generated-feature partitioning or edit-aware LOD invalidation are later
  correctness work;
- browser full-quality startup and frame cost are not yet acceptable; and
- Explorer and game share the terrain representation engine but intentionally
  retain different product cameras, backgrounds, and exact-world ownership.

## Human Review Result

Hosted phone review accepted the corrected quality baseline on 2026-07-27.
The reviewer found the terrain, collar, horizon reach, lighting, and shared
quality tier convincing overall. The remaining visible tree defect is the
known live-source limitation above: the procedural proxy and exact chunk tree
can occupy the same stable occurrence at the frontier. No renderer workaround
was added at closeout.

## Separate Performance Follow-Up

Phone review reports roughly twenty seconds before the hosted game becomes
interactive, with a much shorter desktop startup. This tactical records but
does not optimize that symptom.

The later performance campaign must first compare exact-only and composed
startup, then attribute:

- Wasm, JavaScript, and asset bootstrap;
- integrated-server generation and lighting;
- exact section compile/upload;
- procedural clipmap compute/admission;
- vegetation Worker startup and product compilation; and
- readiness policy that blocks interaction.

Frustum culling, batching, visibility-aware admission, progressive coarse
coverage, and startup scheduling are candidate optimizations. Uniformly
deleting terrain cells is not an acceptable first response.

## Acceptance

- No sky-colored crack is visible at a fine/coarse boundary in elevated or
  low-angle full-quality captures.
- Same-LOD tile borders and fine/coarse transitions have no stable lighting
  grid under the production directional preview.
- Matched Explorer and game horizon diagnostics agree under the same
  configuration, allowing only explicitly recorded host target/color
  differences.
- Native and headed-browser game captures use ten levels, stride one, and the
  same camera facts.
- The composed camera shows the resident horizon instead of clipping near the
  exact render distance.
- Exact-only pixels and zero-horizon-work behavior remain protected.
- The human checkpoint includes all desktop and browser images plus known
  limitations; no desktop interaction is required.
