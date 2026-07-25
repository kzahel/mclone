# World View Navigation And Exploration

Topic: `world-view-navigation`

Status: accepted product and shared-architecture direction recorded on
2026-07-25. Terrain Lab currently proves much of the terrain presentation, but
its duplicated TypeScript camera and gesture logic is prototype code. No
shared Rust view-control owner or public World Explorer flow is implemented
yet. Tactical
[`247`](../tactical/247-standalone-world-explorer-foundation.md) proposes the
first two-host implementation: a small native Explorer, shared Rust view
controller, and Terrain Lab migration.

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

The weak point is navigation. Pure camera math currently lives in
[`state.ts`](../../tools/terrain-lab/src/state.ts), while pointer gesture state
is duplicated between
[`TerrainCanvas.tsx`](../../tools/terrain-lab/src/web/TerrainCanvas.tsx) and
[`CanonicalTerrainCanvas.tsx`](../../tools/terrain-lab/src/web/CanonicalTerrainCanvas.tsx).
That code has been valuable for discovery, but copying it into the engine
would make its janky edge cases permanent.

The direction is to migrate proven semantics into shared Rust and leave DOM
components as event adapters.

## Shared View-Control Contract

Introduce a small sans-I/O shared owner, tentatively
`mclone-view-control`. It must not depend on:

- WGPU or renderer resources;
- world generation or chunk state;
- lighting or materials;
- client/server simulation;
- DOM, `winit`, Android, or OpenXR; or
- one product's UI hierarchy.

Its conceptual state includes:

```text
ViewState {
  mode: Map | Orbit | Follow | Tabletop
  focus_world_position
  scale_or_distance
  yaw
  pitch
  projection
  optional follow target
}
```

It consumes semantic intentions rather than platform events:

```text
Orbit(delta)
GrabPan(delta)
Zoom(log_delta, anchor)
Recenter
FocusAt(world_position)
Tap(target)
DoubleTap(target)
```

Names are illustrative until a tactical settles the API. The durable
requirements are that the shared reducer owns:

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

World Explorer should be a small player-facing WASM application or loading
phase built from the same terrain-view and view-control services. Its UI is
not the Terrain Lab UI with diagnostic controls hidden.

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

The prospective split is:

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

If a new crate is not justified initially, the reducer may begin as a clean
module in an existing shared crate. Its dependency boundary matters more than
the crate name.

## Work Streams

Tactical
[`247`](../tactical/247-standalone-world-explorer-foundation.md) owns work
streams 1–3 and the first native Explorer shell as one bounded two-host proof.
Its completion handoff identifies the later player-facing web shell and
validated-arrival tacticals.

1. **Specify and test view math.** Extract a small behavior table from current
   Lab zoom, pan, grab, pinch, and orbit behavior; fix directions and edge
   cases in deterministic Rust tests.
2. **Migrate one Lab pane.** Route it through the shared reducer while keeping
   current visuals and diagnostics; then remove duplicated pointer policy from
   the second pane.
3. **Add native input adapters.** Prove mouse, trackpad, touch, and gamepad
   intent parity without importing platform types into the reducer.
4. **Connect tabletop Slice 2.** Reuse the same manipulation contract while
   retaining scene-owned follow, authority, and target mapping.
5. **Build a player-facing Explorer shell.** Use the shared terrain view,
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
