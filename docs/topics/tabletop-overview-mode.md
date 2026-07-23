# Tabletop Overview Mode

Topic: `tabletop-overview-mode`

Status: **potential feature direction researched 2026-07-23; no implementation
or tactical is open.** The recommended product shape is one shared active-world
overview mode with flat, touch, gamepad, tracked-controller, and later
hand-tracking interaction. XR may place that mode over passthrough when the
runtime supports it, but passthrough is a presentation capability rather than
the feature's owner or availability boundary.

Last reconciled: **2026-07-23**.

## Scope

This topic owns the potential player-facing mode in which the active world is
presented as a manipulable scale model:

- a flat-screen orbit or god-view building mode;
- an XR tabletop or floating-diorama mode;
- passthrough-backed mixed reality where the XR runtime supports it;
- shared pan, rotation, scale, pointing, selection, place, and break intent;
- inverse mapping from presentation-space hits to canonical world targets;
- creative, survival, multiplayer, and remote-authority policy for actions
  performed away from the player's embodied reach; and
- the bounded implementation and validation path from read-only overview to
  authoritative editing.

It does **not** own:

- general second-world composition, lobby destination previews, warm-world
  transfer, portals, or nested-world recursion; those remain in
  [`embedded-worlds.md`](embedded-worlds.md);
- durable realm, dimension, player, or observer topology; that remains in
  [`realm-dimension-runtime.md`](realm-dimension-runtime.md);
- the general all-device semantic input architecture; that remains in
  [`controller-input.md`](controller-input.md);
- the OpenXR frame-loop sequence or platform activity/window ownership; those
  remain in `mclone-xr-host` and the desktop/Android XR adapters;
- a general creative-mode, permissions, blueprint, or remote-construction
  system; or
- spatial room meshes, semantic furniture recognition, camera-frame access,
  or persistent real-room anchors.

## Product Thesis

Mclone should let a player experience one living world at two useful scales:

- **embodied**, where one block has its ordinary physical scale and the player
  moves through the world; and
- **overview**, where a bounded region of that same canonical world is
  re-presented as a scale model that can be inspected and, when authority
  permits, edited.

This is not merely a camera teleported high into the sky. A high source-space
camera can imitate the flat visual angle, but it does not create the tactile XR
effect, a room-space model, or a shared interaction transform. The unifying
primitive is a reversible mapping between canonical world coordinates and
presentation coordinates:

```text
canonical active world
  -> bounded source selection
  -> scale / yaw / placement transform
  -> flat viewport or physical XR space

pointer hit in presentation space
  -> inverse transform
  -> canonical block/entity target
  -> ordinary authoritative command
```

The same mode can therefore feel like an orbit-camera builder on desktop, a
touch-manipulated model on mobile, and a physical model on a table in XR
without creating separate gameplay implementations.

The strongest Mclone-specific version is a **living survival-world model**.
Creatures, players, water, time, farms, machines, and multiplayer activity
continue inside the miniature. The presentation is useful for building and
planning, while Mclone's world depth and cross-device continuity distinguish
it from a creative block-placement novelty.

## Discovery 2 Reference

The immediate product reference is
[Discovery 2](https://www.meta.com/experiences/discovery-2/8734464566611164/),
a Quest block-building game by noowanda. Its current listing and trailer pair
immersive first-person play with a mixed-reality tabletop mode. The footage
shows a circularly cropped block world floating in the player's room, virtual
hands/controllers, block-level placement and removal, and multiple model
scales:

- [Discovery 2 listing and trailer](https://www.altlabvr.com/discovery-2)
- [Developer's original tabletop prototype notes](https://devpost.com/software/discovery-tabletop-edition)
- [January 2025 Quest 3 hands-on](https://mixed-news.com/en/discovery-2-quest-3-hands-on/)

The developer describes the prototype as a controller-free, hand-tracked MR
version made for family members who should not need artificial locomotion. The
hands-on report describes a circular world section that can be moved, rotated,
and zoomed, and finds the overview materially useful for building and comfort.

Relevant lessons:

- the model must be manipulable, not a passive minimap;
- direct block targeting is the core interaction proof;
- switching between embodied and overview presentations is valuable;
- passthrough makes a compelling XR setting but is not the underlying game
  mechanic;
- an overview reduces artificial-locomotion discomfort and broadens the XR
  audience; and
- hand tracking is attractive, while controllers remain the more precise
  high-throughput building tool.

Discovery 2 should remain a product/interaction reference, not an
implementation source. Mclone should not copy its visual identity, circular
crop treatment, menus, hands, or control details blindly.

## Relationship To Existing Mclone Work

Mclone has already landed much of the difficult renderer foundation through
the live hosted-world diorama campaign:

- `mclone-render::WorldPlacement` maps a source anchor to a composition anchor
  under a positive uniform scale;
- placed terrain has mono, per-eye, and full-frame multiview variants;
- placed terrain and the active world share physical depth and globally
  ordered translucency;
- placed actor rendering covers creatures and remote players;
- retained previews receive authoritative block and actor mutations;
- flat, touch, and XR rays can target the preview volume; and
- the normal one-world path remains structurally direct when no preview is
  present.

The current product diorama is nevertheless a different feature. It shows a
bounded region from a retained destination world while the lobby remains
active, and `Use` on the preview begins whole-slot activation. It does not let
the player point through the placement transform and edit an individual
miniature block.

This topic should reuse the placed-world renderer but should not require a
second runtime:

```text
existing lobby diorama
  active lobby slot + retained destination slot
  destination is observer-backed
  preview Use means activate/swap worlds

proposed active-world overview
  one active slot
  bounded records come from that same slot
  pointer means inspect/select/edit a canonical target
  no warm-world lifecycle or slot exchange is required
```

This reuse was already anticipated by `embedded-worlds.md`: active-world
regions are valid non-recursive preview sources, and current-location/distant
active-world tabletop views are named follow-ups. The new topic exists because
turning that renderer reuse into a complete interaction mode introduces its
own camera, input, authority, streaming, comfort, and passthrough decisions.

## Terminology

- **Embodied mode**: ordinary first-person or configured player-camera play.
- **Overview mode**: the shared active-world scale-model presentation and
  input context.
- **Tabletop**: the spatial XR expression of overview mode. The model may sit
  on a real table, a virtual plinth, or float at a comfortable height; a
  detected physical table is not required.
- **Presentation transform**: the reversible source-to-composition mapping
  used for rendering, culling, pointing, and manipulation.
- **Focus**: the canonical source-space point around which overview selection,
  bounds, and interest are resolved.
- **Overview interest**: optional bounded world residency contributed around
  the focus independently of the embodied player's location.
- **Interaction policy**: authoritative rule deciding whether an overview hit
  is read-only, limited to embodied reach, or eligible for remote editing.

“God mode” is useful conversational shorthand but should not be the contract
name. It conflates presentation with elevated gameplay permission.

## Proposed User Experience

### Entering and leaving

Overview mode should be an explicit shared action available from an appropriate
menu or binding. Its initial focus is the player's canonical location or a
selected point of interest. Leaving the mode returns to the same active world
and body rather than loading or swapping a runtime.

The player's body remains a canonical server fact while overview is active.
The mode must explicitly decide whether that body is visible as a miniature,
stationary, vulnerable, or protected by a separate game rule. It must never
silently reinterpret physical model manipulation as player movement or publish
the overview camera as the player's body pose.

Multiplayer cannot pause merely because one local participant entered
overview. Any single-player pause behavior, if later desired, must be explicit
and use the normal pause policy.

### Manipulating the view

The shared semantic intents should be independent of source hardware:

| Intent | XR expression | Flat/gamepad/touch expression |
|---|---|---|
| Move model or focus | one-hand/grip drag | pointer drag, stick pan, or one-finger pan |
| Rotate around vertical | controller/hand twist | pointer drag, keys, or right stick |
| Change scale | two-hand spread/pinch | wheel, triggers, keys, or two-finger pinch |
| Point/select | controller ray, hand ray, or fingertip | cursor, reticle, stick pointer, or tap |
| Primary/secondary tool | trigger, button, or pinch | attack/use bindings or contextual buttons |
| Recenter | explicit action | explicit action |

The mode should preserve stable focus during rotation and scaling. Gesture
recognition belongs at the platform/input edge, but manipulation state,
constraints, smoothing, and meaning belong in shared Rust.

The first implementation should prefer a constrained yaw rotation and positive
uniform scale. Arbitrary pitch/roll of a voxel world makes controls, gravity,
text, and vertical construction harder to read and is not necessary for the
Discovery-style effect.

### Selecting and editing

Picking must operate on the displayed geometry:

1. construct a pointer ray in composition space;
2. inverse-transform it into active-world source space;
3. intersect canonical blocks/entities under the bounded presentation policy;
4. resolve the same face/adjacent-placement target used by embodied play; and
5. submit an ordinary typed interaction command with an explicit overview
   interaction context.

Selection outlines, break progress, placement previews, particles, and world
UI eventually need the same presentation transform as terrain and actors.
They must not be drawn only in mono/per-eye while disappearing in multiview.

## Authority And Game-Mode Policy

Presentation does not grant authority. Unlimited distant editing would bypass
survival reach, multiplayer trust, protected-region policy, and any future
game-mode distinction.

Recommended initial policy:

| Context | Overview viewing | Overview place/break |
|---|---|---|
| Local creative/editor world | allowed | allowed through explicit creative/editor authority |
| Authorized multiplayer builder/admin | allowed | allowed within server-granted scope |
| Ordinary survival | allowed near loaded/visible scope | denied outside ordinary embodied reach |
| Protected lobby | allowed only if product UX calls for it | denied by authoritative world behavior |
| Unknown remote server capability | conservative/read-only | denied |

A read-only overview can ship before the project has a complete creative-mode
or permission system. The first editable slice should add a narrow typed
authority contract rather than infer permission from client UI state.

Possible later survival-native interactions include blueprints, construction
orders, helper creatures, machines, or drones. Those would preserve the
fantasy of planning from above without making resources and reach irrelevant,
but they are separate gameplay features rather than prerequisites for overview
presentation.

## Shared Ownership

The feature should follow existing shared-first boundaries:

- **`mclone-input`**: neutral manipulation and pointing observations, shared
  semantic actions, hand/controller identity, and device-independent intent.
- **`mclone-scene`**: `Embodied` versus `Overview` state, transition policy,
  focus, manipulation reduction, input context, physical-view preparation,
  active-slot selection, render submission, and camera/body publication
  separation.
- **`mclone-render-session`**: neutral mono/stereo view and ray contracts where
  renderer-facing presentation facts belong.
- **`mclone-render`**: reversible placement including yaw, placed terrain and
  actor draws, presentation-aware culling, clipping, and later outlines or
  effects.
- **`mclone-client`**: canonical target selection and client interaction state
  where the existing block/entity interaction owner requires it.
- **`mclone-server`**: reach, game-mode, permission, protected-world, and
  remote-edit authorization.
- **`mclone-xr-host`**: OpenXR passthrough handle/layer lifecycle and
  composition-layer submission through `OpenXrFrameDriver`.
- **Platform apps**: raw mouse/touch/gamepad/OpenXR collection, concrete
  surface/session capabilities, and presentation of requested host effects.

No shared scene or gameplay crate should acquire OpenXR, `winit`, DOM, Android
activity, or camera-frame types.

A provisional shared shape is:

```rust
enum WorldPresentationMode {
    Embodied,
    Overview(OverviewState),
}

struct OverviewState {
    focus: Vec3d,
    presentation: OverviewPresentation,
    source_bounds: OverviewBounds,
    interaction_policy: OverviewInteractionPolicy,
}

struct OverviewPresentation {
    composition_anchor: Vec3d,
    uniform_scale: f64,
    yaw_radians: f64,
}
```

Exact type placement should follow implementation evidence. This sketch records
the distinction between canonical focus, visual placement, and authoritative
interaction policy.

## Active-World Rendering And Interest

The first renderer proof should reuse the active slot's already-prepared
records and immutable resources. It should not create a second server,
connection, replica, compiler, terrain store, or actor cache.

Overview mode may suppress the ordinary full-scale active-world submission and
submit only the placed bounded region. Drawing the same source both directly
and as a miniature is useful for later in-world tables, but it is not required
for a full-screen flat editor or passthrough tabletop and would add unnecessary
cost.

Initial focus should remain near the embodied player and inside the existing
resident set. Free panning beyond that set requires a separate bounded
overview-interest source. `RealmServer` already has player/observer interest
concepts, but a remote session does not yet have a general independent
observer subscription stream. Do not move the canonical body merely to make
distant chunks load, and do not promise unrestricted remote panning before the
transport and server authority contract exists.

The direct embodied path must retain the current structural invariant:

```text
overview off -> existing direct one-world prepare/draw/input path
```

No per-section world-id lookup, placement branch, overview observer, or
passthrough work should enter ordinary play when the feature is off.

## Placement, Rotation, And Bounds

Current `WorldPlacement` supports source anchor, composition anchor, and a
positive uniform scale. It explicitly assumes no camera-basis rotation.
Discovery-style manipulation needs at least a yaw-capable reversible transform
applied consistently to:

- terrain and actor shaders;
- CPU frustum and source-view derivation;
- translucent ordering;
- ray and selection inverse mapping;
- composition bounds and activation/selection volumes;
- particles, outlines, debug geometry, and world UI as they adopt the mode;
  and
- audio positions if overview retains spatial world audio.

The existing preview uses rectangular chunk/section source bounds. A polished
circular or rounded tabletop crop is optional. Arbitrary fragment clipping can
expose faces omitted by canonical neighbor meshing, so a visual boundary needs
one of:

- an authored or generated plinth/skirt that hides the cut;
- boundary-aware cap faces;
- a deliberately rectangular complete-chunk presentation;
- fog/dissolve that hides rather than exposes the seam; or
- a later clip-volume renderer with explicit mesh-boundary behavior.

The first proof should choose the cheapest honest boundary and avoid making a
circular crop an acceptance requirement.

## Passthrough Contract

Passthrough is a capability-gated XR background:

```text
shared scene requests overview presentation
  -> XR host reports passthrough capability
     -> available and enabled: runtime-composited passthrough underlay
     -> unavailable/disabled: virtual room, plinth, or neutral environment
```

The Android XR adapter currently discovers and enables
`XR_FB_passthrough` and `XR_FB_composition_layer_alpha_blend` when available,
but Mclone does not create, start, submit, pause, or destroy a passthrough
feature/layer. `mclone-xr-host::selected_environment_blend_mode` currently
prefers `OPAQUE`. The extension advertisement is therefore bring-up
preparation, not a working product feature.

The implementation should:

- expose a typed host capability rather than checking “Quest” in shared code;
- let `OpenXrFrameDriver` remain the exclusive owner of
  poll/wait/begin/render-or-skip/end sequencing;
- create and destroy runtime passthrough handles with the session;
- submit the passthrough composition layer beneath the projection layer;
- clear non-world background pixels appropriately and preserve projection
  alpha/composition flags required by the chosen runtime path;
- pause/resume passthrough with application/session lifecycle;
- fall back cleanly when unsupported or when the user disables it; and
- validate the exact layer/blend behavior on Quest rather than assuming that
  enabling the extension is sufficient.

`XR_FB_passthrough` lets the runtime compositor supply the camera imagery; the
game need not receive raw camera frames for this mode. That is the preferred
privacy boundary. Store declarations and user-facing disclosure still need to
match the actual platform behavior. Scene meshes, table detection, raw camera
access, or persistent spatial anchors would be separate capability and privacy
expansions.

Desktop OpenXR runtimes may or may not expose a compatible passthrough path.
They should receive the same capability check and virtual fallback. Flat
desktop, web, and Android use the overview mode without passthrough; this topic
does not require adding WebXR.

## Hand Tracking

Quest bring-up currently enables hand-tracking-related extensions when
available but does not create hand trackers or publish articulated hand
observations. The mode should therefore reach a complete, efficient tracked-
controller interaction before hand tracking becomes an acceptance dependency.

Later hand support should reuse the same semantic intents:

- pinch or poke to select;
- pinch plus motion to manipulate;
- two-hand spread/twist for scale and yaw;
- explicit capture so a placement pinch cannot also move the world; and
- controller-equivalent menu/tool access.

Direct fingertip placement should be attempted only at a scale where one block
is a reliably selectable physical size. Ray interaction remains a necessary
fallback for distant or very small blocks.

## Proposed Implementation Sequence

No tactical should be opened until the product authority decision and first
proof boundary are accepted.

### Slice 0 — Contract and direct-path lock

- Characterize current active-world prepare/draw/input paths.
- Define overview state, focus, interaction policy, and source bounds without
  OpenXR types.
- Add source locks proving ordinary play constructs no overview state or work.

### Slice 1 — Read-only active-world overview

- Reuse a bounded set of the active slot's terrain and actor resources.
- Render it through the existing placed mono/per-eye/multiview paths.
- Start centred near the player with no distant-interest expansion.
- Add a flat offscreen view and synthetic-stereo capture.

This is the first drawable review checkpoint.

### Slice 2 — Shared manipulation

- Add a yaw-capable presentation transform and inverse mapping.
- Add neutral pan/scale/yaw/recenter intents.
- Implement mouse, touch, gamepad, and tracked-controller mappings through the
  shared input/session owner.
- Keep the mode read-only while exact pointing and target highlighting are
  proven.

### Slice 3 — Authoritative creative editing

- Define the narrow server-validated overview editing capability.
- Route exact placed-world block hits through ordinary place/break commands.
- Prove persistence, protected-world denial, multiplayer denial/allow, and
  local/remote behavior supported by the actual protocol.

### Slice 4 — Quest mixed reality

- Add the capability-gated passthrough underlay in the shared OpenXR host
  boundary and Android XR adapter.
- Add user choice and virtual fallback.
- Validate real Quest lifecycle, stereo/multiview pixels, comfort, and frame
  pacing.

### Slice 5 — Expansion only after evidence

- Hand tracking.
- Distant focus and bounded observer interest.
- Circular/rounded crop and boundary polish.
- Miniature local-player embodiment.
- Blueprint or survival-native remote construction.
- Spatial table discovery or persistent room anchoring.

## Validation Requirements

Every implementation slice that produces pixels must capture and inspect the
smallest affected target before adding more complexity.

Required contract evidence should include:

- transform and inverse-transform round trips at ordinary and far coordinates;
- stable focus under scale and yaw;
- exact block-face and adjacent-placement target selection;
- active-world mutations visible in the overview and persisted after reopen;
- no duplicate runtime, replica, draw store, actor cache, or player identity;
- no overview work or measurable regression when the mode is off;
- flat mouse, touch, and gamepad semantic parity;
- synthetic per-eye stereo differences with correct shared depth;
- full-frame multiview execution on a capable device;
- Quest passthrough enable/disable/fallback and session pause/resume;
- survival/protected-world denial enforced by the server;
- multiplayer capability rejection when the remote host lacks support; and
- sustained Quest frame pacing under real terrain and actor activity.

XR screenshots alone may not prove that runtime-composited passthrough was
visible if the platform capture path omits camera imagery. Retain runtime
capability/layer receipts and an explicit in-headset inspection alongside
whatever device capture the runtime supplies.

## Risks

### Survival erosion

Remote direct editing can trivialize movement, danger, resource transport, and
construction labor. Treat interaction authority as a product/gameplay decision
and not a convenience automatically granted by the renderer.

### Camera/body authority conflict

The overview camera is presentation state. Publishing it as the player's body
pose would teleport or fight authoritative reconciliation. The embodied body,
overview focus, and physical XR camera must remain distinct.

### Unbounded residency cost

A freely pannable high-level view can demand large chunk, mesh, actor, and Far
LOD sets. Start bounded near the player, then add an explicit overview budget
and interest source rather than borrowing unrestricted active-player residency.

### Input ambiguity

The same pinch, trigger, or pointer can mean manipulate model, select UI,
break/place, or return to embodied mode. Shared input contexts and pointer
capture must make these states visible and deterministic.

### XR performance

Passthrough composition does not make terrain free. Large overview coverage,
transparent boundaries, actors, selection effects, and hand tracking all add
cost on the target with the strictest frame budget. Scale and crop should
reduce submitted detail under an explicit policy rather than merely draw every
visible source block.

### Platform fork

Shipping Quest-first scene logic would turn passthrough into the owner of a
mode that is equally useful on flat clients. Shared state, interaction, and
authority must land first; XR contributes physical poses and an optional
background capability.

## Decisions Recorded

- Treat the idea as a shared active-world overview mode, not a Quest-only
  passthrough feature.
- Reuse placed-world geometry and actor rendering rather than inventing a
  render-to-texture minimap.
- Keep canonical simulation scale and coordinates unchanged.
- Keep the active-world source in the existing slot; do not require a warm
  second runtime.
- Distinguish overview focus/camera from the embodied player body.
- Require server-authoritative permission for editing beyond ordinary reach.
- Support a read-only first proof before creative/admin authority exists.
- Make tracked controllers the first complete XR tool and hand tracking a
  later input source.
- Keep passthrough capability-gated with a complete virtual fallback.
- Preserve a structurally direct and unaffected embodied path when overview is
  off.

## Open Decisions

- What public name best fits the product: Overview, Tabletop, Architect,
  Overseer, Build View, or another original term?
- Is read-only overview available in ordinary survival, and can the body be
  harmed while using it?
- Which game mode or permission first grants distant editing?
- Does the first view show only the region around the player, a selected build
  site, or a bounded settlement?
- Should the local player appear as a miniature body?
- Which input gestures are reserved for model manipulation versus block tools?
- What scale range keeps blocks selectable without excessive geometry?
- Is the first boundary rectangular, skirted, fogged, or circular?
- Does overview retain spatial world audio, use a focus-centred mix, or reduce
  to UI/ambient sound?
- How should remote servers advertise overview interest and editing support?
- Should entering the model transition directly back to embodied play at the
  selected point, and if so what authoritative travel rule permits it?

## Recommended Next Direction

Keep this as a researched potential until current product priorities select it.
When selected, open a bounded tactical for Slices 0–1 only: a read-only,
active-world, player-centred overview using the landed placed terrain/actor
paths on flat and synthetic stereo, with the ordinary path locked unchanged.

Do not begin with passthrough, hand tracking, circular clipping, distant
streaming, or survival remote editing. Those depend on the shared presentation
proof and each introduces an independent authority, lifecycle, input, or
performance question.

## Related Documents

- [`embedded-worlds.md`](embedded-worlds.md) — placed geometry, live dioramas,
  warm worlds, active-world-source reuse, and the direct-path invariant.
- [`realm-dimension-runtime.md`](realm-dimension-runtime.md) — player and
  observer authority and interest.
- [`controller-input.md`](controller-input.md) — shared semantic input,
  gamepad, touch, and tracked-controller boundaries.
- [`platform-parity.md`](platform-parity.md) — all-target feature ownership and
  validation expectations.
- [`distribution-go-to-market.md`](distribution-go-to-market.md) — Discovery 2
  in the Quest competitive landscape and cross-device positioning.
- [`../native-engine-architecture.md`](../native-engine-architecture.md) —
  shared scene, renderer, input, and OpenXR adapter ownership.
- [`../tactical/175-live-hosted-world-diorama.md`](../tactical/175-live-hosted-world-diorama.md)
  — landed local diorama rendering, mutation, ordering, and activation record.
- [`../tactical/179-composable-world-presentation-and-live-preview-actors.md`](../tactical/179-composable-world-presentation-and-live-preview-actors.md)
  — normalized placement/clip contracts and placed actor rendering.
