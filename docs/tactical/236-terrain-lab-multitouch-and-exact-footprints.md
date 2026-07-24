# Terrain Lab Multitouch And Exact Footprints

Status: completed 2026-07-24, including local real-touch acceptance and
hosted desktop/mobile headed-WebGPU validation.

Topic: `gpu-procedural-terrain`

## Objective

Make Terrain Lab navigation behave like a modern touch map and let reviewers
request meaningfully larger bounded exact-terrain footprints.

The completed Lab must:

- pan from two-finger centroid movement while zooming from finger separation
  in the same gesture;
- apply that gesture in map and 3D views on both procedural and canonical
  panes;
- keep map zoom anchored to the initial gesture centroid;
- pan 3D terrain in the camera ground plane without changing orbit;
- suppress page scrolling and accidental point inspection throughout a
  two-finger gesture; and
- offer exact centered footprints of 49 and 81 chunks in addition to the
  existing 1, 9, and 25.

## Originating Direction

The existing pointer path tracks two touch points but uses only their
distance. Moving both fingers together therefore does nothing, and a diagonal
pinch zooms without following the user's fingers.

The exact pane is independently bounded but currently clamps requests to
radius 2, or `5x5 = 25` chunks. A centered square cannot contain exactly 50 or
75 chunks, so the next symmetric footprints are radius 3 (`7x7 = 49`) and
radius 4 (`9x9 = 81`).

## Gesture Contract

When the second touch begins, the Lab captures:

- the initial terrain state and camera;
- initial finger distance;
- initial centroid; and
- the logical panel under that centroid.

Every move derives one state from that fixed gesture start:

1. zoom by `initial distance / current distance`;
2. in map view, anchor that zoom to the initial centroid;
3. pan the zoomed footprint by current centroid minus initial centroid; and
4. in 3D, transform that pan through the captured camera yaw.

Using one fixed gesture start avoids compounding rounding and asynchronous
React-state publication. A comparison gesture stays attached to the panel
where it began even if its centroid crosses the divider.

One-finger behavior remains unchanged: map drag pans, 3D drag orbits, and a
stationary primary touch inspects. A gesture that ever acquires a second
touch cannot become an inspection tap.

## Exact Footprint Contract

`CANONICAL_TERRAIN_MAX_CHUNK_RADIUS` becomes 4. The URL continues to store the
radius because it is the stable centered-footprint identity:

- radius 0: `1x1 = 1`;
- radius 1: `3x3 = 9`;
- radius 2: `5x5 = 25`;
- radius 3: `7x7 = 49`; and
- radius 4: `9x9 = 81`.

Generation remains center-first in the replaceable canonical Worker. Each
chunk appears as soon as it is generated and meshed. Seed, center, stage,
radius, or cache changes still replace the epoch and terminate the remaining
queue. This does not increase gameplay render distance or move canonical world
authority into the Lab.

## Ownership

- `tools/terrain-lab/src/state.ts` owns host-neutral pan/zoom composition.
- The procedural and canonical React canvases own pointer translation and
  gesture lifecycle.
- `mclone-terrain-view` owns the bounded centered canonical chunk order.
- `mclone-terrain-lab` continues to expose that shared order to the Worker
  host without duplicating it in TypeScript.

## Implementation And Acceptance

1. Add a unit-tested simultaneous pan/zoom state transform.
2. Use it in both procedural and canonical touch paths.
3. Expand shared radius validation and UI options through radius 4.
4. Pin center-first 81-chunk ordering in Rust.
5. Exercise simultaneous pan and zoom in map and 3D with real touch events.
6. Complete and inspect an 81-chunk exact footprint on a phone-sized browser.
7. Run local and hosted desktop/phone validation, deploy only Terrain Lab
   objects, and record the receipt here.

Commit each coherent slice with `Topic: gpu-procedural-terrain`.

## Implementation Receipt

The procedural and canonical canvases now use the same fixed-start gesture
transform. Moving a two-touch centroid pans while changing finger separation
zooms during that same gesture. Map view keeps the initial centroid over the
same terrain point; 3D view applies centroid motion in the captured camera's
ground plane without changing yaw or pitch. The gesture remains attached to
its starting comparison pane, suppresses page scrolling, and cannot fall
through to point inspection. Existing one-touch pan/orbit and mouse behavior
remain unchanged.

The shared exact-terrain bound is now radius 4. The UI presents the centered
square footprint explicitly as `1x1 (1)`, `3x3 (9)`, `5x5 (25)`, `7x7 (49)`,
and `9x9 (81)` chunks. Radius 3 and 4 requests retain center-first,
per-chunk publication and epoch cancellation; this is a Lab preview limit,
not a gameplay render-distance change.

Local acceptance passed:

- `cargo test --manifest-path native/Cargo.toml
  -p mclone-terrain-view --lib`: 16 tests;
- `pnpm terrain-lab:typecheck`;
- `pnpm --dir tools/terrain-lab test`: 16 tests; and
- `pnpm terrain-lab:web:test`: 6 passed and 2 deliberately skipped by
  viewport.

The browser suite drives real two-contact events through Chrome's touch input
path. It proves simultaneous center and scale changes in map and 3D on both
procedural and canonical canvases, unchanged 3D orbit, no page scroll, and no
accidental inspection. A separate phone-sized test observed intermediate
publication and completed all 81 final-feature chunks in about four seconds.
The resulting full `9x9` footprint and the standard mobile Lab view were
captured under `/tmp` and visually inspected.

The targeted upload changed only `/terrain/` objects. Immutable objects were
uploaded and downloaded for byte comparison before `terrain/index.html` was
switched. Production now serves:

- JavaScript `index-CIAiMsCY.js`, SHA-256
  `6a8976c88ff31118f452b01307aab379c48461c35d48ca3349b5a568d85ee96d`;
- canonical Worker `canonical-worker-_qmi-p7e.js`, SHA-256
  `c5085f86211ece037acfdd2ea2597bb7da59715a712ca78efdac323927f48ef8`;
- stylesheet `index-K-isbiIr.css`, SHA-256
  `6f5faf8a7f3a76004a68ceca6aba710a6c8c93afd6ab97e22985ffdab3fbdd48`;
- Wasm `mclone_terrain_lab_bg-C15BOEQ2.wasm`, SHA-256
  `5acbb57f51addacbb37acd676c82c26e8c8fdf47e8bcf35431696337b507749b`;
  and
- `terrain/index.html`, SHA-256
  `226aea09305ffa2ed1d092d7ca580ab97519947031928c1079252921d6488db2`.

Hosted headed-Wayland desktop and phone smokes passed at
`https://mclone.kzahel.com/terrain/`, including canonical completion,
procedural CPU/GPU agreement, exact/LOD navigation, hydrology receipts, and
the uncached stress race. The immutable bytes exercised by the local
multitouch and 81-chunk tests are the byte-verified hosted bytes; the hosted
standard smoke does not itself synthesize a two-contact gesture.

The implementation series is `ac5db621`, `75c04919`, and `30648f5e`, followed
by the documentation receipt commit.
