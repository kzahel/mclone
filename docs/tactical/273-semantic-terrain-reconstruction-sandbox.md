# Tactical 273: Semantic Terrain Reconstruction Sandbox

Status: **Implementation complete at Human Review R1 as of 2026-07-27.
Commits `fb1929c2`, `b0889375`, and `25111ddb` record the specification,
shared reconstruction, and Terrain Lab diagnostic. Mclone Overworld,
persisted generator profiles, block chunks, and selective 3D remain
unchanged. Commit `7214e28a` records the Human Review R1 navigation
correction. Commit `5e138c23` records the first stable-Y attempt, and
`7e295bd1` supersedes it with physically coherent world-scale zoom. Do not
continue into production integration before review.**

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
- correction: regional or local;
- vertical scale: physical 1×, diagnostic 8×, or diagnostic 24×; and
- feature guides: hidden or visible.

The 3D comparison uses one explicit vertical datum shared by all three
terrain panels. It must not derive its Y origin or scale from the minimum and
maximum height of each requested viewport. Physical 1× uses the same
world-to-screen scale for X, Y, and Z; explicit diagnostic exaggeration
multiplies that world-scale Y component rather than defining a screen-space
Y axis.

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

## Execution Result

The shared sampler lives in
`mclone-worldgen::semantic_terrain_sandbox`. It reconstructs all three
courses over one 65-sample-wide lattice and exposes a separate direct-detail
compiler for honest cost measurement. The first implementation also corrected
Tactical 272's coarse path: parent requests now return before regional facts
are constructed, and regional requests never construct local facts.

The pinned native and Wasm suite is:

```text
14250ea1a92a72246abfd256d3ffb2caca021a5805a4be692299bfecc5017f8e
```

It covers three seeds by plane, X-cylinder, and torus. Raster, reverse,
even/odd, and deterministically shuffled traversal agree exactly. Periodic
lifts agree, adjacent viewports share exact edges, independently compiled
half-viewports reassemble the same full surface, and native serial/parallel
compilation agrees. The Wasm Worker recomputes this suite once at startup and
fails closed if it differs from the native pin; it does not merely display a
compiled constant.

The release receipt is written outside the repository at:

```text
/tmp/mclone-semantic-terrain-sandbox/receipt.json
```

The 2026-07-27 release run produced a 7,608-byte receipt and these uncached
fixed-output averages on the current Linux host:

| Width | Parent | Regional | Local |
|---:|---:|---:|---:|
| 6,144 blocks | 0.538 ms · 8 facts · 33,800 distance evaluations | 0.756 ms · 16 facts · 67,600 evaluations | 1.267 ms · 32 facts · 135,200 evaluations |
| 16,384 blocks | 1.174 ms · 32 facts · 135,200 evaluations | 2.040 ms · 64 facts · 270,400 evaluations | 3.788 ms · 128 facts · 540,800 evaluations |
| 65,536 blocks | 7.939 ms · 288 facts · 1,216,800 evaluations | 16.314 ms · 576 facts · 2,433,600 evaluations | 34.993 ms · 1,152 facts · 4,867,200 evaluations |

These times hold output samples fixed at 4,225. Physical extent increases the
number of bounded feature owners intersecting the viewport; requested detail
approximately doubles feature segments at each refinement. This is the
intended measurable tradeoff, not constant work across arbitrary extent.

One headed-Wayland browser run of the 6,144-block combined/quiet view measured
5.21 ms Worker compilation and 37.51 ms Canvas drawing at desktop size, and
4.66 ms compilation and 23.56 ms drawing in the stacked phone layout. These
are interaction evidence, not normalized performance baselines.

Terrain Lab now includes the Mclone-only `Semantic terrain` pane. Rust/Wasm
returns typed arrays and metadata; TypeScript owns only Worker scheduling,
navigation, and drawing. Desktop is 2-by-2 and phone stacks four full-width
panels. The following controls round-trip through the URL:

- `semanticSubstrate=flat|quiet`
- `semanticFeatures=range|basin|combined`
- `semanticTopology=plane|cylinder-x|torus`
- `semanticCorrection=regional|local`
- `semanticVertical=1x|8x|24x`
- `semanticGuides=0|1`

Map/3D, orbit, pan, and zoom remain synchronized across all panels. Inspected
pixels are retained only under `/tmp`:

```text
/tmp/mclone-semantic-terrain-desktop-canvas.png
/tmp/mclone-semantic-terrain-phone-canvas-3.png
```

Validation completed:

- all 401 active `mclone-worldgen` library tests passed; one existing
  gauntlet remains ignored;
- all semantic sandbox and browser ownership-lock tests passed;
- Terrain Lab Wasm build, TypeScript typecheck, state tests, and production
  web build passed;
- the dedicated desktop and Pixel 7 browser test passed;
- the browser test proved a torus period lift preserves the exact terrain
  checksum; and
- headed Chrome reported no page or console errors.

### Human Review R1 navigation correction

The first hosted review found that pan and zoom discarded the last completed
reconstruction while a debounced replacement request was pending. All four
panels therefore became black during the interaction, and continuous motion
prevented the debounce from producing intermediate terrain frames.

Commit `7214e28a` changes presentation scheduling without changing the
reconstruction or its pinned suite:

- the last completed response remains drawable until its replacement is
  complete;
- the visible canvas receives only completed frames from one reusable drawing
  buffer, so an expensive 3D redraw cannot expose its initial black clear;
- at most one Worker reconstruction is in flight while navigation coalesces
  all newer states into the most recent pending request;
- the next request starts as soon as the current response arrives, allowing
  continuous pan and zoom to publish intermediate absolute-coordinate frames
  without an unbounded Worker backlog; and
- the badge distinguishes the initial reconstruction from an update whose
  preceding frame remains visible.

The desktop and Pixel 7 browser lanes now orbit the 3D surface and perform a
continuous shift-drag pan. They prove that visible pixels change during orbit,
new terrain checksums and pixels arrive before the pointer is released, and
`data-render-ready` never drops during the pan. The retained frame and report
keep their completed-response metadata; scheduling never manufactures a
checksum for pending coordinates, and the completed replacement remains the
same order-independent Rust reconstruction.

The inspected phone frame remains outside the repository at:

```text
/tmp/mclone-semantic-terrain-interactive-phone.png
```

### Human Review R1 vertical projection correction

The next hosted review identified a second presentation ambiguity. The
original 3D projection centered every completed viewport on
`(minimum_height + maximum_height) / 2` and divided height by that viewport's
observed range. Zooming changed both values whenever relief entered or left
the sample window. The underlying absolute heights remained deterministic,
but the same elevation moved vertically like a graph with automatic Y-axis
fitting.

Commit `5e138c23` first replaced that fit with:

- a fixed Y=64 display datum, matching the sandbox's flat substrate;
- a fixed 192-block vertical display span shared by Parent, Regional, and
  Local;
- no viewport minimum, viewport maximum, or horizontal zoom input to the
  vertical-offset function; and
- a visible `fixed Y · 192-block span` badge in 3D mode.

That removed extrema-driven jumps, but the next Human Review R1 pass correctly
rejected it: keeping Y at a fixed screen-space size while horizontal terrain
shrinks still behaves like an independently scaled graph. Peaks did not get
smaller when the camera zoomed out.

Commit `7e295bd1` supersedes the 192-block screen span with a world-scale
projection:

- physical 1× is the default and maps one vertical block with the same scale
  as one horizontal block;
- doubling `blocksAcross` therefore halves the on-screen height of an
  unchanged peak;
- Y=64 remains the stable vertical datum, so viewport extrema still cannot
  recenter the terrain;
- explicit 8× and 24× diagnostic exaggeration multiply the world-scale
  vertical component and therefore also shrink coherently with zoom; and
- the URL-addressed control and canvas badge always disclose whether the view
  is physical or exaggerated.

Map colors, correction colors, semantic checksums, the pinned Rust suite, and
production terrain remain unchanged. Focused projection tests lock 1× aspect,
the inverse zoom-to-height relationship, datum symmetry, and explicit
exaggeration. The desktop and Pixel 7 browser lane covers physical zoom and
the 24× control.

The fixed-screen-span captures below are retained only as evidence of the
rejected intermediate:

```text
/tmp/mclone-semantic-fixed-y-6144.png
/tmp/mclone-semantic-fixed-y-zoom.png
```

The inspected physical 1× sequence and optional coherent 24× diagnostic
remain outside the repository at:

```text
/tmp/mclone-semantic-physical-3072.png
/tmp/mclone-semantic-physical-6144.png
/tmp/mclone-semantic-physical-12288.png
/tmp/mclone-semantic-world-scale-24x.png
```

The review route is:

```text
/terrain/?profile=mclone-overworld-v1&panes=semantic&seed=12345&x=1024&z=-768&blocks=6144&view=3d&semanticSubstrate=quiet&semanticFeatures=combined&semanticTopology=plane&semanticCorrection=local&semanticVertical=1x&semanticGuides=1
```

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
