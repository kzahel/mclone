# World View Navigation And Exploration

Topic: `world-view-navigation`

Status: cross-platform Explorer foundation completed and deployed on
2026-07-25 by Tacticals
[`247`](../tactical/247-standalone-world-explorer-foundation.md); Terrain Lab
navigation migration and browser-runtime modernization completed and deployed
on 2026-07-25 by Tactical
[`248`](../tactical/248-terrain-lab-navigation-and-worker-modernization.md).
Tactical
[`249`](../tactical/249-cross-platform-procedural-horizon-proof.md) extended
the lightweight Explorer across native and browser hosts with the first
shared toroidal horizon. The Explorer is a product
profile, not a native-only application: its Rust runtime must work through the
same platform boundary that can also host the full browser game.
Tactical
[`250`](../tactical/250-continuous-explorer-presentation-and-cadence.md) is
active to separate continuous fractional presentation from snapped residency
and replace host-specific keyboard stepping with shared frame-time motion.
`mclone-view-control` now owns shared map/orbit/contact semantics for both the
standalone native `mclone-world-explorer` and Terrain Lab's procedural and
canonical panes. Terrain Lab retains thin DOM focus, capture, local-coordinate,
scroll, and inspection mechanics through one shared React hook; its former
TypeScript camera arithmetic and duplicated contact reducers are deleted.
Gamepad support remains deferred. The native Explorer is currently a
procedural-view smoke and architecture host, not a replacement for Terrain
Lab's already functioning exact-chunk view.

## Scope

This topic owns two connected concerns:

1. first-class, platform-neutral map/orbit/focus/zoom manipulation shared by
   Terrain Lab, the game, tabletop mode, and a lightweight explorer; and
2. the player journey from a broad world map through 3D terrain to an
   authoritative “Enter Here” handoff.

The procedural horizon's geometry, residency, seams, and exact replacement
belong to
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md).
[`tabletop-overview-mode.md`](tabletop-overview-mode.md) continues to own
active-world tabletop policy, targeting, cutaways, and authority. This topic
owns the reusable view manipulation those products consume.

It does not own aircraft physics, ordinary embodied locomotion, XR session
management, world authority, or Terrain Lab's diagnostic UI.

## Product Thesis

Terrain Lab has revealed a useful product surface rather than only an internal
worldgen tool. One shared terrain presentation can support a continuous
journey:

```text
continent / world map
        -> 3D orbit
        -> high-altitude or flight view
        -> tabletop / local overview
        -> embodied play
```

A prospective player should be able to open a web link, inspect a world
without installing the full client, zoom from geographic scale into an
interesting place, and choose **Enter Here**. Shareable URLs can preserve the
world descriptor, view, and selected arrival area.

This is also useful inside the full client:

- a map or “god mode” overview;
- a first-class tabletop presentation;
- high-altitude free exploration;
- a future flight-simulator-style activity; and
- world-selection and seed-exploration UI.

These are consumers of common terrain and view facts, not reasons to build
separate renderers and gesture systems.

## Existing Foundation

Terrain Lab already reuses the production stack more deeply than a one-off web
mock:

- its canonical pane uses the production world generator;
- exact terrain uses the production block catalogue, atlas, mesher, and
  renderer;
- GPU terrain and tree summaries live in shared Rust/WGSL; and
- its bounded 64×64-cell view can move through very large sample spacings.

The Lab remains a deliberately small host. It does not need full lighting,
client/server authority, persistence, collision, entities, or simulation ticks
to present a useful map.

Terrain Lab previously kept camera math in `state.ts` and duplicated pointer
and pinch state across `TerrainCanvas.tsx` and
`CanonicalTerrainCanvas.tsx`. Tactical 248 removed those paths. The browser
now forwards pane-local numeric facts into a Wasm
`TerrainLabNavigationSession`, and one `useWorldViewNavigation` hook serves
both canvases. URL state, tap-to-inspect, focus, pointer capture,
`preventDefault`, split-panel coordinates, and scroll gutters remain
browser-host concerns.

Focused Rust, Wasm, desktop mouse/wheel/keyboard, and phone two-finger/gutter
evidence passed at the migration checkpoint. Tactical 248 then completed the
independent Worker/cache modernization without changing this navigation
contract. The next navigation work remains a player-facing Explorer and
authoritative enter-world handoff, not more Terrain Lab host policy.

## Implemented Native Foundation

Tactical 247 added two deliberately separate pieces:

- [`mclone-view-control`](../../native/crates/mclone-view-control/) is a
  dependency-free, sans-I/O state/intent and contact-gesture reducer; and
- [`mclone-world-explorer`](../../native/apps/mclone-world-explorer/) is a
  small `winit`/WGPU leaf host over `mclone-terrain-view`.

The controller's landed public vocabulary includes `WorldViewState`,
`WorldViewMode`, `WorldViewProjection`, `WorldViewIntent`,
`WorldViewReducer`, `ContactEvent`, and `ContactGestureReducer`. It owns map
grab, 3D orbit, world pan, anchored logarithmic zoom, simultaneous pinch pan
and zoom, focus/recenter, tap/double-tap/drag classification, contact
cancellation, pointer-count transitions, and numeric constraints. Platform
types and renderer/world facts do not enter the crate.

The native adapter translates mouse, wheel/trackpad, keyboard, and touch
events into those contracts. The Explorer drives the existing procedural
terrain and tree renderer in map, orthographic 3D, and perspective 3D modes.
It is a separate executable with an enforced dependency firewall rather than
a mode of the game client.

The procedural horizon's orbit target uses the profile's fixed sea-level
datum rather than sampling terrain directly beneath the moving view center.
This keeps horizontal pan and movement vertically stable and avoids coupling
camera placement to integer clipmap-center changes. Explicit terrain focus or
ground-following behavior can be added later as a separate navigation action.
Terrain Lab retains its local sampled focus behavior.

The 2026-07-26 sea-level correction passed all 30 focused terrain-view tests,
the native real-window and offscreen movement smoke, and local plus hosted
headed-Wayland browser movement, negative-coordinate, and teleport smokes.
Initial, moved, negative-coordinate, and teleported frames were visually
coherent with consistent vertical framing. Production version
`d852be90-066e-46a1-895f-9287964c0a45` serves Wasm SHA-256
`5c6eb01f9ff5f66727e141e83418cc1e75f00777ea090baa833bea7d7d3d2af4`.

The pinned release acceptance sequence rendered initial 3D, continuous X/Z/
diagonal movement, anchored zoom, map, and orbit through both a real
Wayland/Vulkan surface and an offscreen target. Corresponding captures were
byte-identical and had direct depth coverage. On the closeout host, native
first-coarse and target-ready times were 83.26 and 103.79 ms; 30 input frames
averaged 1.14 ms with 4.97 ms p95; peak bounded-preview residency was
171,994,752 bytes; and final pending work was zero. These are proof-host
observations, not product budgets. Full receipts, hashes, artifact sizes, and
commands are recorded in Tactical 247.

## Browser Input-Shell Audit

The 2026-07-25 audit compared:

- the main game's
  [`mclone-web-input.ts`](../../native/apps/mclone-web-client/www/mclone-web-input.ts),
  [`mclone-web-touch.ts`](../../native/apps/mclone-web-client/www/mclone-web-touch.ts),
  [`mclone-web-app.ts`](../../native/apps/mclone-web-client/www/mclone-web-app.ts),
  Rust `WebSceneHost`, and browser gamepad collector;
- `mclone-input` source, snapshot, preference, context, and controller-session
  contracts;
- both Terrain Lab React terrain panes and their TypeScript state helpers; and
- the native Explorer's `winit` adapter over `ContactGestureReducer`.

### What the main game shell actually owns

The main web shell has useful, mature mechanics:

- CSS-to-canvas coordinate conversion;
- keyboard code forwarding and synchronous handled/default disposition;
- pointer-lock acquisition, fallback, and release;
- click-versus-drag classification for gameplay interaction;
- relative mouse motion;
- touch/pen pointer capture and synthetic-mouse suppression;
- blur, visibility, resize, and transient-input clearing;
- demand-aware rAF rendering plus a 33 ms controller poll while static; and
- Rust-owned W3C standard-gamepad normalization, source identity, hotplug,
  dead zones, bindings, prompt activity, and scene/UI contexts.

Those mechanics are assembled around the full `WebSceneHost`. The shell
expects pointer-locked first-person input, gameplay/UI routing, touch control
overlays, first-touch fullscreen behavior, scene dispositions, session
lifecycle, and game-specific frame demand. Its wheel path intentionally
reduces input to up/down actions, losing the magnitude and cursor anchor a map
requires.

Terrain Lab has a different physical shape:

- input is scoped to one of several focusable stages in a scrollable page;
- the procedural pane may contain split subpanels whose local viewport and
  anchor differ;
- one shared geographic/camera state drives several independent renderers;
- absolute contacts, continuous wheel magnitude, and pane-local anchors are
  required;
- touch-scroll gutters must remain usable; and
- pointer lock, gameplay click synthesis, touch overlays, fullscreen, scene
  sessions, persistence, and game UI contexts are unwanted.

Both Lab panes currently duplicate `PointerStart`, active-contact and pinch
maps, capture, one/two-contact transitions, orbit/pan selection, wheel,
keyboard, and pointer helper functions. The pure TypeScript state helpers
duplicate behavior now tested in `mclone-view-control`. This is the immediate
convergence target.

The local delivered artifacts also show why importing the game application is
not a small-shell strategy: the current full web-client Wasm is 13,053,079
bytes, while Terrain Lab's Wasm is 4,248,122 bytes. These are observations,
not budgets, and the game's small TypeScript input modules are not themselves
the size problem. The problem is their current `WebSceneHost` contract and
product lifecycle.

### Decision

Do **not** reuse the main game's browser input shell as one indivisible module.
Do **not** make the Explorer depend on the web-client app crate or emulate a
game session merely to obtain input.

Also do **not** accept a basic demo shell that owns another camera reducer,
pinch recognizer, or gamepad mapping. A minimal Web Explorer should be a small
product host, not disposable input code.

Reuse occurs at two narrower boundaries:

```text
per-app browser DOM mechanics
  pointer capture / preventDefault / focus / local coordinates / lifecycle
        |
        v direct typed calls, no JSON event bus
mclone-view-control
  ContactEvent -> WorldViewIntent -> WorldViewState

browser or native controller collector
        |
        v
mclone-input canonical source + StandardGamepadSnapshot
        |
        v view-specific mapping
mclone-view-control intents
```

The game keeps its existing browser shell. When an in-game overview or
tabletop mode is active, `mclone-scene` routes the already collected neutral
input into the shared view controller. The standalone site keeps a minimal
canvas/DOM host and reaches the same controller directly. They share meaning
without pretending their lifecycle and surfaces are identical.

### Browser contact contract

The browser adapter should forward only facts needed to construct existing
shared intents:

- contact ID, start/move/end/cancel phase, pointer kind, local position, and
  monotonic time;
- physical button and modifier facts on contact start;
- local viewport width and height;
- wheel delta/mode plus local cursor anchor;
- physical key code, pressed/released state, and repeat;
- blur, lost-capture, visibility, and mode-transition cancellation; and
- synchronous handled/redraw/capture dispositions where the browser must make
  an immediate mechanical decision.

TypeScript may retain active DOM capture IDs and synthetic-mouse suppression.
It must not retain start camera snapshots, pinch distances, gesture
thresholds, pan/orbit direction, scale constraints, or view state. Do not
introduce a universal serialized event stream or make ordinary native input
pass through browser-shaped records.

The exact reusable DOM module remains deliberately undecided. First implement
one Terrain Lab hook/adapter used by both panes. When Web Explorer becomes the
second independent browser consumer, extract only the subset that remains
product-neutral. The main game may keep its relative-pointer/pointer-lock
adapter even if all three hosts share a few small keyboard, lifecycle, or
contact helpers.

### Deferred gamepad boundary

Gamepad support is not required for the Terrain Lab navigation migration or
the small standalone browser shell. Defer its implementation until a concrete
Explorer, tabletop, or game product needs it. The opaque browser shell should
forward pointer, key, wheel, resize, focus, and lifecycle facts without
learning view semantics; it does not need speculative controller machinery.

When gamepad support is selected later, the game's gameplay action map is not
the view-control map. Do not translate a controller into fake keyboard events
and do not bind left stick to `MoveAnalog`, right stick to first-person `Look`,
or face buttons to Jump/Attack in the Explorer.

Preserve the existing canonical physical boundary in `mclone-input`:

- `InputSourceId` and source descriptors;
- `ControllerInputBatch` and observations;
- `StandardGamepadSnapshot`;
- shared dead-zone/response, trigger, lifecycle, layout, and preference facts;
  and
- W3C standard-mapping normalization in a browser platform collector.

Add a view-specific downstream adapter that turns adjusted sticks, buttons,
and frame time into `WorldViewIntent` values. Likely defaults are left-stick/
D-pad pan, right-stick orbit in 3D, continuous trigger or shoulder zoom, and
explicit map/3D, focus, and back actions. Lock the exact mapping with
controller tests and accessibility review rather than inheriting gameplay
bindings accidentally.

The current `BrowserGamepadCollector` is app-local Rust and has no TypeScript
button policy. Its normalization and reconnect tests are reusable evidence,
but physical browser-controller acceptance is still open. Do not put
`web-sys` into `mclone-input`. When a second browser app needs the collector,
move or factor its platform-only mechanics into a small browser adapter owner
that still emits `mclone-input` canonical snapshots.

Like the game, a static Explorer cannot rely only on DOM gamepad events.
Poll neutral/static state at a low cadence, switch to rAF while a controller
is active or terrain is refining, and return to the low cadence when neutral.
That demand pattern is worth reusing; the full scene host is not.

### Terrain Lab first — implemented

The first Tactical 248 implementation slice completed this sequence:

1. expose a narrow Wasm view-control session from the existing
   `mclone-terrain-lab` app without adding the full client or a new web app;
2. replace both panes' TypeScript camera/gesture reducers with one shared Rust
   state/intent path;
3. keep pane selection, split-panel local coordinates, pointer capture,
   `preventDefault`, focus, scroll gutters, URL synchronization, inspection
   UI, and React scheduling in the Lab host;
4. preserve tap-to-inspect as a host reaction to the reducer's tap signal;
5. validate mouse buttons/modifiers, continuous anchored wheel, focused
   keyboard input, one-contact map/orbit, simultaneous pinch pan/zoom,
   cancellation, interrupted contacts, and page-scroll containment; and
6. leave gamepad support out of this slice.

Only after that migration should a minimal Web Explorer choose whether the
Lab's DOM adapter is reusable as-is or should be factored into a small shared
browser module.

### Exact view status and sequencing

The word “Explorer” currently names two different levels of proof:

- the standalone native
  [`mclone-world-explorer`](../../native/apps/mclone-world-explorer/) exercises
  crate boundaries, native navigation, procedural terrain, capture, and
  profiling; and
- Terrain Lab's canonical pane already exercises real chunk generation,
  block/biome arrays, textured section meshing, production block assets, and
  the production chunk renderer.

The native Explorer currently requests the GPU `Cover` stage through
[`terrain.rs`](../../native/apps/mclone-world-explorer/src/terrain.rs). It does
not instantiate the canonical compiler, exact chunk cache, textured section
mesher, or chunk draw resources. First-party terrain textures and procedural
tree summaries make it visually useful, but they do not make it an exact
chunk view.

The exact path is already functional, but its ownership is split:

- `mclone-terrain-view` owns `CanonicalTerrainCompiler`;
- `mclone-render` owns textured chunk meshing and drawing;
- Terrain Lab's
  [`canonical_mesh.rs`](../../native/apps/mclone-terrain-lab/src/canonical_mesh.rs)
  currently owns the exact-chunk desired set, raw cache, compilation session,
  and presentation assembly; and
- Terrain Lab's
  [`canonical_web.rs`](../../native/apps/mclone-terrain-lab/src/canonical_web.rs)
  owns the current Web/WGPU upload and draw orchestration.

Do not lift that Lab orchestration wholesale into a new shared cache or worker
framework. The game already has the stronger reusable lifecycle:
`mclone-client` owns loaded `ChunkSnapshot` values,
`mclone-render-session` owns desired/dirty section state, compact resident
metadata, compile acceptance, and upload backpressure, and the production web
client owns a Rust-directed render Worker with a resident Wasm snapshot mirror
and shared-memory delta/result arenas. Native uses the same render-session
contract with ordinary worker threads.

The reusable extraction should instead adapt locally compiled canonical chunks
into the game's neutral snapshot/section pipeline, plus retain only the small
Explorer-specific policy for a generated desired set and optional warm
near-field residency. Cache authoritative raw chunks in Rust-owned memory on
the execution host. On the web that means Worker Wasm memory, not a semantic
JavaScript cache; JavaScript should provide only Worker/DOM mechanics.
Transferable buffers remain a valid fallback transport, while the production
shared-memory render-worker path is the convergence target.

Reuse the production browser Worker inversion of control as well, but not the
complete game bootstrap. Today `mclone-web-app.ts` constructs a generic
`PolledWorkerTransport` factory and supplies it to Rust;
`WebRenderWorkerCoordinator` owns worker identity, priorities, epochs,
requests, stale results, failure recovery, and shutdown. The TypeScript
transport knows only module-Worker construction plus `post`, `poll`, and
`terminate`, while the Worker entry loads Wasm and forwards opaque frames to a
Rust actor.

The Explorer should consume that same boundary after its currently app-local
pieces are made reusable. Its browser boot may supply versioned bindgen and
Worker URLs and a generic module-Worker factory, but must not manually recreate
Terrain Lab's `onmessage`, batch, epoch, cache, or admission state machine.
Rust decides which render or exact-source actors are needed. If more than one
Worker kind is ultimately required, generalize the factory to accept an opaque
URL/name or resource handle rather than adding a semantic TypeScript switch.
The exact module/package location should be selected by the implementation
tactical; importing files from the game app's deployment directory is not a
shared boundary.

Terrain Lab itself is now the first additional consumer of that Worker boundary,
not merely a temporary host to leave unchanged after navigation migration.
Modernize it as a parity-preserving follow-on: keep the current panes,
controls, URLs, diagnostics, exact footprints, pixels, raw-cache behavior, and
warm GPU returns while moving Worker lifecycle, epochs, batching,
backpressure, cache ownership, and result interpretation out of React and into
Rust. Replace transferred packed-mesh results with persistent external
`SharedArrayBuffer` mailboxes after the Rust actor boundary is green. This
continues the production isolated-heap architecture; it does not introduce one
shared Wasm linear memory.

The hosted `/terrain/` deployment already receives COOP/COEP/CORP headers from
the aggregate web deployment. Terrain Lab's direct Vite development server
does not currently set them, so the SAB slice must add and assert local
cross-origin isolation. Extend the Worker ownership inventory to cover the Lab
before removing its old TypeScript protocol. Delete the unused main-thread
raw-chunk admission/remeshing facade only after the packed/SAB path has parity
evidence.

Therefore, “migrate Terrain Lab” means replacing its duplicated browser
camera and gesture policy while preserving both its procedural and canonical
renderers. That navigation work does not need to wait for exact chunks in the
native Explorer.

Conversely, do not retire Terrain Lab or present the procedural-only Explorer
as its product replacement. Before a standalone Explorer becomes the real
map-to-world view, extract a reusable exact-view session from the Lab-local
orchestration, prove it in a small host, and compose its near-field chunks over
the procedural horizon with the transition and masking rules owned by
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md). A
procedural-only web shell may still be useful as a deployment smoke, but it
must be labeled as such.

## Shared View-Control Contract

The small sans-I/O shared owner is `mclone-view-control`. It must continue not
to depend on:

- WGPU or renderer resources;
- world generation or chunk state;
- lighting or materials;
- client/server simulation;
- DOM, `winit`, Android, or OpenXR; or
- one product's UI hierarchy.

Its first landed state includes:

```text
WorldViewState {
  mode: Map | Orbit
  focus_x
  focus_z
  blocks_across
  yaw
  pitch
  projection: Orthographic | Perspective
}
```

It consumes semantic intentions rather than platform events:

```text
SetMode(mode)
SetProjection(projection)
Orbit(delta, viewport)
GrabPan(delta, viewport)
AnchoredZoom(log_delta, anchor, viewport)
PinchPanZoom(log_delta, anchor, centroid_delta, viewport)
PanWorld(delta)
FocusAt(world_position)
Recenter(world_position, optional_scale)
Tap(position)
DoubleTap(position)
CancelContacts
```

The durable requirements are that the shared reducer owns:

- axis and direction conventions;
- yaw, pitch, distance, and scale constraints;
- cursor- or centroid-anchored zoom;
- simultaneous pinch zoom and centroid pan;
- gesture dead zones and drag classification;
- tap, double-tap, and cancellation timing;
- damping or inertia when enabled;
- follow/recenter behavior; and
- frame-rate-independent updates.

Continuous presentation and clipmap residency are separate contracts. The
view controller retains fractional `f64` focus and scale; the renderer
receives them every admitted frame. Only procedural residency snaps to
power-of-two tile boundaries. Platform loops provide monotonic elapsed time
and held-control facts without assigning speed or accumulating movement.

The reducer produces camera/view facts. It does not choose what a selected
block means, teleport a player, mutate terrain, or issue a network command.

## Input Layering

Keep the path explicit:

```text
raw mouse / touch                 ordinary gamepad / controller
        |                                      |
        v                                      v
thin platform contact adapter     mclone-input source/snapshot contract
        |                                      |
        v                                      v
ContactEvent / direct intent       view-specific controller intents
        +----------------------+---------------+
                               |
                               v
                    shared view-control reducer
        |
        v
product mode policy in Lab UI or mclone-scene
        |
        v
mclone-render-session view/projection facts
```

Platform adapters own pointer capture, browser scroll prevention, lifecycle,
safe-area behavior, and translation from native events. They must not own
camera policy.

`mclone-view-control` owns its neutral contact classification and manipulation
math. `mclone-input` owns controller source identity, canonical snapshots,
settings, and shared physical normalization. `mclone-scene` owns which game
mode is active, which world or player is focused, and whether an intent is
permitted. The renderer consumes the result.

XR head pose is not an orbit gesture. An overview may have a stable model
transform manipulated by controllers while each eye still gets its own
tracked view/projection. Ordinary head wobble must not move the terrain
residency anchor.

## Contextual Gesture Meaning

Gesture recognition should be consistent even when the resulting command
depends on context.

For example:

| Context | Double-tap result |
|---|---|
| World Explorer | focus and zoom toward the selected terrain |
| Tabletop overview | focus or recenter the active model |
| Build/edit mode | select or invoke the configured tool |
| Embodied gameplay | unbound unless explicitly assigned |

Likewise, a one-finger drag may orbit in 3D but grab-pan in a 2D map. That is a
mode mapping over the same contact reducer, not unrelated pointer code.

The initial touch contract should cover:

- one-finger map pan;
- one-finger 3D orbit;
- two-finger pinch with simultaneous centroid pan;
- mouse wheel or trackpad anchored zoom;
- mouse grab-pan and orbit bindings;
- tap versus drag and double-tap;
- cancellation on lost capture, focus loss, or mode transition; and
- equivalent gamepad actions without pretending a stick is a touch pointer.

Tracked-controller manipulation can later map pose deltas into the same
semantic pan/scale/yaw intents used by tabletop mode.

## Product Consumers

### Terrain Lab

Terrain Lab remains the fastest diagnostic host. Its React controls, pane
layout, comparison modes, status readbacks, and developer parameters stay
Lab-only. Its camera reducer and gesture semantics move to shared Rust once the
contract has tests.

The migration should preserve a small TypeScript surface:

- collect browser events;
- maintain pointer capture;
- call the Wasm view-control API;
- request rendering; and
- expose diagnostic state.

Avoid requiring the full client, lighting system, or server just to orbit a
terrain viewport.

### World Explorer

World Explorer now begins as a small standalone native foundation and
acceptance host built from the same terrain-view and view-control services.
Keep it useful for native profiling, input smoke, capture, and clipmap proof,
but do not treat that binary as the primary product merely because it landed
first. The real player surfaces are the lightweight standalone website and
the view embedded in the game. Both should reuse these same services, and the
Web Explorer must not become the Terrain Lab UI with diagnostic controls
hidden.

The first delivery can navigate to or dynamically load the full web client
when the player selects **Enter Here**. A later version may preserve the GPU
device and procedural residency across the transition if measurement shows a
meaningful benefit. Seamlessness is desirable but not a prerequisite for the
product proof.

### Tabletop and overview

Tabletop mode consumes the same scale, pan, orbit, focus, and recenter
semantics. `mclone-scene` still owns:

- active-world selection;
- player-follow and leashed recentering;
- inverse target mapping;
- read-only versus editing authority;
- keyhole/cutaway policy; and
- transition back to embodied play.

This turns Slice 2 of
[`tabletop-overview-mode.md`](tabletop-overview-mode.md) into a shared input
and controller capability rather than a mode-local implementation.

### High-altitude flight

A flight activity can reuse the procedural horizon, fog, materials, map, and
view transitions. It should have a vehicle/controller and flight physics
appropriate to that activity, not drive an orbit camera and call it an
aircraft. Flight controls are a separate future topic once selected.

## Enter-World Handoff

A local or single-player explorer link can carry a compact, non-authoritative
arrival recipe:

- source or world identity;
- generator profile and revision;
- selected X/Z or region;
- desired arrival orientation; and
- optional presentation mode and scale for restoring the view.

The integrated game host resolves that recipe, loads the relevant exact
chunks, validates a safe spawn, and only then places the player. The URL must
not assert Y, collision safety, or authoritative player state.

For a remote server:

- the server owns world identity, admission, and final spawn;
- a hidden seed must not be disclosed just to render the explorer;
- the server may stream coarse terrain/vegetation descriptors, publish a
  curated preview, or expose predefined destinations; and
- **Enter Here** becomes a request the server may accept, adjust, or reject.

Explorer cache and URL identity must include source revision so a preview does
not silently hand off into a different world generation contract.

## Shared Ownership

The current and prospective split is:

- `mclone-view-control`: deterministic navigation state, contact-gesture
  classification, view intents, and reducer;
- `mclone-input`: canonical controller sources, observations, standard
  snapshots, and preferences; a physical-to-view gamepad bridge is deferred;
- `mclone-terrain-view`: bounded procedural terrain presentation, canonical
  terrain compilation, and coordinate picking;
- `mclone-render-session`: final projection/view facts and render lifecycle;
- `mclone-scene`: in-game mode, active-world focus, follow/recenter, admission,
  and authoritative command routing;
- Terrain Lab and Explorer hosts: product UI, URLs, browser lifecycle, and raw
  input adaptation; and
- native/Android/XR apps: platform event and session adaptation only.

The shared terrain presentation now has a second independent consumer in the
native Explorer, and its navigation semantics have a clean reusable owner.
Browser and in-game consumers should adopt those contracts rather than
recreating the reducer.

## Work Streams

Tactical
[`247`](../tactical/247-standalone-world-explorer-foundation.md) completed the
shared view math, native adapter, and first native Explorer shell as one
bounded proof. Tactical 248 subsequently completed the browser input and
Worker-ownership migration using Terrain Lab's existing product surface.
Tactical 249 completed the first cross-platform Explorer and
toroidal-horizon proof.

1. **Browser input ownership audit — complete.** Keep the main game shell and
   standalone DOM hosts separate; share view semantics and canonical
   controller facts.
2. **Migrate Terrain Lab — complete.** Route both panes through one narrow Wasm wrapper
   over the shared reducer while preserving current visuals, inspection,
   scroll containment, URL state, and diagnostics. Delete superseded
   TypeScript camera and gesture policy.
3. **Converge Terrain Lab's Worker runtime — complete.** Reuse the generic browser
   transport, move exact-worker coordination and caches into Rust actors, adopt
   persistent external SAB mailboxes, extend ownership gates, and delete
   superseded TypeScript and legacy raw-admission code. Preserve the current UI,
   separate-pane rendering, URLs, diagnostics, cache behavior, and pixels at
   each checkpoint.
4. **Prove the shared toroidal horizon in the cross-platform Explorer —
   complete.** Treat product scope and platform hosting as independent axes.
   Drive one shared Rust horizon through the lightweight native and web
   Explorer without creating an Explorer-specific browser ABI. Keep Terrain
   Lab unchanged.
5. **Extract a reusable exact-view source.** Adapt locally compiled canonical
   chunks into the shared snapshot, render-session, compile, upload, and draw
   lifecycle. Do not promote Terrain Lab's TypeScript scheduler or duplicate
   the game's cache and Worker framework. Preserve the Lab's working canonical
   view while proving the narrower boundary.
6. **Build the minimal Web Explorer smoke — complete and deployed.** Keep
   JavaScript or TypeScript
   limited to canvas, rAF, lifecycle, URL, raw-observation forwarding, and
   mechanical browser dispositions. Add a deployment smoke and measure the
   independent Wasm/asset payload. A procedural-only result remains a smoke,
   not the player-facing replacement for Terrain Lab.
7. **Compose procedural and exact terrain.** Add the reusable exact near field
   to an Explorer host and validate masking, skirts, replacement, and
   movement before calling it the real map-to-world view.
8. **Connect tabletop Slice 2.** Reuse the same manipulation contract while
   retaining scene-owned follow, authority, and target mapping.
9. **Build the player-facing Explorer UI.** Use the shared terrain view,
   accessible controls, shareable view state, and a deliberately small Wasm
   payload.
10. **Add validated local handoff.** Turn a selected X/Z into a safe,
   authoritative integrated-world arrival.
11. **Design remote preview descriptors.** Do this only when a concrete remote
   product needs seed privacy and server-controlled destinations.
12. **Explore continuous transitions.** Preserve GPU/device/residency state
   only after the simple load-or-navigate flow is useful and measured.
13. **Add gamepad navigation when demanded.** Consume canonical
    `mclone-input` snapshots and emit tested view intents without inheriting
    gameplay bindings or putting semantics in the browser shell.

The clipmap work stream in
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md) can progress
in parallel at the architecture level. A first controller extraction should
not wait for the complete in-game horizon.

## Validation

The shared reducer needs deterministic tests for:

- positive and negative pan/orbit directions;
- anchored zoom preserving the world point beneath the cursor or centroid;
- simultaneous pinch and centroid movement;
- tap, double-tap, drag, cancellation, and pointer-count transitions;
- pitch/distance/scale limits;
- frame-rate independence and damping;
- map/orbit/follow mode transitions; and
- stable results across native and Wasm floating-point paths.

Every host then needs interaction and rendered evidence:

- desktop mouse, wheel, trackpad, and gamepad;
- browser mouse and representative phone/tablet touch;
- flat Android touch and controller;
- synthetic stereo before XR integration;
- tracked-controller tabletop manipulation when that slice exists;
- screen-reader labels, keyboard reachability, and reduced-motion behavior for
  the player-facing Explorer; and
- safe-spawn and stale-source rejection at the enter-world boundary.

Touch validation must include interrupted gestures and page-scroll
containment, not only a successful pinch in an automated happy path.

## Non-Goals For The First Slice

- replacing ordinary first-person camera and locomotion;
- implementing aircraft physics;
- adding terrain editing to the Explorer;
- exposing private server seeds;
- bundling the full client into the initial map page;
- moving Terrain Lab diagnostics into the player UI;
- retiring Terrain Lab before a reusable exact view is proven;
- solving seamless GPU ownership transfer before basic handoff works; or
- making overview mode authoritative merely because it can pick terrain.

## Open Questions

- Which projection transition best connects a 2D map to perspective orbit
  without disorienting touch users?
- What URL state is stable and compact enough for long-lived shared links?
- Can Explorer and the full web client share one `GPUDevice` and canvas
  lifecycle cleanly, or is regeneration cheaper and simpler?
- How should controller focus move between map UI, terrain manipulation, and
  the **Enter Here** action?
- Which parts of orbit inertia and double-tap timing should be user settings?
- What remote summary format preserves useful terrain without revealing
  private generation inputs?
- When does the high-altitude view become a distinct flight mode rather than
  another camera scale?

## Related Documents

- [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md) — shared
  distant terrain and exact-chunk handoff.
- [`gpu-procedural-terrain.md`](gpu-procedural-terrain.md) — Terrain Lab
  product direction and current shared terrain evidence.
- [`tabletop-overview-mode.md`](tabletop-overview-mode.md) — active-world model
  policy, follow, targeting, authority, and implementation sequence.
- [`embedded-worlds.md`](embedded-worlds.md) — seed-explorer console, local
  previews, warm worlds, and world activation.
- [`controller-input.md`](controller-input.md) — all-target semantic input and
  controller architecture.
- [`platform-host-boundary.md`](platform-host-boundary.md) — thin host adapter
  and shared Rust policy boundary.
- [`../native-web.md`](../native-web.md) — browser client runtime, build, and
  host ownership.
- [`../native-engine-architecture.md`](../native-engine-architecture.md) —
  shared scene, render-session, renderer, and platform architecture.
