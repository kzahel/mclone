# Tactical 250: Continuous Explorer Presentation And Cadence

Status: active 2026-07-26.

Topics:

- `world-view-navigation`
- `procedural-horizon-clipmap`
- `platform-host-boundary`

Parent directions:

- [`world-view-navigation.md`](../topics/world-view-navigation.md) owns shared
  map, orbit, gesture, and frame-rate-independent view manipulation;
- [`procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  owns snapped toroidal residency and camera-relative large-coordinate
  presentation;
- [`platform-host-boundary.md`](../topics/platform-host-boundary.md) owns the
  thin browser-rAF and native-winit loops; and
- [`249`](249-cross-platform-procedural-horizon-proof.md) completed and
  deployed the first fixed-budget native/browser procedural horizon.

## Objective

Make Explorer navigation visually continuous without importing the complete
game runtime into the lightweight product.

The implementation must distinguish three independent cadences:

```text
raw input observations
        |
        v
continuous shared view state       every input/frame as appropriate
        |
        +----> presentation         every admitted display frame
        |
        +----> clipmap residency    only at snapped tile boundaries
```

Fractional pan, two-contact strafe, anchored zoom, and held keyboard movement
must update presentation every frame. They must not be rounded to block
coordinates merely because procedural sample identity and clipmap storage use
integers.

Native and browser hosts must continue to use the same Rust view and terrain
session. Browser JavaScript forwards physical observations and rAF time only.
Native owns winit and surface mechanics only. Neither host gains terrain,
gesture, direction, speed, or residency policy.

## Originating Direction

The first deployed horizon proved fixed allocation, toroidal updates, broad
movement, negative coordinates, teleports, and a small product dependency
closure. Interactive use then exposed three presentation defects:

1. `WorldViewState` retained fractional `f64` focus and scale, but
   `WorldExplorerSession` passed rounded `i32` centers and `u32` extents to the
   renderer. Two-finger strafe and close navigation therefore appeared
   quantized.
2. The browser treated each arrow-key press as an immediate move by one eighth
   of the visible footprint and ignored held/released cadence. A 4,096-block
   view consequently moved 512 blocks per press.
3. Native held movement used render-time deltas, but the first frame after an
   idle period could consume the maximum clamped delta, and the surface
   preferred non-vsync presentation.

The prior terrain-following camera target was independently corrected to use
the profile sea-level datum. This tactical preserves that stable vertical
contract while fixing horizontal and scale continuity.

These are not disposable standalone-app polish issues. Continuous
camera-relative presentation, snapped residency, and frame-rate-independent
view motion are required by the eventual full game, tabletop, browser,
Android, and XR consumers of the same horizon.

## Binding Architecture

### Continuous presentation versus snapped residency

The shared session retains `f64` view facts:

- focus X and Z;
- horizontal blocks across;
- aspect-derived vertical blocks;
- yaw and pitch; and
- projection mode.

The renderer receives those presentation facts every frame. It independently
retains the integer source and clipmap coverage required to populate fixed GPU
slots.

The shader must not cast a large absolute fractional camera coordinate
directly to `f32`. Pack:

- an integer camera anchor near the view center;
- a small fractional X/Z remainder;
- continuous view width and height; and
- integer sample positions.

Vertex-relative position is:

```text
f32(world_integer - camera_anchor) - camera_fraction
```

Changing the anchor at a whole-block boundary must be algebraically
continuous. Large positive and negative centers must retain the existing
integer subtraction before float conversion.

Clipmap residency changes only when the continuous focus crosses the finest
tile footprint. Because every coarser tile boundary is aligned to that
power-of-two footprint, this is sufficient to discover changes at every
level. Fractional motion, whole-block motion inside the same tile, orbit-only
changes, and zoom-only changes must not increment residency revision or
schedule refills.

### Shared held motion

`mclone-view-control` gains a small sans-I/O held-motion owner. It records
abstract view directions and produces `PanWorld` distance from:

- normalized direction;
- current blocks across;
- elapsed frame seconds; and
- one shared footprints-per-second constant.

It must be deterministic across frame partitions. Advancing one second at
60 Hz and 120 Hz must produce the same final focus within floating-point
tolerance. Starting motion after an idle period begins with a zero-duration
frame rather than consuming accumulated idle time. Large or invalid deltas
are bounded.

Native converts winit key codes to shared directions. Browser Rust converts
opaque `KeyboardEvent.code` strings to the same directions. Key repeat is
idempotent; keyup releases state; blur/cancellation clears all held motion.
JavaScript does not own a movement timer, distance, or direction table.

### Physical frame loops

Do not import `mclone-scene`, `mclone-app-runtime`, the full-game input router,
or the game web shell.

The loops remain intentionally asymmetric:

- browser uses `requestAnimationFrame`, forwards its monotonic timestamp, and
  renders the shared session;
- native uses winit redraw demand and a vsynced surface by default;
- both keep drawing while shared motion or terrain refinement is active; and
- native returns to `ControlFlow::Wait` when idle.

The full game may later consume the same view-motion and horizon-presentation
contracts through its own scene and pacing policy. Physical surface cadence
remains host-owned.

## Implementation Slices

### Slice 0: record the correction

- Add this tactical and index entry.
- Update navigation and clipmap topics with the continuous-presentation
  contract.
- Preserve Tactical 249 as the completed initial proof record.

### Slice 1: continuous shared presentation

- Add one explicit horizon presentation value carrying continuous focus and
  extents.
- Extend the shared uniform layout with camera-anchor remainder and continuous
  extent facts.
- Update terrain and tree shaders to use the same continuous camera-relative
  transform.
- Keep ordinary Terrain Lab viewport rendering on its existing integer request
  path.
- Add packing, anchor-crossing, large-coordinate, and shader validation tests.

### Slice 2: residency decoupling

- Replan the horizon only when the continuous focus crosses a finest-tile
  boundary or source identity changes.
- Remove zoom and sub-tile position from the residency key.
- Prove fractional pan, whole-block same-tile pan, orbit, and zoom do not
  change allocation, residency revision, or refill totals.
- Preserve negative-coordinate and teleport behavior.

### Slice 3: shared held movement

- Add the view-specific held-direction reducer to `mclone-view-control`.
- Move native and browser Explorer movement state into the shared session.
- Advance motion from the same elapsed-frame contract in both hosts.
- Delete the browser one-eighth-footprint step and native idle-delta path.
- Forward rAF time to Rust and clear motion on release, blur, or cancellation.

### Slice 4: physical cadence and interaction evidence

- Select FIFO/vsync first for the native Explorer surface.
- Extend reports with exact continuous focus/scale facts needed by acceptance
  tests.
- Add a real browser held-key sequence and two-contact strafe sequence.
- Capture and inspect native/offscreen and headed-browser pixels after
  continuous movement.

### Slice 5: closeout

- Run focused Rust, native, Wasm, dependency-firewall, native-window,
  offscreen, desktop-browser, and mobile-browser gates.
- Record allocation, residency, cadence, artifact, and pixel evidence here and
  in the living topics.
- Deploy `/explore`, verify the hosted artifact hash, run the hosted smoke, and
  leave a clean worktree.

## Validation

Required shared tests:

- camera-anchor crossing is continuous on both axes;
- fractional presentation survives uniform packing;
- large positive and negative centers retain a small fractional remainder;
- continuous zoom affects projection without changing residency;
- sub-tile movement causes zero refills and zero residency revision changes;
- tile-boundary movement produces the expected toroidal update;
- held movement is partition-independent at 60 and 120 Hz;
- diagonal held movement is normalized;
- idle-to-held transition contributes no stale elapsed time; and
- cancellation clears every direction.

Required rendered evidence:

- native real-window and offscreen initial/moved captures;
- native held movement with fixed allocation;
- headed browser pointer orbit, two-contact strafe, held keyboard movement,
  negative coordinates, and teleport;
- narrow/mobile two-contact interaction;
- no camera jump while the integer anchor changes;
- no procedural hole or seam after tile-boundary movement; and
- identical fixed allocation before and after presentation-only movement.

Required boundary evidence:

- dependency firewall still excludes game, server, scene, XR, and app-runtime
  dependency closure;
- browser shell contains no movement speed, terrain, camera, or residency
  semantics;
- native and Wasm checks pass from the same Rust session; and
- production Wasm matches the locally validated artifact.

## Non-Goals

This tactical does not add:

- exact chunks or the exact/procedural mask;
- inertia, kinetic scrolling, or gesture smoothing;
- gamepad support;
- game UI, settings, FPS selectors, or timing overlays;
- browser Worker-backed vegetation;
- Android or XR application shells;
- a universal platform event ABI; or
- the full game's scene, runtime, simulation, or input contexts.

Coalesced pointer events or presentation interpolation may be considered only
if inspected interaction remains visibly stepped after integer presentation
quantization is removed.

