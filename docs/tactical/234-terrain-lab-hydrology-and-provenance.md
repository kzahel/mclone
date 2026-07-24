# Terrain Lab Hydrology And Provenance

Status: completed 2026-07-24, including targeted hosted desktop and phone
BrowserWebGPU validation.

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

## Implementation Receipt

The dependency-ordered stage contract is live as preview schema
`mclone-terrain-preview-reference-grid-v5` and GPU evaluator
`mclone-overworld-v1-gpu-preview-a5`. `Base`, `Hydrology`, `Structured`,
`Surface`, and `Cover` are tile/cache identities and remain outside
authoritative world-generation configuration.

Natural hydrology now has one fixed-work CPU/WGSL contract covering river
geometry, analytic warp derivatives, variable width, channel and bank carving,
wetlands, pools, submerged outlets, final height/water state, visible
material, biome recipe, surface recipe, and landform class. Signed centerline
distance preserves a narrow river contour at coarse display levels without
changing canonical geometry.

Planned streams remain CPU-owned bounded records. At `1:1` through `1:4`, the
preview compiler reconstructs intersecting production routes once, samples a
compact influence into the reference payload, and lets either LOD lane consume
that structured result. Coarser requests explicitly report planned streams as
unavailable. The reviewed seed `-98765` point `(2369, -1977)` identifies
planned-stream owner chunk `(147, -126)` in the point receipt and appears in
both panes.

The Lab now exposes terrain, height/error, continents, climate, river,
wetland, landform, biome, surface, and planned-stream views. Landform colors
reuse the in-engine Worldgen Lens vocabulary. Click/tap inspection works in
map and 3D and reports block/chunk/quart coordinates, ordered production
decisions, structured ownership, carve and climate values, detailed hydrology
fields, and field/preview/GPU/decoration revisions. The selected point alone
receives this full receipt.

Final local validation passed:

- 8 focused `mclone-worldgen` preview tests;
- all 16 `mclone-terrain-view` library tests, including WGSL validation;
- all 3 `mclone-terrain-lab` library tests;
- TypeScript typecheck and all 13 URL/input state tests; and
- all 4 headed-Wayland Playwright cases across desktop and Pixel layouts.

The structured-stream captures were inspected at desktop and phone sizes. The
CPU/GPU river, bank, coast, lowland, and planned-stream shapes align. Natural
channel agreement is 100%. The near structured landform grid records one
honest f64/f32 category-boundary disagreement per layout, for greater than
99.99% agreement, rather than substituting CPU categories into the GPU pane.

The targeted production upload changed only Terrain Lab objects. The
unchanged Worker serves:

- `terrain/assets/canonical-worker-BQgpcAAp.js`, SHA-256
  `3ac3e63408cd8f3886a4d8dd091f5fc764bbc92c2c64a778eb124179d3315141`;
- `terrain/assets/index-K-isbiIr.css`, SHA-256
  `6f5faf8a7f3a76004a68ceca6aba710a6c8c93afd6ab97e22985ffdab3fbdd48`;
- `terrain/assets/index-V8OlCP6Y.js`, SHA-256
  `dacbe0f1b212f0d9a9528adbeddc209d71ba5ac6c977ce9d4f0061abf0d77abd`;
- `terrain/assets/mclone_terrain_lab_bg-DVyvzS8N.wasm`, SHA-256
  `5f571bcaa58236f278d457c036ad20563286d79f27a4216a2f0cc62758676e6f`;
  and
- `terrain/index.html`, SHA-256
  `35cb3505804b963c5ac25aa05efd58437ea49a1b1be15dd76b15cf9ae67953b1`.

Every hashed object was downloaded and byte-verified before the HTML switch.
Hosted headed-Wayland desktop and Pixel smokes then passed against
`https://mclone.kzahel.com/terrain/`. Both reported zero natural final/base
height error, 100% river, visible-material, biome, surface, and reviewed
landform agreement, and no cache hits in the cold race. Desktop GPU target
plus validation readback completed in 1,579.4 ms versus 22,341.6 ms for CPU
target publication. Pixel measured 2,010.3 ms versus 21,050.8 ms. These are
different end-to-end publication boundaries, not pure GPU execution timing.

The implementation series is:

- `e289f102` — plan the hydrology/provenance contract;
- `bb42f605` — expand the shared semantic payload;
- `a24ae14f` — port production natural hydrology to WGSL;
- `0d51f40e` — merge planned-stream records into both LOD lanes;
- `d56b297f` — expose stages, layers, picking, and provenance;
- `c216db53` — pin the reviewed planned-stream evidence;
- `a9b3aa0e` — complete detailed production point receipts; and
- `feb11eb6` — add the final production landform diagnostic.
