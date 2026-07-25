# Tactical 249: Cross-Platform Procedural Horizon Proof

Status: active 2026-07-25.

Topics:

- `procedural-horizon-clipmap`
- `world-view-navigation`
- `platform-host-boundary`
- `gpu-procedural-terrain`

Parent directions:

- [`procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  owns toroidal residency, nested levels, seams, exact-terrain replacement,
  and the eventual game/XR horizon;
- [`world-view-navigation.md`](../topics/world-view-navigation.md) owns the
  lightweight Explorer product and shared map/orbit controls;
- [`platform-host-boundary.md`](../topics/platform-host-boundary.md) owns the
  thin native, browser, Android, and XR adapter boundary;
- [`gpu-procedural-terrain.md`](../topics/gpu-procedural-terrain.md) owns the
  current procedural evaluator and Terrain Lab evidence; and
- [`247`](247-standalone-world-explorer-foundation.md) established the native
  World Explorer and dependency firewall, while
  [`248`](248-terrain-lab-navigation-and-worker-modernization.md) preserved
  Terrain Lab and modernized its browser runtime.

## Objective

Implement the first real moving procedural-horizon LOD system as a shared
engine facility, then prove that same Rust implementation through a
lightweight standalone Explorer on native desktop and in the browser.

This tactical must meet in the middle between a full game and a one-off web
demo:

```text
product/runtime axis

  lightweight Explorer     full game     Terrain Lab     tabletop
             \                 |              |              /
              +------ shared engine systems and scenes -----+
                              |
                    terrain horizon renderer
                              |
              +---------------+----------------+
              |               |                |
            browser       desktop/winit    Android/OpenXR

                         platform axis
```

Browser is a platform, not a reduced product profile. The browser platform
contract must be capable of hosting either the lightweight Explorer or the
entire game. Conversely, desktop and Android may host small tools or the full
game. Product dependency closure selects the systems included in an artifact;
platform adapters do not decide product semantics.

Completion requires:

1. a fixed-budget, toroidally addressed procedural terrain horizon whose
   residency behaves deterministically while stationary, moving across rows
   and columns, moving diagonally, crossing negative coordinates, and
   teleporting;
2. a shared Rust render/session owner with no DOM, `winit`, Android, OpenXR,
   React, or game-runtime dependency;
3. a lightweight Explorer application that consumes that owner through thin
   native and browser adapters;
4. inspected native/offscreen and headed-browser WebGPU pixels from equivalent
   pinned views;
5. diagnostics proving fixed allocation, bounded refill work, stable LOD
   coverage, and explicit recovery after discontinuous movement; and
6. a deployed browser proof whose JavaScript contains only platform mechanics
   and opaque observation forwarding.

## Originating Direction

Terrain Lab made the procedural terrain evaluator and trees visibly useful,
but it should not become the owner of the game-facing LOD system. Composing
separate React canvases or masking terrain in CSS would prove only a browser
presentation trick. The LOD system must instead be useful to the full web
game, desktop, Android, XR, lightweight hosted applications, and later Terrain
Lab views.

The standalone World Explorer is the smallest useful integration host. It
already proves that procedural terrain and shared view controls do not require
gameplay, AI, networking, audio, UI, or a server. This tactical extends that
proof across both native and browser targets rather than treating the Explorer
as a native-only diagnostic.

The requested implementation sequence is deliberate:

- prove the moving toroidal LOD system before exact-chunk replacement;
- prove it in a real shared renderer before adding Terrain Lab controls;
- keep the browser adapter capable of running the full engine;
- keep the Explorer artifact small by dependency selection, not by weakening
  the browser host contract; and
- preserve Terrain Lab unchanged as an independent visual baseline.

## Binding Architecture

### Independent product and platform axes

The implementation has three ownership layers:

1. **Shared engine systems.** Clipmap addressing, level selection, refill
   planning, terrain evaluation, draw ordering, seams, GPU resources, and
   diagnostics live in shared Rust crates.
2. **Product runtimes/scenes.** The lightweight Explorer selects terrain,
   navigation, minimal assets, and diagnostics. The full game may select the
   same terrain system alongside gameplay, lighting, AI, networking, and UI.
3. **Platform adapters.** Browser, desktop, Android, and OpenXR code owns only
   its surface/session, lifecycle, raw observations, platform services, and
   presentation mechanics.

Do not create an `ExplorerTerrainRenderer` that the game would need to import.
The shared type should describe a procedural horizon or terrain composition
session. The Explorer is merely its first cross-platform consumer.

Do not create an Explorer-specific browser host contract. Reuse or extract the
domain-blind parts of the existing full-game browser host where practical:
canvas and surface setup, animation cadence, resize and visibility facts, raw
input observations, opaque Worker construction, storage/network adapters, and
mechanical platform dispositions. A small Explorer entry point may instantiate
fewer services, but it must not assign terrain or camera meaning in
JavaScript.

### Shared application seam

The existing `mclone-world-explorer` native executable may gain a library or
shared runtime owner, or a narrowly named shared crate may be introduced if
the dependency graph requires it. The durable contract is:

- both native and Wasm instantiate the same Rust Explorer/session logic;
- browser and native adapters do not duplicate camera or terrain policy;
- the terrain horizon remains below the Explorer product layer so the game can
  consume it directly;
- WGPU surfaces remain platform-owned while device-independent planning and
  device-bound renderer state remain reusable; and
- a target-specific binary or Wasm artifact is allowed, but its semantic Rust
  implementation is shared.

### Toroidal clipmap contract

The first implementation uses fixed-capacity levels centered on a snapped
world-space origin. Each level records:

- cell spacing and covered world extent;
- a fixed two-dimensional physical allocation;
- the logical world-cell origin mapped into that allocation;
- valid rows/columns and pending refill bands;
- the inner hole reserved for the next-finer level;
- seam/skirt geometry and transition width; and
- monotonic update, refill, and discontinuity counters.

Moving within a finest-level cell changes only the view. Crossing a snapped
cell boundary produces the newly exposed row, column, or both while retaining
overlap. Teleports or changes larger than retained overlap explicitly rebase
the level rather than pretending to perform many incremental shifts.

Toroidal addressing must be defined with Euclidean arithmetic so negative
world coordinates behave identically to positive ones. CPU property tests
must establish:

- every logical cell maps to exactly one physical slot;
- retained cells keep their slot and generation after a one-cell shift;
- refill sets contain no duplicates, including diagonal corner cells;
- repeated shifts wrap in both axes without growing allocation;
- reverse movement restores the expected logical coverage;
- teleport/rebase replaces the full logical footprint exactly once; and
- level plans and diagnostics are deterministic.

The current evaluator may calculate height and material procedurally in GPU
shaders, so the first refill payload need not be a CPU height cache. The
residency and render topology must nevertheless expose real dirty bands and
fixed allocation rather than disguising a full per-frame remesh as a
clipmap.

### Renderer compatibility

The shared renderer must move toward normal engine contracts:

- reversed-Z depth, with clear zero and `GreaterEqual` comparison;
- caller-owned color/depth render targets or an explicit reusable target
  contract;
- stable world-coordinate evaluation at large positive and negative
  positions;
- explicit resource creation/rebuild and fixed-capacity receipts;
- one ordinary mono path suitable for both native and browser now; and
- no design that prevents later stereo, XR multiview, or exact-chunk drawing
  into the same target.

This tactical does not need to add lighting or copy the full game frame graph.
It must avoid conventions that would require a second procedural renderer for
the game.

### Browser boundary

The browser shell may:

- load the selected Wasm application;
- create or locate a canvas;
- initialize browser-required WebGPU and lifecycle mechanics;
- run `requestAnimationFrame`;
- observe resize, visibility, focus, pointer, wheel, touch, and keyboard data;
- perform pointer capture and browser-required `preventDefault`;
- create generic Workers or other platform services when Rust requests them;
- execute mechanical browser dispositions; and
- display opaque diagnostics returned by Rust.

It must not:

- understand clipmap levels, LOD selection, terrain sources, masks, skirts,
  camera gestures, or gameplay actions;
- schedule terrain generation or decide refill priority;
- contain a second view reducer;
- define a reduced host ABI which cannot also support the full game; or
- compose the result from multiple canvases or DOM layers.

The first procedural proof may not need Workers. Their omission from the small
artifact is a product capability decision, not a different browser
architecture.

## Implementation Slices

### Slice 0: record the boundary

- Add this tactical to the implementation index.
- Update the clipmap, navigation, GPU-terrain, and platform-host topics with
  the independent product/platform axes.
- Record Terrain Lab and exact composition as intentional follow-ups.

### Slice 1: shared toroidal planning and residency

- Add fixed-capacity level descriptors and toroidal address helpers in the
  shared terrain owner.
- Implement incremental row/column/diagonal refill plans and explicit rebases.
- Add deterministic property and scenario tests, including negative
  coordinates and large teleports.
- Add aggregate diagnostics suitable for native and Wasm receipts.

### Slice 2: shared moving horizon renderer

- Convert the procedural viewport renderer to the engine-compatible depth and
  render-target contract.
- Draw one fixed-budget level from the shared residency state.
- Add nested levels, fixed inner holes, and skirts/transitions.
- Preserve vegetation as a shared optional layer and make its coverage follow
  the same logical origin.
- Demonstrate that camera motion changes residency only at snapped boundaries.

### Slice 3: lightweight native Explorer adoption

- Keep `winit`, surface ownership, native asset acquisition, CLI, and capture
  in the native adapter.
- Route all horizon and navigation semantics through shared Rust.
- Add diagnostic movement scenarios for stationary, axial, diagonal,
  negative-coordinate, wrap, and teleport behavior.
- Capture and inspect window and offscreen output at the first drawable ring
  and after nested-level/seam work.

### Slice 4: lightweight browser Explorer adoption

- Add a Wasm entry point backed by the same Rust session and renderer.
- Use a minimal, domain-blind web bootstrap compatible with the full-game host
  boundary.
- Add browser URL/default view facts only where they are product inputs, not
  terrain semantics.
- Run local desktop and narrow/mobile headed-WebGPU interaction/capture
  smokes, then deploy the proof.

### Slice 5: closeout

- Record native/browser pixel evidence, clipmap receipts, dependency and
  artifact sizes, browser-host audit results, commands, and deployment.
- Update parent topic status and next work.
- Leave the worktree clean.

## Validation

Required shared tests:

- toroidal address and inverse-address properties;
- retained-slot and refill-set properties;
- diagonal corner deduplication;
- negative-origin and wrap behavior;
- teleport/rebase behavior;
- fixed allocation across long scripted movement;
- nested footprint continuity; and
- stable diagnostic serialization on native and Wasm.

Required native evidence:

- focused crate and Explorer checks/tests;
- dependency-firewall check;
- inspected offscreen capture;
- inspected real-window capture;
- scripted axial, diagonal, negative-coordinate, and teleport receipts; and
- proof that allocations remain constant while update counters change.

Required browser evidence:

- Wasm compilation and optimized web build;
- host-boundary test showing raw events reach Rust without semantic JavaScript;
- headed Wayland WebGPU capture on this Linux host;
- inspected desktop and narrow/mobile captures;
- movement receipt equivalent to the native scenario;
- payload-size receipt; and
- production deployment and hosted smoke.

## Explicit Non-Goals

Tactical 249 does not:

- modify Terrain Lab UI, React state, canvases, or Worker topology;
- compose exact chunks with procedural terrain;
- add exact coverage masks, frontier collars, or persisted-edit summaries;
- integrate the horizon into the full game scene;
- add gameplay, AI, full lighting, audio, networking, or a game HUD to the
  Explorer;
- finish stereo or XR multiview rendering;
- add a quadtree alternative; or
- claim that the first ring is the final 100–200 km quality configuration.

The next bounded tactical should prove exact/procedural replacement through
the same native and browser-capable shared session. Terrain Lab adoption and
full-game integration follow only after those shared proofs are credible.

## Completion Ledger

- [x] Slice 0: architecture and execution record.
- [x] Slice 1: shared toroidal planning and residency.
- [ ] Slice 2: shared moving horizon renderer.
- [ ] Slice 3: lightweight native Explorer adoption.
- [ ] Slice 4: lightweight browser Explorer adoption.
- [ ] Slice 5: evidence, deployment, topic closeout, and clean tree.

### 2026-07-25: shared residency checkpoint

`mclone-terrain-view::TerrainClipmap` now owns fixed-capacity level state,
Euclidean logical-to-physical addressing, incremental axial and diagonal
refill sets, explicit teleport rebases, nested footprint/hole facts, and
portable diagnostics. Eight focused tests cover initial allocation,
sub-cell retention, axial and diagonal bands, negative-coordinate wrapping,
retained-slot identity, teleport recovery, nested bounds, and a long
positive/negative walk. The controller allocates no platform or GPU objects
and is ready to drive both native and Wasm render sessions.
