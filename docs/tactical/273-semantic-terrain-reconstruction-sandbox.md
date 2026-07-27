# Tactical 273: Semantic Terrain Reconstruction Sandbox

Status: **Active research implementation as of 2026-07-27. Authorized by the
user's instruction to record this tactical and proceed end to end. Stop at
Human Review R1. Mclone Overworld, persisted generator profiles, block chunks,
and selective 3D remain unchanged.**

Topic:

- `multiscale-terrain-representation`

## Motivation

Tactical 272 proves that one canonical parent feature can answer direct
coarse queries and retain exact identity while bounded regional and local
children elaborate it. Human Review R1 found the facts highly stable while
panning, but the atlas remains colored line geometry. It does not show how
parent, regional, and local representations become terrain, and drawing all
three together makes their differences difficult to read.

Applying the first reconstruction directly to Mclone Overworld would entangle
the experiment with existing inland fabric, coast intent, hydrology, biomes,
surface materials, snow, vegetation, and structures. A bad result would not
identify whether semantic refinement failed or whether integration failed.

The next experiment therefore needs a deliberately small terrain generator
whose only substantial variables are substrate, feature family, and semantic
detail.

## Objective

Build a standalone research heightfield that turns Tactical 272's exact
feature hierarchy into continuous terrain:

```text
quiet or flat substrate
    + range-axis reconstruction
    - basin-route reconstruction
    = research surface
```

Terrain Lab must present synchronized Parent, Regional, Local, and correction
views over the same seed, viewport, topology, and camera. The implementation
must make it possible to judge:

1. whether a parent alone describes a useful broad landform;
2. whether regional and local representations preserve that identity while
   adding meaningful shape;
3. whether refinement is bounded correction rather than wholesale redraw;
4. whether independently owned features read as geography or isolated
   stamps;
5. whether range and basin influences interact legibly;
6. whether boundaries and periodic seams remain continuous; and
7. whether direct coarse reconstruction is materially cheaper than local
   reconstruction.

Pause for human review after the isolated generator, exact evidence, and
interactive desktop/phone diagnostic are complete.

## Non-Goals

This tactical does not:

- import or call the generator from Mclone Overworld;
- add a persisted world-generation profile;
- generate block chunks, biomes, surface recipes, snow, vegetation,
  structures, caves, or ecology;
- add coastline logic;
- implement exact hydrology, erosion, flow accumulation, or watershed solves;
- claim that the schematic Tactical 272 features are production geography;
- add universal or selective three-dimensional density;
- tune Mclone's current inland or coast output;
- let camera distance alter exact local terrain identity; or
- authorize later integration merely because objective invariants pass.

## Shared Ownership

- `mclone-worldgen` owns the research descriptor, substrate, feature
  reconstruction, topology-aware sampling, viewport compiler, exact
  checksums, metrics, and standalone receipt.
- `mclone-terrain-lab` owns a narrow Wasm adapter returning typed arrays.
- Terrain Lab TypeScript owns one Worker, navigation, synchronized
  presentation, colors, labels, and URL-addressed controls. It may not
  reconstruct terrain or feature relationships.
- Production samplers and profiles must not depend on the sandbox.

## Revision 1 Contract

### Descriptor

The canonical experiment descriptor contains:

- signed 64-bit seed;
- plane, 6,144-block X-cylinder, or 6,144-by-6,144-block torus topology;
- substrate: `flat` or `quiet`;
- feature mode: `range`, `basin`, or `combined`;
- representation: `parent`, `regional`, or `local`; and
- revision string.

Viewport, camera, request order, cache state, Worker, thread, and sample
spacing are not canonical inputs.

### Substrates

`flat` is a constant 64-block surface. It is the attribution control.

`quiet` is a topology-aware, low-amplitude, coordinate-pure rolling field.
Its frequency cells divide the 6,144-block period. It must be exactly periodic
on wrapped axes and require no stored or discovery-time state. It exists only
to test blending against restrained ordinary relief.

### Range reconstruction

One Tactical 272 range parent supplies identity, conservative bounds, height
envelope, and a broad compact-support corridor. The requested representation
selects exactly one course:

- parent: one coarse segment;
- regional: its two child segments; or
- local: its four child segments.

The three courses use the same root amplitude and broad influence width. A
finer representation replaces the coarser course for that feature; it is not
summed on top and cannot triple-count uplift. Distance to the requested
course produces smooth positive relief with finite support.

### Basin reconstruction

One Tactical 272 basin parent supplies identity, bounds, route envelope, and
explicit sink. Parent, regional, and local again select one route rather than
stacking three carves. Distance to the requested route produces a smooth
negative valley influence and a narrower channel core. A simple fixed research
water level may be shown where the local terrain lies below it. This is visual
water occupancy, not a hydrologic claim.

### Combination

`range` evaluates only uplift. `basin` evaluates only valley/channel carving.
`combined` adds both independent bounded influences to the same substrate.
The combined mode deliberately exposes collisions; Revision 1 does not add a
hidden precedence solver.

### Output

A viewport compilation produces one shared sample lattice containing:

- substrate height;
- parent, regional, and local surface heights;
- regional-minus-parent and local-minus-regional corrections;
- parent, regional, and local water occupancy;
- requested feature and owner counts;
- minimum, maximum, RMS correction, and changed-sample metrics;
- topology and coverage metadata;
- semantic-feature checksum; and
- quantized terrain checksum.

Final heights are quantized to 1/256 block before checksumming and transport.
This keeps the experimental equality contract explicit across native and
Wasm while preserving more precision than the diagnostic needs.

## Exact Invariants

For a fixed descriptor and absolute sample coordinate:

- flat substrate is exactly 64;
- quiet substrate repeats exactly across wrapped periods;
- raster, reverse, shuffled, tiled, and independently partitioned sampling
  produce the same quantized heights and checksum;
- native serial and parallel compilation agree;
- native and `wasm32-unknown-unknown` agree on the pinned corpus;
- equivalent cylinder and torus lifts agree;
- reconstruction uses the same parent identities and children as Tactical
  272;
- direct parent construction reports zero regional/local feature facts;
- direct regional construction reports zero local feature facts;
- every influence is zero outside its declared compact support;
- correction metrics are finite and bounded;
- adjacent viewport tiles agree at shared sample coordinates; and
- Mclone Overworld field, biome/surface-language, and chunk fingerprints
  remain unchanged.

## Terrain Lab Diagnostic

Add a Mclone-only pane:

```text
Semantic terrain
```

It is separate from Planner atlas and production CPU/GPU LOD. Its four
synchronized panels are:

```text
A · Parent terrain
B · Regional terrain
C · Local terrain
D · Refinement correction
```

Parent, regional, and local panels honor the shared Map/3D view and camera.
The correction panel shows either regional-minus-parent or
local-minus-regional with a centered diverging scale. Controls are
URL-addressed:

- substrate: flat or quiet;
- features: range, basin, or combined;
- topology: plane, cylinder X, or torus;
- correction: regional or local; and
- feature guides: hidden or visible.

Panning and zooming recompile the same absolute terrain. Zoom chooses sample
footprint and presentation only; it does not choose exact terrain identity.
The panels always remain side-by-side evidence rather than silently switching
canonical detail by camera distance.

Desktop uses a 2-by-2 comparison. Phone uses four stacked panels with enough
height to inspect the local terrain and correction view. The evidence receipt
shows checksums, sample count, compile time, height range, correction RMS,
changed-sample fraction, feature counts, and the research-only/production-
unchanged state.

## Performance Evidence

Measure cold native viewport compilation for at least:

- 6,144 blocks at the diagnostic sample count;
- 16,384 blocks at the same sample count; and
- 65,536 blocks at the same sample count.

Record parent, regional, and local feature facts, distance evaluations,
sample count, receipt bytes, native time, Wasm Worker time, and Canvas draw
time. A direct coarse query is not accepted as cheaper if it constructs and
discards children.

These are fixed-output representation costs, not exact chunk throughput and
not a 500 km claim.

## Human Review R1

Review flat and quiet substrates in range, basin, and combined modes at
regional and broad scales. Toggle Map/3D, rotate the synchronized views, pan
across ordinary owner boundaries, and cross cylinder and torus seams.

Answer:

1. Is the parent terrain already a legible broad landform?
2. Do regional and local terrain look like refinements of it?
3. Does the correction panel show bounded detail rather than replacement?
4. Are range forms too much like isolated ridges or stamps?
5. Are basin forms useful without pretending to be complete hydrology?
6. Does the quiet substrate help composition without hiding the mechanism?
7. Are periodic seams and owner boundaries visually acceptable?
8. Is one family promising enough for a later frozen-Mclone-foundation trial?

The review may:

- advance range reconstruction only;
- advance basin reconstruction only;
- request another isolated revision;
- reject both while retaining the determinism machinery; or
- stop semantic reconstruction research.

Do not integrate with Mclone Overworld or begin selective 3D at this gate.

## Commit And Evidence Trail

1. Commit this tactical before implementation.
2. Commit the shared Rust sampler, receipt, and exact tests separately.
3. Commit the Terrain Lab adapter and diagnostic separately.
4. Keep JSON receipts and screenshots under `/tmp`.
5. Inspect desktop and phone pixels at the first drawable milestone.
6. Update this tactical and the living topic with exact outcomes before
   pausing.
7. Use `Topic: multiscale-terrain-representation` throughout the series.

## Exit Condition

This tactical reaches Human Review R1 only when:

- all three representations reconstruct continuous terrain in shared Rust;
- flat/quiet and range/basin/combined modes are implemented;
- native, Wasm, order, partition, tile-boundary, and topology checks pass;
- direct coarse work is honestly counted;
- the fourth correction view and independent controls work in Terrain Lab;
- desktop and phone Map/3D pixels are inspected;
- performance and transfer costs are recorded;
- production fingerprints pass; and
- Mclone Overworld remains disconnected.
