# World View Navigation And Exploration

Topic: `world-view-navigation`

Status: native foundation implemented and validated on 2026-07-25 by Tactical
[`247`](../tactical/247-standalone-world-explorer-foundation.md).
`mclone-view-control` now owns shared map/orbit/contact semantics, and the
standalone native `mclone-world-explorer` consumes it with the shared
procedural terrain renderer. Terrain Lab still uses its duplicated TypeScript
camera and gesture logic. Browser-shell and Lab migration remain deferred
until a follow-up compares the main game's browser rim, `mclone-input`, the
Lab, and the landed native adapter.

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

The remaining Lab weak point is navigation. Pure camera math currently lives in
[`state.ts`](../../tools/terrain-lab/src/state.ts), while pointer gesture state
is duplicated between
[`TerrainCanvas.tsx`](../../tools/terrain-lab/src/web/TerrainCanvas.tsx) and
[`CanonicalTerrainCanvas.tsx`](../../tools/terrain-lab/src/web/CanonicalTerrainCanvas.tsx).
That code has been valuable for discovery, but copying it into the engine
would make its janky edge cases permanent.

The first shared Rust semantics now exist; the direction is to migrate the Lab
through the selected browser-input boundary and leave DOM components as event
adapters.

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

The pinned release acceptance sequence rendered initial 3D, continuous X/Z/
diagonal movement, anchored zoom, map, and orbit through both a real
Wayland/Vulkan surface and an offscreen target. Corresponding captures were
byte-identical and had direct depth coverage. On the closeout host, native
first-coarse and target-ready times were 83.26 and 103.79 ms; 30 input frames
averaged 1.14 ms with 4.97 ms p95; peak bounded-preview residency was
171,994,752 bytes; and final pending work was zero. These are proof-host
observations, not product budgets. Full receipts, hashes, artifact sizes, and
commands are recorded in Tactical 247.

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

The reducer produces camera/view facts. It does not choose what a selected
block means, teleport a player, mutate terrain, or issue a network command.

## Input Layering

Keep the path explicit:

```text
raw mouse / touch / gamepad / tracked controller events
        |
        v
mclone-input contacts, capabilities, and semantic sources
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

`mclone-input` owns neutral contact and controller semantics.
`mclone-view-control` owns manipulation math. `mclone-scene` owns which game
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

World Explorer now begins as a small standalone native product/diagnostic
host built from the same terrain-view and view-control services. It proves the
crate and lifecycle boundary, but it is not yet the intended public web
onboarding product. That Web Explorer should reuse these same services and
must not become the Terrain Lab UI with diagnostic controls hidden.

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

- `mclone-view-control`: deterministic navigation state and intent reducer;
- `mclone-input`: device-neutral pointer/contact, gamepad, and XR manipulation
  inputs;
- `mclone-terrain-view`: bounded terrain presentation and coordinate picking;
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
bounded proof. The next implementation must begin with the recorded
input-boundary audit, then proceed through the minimal web shell, Terrain Lab
migration, player-facing UI, and validated arrival.

1. **Audit browser input ownership.** Compare the main browser client,
   `mclone-input`, both Terrain Lab panes, and the native Explorer adapter;
   select the neutral raw-contact/intent boundary before adding another
   TypeScript implementation.
2. **Build the minimal Web Explorer.** Keep JavaScript or TypeScript limited
   to canvas, rAF, lifecycle, URL, and raw-observation forwarding; add a
   deployment smoke and measure the independent Wasm/asset payload.
3. **Migrate Terrain Lab.** Route both panes through the selected boundary and
   shared reducer while preserving current visuals and diagnostics, then
   delete superseded TypeScript camera and gesture policy.
4. **Connect tabletop Slice 2.** Reuse the same manipulation contract while
   retaining scene-owned follow, authority, and target mapping.
5. **Build the player-facing Explorer UI.** Use the shared terrain view,
   accessible controls, shareable view state, and a deliberately small Wasm
   payload.
6. **Add validated local handoff.** Turn a selected X/Z into a safe,
   authoritative integrated-world arrival.
7. **Design remote preview descriptors.** Do this only when a concrete remote
   product needs seed privacy and server-controlled destinations.
8. **Explore continuous transitions.** Preserve GPU/device/residency state
   only after the simple load-or-navigate flow is useful and measured.

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
- solving seamless GPU ownership transfer before basic handoff works; or
- making overview mode authoritative merely because it can pick terrain.

## Open Questions

- Should the shared reducer be a new `mclone-view-control` crate or a module in
  `mclone-input` with a strict renderer-free boundary?
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
