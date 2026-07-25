# Tactical 249: Cross-Platform Procedural Horizon Proof

Status: completed and deployed 2026-07-25.

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
- [x] Slice 2: shared moving horizon renderer.
- [x] Slice 3: lightweight native Explorer adoption.
- [x] Slice 4: lightweight browser Explorer adoption.
- [x] Slice 5: evidence, deployment, topic closeout, and clean tree.

### 2026-07-25: shared residency checkpoint

`mclone-terrain-view::TerrainClipmap` now owns fixed-capacity level state,
Euclidean logical-to-physical addressing, incremental axial and diagonal
refill sets, explicit teleport rebases, nested footprint/hole facts, and
portable diagnostics. Eight focused tests cover initial allocation,
sub-cell retention, axial and diagonal bands, negative-coordinate wrapping,
retained-slot identity, teleport recovery, nested bounds, and a long
positive/negative walk. The controller allocates no platform or GPU objects
and is ready to drive both native and Wasm render sessions.

### 2026-07-25: first drawable native horizon

`TerrainHorizonRenderer` now binds ten shared toroidal levels to `160`
preallocated GPU slots and refills them coarsest-first in bounded batches of
`16`. Ready coarser levels remain visible until the next finer level is
complete; the render shader then cuts the exact finer footprint from the
coarser level. The same shared shader and renderer now use reversed-Z
(`GreaterEqual`, clear zero), and the Explorer depth validator follows that
contract.

The inspected first offscreen capture at
`/tmp/mclone-world-explorer-clipmap-first.png` was visually coherent at
`1280x720`, with no blank rings or rectangular coarse/fine holes. It reached
all `160/160` ready slots in ten frames, reported a fixed `173,079,040`-byte
allocation, and produced meaningful reversed depth over `875,273` pixels with
a `0.000000..0.934411` range on the Radeon 890M Vulkan adapter. Vegetation,
explicit frontier skirts, movement receipts, and browser proof remain open.

### 2026-07-25: final shared renderer shape

The final GPU-only horizon allocation removed the unused per-slot canonical
reference buffer. Ten levels of `4x4` slots now retain exactly `160` terrain
sample buffers and uniforms totaling `86,551,040` bytes. The renderer admits
at most `16` GPU terrain refills per frame, draws only complete levels, and
falls back to the next complete coarser level while a finer level is
incomplete.

Procedural levels use power-of-two aligned footprints and one fixed
rectangular inner-hole test per coarse level. This is not a quadtree with
several changing cutouts. Complete finer-level publication and the aligned
ownership boundary produced continuous inspected pixels across stationary,
moving, negative-coordinate, and teleport views. No explicit vertical skirt
was needed to hide a procedural/procedural crack in these captures. The
exact/procedural frontier still needs the planned collar or skirt because its
independently generated surfaces will not share this alignment guarantee.

Stable tree records are an optional layer on the same shared renderer. Native
Explorer refills one vegetation tile per frame over the three levels whose
sample spacing is at most four. Tree fragments obey the same nested inner
holes as terrain. Terrain allocation remains fixed; variable tree-instance
bytes are reported separately. A native movement checkpoint drew `5,267`
tree proxies using `505,632` bytes beyond the fixed terrain allocation.

The first browser experiment exposed an important limit rather than a reason
to extend a timeout: synchronous tree-record compilation took about two
seconds per tile in Wasm and blocked the browser main thread. Browser Explorer
therefore instantiates the shared renderer with optional vegetation disabled.
It does not ship the observed stall. Browser tree records remain follow-up
work for the shared Worker/job contract; terrain generation itself remains
the same GPU path as native.

### 2026-07-25: shared session and platform adapters

`WorldExplorerSession` now owns view reduction, contact gestures, clipmap
replanning, readiness, diagnostics, and frame encoding in ordinary Rust. The
native adapter supplies filesystem asset packs, native elapsed time, `winit`
events, its surface, and capture mechanics. The Wasm adapter supplies fetched
pack bytes, browser elapsed time, WebGPU surface acquisition, and raw DOM
observations.

The standalone browser shell is one canvas and no React. Its JavaScript loads
opaque resources and Wasm, follows browser resize/visibility/rAF rules,
forwards raw pointer, wheel, and key observations, and displays a Rust-authored
report. It contains no clipmap, terrain-source, camera-gesture, LOD, or refill
policy. This small product does not import the full game, but nothing in the
browser platform boundary assumes that browser means a reduced product; the
full game remains another Rust dependency closure over the same kind of
physical host.

The dependency firewall now evaluates native and
`wasm32-unknown-unknown` graphs separately. It passed with `159` native and
`75` browser packages while excluding game-runtime, server, scene, XR, and
cross-target platform leakage.

### 2026-07-25: native evidence

`pnpm native:world-explorer:smoke` completed the same scripted sequence through
an offscreen target and a real `winit` surface:

- initial 3D, continuous X, continuous Z, and diagonal movement;
- focus at `(-8193, -4097)`;
- teleport to `(1000000, -1000000)`;
- anchored zoom, map, and changed orbit view; and
- six color/depth checkpoints in each target.

Both receipts recorded `344` frames, `811` total refills, `26` level rebases,
`160/160` final slots, zero final terrain work, and exactly `86,551,040`
fixed terrain bytes at every frame. Peak total residency was `87,056,672`
bytes with vegetation. The offscreen first coarse/target times were
`22.95/218.82 ms`. Movement frames averaged `4.20 ms`, reached `11.48 ms`
P95, and peaked at `14.48 ms` on the Radeon 890M Vulkan adapter.

The window and offscreen PNG hashes matched at all six checkpoints. Reversed
depth covered `875,274` pixels in the initial 3D view, `875,148` at negative
coordinates, and `875,421` after teleport. The movement, negative, teleport,
map, and final orbit captures were inspected. They contain no unpainted ring,
stale rectangle, or visible fine/coarse crack.

### 2026-07-25: browser and deployment evidence

`pnpm host:check` selected headed Chrome over Wayland `wayland-0` with hardware
WebGPU. Local desktop and Pixel-sized release smokes then passed initial
rendering, a real pointer drag, keyboard movement, negative coordinates, and
teleport. Their inspected initial and teleport images show continuous
coverage in landscape and narrow portrait presentations.

Each browser checkpoint retained the same `160` allocation slots and
`86,551,040` fixed bytes. Initial fill recorded `160` refills and `10`
rebases. The keyboard move retained `92` tiles and reached `228` total
refills. The negative relocation retained `26` tiles and reached `362`
refills. The million-block teleport retained zero tiles, reached `522`
refills and `28` rebases, then returned all ten levels and 160 draws to ready
state.

The optimized artifact contains:

- `1,551,748` bytes of Wasm;
- `67,201` bytes of generated Wasm JavaScript; and
- `6,008` bytes of hand-authored opaque host JavaScript.

The aggregate production bundle preserved the Explorer Wasm SHA-256
`eccb0267d580684ec71ceda76827a898f3d4fd897f86d7c371829445a9956d6e`
under `/explore`. Cloudflare Worker version
`3cb53a27-9f35-4fbc-929c-295fc0b833c1` serves
`https://mclone.kzahel.com/explore/`. A direct hosted headed-Wayland smoke
passed the complete pointer, movement, negative, and teleport sequence with
no page or console errors.

### Validation commands

- `cargo test --manifest-path native/Cargo.toml -p mclone-terrain-view --lib`
- `cargo check --manifest-path native/Cargo.toml -p mclone-world-explorer`
- `cargo check --manifest-path native/Cargo.toml -p mclone-world-explorer --target x86_64-unknown-linux-gnu`
- `cargo check --manifest-path native/Cargo.toml -p mclone-world-explorer --lib --target wasm32-unknown-unknown`
- `pnpm native:world-explorer:deps`
- `pnpm native:world-explorer:smoke`
- `pnpm native:world-explorer:web:build`
- `pnpm native:world-explorer:web:smoke -- --skip-build`
- `pnpm native:world-explorer:web:smoke:mobile -- --skip-build`
- `./scripts/deploy-native-web.sh --bundle-only`
- `./scripts/deploy-native-web.sh`
- `WORLD_EXPLORER_SMOKE_BASE_URL=https://mclone.kzahel.com/explore node native/apps/mclone-world-explorer/scripts/browser-smoke.mjs --skip-build`

### Handoff

The next tactical should compose exact terrain into this shared session using
one immutable exact-painted snapshot, one procedural coverage mask, and an
explicit exact/procedural frontier collar or skirt. It should preserve the
existing native and browser receipts while adding exact admission and
eviction.

Independent hardening can add browser Worker-backed tree records, retained
committed origins instead of coarse fallback during every refill, device-loss
rebuild evidence, camera-relative large-coordinate precision, stereo and
multiview, and device-specific clipmap budgets. None requires moving terrain
or navigation semantics into the browser shell or Terrain Lab.
