# Tactical 304: LOD Frontier and Near-Field Voxel Convergence

Status: implemented 2026-08-15; Human Review pending

Topic: `procedural-horizon-clipmap`

Topic: `procedural-horizon-surface-appearance`

## Instruction Synthesis

Correct three conspicuous weaknesses in the current composed terrain:

1. the outermost exact blocks can overlap near-coincident procedural terrain
   and z-fight;
2. exact chunks and the procedural horizon use visibly different color and
   lighting languages; and
3. the smooth heightfield has a different character from the block terrain it
   replaces, especially near the exact frontier.

Keep the fixed-budget toroidal procedural horizon as the LOD system. Give its
closest representation a deliberately block-like visible surface, including
flat tops, vertical exposed faces, and plausible side strata such as a grass
top over grass-block side, dirt, and then stone. Preserve a progressively
smoother distant horizon instead of extending full exact chunk meshing across
the view or reviving the retired chunk-based Far LOD.

This is an implementation and visual-acceptance tactical. It must land through
the shared `mclone-terrain-view`, `mclone-worldgen`, `mclone-render`, and
`mclone-scene` contracts and serve native, browser, flat Android, and XR hosts
without a platform-private terrain policy.

## Starting Point

The current system already has the right large-scale ownership shape:

- one ten-level, four-by-four-tile-per-level toroidal clipmap with a finest
  sample spacing of one block;
- one renderer-neutral exact-painted snapshot and bounded GPU coverage mask;
- one shared projection and reversed-Z depth convention for exact and
  procedural terrain;
- committed fine/coarse geometry stitching, normal halos, and atomic terrain
  admission;
- one active block atlas with mip generation and raw-material-to-sprite
  bindings; and
- whole-record procedural vegetation ownership over the detached canonical
  proof, with the separately recorded live authoritative-tree limitation.

The remaining frontier defect is intentional in the current shader. An
exact-painted chunk discards procedural fragments only in its interior. A
`1.5`-block strip remains around any side adjacent to an unpainted chunk. That
strip replaced the earlier open crack and full-height footprint wall, but it
also leaves two opaque horizontal surfaces over the outermost exact blocks.
Where exact and procedural heights nearly agree, depth correctly exposes
z-fighting rather than establishing representation ownership.

Appearance has a similarly explicit split. Exact blocks use baked
direction-specific model faces, active-pack biome tint, per-face shade and
ambient occlusion, stored block/sky light, and the shared time-of-day lightmap.
The horizon selects one upward or representative sprite per raw material,
multiplies a renderer-authored material/biome base color by a smooth
heightfield directional light, and blends atlas detail at a distance-dependent
weight. The output transfer and fog are shared, but the material and lighting
inputs are not. Tuning a few RGB constants cannot close that gap.

The finest procedural level already evaluates a one-block lattice. The missing
block character is therefore primarily a presentation/topology problem, not a
request for denser canonical generation. The packed sample also already
carries display height, visible material, biome recipe, and surface recipe.
Those facts are a useful starting point for a visible column shell, although
worldgen must add a compact side-strata summary if the existing fields cannot
express the canonical surface recipe without renderer-owned guesses.

## Objective

Produce one composed terrain presentation in which:

- every horizontal location has one opaque terrain owner;
- height disagreement at the exact frontier is covered by explicit,
  diagnosable connector geometry rather than opaque horizontal overlap;
- the closest procedural ring reads as a bounded block surface with horizontal
  tops and vertical risers;
- top, side, subsurface, biome-tint, face-shade, and daylight behavior converge
  on the exact block renderer where the representations meet;
- farther rings retain the current broad, smooth, fixed-budget horizon; and
- exact-only behavior and allocation remain unchanged.

The result need not make approximate natural terrain identical to canonical
chunks. It must make the change of representation coherent, stable, and
visually intentional.

## Binding Decisions

### One horizontal owner, plus explicit frontier geometry

In composed mode, the procedural horizontal surface must be discarded across
the complete exact-painted footprint. Do not leave a horizontal procedural
collar over exact blocks.

Extract exposed exact/procedural boundary edges from the same immutable
coverage snapshot that selects exact draw readiness. Corners, holes, negative
coordinates, admission, eviction, movement, teleport, and world switches must
derive from that snapshot rather than from an independently updated mesh list.

At each exposed boundary segment, compare the exact surface profile with the
committed procedural profile and emit an explicit block-aligned vertical or
stepped connector:

- when exact terrain is higher, expose the exact column's side profile;
- when procedural terrain is higher, expose the procedural column's side
  profile; and
- when heights agree, emit no coincident face.

A bounded downward skirt is an acceptable fallback where one side's complete
height is unavailable, but its ownership must be explicit. Do not restore a
full-height footprint wall or allow stale source generations to meet.

Polygon offset, a constant Y sink, depth bias, alpha blending, dithered dual
surfaces, and draw order may be useful diagnostics, but they are not accepted
solutions. They hide rather than resolve terrain ownership.

### Keep exact coverage and vegetation coherent

Exact draw readiness, full-footprint procedural discard, the connector, and
exact/proxy vegetation arbitration must agree on source and generation.
Removing the terrain collar changes the exact-safe interior used by whole-tree
ownership; update that contract deliberately rather than only changing WGSL.

Detached canonical composition must retain complete-tree XOR. The live game's
ordinary chunk meshes still contain natural tree blocks and have the separate
known frontier proxy overlap. This tactical must not broaden that limitation
or mistake terrain seam closure for generated-feature partitioning.

### Use a surface voxel shell, not more exact chunks

The first block-character implementation applies only to the finest
spacing-one procedural level. Each one-block horizontal cell emits:

- one flat top quad at its selected integer display height; and
- only those north, south, east, or west riser faces exposed by a neighboring
  height difference.

It emits no underground volume, bottom faces, internal column faces, caves, or
one object per block. It remains a derived clipmap-tile product and follows the
requested/staged/committed lifecycle. It does not request, persist, mesh, or
retain canonical chunks beyond exact range.

Use the existing `65x65` sample and halo facts to establish stable `64x64`
cell ownership at tile edges. Pin which absolute sample owns each cell for
negative coordinates and periodic topologies. Adjacent tiles must choose
identical height and material facts for a shared boundary column.

The second-finest level remains smooth in the first accepted candidate. A
second blocky level requires separate pixel and GPU evidence. Do not turn
spacing-two and coarser samples into ever-larger fake blocks by default.

### Side strata are worldgen semantics

A vertical riser is more than a stretched top texture. It needs a bounded
visible column profile. For example:

```text
grass-block top
grass-block side
dirt subsurface
dirt subsurface
stone body for any deeper exposed run
```

Sand, gravel, snow cover, river bank, wetland bed, eroded slope, rocky coast,
and exposed stone need similarly stable profiles derived from their ordinary
surface recipe. Texture repetition may cover a multi-block run, but material
identity changes at the declared stratum boundary.

First determine whether `visible_surface_material`, `surface_recipe`, and the
existing terrain fields express the profile without ambiguity. If they do,
add a shared worldgen helper used by CPU and GPU preview paths. If not, revise
the fixed preview-sample schema and both evaluators together. Do not encode
canonical surface-writing policy as raw-id conditionals in WGSL.

The profile remains approximate untouched-natural-terrain data. It is not a
persisted column cache or edit summary.

### Reuse direction-specific active-pack materials

Extend the horizon from one `gui_icon_uv` rectangle to a compact face-aware
contract distinguishing upward surface, cardinal side, subsurface/body, and
water presentation. Resolve sprites from the same baked active-pack catalogue
as exact meshing.

A grass top uses the pack's upward grass texture and biome tint. Its upper
riser uses the grass block's side presentation. Deeper strata use dirt and
stone rather than vertically stretched grass.

At near scale, use exact-style atlas texel multiplication as the baseline. Do
not retain an arbitrary synthetic base-color contribution merely to make LOD
look different. Far levels may reduce texture contrast and use mip-filtered
representative color when measured aliasing requires it.

### Converge biome tint and light before tuning RGB constants

Load the active pack's biome color maps into the shared horizon contract.
Compare exact five-by-five biome tint with a cheaper near-LOD approximation
from the spacing-one neighborhood. It must be stable across tile borders and
periodic topology. Far levels may use a footprint tint summary.

The near shell uses the exact chunk lightmap curve and current scene
sky-darken/time-of-day input. Exposed untouched tops may assume full sky light
and zero block light unless a compact semantic fact proves otherwise.
Cardinal risers use the ordinary block face-direction shade convention. Add
bounded height-neighborhood AO only after the unoccluded formula matches.

Smooth farther rings may retain slope-derived directional lighting. Transition
lighting parameters by committed level or distance; never crossfade two opaque
geometries. Fog, target transfer, and per-view camera data remain shared.

### Preserve a single-owner blocky-to-smooth transition

Changing the finest topology invalidates the assumption that its outer edge is
another triangulated heightfield. The near level owns horizontal cells through
its declared edge; the coarser smooth level begins outside it. A short vertical
or stepped curtain covers their height disagreement from the same committed
level snapshot.

Power-of-two alignment and coarse-edge interpolation remain useful, but do not
authorize horizontal overlap. The transition follows clipmap geometry rather
than gaining private camera-local residency.

### Keep water and volumetric terrain bounded

Sampled water may use a flat block-aligned top in the near shell. Existing
analytic river and wetland-pool coverage may remain a bounded color overlay if
it stays coherent with the new top geometry. Independent flat water sheets,
decoration-lake summaries, translucent-water parity, caves, overhangs, arches,
and sparse volumetric terrain remain separate contracts.

If blockification makes analytic water visibly worse, route that result to the
water-geometry follow-up instead of expanding this tactical into hydrology.

### Bound topology and GPU cost

Do not permanently emit four potential side quads for every cell without
measurement. Prefer a compact top/riser product prepared when a tile changes,
reused across frames and views, and admitted with the tile. A fixed
degenerate-face shader proof is a useful first drawable milestone but not
automatically the production topology.

Keep compaction in the shared terrain-view service. The live-game exact-only
session must allocate, prepare, and draw no horizon resources; the switchable
World Explorer proof host may retain its existing ready horizon. Record changes
to the current `160` terrain slots and approximately `129 MB` fixed allocation.

Every new seam and terrain pipeline must support mono, stereo/per-eye, and
full-frame multiview. No near-field feature may disappear in an XR topology.

## Non-Goals

- Increasing exact chunk radius as the primary solution.
- Reintroducing `LodTileKey`, chunk-granular approximate residents, or a
  persisted distant chunk mesh cache.
- Representing caves, overhangs, arbitrary edits, structures, or distant
  player builds.
- Changing canonical generation or persisted world identity for LOD pixels.
- Replacing the toroidal clipmap or its admission model.
- Alpha-fading exact and procedural opaque surfaces.
- Solving the live authoritative-tree mesh-partitioning limitation.
- Adding independent procedural water sheets or a hydrology rewrite.
- Promoting Composed to a new default as part of this visual correction.

## Slice 0: Diagnostic Baseline and Contract Fixtures

Status: completed 2026-08-15.

- Capture matched exact-only, Horizon, Composed, Coverage, and source-colored
  low-grazing views where outer exact blocks currently z-fight.
- Record exact and procedural boundary height, visible material, biome/tint,
  atlas face, light input, and ownership generation for selected columns.
- Add a focused render fixture with exact and LOD faces under identical atlas,
  tint, fog-off, target-transfer, and full-sky inputs.
- Add coverage-boundary fixtures for straight edges, corners, holes, negative
  coordinates, delayed admission, and eviction.
- Record allocation, refill work, CPU frame time, and GPU time before adding
  geometry.

Gate: evidence distinguishes horizontal overlap, height disagreement,
material/tint disagreement, and light disagreement. A screenshot alone is not
sufficient diagnosis.

## Slice 1: Full Exact Ownership and Frontier Connector

Status: completed 2026-08-15.

- Discard procedural horizontal terrain over the complete exact footprint.
- Derive boundary segments from the immutable coverage snapshot.
- Expose exact and procedural boundary profiles through a narrow adapter and
  build the first vertical or stepped connector.
- Handle corners, holes, negative coordinates, periodic topology, movement,
  teleport, reset, and stale completion rejection.
- Update exact-safe vegetation ownership for the removed terrain collar.
- Inspect low and elevated composed pixels before continuing.

Gate: no outer exact block has two horizontal owners, no open crack or
full-height wall appears, and delayed movement never combines generations.

## Slice 2: Finest-Level Surface Voxel Shell

Status: completed 2026-08-15.

- Define stable absolute cell ownership over the spacing-one lattice.
- Emit flat integer-height tops and only exposed vertical risers.
- Derive a compact visible column profile from worldgen-owned facts.
- Compare fixed degenerate faces with compacted topology before selecting the
  resident representation.
- Add the block-shell-to-smooth-ring seam under committed-level admission.
- Prove tile edges, ring edges, negative coordinates, periodic seams, and the
  exact frontier.

Gate: closest procedural terrain reads as one-block columns rather than
quantized diagonal triangles. Grass-soil, sand, gravel, snow, exposed stone,
and representative water sites have stable top and side geometry.

## Slice 3: Face Materials, Biome Tint, and Near Light

Status: completed 2026-08-15.

- Add direction-specific active-pack material bindings.
- Present top, upper-side, subsurface, and body strata through actual sprites.
- Add active-pack grass color maps and the accepted neighborhood approximation.
- Feed sky darken into the horizon and reuse exact lightmap and cardinal shade
  math for the block shell.
- Compare unoccluded fixture pixels before adding bounded height AO.
- Transition appearance toward the smooth far-ring model without overlap.

Gate: the controlled exact/LOD face fixture agrees under identical inputs,
real composition has no stable square color/daylight step at the frontier, and
grass risers visibly use grass side, dirt, and stone strata.

## Slice 4: Platform, Performance, and Human Acceptance

Status: implementation and automated acceptance completed 2026-08-15; Human
Review remains pending.

- Run focused Rust, WGSL/Naga, source identity, topology, and rebuild tests.
- Capture and inspect native pixels at the first drawable milestone and after
  each topology or appearance expansion.
- Prove headed WebGPU, synthetic stereo, per-eye XR, and full-frame multiview
  pixels through the shared renderer.
- Run current flat Android and Android XR validation, followed by physical
  evidence where `docs/platforms.md` requires it.
- Compare exact-only and composed CPU, GPU, refill, upload, memory, and movement
  receipts at matched cameras.
- Do not add a second blocky level if its median GPU regression exceeds `5%`
  without a separate explicit quality/performance decision.
- Present low-grazing, walking-height, elevated, forest, coast, snow, and
  mountain captures for Human Review.

Gate: supported paths show the same feature, live exact-only stays
allocation-free and pixel-stable, composed work stays bounded, and Human Review
accepts the frontier, appearance transition, and block character.

## Execution Record

The implementation landed as four reviewable commits:

- `b1bd8313` removes the horizontal exact-footprint collar and gives the
  spacing-one level a fixed top-and-four-riser voxel shell plus a bounded
  procedural-side frontier curtain;
- `5c64002b` adds worldgen-owned visible column profiles, active-pack top and
  side faces, pack-native grass tint, exact face shade, and the shared
  sky-darken/lightmap input;
- `1491ba68` removes procedural risers from exact-owned solid land and makes
  the opaque procedural water surface the single visible water owner through
  exact-painted chunks; and
- `8646273e` gives water one uniform procedural texture treatment through the
  near/far transition and hardens the desktop/mobile browser acceptance lane.

### Ownership and topology

The exact-painted mask now removes procedural solid tops and ordinary risers
over the entire painted footprint. It no longer retains the former
`1.5`-block horizontal collar. A frontier riser is emitted on the procedural
side only when an adjacent cell is exact-owned; it is offset by `0.001` block
and bounded to 32 blocks downward because the coverage snapshot does not yet
carry the exact boundary height profile. Water does not emit this fallback
wall.

Water is the one deliberate surface-class exception to chunk-footprint
ownership. Exact water is translucent, while the procedural horizon is an
opaque surface and the compact exact snapshot contains no compositing depth
for the water column. Keeping procedural water through painted chunks gives
the composed frame one stable visible water surface instead of a second dark
exact patch. Exact solid and cutout geometry still owns painted land.

The spacing-one level uses 30 procedural vertex invocations per cell: six for
the flat rounded top and six reserved for each cardinal riser. Hidden risers
collapse to degenerate triangles. Spacing two and coarser levels remain the
six-vertex smooth heightfield. The finest outer edge keeps the existing
committed fine/coarse connector, so geometry changes owner without drawing two
horizontal surfaces.

### Material and light contract

`mclone-worldgen` now exports a compact surface-column profile shared with the
generated WGSL. Grass resolves to grass top, grass-block side, three blocks of
dirt, and then stone; sand, gravel, clay, snow, water, and exposed-stone
recipes have bounded counterparts. `mclone-mesh` resolves the upward and
cardinal sprites plus tint flags from the same baked active-pack catalog used
by exact chunks.

Near solid faces use pack texels without the far synthetic texture blend,
active-pack grass colormap samples, exact cardinal shades (`1.0`, `0.8`, and
`0.6`), and the shared full-sky lightmap curve with scene sky darken. Material
character transitions over the outer 32 cells of the finest tiles; farther
rings retain slope light and reduced atlas contrast. Water intentionally uses
the same far-style treatment on both sides of that transition so the finest
ring is not visible as a square in open ocean.

Mono, per-eye stereo, and full-frame multiview are generated from the same
shader source. Coverage is visible to vertex and fragment stages, and scene
sky darken is threaded through flat, stereo, and multiview render entry points.

### Bounded cost

The clipmap remains 160 terrain slots plus 70 staging slots. No vertex buffer
is added: topology is generated from `vertex_index`, so the final native smoke
still reports `128,941,304` fixed resident bytes and zero pending work. The
face-aware material uniform is 16 KiB, 12 KiB larger than the former one-face
UV table.

Worst-case terrain vertex invocations rise from `3,932,160` to `5,505,024`:
the 16 finest tiles contribute `1,966,080` and the 144 smooth tiles contribute
`3,538,944`. That is a bounded 40% invocation increase, confined to the
finest level. Actual accepted captures reported `4,709,496` to `5,294,208`
terrain-plus-tree vertices after covered/frustum/far culling. No portable GPU
timestamp comparison was available in this slice, so these counts are the
honest topology evidence rather than a claimed GPU-time result.

### Validation and inspected pixels

The following passed on macOS/Metal unless noted:

- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh
  -p mclone-terrain-view --lib` (`104` mesh tests; `94` terrain-view tests
  passed and one adapter test remained ignored);
- focused `mclone-worldgen`, `mclone-app-runtime`, and `mclone-scene` library
  suites, plus checks for Terrain Lab, World Explorer, terrain-view, and scene;
- `pnpm native:world-explorer:smoke /tmp/mclone-lod-acceptance
  --composition composed` for native-window and offscreen movement, negative
  coordinates, teleport, map, and orbit;
- `pnpm native:world-explorer:web:smoke` and
  `pnpm native:world-explorer:web:smoke:mobile` for headed desktop and Pixel 7
  WebGPU profiles;
- `pnpm native:xr-emulation:smoke`, including two distinct eye images;
- `pnpm native:android:apk`; and
- `pnpm native:android-xr:apk`.

The final offscreen receipt records frontier
`single-owner-voxel-curtain`, 160/160 ready allocation slots, ten committed and
drawn levels, 842 refills, 30 rebases, and zero pending work. Inspected final
captures are under `/tmp/mclone-lod-acceptance/offscreen/`; the focused
land/river shell is `/tmp/mclone-lod-shell-detail.png`, the exact-water
ownership proof is `/tmp/mclone-lod-water-single-owner.png`, and the uniform
water transition proof is `/tmp/mclone-lod-water-uniform.png`. Browser and XR
captures remain under `/tmp/mclone-world-explorer-web-{desktop,mobile}-*.png`
and `/tmp/mclone-xr-emulation.png`.

### Acceptance disposition and remaining limits

Automated acceptance and agent pixel inspection pass. Human Review is still
required for the subjective block-character gate; it is not self-certified by
this record. Physical flat-Android pixels were not captured, while both flat
Android and Android-XR APK build boundaries pass and the shared stereo renderer
has inspected pixels.

The bounded frontier curtain is an explicit approximation until the immutable
exact snapshot carries a per-edge exact height/material profile. The
spacing-one shell uses fixed degenerate riser slots rather than a compacted
resident topology; the measured 40% worst-case vertex increase is the reason
to consider compaction only in a separate performance slice. Standalone World
Explorer `Exact` mode continues to keep switch-ready horizon residency, as it
did before this tactical; the live game's exact-only allocation policy is a
separate host/session contract.

## Acceptance

- Exactly one opaque horizontal terrain owner exists at every tested X/Z.
- No outer-exact-block z-fighting remains in stationary or moving low-grazing
  captures from four directions.
- Height disagreement produces a bounded material-correct vertical or stepped
  seam, not a crack, wall, or horizontal collar.
- The finest level has flat one-block tops and vertical risers, not merely
  rounded triangular geometry.
- Grass-soil risers show the declared grass-side/dirt/stone profile and other
  recipes use worldgen-owned strata.
- The near shell uses active-pack face sprites, biome tint, shared daylight,
  and exact-style face shade.
- A controlled exact/LOD face fixture agrees under equal inputs; real residual
  differences are attributable to documented semantic approximation.
- The near-to-far seam is watertight across tiles, negative coordinates,
  periodic topology, delayed admission, and teleport.
- Exact/proxy vegetation remains complete-record coherent with no new missing,
  dual-owned, or terrain-clipped record.
- Live-game exact-only remains horizon-allocation-free and unchanged; the
  switchable World Explorer exact host retains its existing ready horizon.
- CPU, GPU, and memory changes are measured without a platform quality fork.
- Mono, stereo/per-eye, and multiview share geometry, material, light, fog, and
  ownership contracts.
- Human Review accepts the composed frontier and terrain character.

## Code and Documentation Map

- `native/crates/mclone-terrain-view/src/composition.rs` — exact coverage and
  frontier ownership policy.
- `native/crates/mclone-terrain-view/src/shaders/terrain_preview_render.wgsl`
  — collar, heightfield, texture, light, fog, and discard behavior.
- `native/crates/mclone-terrain-view/src/viewport_renderer.rs` — tile
  resources, pipelines, materials, coverage, and multiview.
- `native/crates/mclone-terrain-view/src/clipmap.rs` — aligned level geometry.
- `native/crates/mclone-worldgen/src/terrain_preview.rs` — packed semantic
  sample shared by CPU and GPU evaluation.
- `native/crates/mclone-worldgen/src/levelgen/mclone_overworld/surface.rs` —
  canonical surface recipes and column strata.
- `native/crates/mclone-mesh/src/catalog.rs` — direction-specific model faces
  and active-pack atlas sprites.
- `native/crates/mclone-mesh/src/builder.rs` — exact tint, face shade, AO, and
  packed light.
- `native/crates/mclone-render/src/shaders/chunk_textured*.wgsl` — exact
  lightmap, texture, fog, and output behavior.
- `native/crates/mclone-scene/src/terrain_view.rs` — live coverage adapter,
  atlas binding, and scene orchestration.
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  — parent residency, ownership, platform, and validation contract.
- [`../topics/procedural-horizon-surface-appearance.md`](../topics/procedural-horizon-surface-appearance.md)
  — material, tint, texture, water, light, and performance baseline.
- [`262-world-explorer-exact-procedural-composition.md`](262-world-explorer-exact-procedural-composition.md)
  — accepted composition proof and known outer-block overlap.
- [`271-procedural-horizon-quality-baseline.md`](271-procedural-horizon-quality-baseline.md)
  — accepted ten-level/stride-one seam and projection baseline.
