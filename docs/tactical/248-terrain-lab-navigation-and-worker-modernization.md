# Tactical 248: Terrain Lab Navigation and Worker Modernization

Status: active 2026-07-25.

Topics:

- `world-view-navigation`
- `web-worker-runtime-ownership`
- `gpu-procedural-terrain`

Parent directions:

- [`world-view-navigation.md`](../topics/world-view-navigation.md) owns the
  shared map/orbit/pan/zoom semantics and the long-term Explorer, tabletop,
  flight, and in-game view direction;
- [`web-worker-runtime-ownership.md`](../topics/web-worker-runtime-ownership.md)
  owns the browser rule that JavaScript forwards opaque events while Rust owns
  worker identity, epochs, scheduling, recovery, and payload meaning;
- [`gpu-procedural-terrain.md`](../topics/gpu-procedural-terrain.md) owns
  Terrain Lab's current procedural/canonical evidence and the future
  procedural-horizon research;
- [`platform-host-boundary.md`](../topics/platform-host-boundary.md) owns the
  platform-adapter boundary; and
- [`247`](247-standalone-world-explorer-foundation.md) proved the independent
  native executable and introduced `mclone-view-control`.

Predecessor implementation records:

- [`239`](239-terrain-lab-canonical-pan-responsiveness.md) established exact
  pan reuse and one-admission-per-frame behavior;
- [`241`](241-terrain-lab-large-canonical-footprints.md) established the
  `31x31` footprint, bounded raw cache, and memory receipts; and
- [`242`](242-terrain-lab-worker-canonical-meshing.md) moved exact generation
  and meshing into a persistent Worker, but left its protocol and coordination
  in TypeScript and its result handoff copy-heavy.

## Objective

Modernize Terrain Lab without redesigning the product surface:

1. replace its duplicated TypeScript navigation policy with the shared Rust
   `mclone-view-control` reducer;
2. replace its hand-authored semantic Worker protocol and React scheduling
   loop with a Rust worker actor and Rust coordinator;
3. keep JavaScript/TypeScript as a small domain-blind browser transport and
   raw-event adapter;
4. move steady-state exact mesh results through persistent external
   `SharedArrayBuffer` mailboxes rather than per-result transferable arrays;
5. preserve the existing exact, CPU LOD, fast CPU LOD, and GPU LOD panes,
   controls, URL behavior, diagnostic vocabulary, pixels, and admission
   pacing; and
6. delete superseded navigation, protocol, raw-chunk, and cache paths only
   after their replacements pass the same browser evidence.

This is a boundary modernization, not a new terrain feature. It must leave
Terrain Lab recognizable and useful throughout the commit series.

## Originating Direction

Terrain Lab began as a one-off terrain-generation workspace but has become a
small, mature broad-terrain viewer. The standalone World Explorer proved that
the underlying terrain renderer and navigation state can exist independently
from the game. The next useful proof is to make the browser Lab consume those
shared boundaries without first inventing a new browser product.

The requested shape is deliberately incremental:

- keep the current Lab UI/UX and its separate comparison panes;
- strip platform and scheduling policy out of TypeScript;
- preserve the existing Worker topology and improve its ownership;
- use a standard opaque browser transport which can also support later
  standalone-web and game consumers;
- keep gamepad and product-level map/enter-world behavior out of this slice;
  and
- retain exact generation, caching, and responsive pan behavior while changing
  their internal owner.

No further product decision blocks implementation. Exact/procedural masking,
clipmap rings, building persistence summaries, and a player-facing map shell
remain follow-ups.

## Starting Architecture

### Navigation

`tools/terrain-lab/src/state.ts` currently owns map/orbit/grab/pinch/arrow
navigation arithmetic. `TerrainCanvas.tsx` and
`CanonicalTerrainCanvas.tsx` each implement a separate pointer-contact state
machine around those functions.

`mclone-view-control` now owns host-neutral:

- `WorldViewState`, mode, and projection;
- map and orbit grab-pan;
- cursor/centroid-anchored zoom;
- simultaneous pinch pan and zoom;
- contact identity, tap/drag classification, and cancellation; and
- deterministic clamps and camera conventions.

The Lab has not adopted it, so its two canvases and the native Explorer can
still drift.

### Exact Worker

The current exact path is:

```text
React scheduling and cache-key policy
        |
        v
canonical-worker-protocol.ts request union
        |
        v
canonical-worker.ts semantic switch
        |
        v
CanonicalTerrainMeshSession (Worker Wasm)
        |
        v
Worker Wasm Vec -> JS Uint8Array copy
        |
        v
transferable ArrayBuffer
        |
        v
main JS Uint8Array -> main Wasm Vec copy
        |
        v
CanonicalTerrainLab GPU upload/cache
```

The Worker owns canonical generation, meshing, a bounded `1024`-chunk raw
cache, and its generation dependency cache. The main Wasm renderer owns active
GPU meshes plus a `64`-chunk warm GPU LRU. React owns worker identity,
readiness, epochs, missing-coordinate queues, `1/2/4/8/16` batch growth,
in-flight limits, admission pacing, resident key mirrors, and diagnostic
aggregation.

The game browser client already demonstrates the intended middle
architecture:

- isolated main and Worker Wasm heaps;
- Rust-owned worker actor and Rust-owned coordinator;
- a generic polled JavaScript Worker transport;
- persistent external `SharedArrayBuffer` control/input/result arenas; and
- atomics for mailbox publication.

This tactical reuses and, where necessary, extracts that generic boundary. It
does not import the game scene or render-section domain into Terrain Lab.

## Binding Decisions

### Parity before combination

Keep all existing Terrain Lab panes separate. Do not add exact/procedural
masking, a combined LOD scene, clipmap rings, terrain-edit summaries, or an
Explorer product shell while migrating ownership.

The same seed/profile/view/projection/controls must continue to produce the
same visible result. Small timing variation is acceptable; changed semantics,
new pop behavior, or a new page layout is not.

### Shared Rust navigation, browser-owned event mechanics

Expose a narrow Terrain Lab Wasm navigation session backed by
`mclone-view-control`. Browser code continues to own:

- DOM listener registration;
- pointer capture and release;
- wheel and touch `preventDefault`;
- focus and keyboard delivery;
- CSS scroll gutters and layout; and
- conversion of browser coordinates into viewport-local numeric facts.

Rust owns contact history and every semantic view transition. Both canonical
and procedural panes use one TypeScript adapter/hook around the same Wasm
session contract. React retains the serializable `TerrainLabState` used by URL
and controls, but it no longer computes view changes.

The adapter must preserve the Lab's existing integer-facing center and
footprint behavior even though the shared reducer internally uses floating
point state.

### Domain-blind JavaScript transport

Extract or share one small browser Worker transport with this conceptual
contract:

```text
new transport(worker_url, label)
post(opaque_frame, optional_transferables)
poll() -> opaque_frame | undefined
pending_event_count()
terminate()
```

The transport may know about Worker construction, module URLs, `postMessage`,
transfer lists, queueing, browser error events, and termination. It must not
know:

- terrain coordinates or profiles;
- cache keys or residency;
- epochs, retry rules, batch growth, or admission budgets;
- mesh bundle fields;
- render-section semantics; or
- the meaning of a request or result opcode.

Terrain Lab may retain a tiny Worker entry module that loads bindgen and
forwards opaque frames into the Rust actor. It may not rebuild the semantic
switch currently in `canonical-worker.ts`.

### Rust actor and coordinator

The Worker Rust actor owns:

- bindgen/session initialization;
- asset/catalog hydration;
- exact generation and meshing requests;
- raw canonical cache lifetime;
- packed result encoding;
- mailbox publication;
- request validation and structured failure reports; and
- orderly shutdown.

The main Rust coordinator owns:

- worker identity/readiness and lifecycle;
- current coverage epoch and stale-result rejection;
- desired/missing coordinate ordering;
- `1/2/4/8/16` batch growth and bounded in-flight work;
- retry/recovery decisions;
- one exact admission per animation frame;
- aggregation of the existing exact-worker diagnostics; and
- coordination with active/warm GPU residency.

React tells the coordinator the desired exact footprint and calls one
per-animation-frame pump. It consumes a report suitable for current UI and
test diagnostics. It must not maintain a parallel semantic queue, epoch, or
resident-cache model.

The Terrain Lab coordinator is domain-specific Rust. Share generic transport
and mailbox machinery with the game; do not force exact chunks through the
game's render-section request types.

### External SAB, not shared Wasm linear memory

The target browser topology keeps the main and Worker Wasm instances isolated.
It does not require Wasm threads or a shared Wasm linear memory.

The main side allocates persistent external browser `SharedArrayBuffer`
mailboxes:

- a small atomic control block;
- a bounded input arena where useful; and
- a bounded result arena for encoded exact-mesh batches.

Rust sees typed views over those external buffers. Atomics publish ownership,
byte length, request/epoch identity, completion, overflow, and failure state.
JavaScript only forwards the opaque notification frame; it neither decodes nor
copies mesh payloads.

Initialization assets and rare oversized/recovery messages may use explicit
one-time transferables while the steady-state mesh result path uses the SAB
arena. Capacity and overflow behavior must be visible and tested. An arena
must never silently truncate a result.

Local Vite development and preview must send the same COOP/COEP/CORP posture
as the aggregate hosted route so `crossOriginIsolated` and
`SharedArrayBuffer` are real acceptance conditions rather than deployment-only
assumptions.

### One authoritative cache at each layer

Preserve the useful two-layer policy:

- Worker Rust: canonical raw/generation cache, bounded to `1024` chunks;
- main renderer Rust: active GPU meshes plus a `64`-chunk warm GPU LRU;
- browser/React: no raw chunk or mesh payload cache.

The coordinator may hold small coordinate and receipt metadata. It must not
duplicate raw terrain or packed mesh bundles after GPU admission.

The UI continues to distinguish worker raw-cache bytes, main raw bytes, active
GPU bytes, warm GPU bytes/hits, and admission frames. Main raw bytes remain
zero on the packed path.

## Implementation Slices

Each slice lands as a coherent commit with tests and updates this execution
record before the next slice begins.

### Slice 0: execution contract and baseline

- land this tactical and index it;
- record the existing ownership and cache baseline;
- identify the smallest current desktop and phone browser acceptance cases;
  and
- keep the tree clean before code migration.

### Slice 1: shared navigation adoption

- add `mclone-view-control` to `mclone-terrain-lab`;
- expose a narrow Wasm navigation session;
- add one shared React hook/adapter for both canvases;
- route mouse, wheel, touch, pinch, keyboard, cancellation, and projection
  changes through Rust;
- port focused unit coverage to the Rust owner;
- pass desktop and phone gesture evidence; then
- delete the superseded TypeScript navigation arithmetic and duplicated
  contact reducers.

Green checkpoint: no Worker or cache changes.

### Slice 2: opaque transport and Rust worker actor

- place the reusable polled Worker transport at a neutral browser boundary;
- make both the game shell and Terrain Lab consume it without terrain or game
  semantics;
- replace the Terrain Lab TypeScript request union and semantic Worker switch
  with Rust-authored opaque frames and a Rust actor;
- initially preserve transferable results behind the new boundary if that
  yields a smaller reviewable checkpoint; and
- prove initialization, exact `9x9` generation, structured errors, and clean
  termination.

Green checkpoint: coordination and admission pacing may still be driven by the
existing React loop, but TypeScript no longer defines the worker protocol.

### Slice 3: Rust coordinator

- move worker lifecycle, epochs, stale rejection, desired/missing ordering,
  batch growth, in-flight bounds, admission pacing, and reports into Rust;
- let the renderer/coordinator query active and warm residency directly;
- reduce the canonical React effect to desired-coverage updates, pumping, and
  report presentation;
- prove a one-chunk `9x9` shift reuses `72` chunks and admits only the entering
  `9`;
- prove a warm return does not remesh eligible chunks; and
- add stale/failure/restart coverage at the Rust owner.

Green checkpoint: no UI redesign and no SAB requirement yet.

### Slice 4: persistent SAB mailboxes

- share or extract the generic external-SAB mailbox contract used by the game;
- add Rust-owned encoded exact-result batches and bounds validation;
- allocate persistent control/result arenas on the main side;
- remove steady-state Worker-Wasm-to-JS and JS-to-main-Wasm mesh copies;
- add explicit overflow/retry/failure handling;
- add local Vite COOP/COEP/CORP headers;
- report cross-origin isolation, transport kind, arena capacity/high-water, and
  overflow count; and
- prove local and aggregate-hosted SAB use on desktop and phone viewports.

Green checkpoint: exact pixels and admission behavior match the transferable
path before that fallback is removed.

### Slice 5: deletion and ownership gates

- delete `canonical-worker-protocol.ts`;
- delete the semantic TypeScript Worker request/result mapper;
- delete unused main-thread raw canonical `acceptChunk`, `retainChunks`, and
  presentation/rebuild paths;
- remove redundant resident/cache mirrors and reports;
- extend browser-boundary checks so Terrain Lab cannot regain semantic Worker
  policy;
- retain an explicitly named fallback only if a supported browser contract
  requires it and the fallback still uses the same Rust actor/coordinator
  semantics; and
- refresh generated bindgen declarations and bundle-size receipts.

### Slice 6: closeout evidence

- run focused Rust, Wasm, TypeScript, and browser suites;
- run direct `9x9` desktop and phone acceptance;
- run the existing direct `31x31` maximum-footprint evidence;
- inspect fresh screenshots after the first drawable checkpoint and again at
  closeout;
- validate aggregate `/terrain/` headers and pixels;
- record cache, SAB, scheduling, bundle, and dependency receipts here;
- update the three parent topics with the resulting contract and next step;
  and
- mark this tactical completed only when no superseded path remains.

## Acceptance Contract

### Product and navigation parity

- Existing controls, labels, pane layout, URL state, reset behavior,
  tap-to-inspect, map/orbit views, projection choices, and scroll gutters are
  preserved.
- Exact, CPU LOD, fast CPU LOD, and GPU LOD stay distinct comparison panes.
- Desktop mouse orbit, right-drag/keyboard pan, anchored wheel zoom, touch
  single-drag, two-finger pan/zoom, contact cancellation, and resize continue
  to pass.
- Canonical and procedural panes consume the same Rust navigation owner.

### Worker ownership

- Production TypeScript contains no terrain Worker request/result union,
  coordinate scheduling, batch-growth logic, epoch policy, cache policy, or
  admission policy.
- JavaScript cannot name terrain profiles, chunks, mesh fields, residency, or
  renderer stages in the generic transport.
- Rust tests cover stale results, failure/restart, bounds validation, batch
  progression, and admission pacing.
- Terrain Lab does not depend on the full game application, scene, client,
  server, or render-section compiler domain.

### Cache and responsiveness

- Worker raw cache remains bounded to `1024` chunks.
- Main packed path reports zero retained raw chunk bytes.
- Main GPU warm residency remains bounded to `64` departed chunks.
- A one-chunk `9x9` pan reports `72` overlapping chunks and only `9` entering
  admissions without clearing the view.
- Admissions remain paced at no more than one exact chunk per animation frame.
- `31x31` remains bounded and responsive with truthful memory diagnostics.

### SAB transport

- The ordinary exact result path reports an external-SAB transport.
- Both main and Worker keep private Wasm heaps.
- JS does not decode or clone packed exact mesh payloads.
- Overflow is counted and recovered or rejected explicitly.
- Local development, local preview, and aggregate `/terrain/` are
  cross-origin isolated with COOP/COEP/CORP headers.

### Rendered evidence

- Fresh desktop and phone screenshots are captured under `/tmp` and inspected.
- Canonical exact terrain remains textured, lit, and aligned with the
  procedural comparison at the pinned acceptance coordinates.
- No blank, transparent, headless-only, or stale pre-migration capture counts
  as evidence.

## Execution Record

### Slice 0 complete: execution contract

Commit `01cf925a` established this tactical before implementation. It records
the parity checkpoints, the isolated-Wasm/external-SAB interpretation, cache
ownership, deletion gates, and explicit non-goals.

### Slice 1 complete: shared navigation adoption

Terrain Lab now depends on `mclone-view-control`. A narrow
`TerrainLabNavigationSession` Wasm adapter owns synchronized Lab-facing view
state, contact reduction, pointer purpose, arrow pan, centered UI pan/zoom,
and integer-facing results. One React hook is used by both procedural and
canonical canvases; it retains only DOM focus, pointer capture,
`preventDefault`, pane-local coordinates, rAF publication, and the
tap-to-inspect host reaction.

The former TypeScript pan, grab, orbit, anchored zoom, pinch, arrow, pitch
clamp, active-pointer maps, pinch snapshots, and duplicated pointer helpers
were deleted. Pan-pad and zoom buttons call the same Rust session rather than
retaining a second UI arithmetic path.

Focused evidence on 2026-07-25:

- `cargo test -p mclone-view-control -p mclone-terrain-lab`: 18 tests passed;
- `cargo check -p mclone-terrain-lab --target wasm32-unknown-unknown` passed;
- Terrain Lab TypeScript typecheck and 14 remaining URL/state tests passed;
- the headed Wayland WebGPU probe returned `[0,89,255,255]`;
- the full desktop comparison/navigation case passed in 25.5 seconds;
- the phone scroll-gutter and two-finger test passed in 13.6 seconds; and
- `/tmp/mclone-terrain-lab-desktop-chrome-workspace.png` was inspected and
  showed the expected canonical/CPU/GPU workspace with nonblank terrain.

Worker protocol, cache, and admission behavior were unchanged in this slice.

### Slice 2a complete: Rust-authored worker frames and actor

The canonical exact Worker now loads only the Terrain Lab Wasm module, keeps
one `CanonicalTerrainWorkerActor`, forwards each opaque incoming frame, and
posts the actor's opaque dispatch plus its actor-selected transfer list.
Terrain profile, stage, asset/catalog hydration, epochs, begin semantics,
canonical session lifetime, compilation, result construction, and structured
domain errors all moved into worker Rust.

Main Rust now authors initialization, begin, and compile frames and validates
every response through `CanonicalTerrainWorkerResponse` before transitional
React coordination consumes its getters. The hand-authored
`canonical-worker-protocol.ts`, TypeScript semantic request switch, and old
public `CanonicalTerrainMeshSession`/batch-payload Wasm facade were deleted.

This checkpoint deliberately retains the existing React epoch/batch/admission
loop and transferable `Uint8Array` mesh results. That isolates actor ownership
from the coordinator and SAB changes still to follow.

Focused evidence on 2026-07-25:

- the Terrain Lab Wasm check, typecheck, and 14 URL/state tests passed;
- the Rust Terrain Lab test suite passed;
- the headed desktop `9x9` responsive-pan case passed in 8.4 seconds;
- its one-chunk shift retained `72` chunks and paced the `9` entering chunks
  through the unchanged acceptance assertions; and
- `/tmp/mclone-terrain-lab-desktop-chrome-canonical-responsive-pan.png` was
  inspected and showed the complete `81/81` canonical footprint.

### Slice 3 complete: Rust exact-coverage coordinator

`CanonicalTerrainWorkerCoordinator` now owns the exact-coverage epoch,
worker readiness, desired/resident/missing coordinate sets, warm-residency
admission, `1/2/4/8/16` batch growth, the two-result high-water limit,
stale-result rejection, one-admission-per-pump pacing, completion, and the
existing diagnostic aggregate. It drives the existing Rust renderer's packed
prepare, warm activation, packed-mesh acceptance, and reset methods directly.

The canonical React effect now creates the browser Worker and wraps it in the
same `PolledWorkerTransport` source used by the main game, submits desired
coverage to Rust, calls one opaque coordinator pump per animation frame,
presents the returned report, and schedules a draw when Rust reports a render
change. It no longer owns an epoch, semantic Worker callbacks, coordinate
queues, batch sizing, resident mirrors, packed mesh interpretation, or cache
accounting. Worker creation remains visibly in the browser adapter so Vite can
compile the module URL; the generic transport only queues opaque
messages/errors and provides `post`, `poll`, and `terminate`.

The Worker shell now turns its unavoidable Wasm-bootstrap failure into the
same Rust-recognized structured error shape. Actor-domain errors still
originate in Rust.

Focused evidence on 2026-07-25:

- `cargo test -p mclone-view-control -p mclone-terrain-lab`: 18 tests passed;
- Terrain Lab Wasm build and TypeScript typecheck passed;
- the headed desktop `9x9` responsive-pan case passed twice consecutively in
  8.7 seconds each;
- a one-chunk shift retained `72` residents and admitted only the entering
  `9`, while a warm return admitted one chunk per frame without remeshing;
- main raw bytes remained zero and Worker raw-cache plus packed-render
  diagnostics remained populated; and
- `/tmp/mclone-terrain-lab-desktop-chrome-canonical-responsive-pan.png` was
  inspected again after the coordinator cutover and showed `81/81` exact
  chunks.

Transferable packed arrays remain the active result transport at this
checkpoint. Adding external SAB mailboxes, removing the legacy raw compiler
exports, and adding broader source-ownership gates remain active work.

## Validation Plan

The exact commands may be refined as ownership moves, but closeout includes at
least:

```bash
cargo fmt --all -- --check
cargo test -p mclone-view-control
cargo test -p mclone-terrain-lab
cargo check -p mclone-terrain-lab --target wasm32-unknown-unknown
pnpm --dir tools/terrain-lab test
pnpm --dir tools/terrain-lab typecheck
pnpm host:check -- --probe-browser-webgpu
pnpm --dir tools/terrain-lab web:test
pnpm --dir tools/terrain-lab web:smoke
```

Focused browser commands may select the `9x9`, navigation, multitouch, and
maximum-footprint cases during individual slices. Full matrix and hosted
commands follow the current platform policy and Terrain Lab scripts.

## Non-goals

- implementing the toroidal procedural horizon or quadtree LOD;
- combining exact and procedural terrain in one masked scene;
- persisted edit/building summaries in distant terrain;
- changing terrain generation, biome, hydrology, or vegetation algorithms;
- a new standalone browser product or replacement Terrain Lab UI;
- game scene, tabletop, flight-simulator, teleport, spawn, or enter-world
  integration;
- gamepad bindings or input semantics;
- shared Wasm linear memory or general Wasm threading;
- moving WGPU surface ownership out of browser/platform adapters; or
- generalizing every existing specialized Worker in the repository.

## Commit and Closeout Policy

Use the exact topic trailers listed at the top of this document for the commit
series. Each commit must leave the active page buildable and preserve a
reviewable fallback until the replacement checkpoint passes. Deletions land
immediately after their replacement is proven; they are not deferred to an
unbounded cleanup campaign.

Completion means the modern path is the only production path, the acceptance
suite has fresh rendered evidence, all affected topic docs describe the
resulting ownership, and the worktree is clean.
