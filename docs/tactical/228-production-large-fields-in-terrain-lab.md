# Production Large Fields In Terrain Lab

Status: active implementation 2026-07-23.

Topic: `gpu-procedural-terrain`

## Objective

Turn the first hosted Terrain Lab proof into a trustworthy generator-iteration
tool by porting the production Mclone Overworld's large-scale field graph to
the shared GPU evaluator and making the three-dimensional interaction and
comparison defaults self-explanatory.

This slice should make a seed recognizable on both sides of a CPU/GPU
comparison across kilometer-scale footprints. It does not claim exact final
terrain parity: the CPU reference continues to include major rivers, wetlands,
and planned-stream carving, while this port stops at the production base
surface.

## Originating Direction

The first hosted proof exposed two product problems immediately:

- left mouse drag changed the sampled geographic center when a
  three-dimensional view conventionally suggests orbit; and
- the default split compared production terrain with an unrelated approximate
  field graph, so the two halves did not look like the same seed.

The requested next step is an end-to-end production-field port before another
review. The primary payoff is a fast, hosted macro-scale loop for evaluating
first-party terrain changes, including kilometer-scale and later
tens-of-kilometers views.

## Exact Port Boundary

The A2 evaluator ports the unbounded production graph in
`McloneOverworldSampler` through `base_surface_y`:

1. the production 64-bit signed-seed/domain/lattice hash, emulated from two
   `u32` words because portable WGSL has no 64-bit integer type;
2. cubic value noise and quintic 16-direction gradient noise;
3. continentalness, relief, ruggedness, ridges, and warped mountain detail;
4. warped temperature and moisture;
5. production land-height shaping; and
6. ocean basin selection, shelf/basin depth, and seabed relief.

The shader uses the production domains, scales, weights, warp constants, and
height formulas. It evaluates the same point samples at every spacing; coarse
band limiting and parent summaries remain the next progressive-refinement
experiment rather than silently changing the comparison target.

The A2 boundary deliberately excludes:

- major-river signed-distance derivatives and morphology;
- wetlands and bounded planned-stream records;
- final watercourse carving and water levels;
- surface recipes, vegetation, structures, lighting, chunks, and authority;
  and
- periodic-cylinder lattice wrapping.

Consequently, residual height/water disagreement along real watercourses is
expected and should remain visible. Broad continentalness, land/ocean
classification, base height, bathymetry, and climate disagreement is a defect.

## Shared Ownership

`mclone-worldgen` remains the source of truth. It exposes a compact
large-field specification whose domain and scale values are also used to build
the production sampler.

`mclone-terrain-view` generates the WGSL constant preamble from that
specification, owns the portable 64-bit arithmetic and field evaluator, and
retains bounded readback comparison.

`mclone-terrain-lab` remains a thin browser/WebGPU adapter.

`tools/terrain-lab` owns presentation-only orbit state and browser gestures:

- 3D left drag orbits;
- Shift+left or middle drag pans geography;
- map left drag pans geography;
- wheel and explicit controls change sample spacing;
- the initial source is production Reference, not Split; and
- separate controls reset the camera and load the fixed review site.

Camera orbit is local presentation state. Seed, center, spacing, source, view,
and layer remain URL-addressed terrain-request state.

## Validation

Required gates:

- production large-field fixed-point fixtures in Rust;
- exact shader-spec generation and WGSL parse/validation;
- native and `wasm32-unknown-unknown` checks;
- TypeScript state/gesture tests and Playwright desktop/mobile journeys;
- a real headed-Wayland BrowserWebGPU comparison readback;
- inspected Reference, GPU, Split, Error, and phone captures under `/tmp`;
- fixed-site receipts for mean/p95/max final-height error and water agreement;
- comparison with the CPU base-surface residual floor so river-only
  disagreement is not mistaken for a failed large-field port;
- aggregate bundle staging and hosted-route smoke; and
- a production deployment of the exact validated revision.

## Commit Slices

1. Record this bounded port and its honest residual boundary.
2. Share the production field specification and fixed-point fixtures.
3. Port the exact large-field graph into the A2 WGSL evaluator.
4. Add orbit-first interaction and truthful reset/default semantics.
5. Tighten browser comparison gates from measured A2 evidence.
6. Reconcile the topic, deploy, and record hosted evidence.

Every implementation commit uses:

```text
Topic: gpu-procedural-terrain
```

## Stop Conditions

Pause for review only if portable WGSL cannot reproduce the production 64-bit
hash reliably on admitted WebGPU adapters, the field port reveals a required
generator-contract change, or measured broad-field disagreement remains after
the exact graph is implemented. Ordinary numerical debugging and UX polish
remain inside this tactical.
