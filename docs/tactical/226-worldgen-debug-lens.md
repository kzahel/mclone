# Worldgen Debug Lens

Status: active implementation 2026-07-23.

Topic: `worldgen-debug-lens`

## Objective

Add a read-only, in-app diagnostic lens for the original Mclone overworld so
human review can distinguish:

- the final categorical biome recipe from the continuous fields that selected
  it;
- broad landform intent from final surface treatment;
- hydrology influence from discrete procedural-structure ownership; and
- chunk boundaries from the four-block biome sampling cells that may cross
  them.

The first vertical slice must be useful from a bird's-eye camera without
turning diagnostics into another world-generation pipeline. It provides four
terrain-following color modes, a stable legend, and a crosshair inspector:

1. biome;
2. landform;
3. surface; and
4. hydrology.

Structure starts, bounds, route pieces, scalar heatmaps, a regional atlas,
pinned comparison points, and copyable review receipts remain compatible
follow-ups. They are not required to prove the first ownership seam.

## Originating Review Need

Interactive review has repeatedly identified effects whose visible result did
not reveal its generating owner:

- a small steppe could be a climate-region scale issue or only a surface tint;
- a river-mouth color discontinuity was initially suspected to be far LOD but
  proved to be a physical bathymetry sill exposed by lighting;
- mountain shoulders, riverbanks, wetland influence, and surface erosion can
  overlap without a single chunk-wide label; and
- major rivers are continuous fields while bounded valley streams are
  procedural structures, even though both appear as watercourses.

Screenshots alone therefore leave too much diagnosis implicit. The reviewer
needs stable, production-derived vocabulary at the inspected location and a
regional view of the same classifications.

## Data Truth And Sampling Semantics

The lens consumes the production `McloneOverworldSampler`,
`mclone_overworld_biome_recipe`, and
`mclone_overworld_surface_recipe`. It must not reproduce approximate terrain,
biome, or hydrology equations in scene or rendering code.

The first slice samples a regular four-block lattice aligned with the
production horizontal biome payload:

- a 16-by-16 chunk can contain up to sixteen categorical horizontal biome
  samples;
- the biome lens shows categorical sampled outcomes, not a fictitious
  chunk-wide biome;
- climate, mountain, slope, and hydrology inputs remain continuous even where
  the selected biome or surface recipe is categorical; and
- overlay samples are diagnostic reconstruction from the active seed,
  topology, and field revision. They are not additional persisted chunk data.

The worldgen owner exposes one compact semantic sample containing:

- absolute block and quart-cell coordinates;
- the production terrain and landform sample;
- biome recipe and an ordered-rule selection reason;
- a descriptive landform class;
- surface recipe; and
- hydrology class.

Landform classes are intentionally diagnostic vocabulary rather than new
generation policy. They are derived from existing production predicates and
fixed thresholds in the worldgen diagnostics module.

## Ownership And Dependency Direction

```text
production worldgen samplers
          |
          v
read-only worldgen diagnostic sample
          |
          v
scene lens state + region cache
       /                    \
      v                      v
generic colored mesh       debug pane + legend
      |
      v
single-view and multiview render adapters
```

Concrete ownership:

- `mclone-worldgen` owns semantic diagnostic samples, labels, and selection
  reasons.
- `mclone-scene` owns the active lens mode, region key/cache, palette
  projection, inspector text, and lifecycle reset.
- `mclone-render` owns a generic depth-tested translucent colored-triangle
  renderer with both ordinary per-view and XR multiview paths. It knows
  nothing about biomes.
- `mclone-ui` continues to own the existing debug-pane text presentation.
- platform adapters may bind physical keys to shared scene commands; they do
  not sample worldgen or own lens state.

The dependency path is one-way. Worldgen never depends on scene, UI, or
rendering.

## First-Slice Interaction

Desktop review uses two nearby keys:

- `F3` toggles the Worldgen Lens off or back to the last selected mode.
- `F4` advances `Biome -> Landform -> Surface -> Hydrology -> Biome`.

The existing backquote key continues to toggle the diagnostics pane. When the
lens is active, that pane begins with the lens name, palette legend, and
crosshair sample. The lens remains usable without the pane so screenshots can
show unobstructed color coverage.

The actions are methods on the shared scene host. A later controller/UI
binding can call the same methods without changing lens ownership.

If the active generation profile is not `mclone-overworld-v1`, the shortcut
may retain its selected mode but no diagnostic mesh is produced; the pane
reports that the profile is unsupported by this first slice.

## Overlay Geometry And Palette

The scene samples a square region centered on the camera's four-block cell.
Each diagnostic cell becomes two terrain-following triangles:

- corners use production surface-height samples, so the overlay follows broad
  terrain rather than forming a floating map plane;
- the overlay is raised by a small fixed bias to avoid coplanar flicker;
- translucent colors retain enough underlying material and lighting to keep
  the actual landscape readable;
- reversed-Z depth testing hides the overlay behind nearer terrain; and
- the mesh is rebuilt only when its cache key changes.

The initial radius is deliberately capped. It should cover enough terrain for
a high bird's-eye review while keeping CPU sampling, upload size, and visual
clutter bounded. Palette colors and legend ordering remain stable across
sessions and screenshots.

Biome colors distinguish ocean, shore, river, alpine, conifer, steppe,
woodland, and meadow. Landform colors distinguish water, coast, lowland,
upland, mountain valley, shoulder, and massif. Surface colors distinguish the
nine production recipes. Hydrology colors distinguish dry terrain, bank
influence, wetland, major-river channel, submerged outlet, and planned-stream
facts when they are present in the sampled production intent.

## Inspector Contract

The inspector uses the same diagnostic sample as the overlay and reports:

- block, chunk, and four-block biome-cell coordinates;
- selected biome and ordered-rule reason;
- landform and surface recipe;
- surface Y, base surface Y, slope, mountain strength, and exposure;
- temperature, moisture, and altitude-adjusted temperature; and
- hydrology kind, channel/bank influence, half-width, bed/water Y, grade, and
  flow vector when relevant.

The first slice inspects the horizontal block coordinate beneath the
crosshair/camera column. A later ray-hit inspector may replace that choice
without changing the diagnostic sample contract.

The UI must say `reason` or `rule`, not `biome blend` or `confidence`.
Biome selection is currently an ordered classifier, not weighted blending.

## Disabled-Path And Performance Invariants

When disabled:

- no worldgen sampling occurs;
- no overlay vertices are allocated or uploaded;
- no diagnostic render pass is encoded;
- no chunk or persistence payload grows; and
- no structure plan is reconstructed.

When enabled:

- one cached region is retained for the active world;
- the region rebuilds only after the camera crosses its aligned movement
  threshold, the layer changes, or seed/profile/topology changes;
- every sample has fixed work and the region has a hard sample/vertex cap;
- only the inspector sample may refresh more frequently; and
- structure planning remains absent until its own opt-in layer and cache are
  implemented.

## Validation

Worldgen:

- diagnostic biome and surface outcomes equal direct production classifier
  results;
- ordered biome reasons cover all recipe branches;
- quart-cell alignment is correct for positive and negative coordinates; and
- semantic labels and palette keys remain stable.

Scene:

- disabled state yields no sample region or overlay vertices;
- mode cycling is deterministic and retains the last selected mode across
  off/on;
- cache keys change on aligned movement, layer, seed, profile, and topology;
- non-Mclone profiles report unavailable rather than sampling the wrong
  generator; and
- inspector lines expose biome, rule, landform, surface, climate, and
  hydrology facts.

Renderer:

- empty overlays are no-ops;
- vertex packing and draw counts are exact;
- ordinary per-view rendering is depth-tested and translucent; and
- the renderer implements a two-view multiview pipeline rather than silently
  disappearing in XR.

Rendered review:

- capture a high-view-distance, above-ground Mclone-overworld frame for every
  first-slice layer;
- inspect the biome layer around at least one visible transition;
- inspect landform and surface layers over mountain terrain;
- inspect hydrology at a river or wetland;
- confirm the legend matches pixels and remains readable; and
- compare lens-off and lens-on frames for accidental generator or camera
  changes.

## Follow-Up Queue

After Human Review 1:

1. add a Structures layer with valley-stream start, total bounds, piece bounds,
   centerline, reach water Y, and confluence ownership;
2. add individually selectable scalar heatmaps for climate, mountain,
   morphology, slope, exposure, and bathymetry;
3. add a larger top-down regional atlas using the same semantic samples;
4. add pinned A/B samples and threshold deltas;
5. add a copyable review receipt carrying seed, profile, field/decor revision,
   position, selected layer, and sample facts; and
6. bind the shared actions into controller-accessible debug UI if routine
   non-keyboard review warrants it.

Do not start these merely to make the first implementation appear complete.
The first review should determine which views actually reduce confusion.
