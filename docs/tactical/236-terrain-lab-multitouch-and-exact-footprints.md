# Terrain Lab Multitouch And Exact Footprints

Status: active 2026-07-24.

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
