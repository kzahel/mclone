# Tactical 247: Standalone World Explorer Foundation

Status: active 2026-07-25.

Topic: `world-view-navigation`

Parent directions:

- [`world-view-navigation.md`](../topics/world-view-navigation.md) owns shared
  map/orbit/focus/zoom semantics and the Explorer-to-play product path;
- [`procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  owns the later toroidal horizon, native renderer proof, exact handoff, and
  XR-safe residency contract;
- [`gpu-procedural-terrain.md`](../topics/gpu-procedural-terrain.md) owns the
  current Terrain Lab evaluator, renderer evidence, and canonical comparison
  research; and
- [`platform-host-boundary.md`](../topics/platform-host-boundary.md) owns the
  rule that app crates adapt surfaces and events without acquiring engine
  policy.

## Objective

Create the first independent player-useful consumer of the shared procedural
terrain stack:

```text
mclone-worldgen
        |
        v
mclone-terrain-view <--- mclone-view-control
        |
        +-- mclone-world-explorer    standalone native host now
        +-- mclone-terrain-lab       existing browser diagnostic host
        +-- later minimal web shell  follow-up after input audit
        +-- later game scene         follow-up
```

The slice adds a standalone native desktop `mclone-world-explorer` binary,
introduces a pure shared view-control reducer, and drives the native host
through that reducer. It proves that broad terrain viewing is a small
composable application rather than another startup mode of the full game.
The contracts remain browser-capable, but this tactical does not choose or
implement the browser input shell.

Completion requires:

- a native window rendering the existing shared procedural terrain and tree
  presentation;
- deterministic shared map/orbit/pan/zoom/focus behavior;
- a dependency and artifact-size receipt proving the Explorer does not pull
  in the game runtime; and
- inspected native window and offscreen pixels from the same pinned view.

This tactical establishes the independent native seam needed by later clipmap
and browser work. It does not implement a toroidal ring, a new web shell,
Terrain Lab input migration, exact chunks, or enter-world authority.

## Originating Direction

Terrain Lab began as a biome and terrain-generation workspace. Its bounded
terrain view is now useful enough to suggest a Google-Maps-like product:
players can inspect a seed at continental scale, move into 3D, choose an
interesting region, and eventually enter the world.

That usefulness should be proven outside both the browser diagnostic UI and
the full game executable. A separate binary gives the architecture a concrete
test:

- terrain presentation must compose without scene, server, physics, audio,
  networking, persistence, or gameplay startup;
- the product can have a smaller executable, asset set, startup path, and
  later Wasm payload;
- native WGPU behavior becomes observable before integrating a new horizon
  into the game and XR;
- the Explorer remains useful as a standalone map application; and
- the full game can later embed the same lower-level services without owning
  their only implementation.

The separate binary is not useful evidence if it merely links the full engine
and selects a different mode in `main`. Its narrow dependency graph and
lifecycle are part of the product contract.

## Current Evidence

The reusable boundary already exists in substantial form:

- [`mclone-terrain-view`](../../native/crates/mclone-terrain-view/Cargo.toml)
  has no DOM, `winit`, OpenXR, client, server, scene, or application-runtime
  dependency;
- [`TerrainViewportRenderer`](../../native/crates/mclone-terrain-view/src/viewport_renderer.rs)
  accepts ordinary WGPU device, queue, color-target, size, request, and camera
  inputs;
- [`mclone-terrain-lab`](../../native/apps/mclone-terrain-lab/Cargo.toml)
  already keeps `web-sys`, `wasm-bindgen`, canvas, and browser surface
  dependencies target-gated;
- [`web.rs`](../../native/apps/mclone-terrain-lab/src/web.rs) owns the
  `HtmlCanvasElement`, WGPU surface acquisition, browser timing, Wasm string
  parsing, and JSON receipts around the shared renderer; and
- [`state.ts`](../../tools/terrain-lab/src/state.ts) owns pure navigation math
  while
  [`TerrainCanvas.tsx`](../../tools/terrain-lab/src/web/TerrainCanvas.tsx) and
  [`CanonicalTerrainCanvas.tsx`](../../tools/terrain-lab/src/web/CanonicalTerrainCanvas.tsx)
  duplicate pointer/gesture lifecycle.

There is no native terrain-view application today. The current preview
renderer also uses its own conventional `LessEqual` depth path and
single-view pipelines. Therefore a successful native window is a portability
milestone, not yet evidence for the later game-compatible reversed-Z,
synthetic-stereo, or XR multiview renderer.

## Binding Architecture Decisions

### Separate leaf application

Add:

```text
native/apps/mclone-world-explorer/
  Cargo.toml
  src/main.rs
```

Register it as a workspace member and add the smallest useful package scripts
for check, run, and captured smoke behavior.

The application owns only:

- `winit` event-loop and desktop-window lifecycle;
- WGPU instance, surface, adapter, device, queue, resize, and presentation;
- native asset-source selection;
- raw desktop event translation into shared view intents;
- CLI/default initial view selection;
- title/diagnostic reporting; and
- native capture/smoke glue.

It does not own terrain generation, sample residency, camera mathematics,
gesture policy, material semantics, or future clipmap behavior.

Do not turn `mclone-terrain-lab` into a native product and do not make the new
application depend on the Terrain Lab app crate. The Lab remains a developer
comparison host. The Explorer is the beginning of a player-facing product.

### Explicit dependency firewall

The initial direct application dependency set should remain near:

- `anyhow`, `env_logger`, `log`, and `pollster`;
- `wgpu` and `winit`;
- `mclone-assets` and `mclone-mesh` for native material preparation;
- `mclone-terrain-view` and `mclone-worldgen`; and
- `mclone-view-control`.

Exact low-level support dependencies may change if justified by the
implementation. The completed dependency graph must not contain:

- `mclone-app-runtime`;
- `mclone-client` or `mclone-server`;
- `mclone-scene`;
- `mclone-net` or `mclone-protocol`;
- `mclone-physics`;
- `mclone-audio`;
- `mclone-ui`;
- `mclone-xr-*` or OpenXR; or
- a dependency on either the native or web game-client application.

The native graph must not contain `web-sys`, `wasm-bindgen`, or browser Worker
glue. Add an executable dependency audit so this remains enforced rather than
merely documented.

Do not force a broadly reusable helper into a new crate before two consumers
need it. In particular, browser byte-pack plumbing and native filesystem asset
selection may remain separate adapters. Share prepared material and terrain
contracts, not host-specific asset acquisition.

### Shared view-control owner

Add a small sans-I/O crate:

```text
native/crates/mclone-view-control/
```

It owns deterministic navigation state and reduction only. Its first contract
includes:

```text
WorldViewState {
  mode: Map | Orbit
  focus_x
  focus_z
  blocks_across or equivalent logarithmic scale
  yaw
  pitch
  projection
}

WorldViewIntent {
  Orbit
  GrabPan
  AnchoredZoom
  FocusAt
  Recenter
  Tap
  DoubleTap
  CancelContacts
}
```

Names and exact numeric representation may change during implementation.
Durable requirements are:

- no WGPU, WGPU resources, worldgen, renderer, scene, server, DOM, `winit`,
  Android, or OpenXR dependency;
- explicit map/orbit conventions and constraints;
- cursor- or centroid-anchored logarithmic zoom;
- pinch zoom with simultaneous centroid pan;
- tap/double-tap/drag discrimination and cancellation;
- pointer-count transition handling;
- frame-rate-independent damping if inertia is enabled; and
- an event and state representation that can compile for Wasm without
  importing a browser event type into the reducer.

Platform adapters retain pointer capture, context-menu suppression, browser
scroll prevention, and native event translation. This tactical implements
only the `winit` adapter. It does not decide whether future browser contacts
should travel through `mclone-input`, a shared platform-input shell, or a
narrow Explorer adapter; that decision follows a focused audit of the main
game and Terrain Lab browser rims. The reducer does not issue a teleport,
mutate a world, or decide what a selected block means.

### One terrain-view request

The Explorer consumes one typed host-neutral request for the shared
procedural pane. The request is deliberately suitable for later Terrain Lab,
minimal-web-shell, and game consumers. It includes at least:

- source/profile identity and revision;
- seed;
- center X/Z;
- horizontal footprint;
- logical viewport size;
- map/3D view;
- yaw, pitch, and projection;
- content stage and presentation layer; and
- bounded detail/tile budget.

Do not refactor Terrain Lab's Wasm string parsing or JSON reports merely to
complete this native slice. The browser input-shell follow-up will decide how
typed requests and contact observations cross the Wasm boundary after it
compares the main game and Lab adapters.

Do not move WGPU `Surface` ownership into `mclone-terrain-view`.
`TerrainViewportRenderer::encode` or its successor continues to consume a
host-provided render target.

### First standalone product surface

The first binary deliberately presents only the production-derived procedural
view:

- original Mclone terrain profile;
- GPU procedural terrain source;
- rendered terrain layer;
- map and 3D modes;
- forest summaries/tree instances already supported by the shared renderer;
  and
- continuous movement through center and scale changes.

CLI options should cover at least seed, center X/Z, blocks across, map/3D
mode, and native asset root/profile. Product defaults should open a useful
view without arguments. Window-title or stdout diagnostics should expose seed,
center, footprint, adapter/backend, frame readiness, resident bytes, and
pending work.

Exact/canonical chunks, CPU/GPU comparison, diagnostic field layers, and
developer tuning controls remain Terrain Lab capabilities. Avoid turning the
first native host into a clone of the Lab control rail.

### Smallness is measured

Record:

- `cargo tree -p mclone-world-explorer -e normal`;
- forbidden dependency audit results;
- unstripped and stripped release executable sizes where supported;
- separately packaged asset bytes;
- cold process-to-first-coarse-frame time;
- process-to-target-ready time;
- peak reported terrain-view resident bytes;
- adapter/backend; and
- idle and continuous-movement frame statistics.

Executable size alone is not the architectural gate because WGPU backend and
platform libraries can dominate it. The dependency graph, absence of game
service construction, asset footprint, and startup work together establish
the proof.

## Implementation Slices

### Pinned baseline

Slice 0 started from commit `6c3623b3` with this repeatable evidence state:

- the delivered Terrain Lab browser assets were 4,248,122 bytes of Wasm,
  312,285 bytes of main JavaScript, 52,208 bytes of canonical-worker
  JavaScript, 50,631 bytes of LOD-worker JavaScript, 16,081 bytes of CSS, and
  856 bytes of HTML;
- `mclone-terrain-view` had normal direct dependencies only on `mclone-core`,
  `mclone-worldgen`, and WGPU;
- the native first-party authored pack was 7,945,702 bytes, while the
  generated-fallback and diagnostic-missing packs were 367,463 and 370,428
  bytes respectively; and
- the comparison view is seed 12345, center X/Z 0/0, 4,096 blocks across,
  1,280 by 720 pixels, original Mclone overworld GPU terrain, rendered
  presentation, cover content, with both map and 3D camera evidence.

The browser-input follow-up begins from
`native/apps/mclone-web-client/www/mclone-web-input.ts`,
`native/apps/mclone-web-client/www/mclone-web-touch.ts`,
`native/apps/mclone-web-client/www/mclone-web-app.ts`,
`tools/terrain-lab/src/web/TerrainCanvas.tsx`,
`tools/terrain-lab/src/web/CanonicalTerrainCanvas.tsx`, and the current pure
camera helpers in `tools/terrain-lab/src/state.ts`.

### Slice 0: baseline and source-shape guard

1. Record the current Terrain Lab Wasm/module and delivered asset sizes.
2. Record the current `mclone-terrain-view` dependency graph.
3. Add the proposed Explorer dependency allow/deny audit before substantial
   app code lands.
4. Pin one seed, center, footprint, view, and camera for repeatable native
   evidence and later cross-host comparison.
5. Record the main game and Terrain Lab browser input entry points as
   follow-up audit targets without refactoring them.

This establishes evidence boundaries without rearranging the existing crates.

### Slice 1: fixed native pixels

1. Add the workspace application and a minimal native WGPU surface host.
2. Load a native first-party material profile through existing asset/mesh
   APIs.
3. Construct `TerrainViewportRenderer`, submit one typed viewport request, and
   draw a fixed 3D view.
4. Handle resize plus lost/outdated surface recovery.
5. Add a deterministic captured smoke path under `/tmp`.

The first inspected image is required before adding interactive controls.
This slice should move no browser surface code into a shared crate.

### Slice 2: shared view control

1. Extract and specify the useful behavior from Terrain Lab's zoom, pan, grab,
   pinch, and orbit functions.
2. Implement the pure reducer and deterministic Rust tests.
3. Translate native mouse, wheel/trackpad, keyboard, and relevant touch
   events into semantic intents.
4. Drive Explorer center, footprint, and camera entirely from the reducer.
5. Verify redraw admission follows state and terrain readiness rather than
   spinning avoidable work while idle.

### Slice 3: smallness and native acceptance

1. Run the dependency audit and record the final tree.
2. Record release executable, asset, startup, readiness, residency, and frame
   evidence.
3. Capture and inspect native map and 3D views.
4. Exercise the pinned movement, zoom, map, and orbit sequence through both
   the real window and offscreen target.
5. Update the parent topics with implemented status, evidence, remaining
   ownership questions, and the next tactical boundary.

### Planned browser follow-up boundary

Tactical 247 records but does not implement the following sequence:

1. audit the main game browser input rim, `mclone-input`, Terrain Lab pointer
   code, and the native Explorer adapter;
2. decide whether raw contact collection belongs in an existing shared shell,
   a new neutral `mclone-input` contract, or thin product-specific adapters
   above one intent reducer;
3. build the minimal Web Explorer only after that decision, with TypeScript
   limited to canvas/rAF/lifecycle and raw input forwarding; and
4. migrate Terrain Lab navigation through the selected boundary rather than
   creating a third browser input implementation.

The follow-up must preserve the main game's thin browser rim and must not
adopt Terrain Lab prototype code as the default merely because it already
exists.

## Validation

### Shared unit and source-shape gates

- `mclone-view-control` reducer tests;
- `mclone-terrain-view` planning and renderer tests;
- negative/large center coordinates;
- map/orbit direction and pitch constraints;
- anchored wheel and pinch zoom;
- simultaneous pinch centroid pan;
- tap, double-tap, drag, pointer-count transitions, and cancellation;
- a Wasm compile check for `mclone-view-control` without a browser host;
- workspace formatting and clippy for changed crates; and
- the Explorer forbidden-dependency audit.

### Native rendered evidence

Add a focused package smoke command that:

- builds the release Explorer;
- opens or drives the real native WGPU path;
- renders pinned map and 3D states;
- advances continuous X, Z, diagonal, zoom, and orbit input;
- writes captures and a structured receipt beneath `/tmp`;
- validates non-empty color and meaningful depth; and
- reports bounded resident/pending work.

Inspect the first fixed image before Slice 2. Inspect the final map, 3D, and
movement images before completion.

### Platform boundary

This tactical adds a desktop application and a host-neutral crate. It does not
add Android or XR application behavior. Validate:

- native desktop build and rendered smoke on the current host;
- compile checks on other available desktop toolchains when practical;
- `wasm32-unknown-unknown` compilation of the shared view-control crate;
- the existing thin-adapter/source-shape gate if shared host boundaries move;
  and
- the normal game or Terrain Lab gates only if their shared render, asset, or
  browser code actually changes.

Do not claim Web, Android, or XR Explorer support from host-neutral
compilation alone.

## Completion Gates

The tactical is complete only when:

1. `mclone-world-explorer` runs without constructing or linking game runtime
   services;
2. its dependency audit fails if a forbidden game crate is introduced;
3. a no-argument launch reaches a useful procedural terrain view;
4. native map/3D navigation is driven by `mclone-view-control`;
5. native window and offscreen paths exercise the same renderer and state;
6. the shared controller compiles for Wasm without browser dependencies;
7. native screenshots are inspected;
8. dependency, executable, asset, startup, readiness, residency, and movement
   receipts are recorded; and
9. both parent topics identify the browser-input audit and clipmap boundaries.

## Non-Goals

- toroidal addressing or geometry-clipmap rings;
- game-scene integration;
- exact/procedural coverage masks;
- exact chunks in the Explorer;
- server admission or **Enter Here**;
- remote preview descriptors or hidden-seed handling;
- persisted terrain edits or distant build proxies;
- a minimal Web Explorer shell or deployment;
- Terrain Lab input migration;
- refactoring the main game's browser input shell;
- polished player-facing web UI;
- aircraft physics;
- full lighting parity;
- Android, XR, stereo, or multiview Explorer presentation; and
- seamless GPU-device transfer from Explorer into the game.

## Risks And Controls

### Accidental second terrain engine

Control: the Explorer depends on `mclone-terrain-view`; it does not copy WGSL,
sampling, residency, or vegetation logic.

### Native host grows engine policy

Control: source-shape and dependency gates keep `main.rs` limited to lifecycle,
surface, assets, event adaptation, and reporting.

### Broad speculative refactor delays pixels

Control: draw a fixed native image before extracting the shared controller.
Move only duplication proven by the second consumer.

### Native input accidentally becomes the universal shell

Control: keep platform types out of `mclone-view-control`, treat the `winit`
collector as a leaf adapter, and require the browser follow-up to compare the
main game, Terrain Lab, and Explorer input rims before selecting a shared
contact ABI.

### Product naming hides a diagnostic-only result

Control: require useful no-argument terrain presentation and interactive
navigation, while clearly deferring polished onboarding and enter-world
behavior.

### Existing preview renderer is mistaken for the game horizon

Control: receipts and documentation call it the current bounded preview
renderer. The next clipmap tactical must separately prove reversed-Z,
timestamps, stereo/multiview, and toroidal residency.

## Commit Sequence

Commit coherent slices with:

```text
Topic: world-view-navigation
```

Suggested sequence:

1. lock Tactical 247, dependency rules, and pinned evidence state;
2. add fixed native Explorer rendering and capture;
3. add the shared view-control reducer and tests;
4. adopt shared controls in the native host;
5. record native dependency, size, startup, and rendered closeout.

Do not combine the first toroidal ring with this series. It needs its own
measurements and commit thread under:

```text
Topic: procedural-horizon-clipmap
```

## Handoff After Tactical 247

On completion, update
[`world-view-navigation.md`](../topics/world-view-navigation.md) with the
landed controller API, native evidence, and remaining player-product work.
Then open two independent follow-up lanes.

The terrain-presentation lane begins with:

1. **Single toroidal ring and native renderer contract** under
   `procedural-horizon-clipmap`: property-test two-dimensional wrapping and
   negative coordinates; draw one moving ring in Terrain Lab and Explorer;
   establish reversed-Z, timestamps, device rebuild, and synthetic
   stereo/multiview direction.
2. **Nested rings and skirts** under `procedural-horizon-clipmap`: fixed holes,
   powers-of-two spacing, entering row/column budgets, coarse-first refill,
   precision, and footprint-aware summaries.

The browser/product lane begins with:

1. **Browser input-shell convergence study** under `world-view-navigation`:
   compare the main game, `mclone-input`, Terrain Lab, and native Explorer
   adapters; select a shared raw-contact/intent boundary before adding another
   TypeScript shell.
2. **Minimal Web Explorer shell and smoke deployment** under
   `world-view-navigation`: reuse the selected input boundary; keep JavaScript
   or TypeScript to canvas, rAF, lifecycle, URL, and observation forwarding;
   measure and deploy the independent Wasm/asset payload.
3. **Terrain Lab navigation migration** under `world-view-navigation`: adopt
   the same input boundary and reducer, then delete superseded TypeScript
   camera and gesture policy.
4. **Player-facing Web Explorer** under `world-view-navigation`: replace the
   minimal shell with accessible product UI and shareable view state without
   changing the core session.
5. **Validated local arrival** under `world-view-navigation`: turn selected
   X/Z into an integrated-host-validated safe spawn without making the URL
   authoritative.

After the ring and product lanes have useful evidence:

1. **Game exact handoff** under `procedural-horizon-clipmap`: integrate scene
   snapshots, exact-painted coverage, frontier collars, vegetation
   arbitration, and mono/stereo/multiview presentation only after ring
   behavior is stable in both proof hosts.

Do not resume the removed chunk-based Far LOD producer during any handoff.
