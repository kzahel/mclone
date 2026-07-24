# Terrain Lab Hydrology And Provenance

Status: active 2026-07-24.

Topic: `gpu-procedural-terrain`

## Objective

Make the CPU and GPU LOD panes describe the same recognizable landscape as
the exact first-party terrain by adding the first missing macro-content
family: hydrology. The slice also makes every selected location explainable
without introducing arbitrary gameplay generator flags.

The completed Lab must provide:

- dependency-ordered LOD content stages;
- production natural rivers, banks, submerged outlets, wetlands, and pools in
  both CPU and GPU lanes;
- scale-aware river visibility when a point-sampled grid becomes coarser than
  the channel;
- bounded planned streams as sparse CPU-produced records reusable by either
  LOD lane;
- biome, landform, surface, and hydrology diagnostic views; and
- a pointer-selected production receipt naming the rule, fields, revisions,
  and structured owner responsible for the inspected terrain.

## Originating Direction

Human review of the exact and LOD workspace found a large final river in the
exact pane but no corresponding watercourse in either LOD pane. This is not a
cosmetic water-material mismatch. One watercourse changes:

- terrain elevation through channel and bank carving;
- visible water elevation;
- river, wetland, and shore biome decisions;
- gravel, clay, sand, and grass surface selection; and
- later decoration and structure placement.

The reviewer also wants to answer “how was this generated?” at an arbitrary
location. A boolean flag is usually not the answer: terrain comes from
continuous fields, compact influences, ordered selection rules, and sometimes
a bounded structure record.

## Content Stages, Not Gameplay Flags

LOD panes expose one dependency-ordered `TerrainPreviewContentStage`:

1. `Base` — uncarved large fields, oceans, and macro materials;
2. `Hydrology` — natural rivers, banks, wetlands, pools, and outlets;
3. `Structured` — hydrology plus bounded planned-stream records;
4. `Surface` — structured terrain plus categorical surface and biome
   treatment; and
5. `Cover` — surface treatment plus macro vegetation-cover presentation.

The stage is a preview compiler and cache identity. It is not persisted in a
world, passed through authoritative generation, or offered as a gameplay
world-generation option. Later-stage semantics may depend on earlier stages;
the Lab does not support invalid combinations such as river materials without
river carving.

Natural hydrology is a fixed-work point evaluator and belongs in the shared
CPU/GPU field payload. Planned streams remain structure-shaped records from
the production CPU planner. The GPU lane may consume their compact sampled
overlay, but it must not duplicate the bounded route search in WGSL.

Because planned streams are 48–96-block local features, record reconstruction
is bounded to near preview levels where they can affect visible pixels. Coarse
levels report that structured coverage is unavailable rather than scanning
thousands of candidate starts or pretending a point sample found every route.

## Scale-Aware Watercourse Summary

Exact point parity is insufficient once sample spacing exceeds channel width.
The natural river family is a warped signed zero contour, so the LOD payload
retains signed centerline distance and half-width. The renderer interpolates
the signed field across a cell and preserves a narrow semantic watercourse
band when the zero contour crosses the footprint even if none of the cell
corners lies inside the physical channel.

Near levels continue to use production carved heights and water surfaces.
Coarse contour preservation is explicitly an LOD summary, not canonical block
geometry. It must remain stable in world coordinates, have a bounded minimum
screen presence, and avoid turning the entire influence bank into water.

Wetland influence and pools remain scalar facts in the payload. A later
footprint aggregate may improve sparse pool coverage if review shows the point
representation is insufficient.

## Semantic Payload And Comparison

The shared preview sample grows from base terrain/climate facts to include:

- signed natural-river distance and channel half-width;
- channel, bank, wetland, pool, and submerged-outlet influences;
- planned-stream influence;
- visible hydrology-aware material;
- semantic biome, surface, and vegetation-cover summaries; and
- base and final height/water facts.

Comparison continues to report base-field agreement and adds:

- final surface-height error;
- natural water presence agreement;
- channel classification agreement;
- mean channel/bank/wetland influence error;
- signed river-distance error; and
- hydrology-visible material agreement.

Structured CPU overlays are merged before structured-stage comparison. They
must not be counted as a GPU field error.

## Diagnostic And Inspector Contract

The existing production `McloneOverworldDebugSample` remains the semantic
owner. The Lab adds diagnostic layers for biome, landform, surface, and
hydrology using the same stable category vocabulary as the in-engine Worldgen
Lens.

A click or tap that did not become an orbit, pan, or pinch selects the terrain
under the pointer. Map picking is analytic. Three-dimensional picking uses the
shared camera projection and a bounded CPU ray/heightfield intersection.

The receipt reports:

- block, chunk, and four-block biome-cell coordinates;
- generator field, preview schema, GPU evaluator, and decoration revisions;
- biome recipe and ordered selection reason;
- landform, surface recipe, and hydrology kind;
- base and final height, carve delta, slope, mountain strength, and exposure;
- temperature, moisture, adjusted temperature, relief, ruggedness, and ridge;
- channel/bank/wetland influences, distance, width, bed/water elevations,
  grade, and flow; and
- planned-stream start identity when a bounded record owns the location.

Only the selected point receives the full receipt. Tiles retain compact
semantic facts rather than a large provenance payload.

## Ownership

- `mclone-worldgen` owns watercourse fields, preview semantic packing,
  structured sampling, comparison truth, and diagnostic receipts.
- `mclone-terrain-view` owns GPU evaluation, content-stage rendering,
  scale-aware contour presentation, tile scheduling, and shared picking math.
- `mclone-terrain-lab` owns Wasm serialization and browser-facing methods.
- `tools/terrain-lab` owns URL state, controls, pointer gesture translation,
  inspector presentation, and browser assertions.

## Implementation Order

1. Land this contract and update the living topic.
2. Add stage identity and the expanded production CPU preview sample.
3. Expose exact diagnostic receipts, including planned-stream ownership.
4. Port natural watercourse domains, derivatives, geometry, carving, and
   semantic materials to WGSL.
5. Add hydrology comparison metrics and scale-aware signed-contour rendering.
6. Add bounded near-level planned-stream sampling and GPU overlay merge.
7. Add biome/surface/hydrology layers and macro vegetation-cover presentation.
8. Add map/3D pointer picking, inspector UI, URL identity, and cache labels.
9. Validate Rust, shader, Wasm, TypeScript, desktop, phone, cold scheduling,
   coarse contour visibility, and a reviewed planned stream.
10. Deploy only the Terrain Lab product and record exact hosted object hashes
    and browser receipts.

Commit every coherent slice with `Topic: gpu-procedural-terrain`.

## Acceptance

- The river in the reviewed exact screenshot is recognizable in CPU and GPU
  hydrology stages at matching coordinates.
- Base stage still reproduces the previously accepted uncarved comparison.
- CPU/GPU natural hydrology metrics are explicit and stable.
- A river remains visible in map mode after zooming beyond its physical
  point-sample width without widening nearby dry banks into a lake.
- A reviewed planned stream appears in both LOD lanes from one CPU-produced
  structured overlay and identifies its start in the inspector.
- Diagnostic categories and inspector reasons come from worldgen-owned
  vocabulary rather than TypeScript reimplementations.
- Changing content stage cannot reuse a tile compiled under a different
  stage, and cache-off remains a real cold-generation path.
- Desktop and phone controls retain independent progressive publication,
  orbit, pan, zoom, and page-scroll suppression.
