# Visor VR Architecture Reference

Topic: visor-vr-reference

Status: research snapshot complete on 2026-07-11. No mclone implementation
decision is implied by this note.

## Scope

This note records ideas worth carrying from
[Visor](https://github.com/VisorModStudio/Visor) into mclone, especially its XR
UI, input, rendering, lifecycle, addon, settings, and remote-pose structure.
Visor and Minecraft 1.17.1 are secondary comparative references, not sources
of product truth for Mclone's original gameplay or visual direction.

The useful conclusion is not "port Visor." Visor is a Java mod embedded in
Minecraft 1.20.1 and its codebase exposes Minecraft, OpenGL, GLFW, and AtumVR
types.
The useful conclusion is that several of its *conceptual seams* are more mature
than mclone's first XR slices and can help us generalize those slices without
weakening mclone's host-neutral Rust architecture or multiview path.

## Snapshot And Source Quality

The source was cloned to `reference/Visor` from
`https://github.com/VisorModStudio/Visor.git`.

| Fact | Snapshot |
|---|---|
| Branch | `dev` (the remote default branch) |
| Commit | `7cd54e93579785be0911c7555484c66446251401` |
| Commit date | 2026-07-08 |
| Latest GitHub release observed | [`0.4.0`](https://github.com/VisorModStudio/Visor/releases/tag/0.4.0), released 2026-06-25 |
| Minecraft target in the clone | 1.20.1, Java 17 |
| Loaders | Fabric and Forge |
| XR foundation | OpenXR through bundled AtumVR beta snapshot jars |
| License | LGPL-3.0-only |

The [Modrinth project page](https://modrinth.com/mod/visor) describes Visor as
an OpenXR, addon-oriented platform built from scratch, supporting Fabric and
Forge on Minecraft 1.20-1.20.1. It also explicitly calls the project early and
potentially unstable. The repository README calls it beta.

The public wiki is not a reliable substitute for the code yet. Its home page
says the wiki is in development, and the GUI, rendering, and input pages were
still one-character placeholders when inspected. The lifecycle and addon pages
contain useful high-level descriptions, but the checked-out source is the
authority for the details below.

The repository has release/publish workflows but no `src/test` tree or JUnit
tests in this snapshot. Treat its design as evidence and inspiration, not as a
validated implementation specification. In particular, mclone should translate
the valuable cases into deterministic unit tests, synthetic-stereo tests, and
render captures rather than inherit untested assumptions.

The Visor license and bundled binary dependencies also matter: study and
reimplement the ideas in mclone's own abstractions. Do not copy code, shaders,
textures, or binaries without a separate license and dependency review.

## Is It A "Newer Better Vivecraft"?

Visor is better described as a newer *architecture direction* than as a
universally better or chronologically newer Vivecraft. The local Vivecraft
reference was also active in July 2026. No headset UX, compatibility,
performance, or feature-completeness comparison was run for this study.

What is materially different is Visor's product posture:

- it presents itself as a small VR platform for addons rather than only a VR
  conversion mod;
- it has a named public API module and addon-owned component registries;
- overlays, render effects, body types, action sets, tasks, and presets share a
  component vocabulary;
- its UI supports configured overlay instances made from reusable templates;
- it makes the local VR session optional while retaining remote-VR-player
  rendering as a non-VR capability.

Those choices make Visor especially interesting to mclone because mclone is an
engine with multiple native client profiles, not a patch set that must preserve
one Minecraft desktop client's internal shape.

## Code Structure Map

| Visor area | Responsibility | Closest mclone owner | Main lesson |
|---|---|---|---|
| `visor-api` | Public addon, UI, input, render, pose, and network contracts | `mclone-ui`, `mclone-input`, `mclone-render-session`, `mclone-protocol` | Make extension seams explicit and value-oriented |
| `visor-core` | Shared Fabric/Forge behavior, OpenXR provider, GUI, effects, settings, networking | `mclone-scene` plus focused shared crates | Central orchestration can compose focused registries without making platform apps own policy |
| `visor-fabric`, `visor-forge` | Loader entry points, loader networking, and loader-specific mixins | desktop/Android/XR/web app adapters | Keep true platform integration thin |
| `core/client/gui` | Overlay manager, cursor resolver, built-in overlays, settings screens | `mclone-ui`, `mclone-scene`, `mclone-render` | Separate semantic UI, spatial placement, interaction, and composition |
| `core/client/input` | Context-sensitive action sets and physical-profile bindings | `mclone-input`, XR adapters | Bind devices to neutral actions through an explicit context |
| `core/client/render/decoration` | Active scene decorator, staged effects, hands, HUD/world overlays | `mclone-scene`, `mclone-render` | Update once per frame, render explicitly per view/stage |
| `core/client/network`, `core/server/network` | Capability handshake, pose/state replication, server validation hooks | `mclone-protocol`, `mclone-client`, `mclone-server` | Treat XR pose as a negotiated replication stream, not a local-render detail |

Visor still contains many Minecraft mixins and compatibility packages for Iris,
Sodium, Distant Horizons, ReplayMod, Immersive Portals, and others. That is
reasonable for a mod, but it is not a shape mclone should reproduce. Mclone owns
its renderer and platform adapters, so it can make these seams first-class
instead of patching them after the fact.

## 1. Overlays Are Surfaces, Not A Special-Case Menu

[`VROverlay`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/gui/overlays/VROverlay.java)
combines a stable ID, addon owner, enabled state, priority, pose, visibility,
render target, interaction bounds, and options. It also advertises independent
capabilities:

- depth-tested world layer versus always-on-top HUD layer;
- lit versus unlit;
- no cursor, one cursor, or two cursors;
- draggable and resizable;
- custom cursor bounds within the texture;
- tick-time or render-time visibility updates.

[`VROverlayManagerImpl`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/gui/VROverlayManagerImpl.java)
first renders each visible overlay into its own target, then composites the
surface in a depth or HUD pass. The content and the spatial/composition policy
are therefore separate.

Mclone already has the right seed of this idea:
[`GameUiHost`](../../native/crates/mclone-ui/src/v2.rs) produces a shared draw
list, [`ui_panels.rs`](../../native/crates/mclone-scene/src/ui_panels.rs)
projects it into XR, and
[`WorldGuiRenderer`](../../native/crates/mclone-render/src/gui.rs) renders both
per-eye and multiview. The current implementation is narrower, however:

- `WorldGuiPanel` contains only geometry;
- the renderer composites without a depth attachment, so every panel is
  effectively a Visor HUD-layer surface;
- scene state has one menu-panel pose plus a separate diagnostic-panel path;
- head and left-hand anchors are hard-coded scene choices rather than a
  reusable surface policy.

A useful future mclone vocabulary would be a neutral surface descriptor with:

```text
identity + content revision
placement anchor + local transform + physical size limits
composition: depth-tested | always-on-top
lighting: unlit | world-lit
interaction: none | single-pointer | multi-pointer
interactive bounds + visibility/capability policy
```

This should not become one new god object. Semantic content and options belong
in `mclone-ui`; neutral tracked input belongs in `mclone-input`; placement and
interaction orchestration belong in `mclone-scene`; GPU composition belongs in
`mclone-render`. OpenXR types stay out of all of those contracts.

## 2. Templates And Instances Enable A Real XR Workspace

Visor distinguishes built-in overlays from configurable instances created from
templates. [`VROverlayTemplate`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/gui/overlays/VROverlayTemplate.java)
defines immutable behavior, while each instance owns identity, pose, visibility,
resizing, and other option groups. The registry discovers templates, and
`OverlayConfigsManager` stores built-in and custom instances in separate
per-addon catalogs.

The architectural lesson is definition-versus-instance, not Visor's YAML or
reflection mechanism. Applied selectively, it could let mclone have one
semantic definition for chat, hotbar, diagnostics, keyboard, inventory, or a
status panel while users or profiles own different placements and sizes.

Do not build an addon ABI merely to get this benefit. A small internal
`UiSurfaceDefinition`/`UiSurfaceInstance` split would be enough until mclone has
a real content-extension requirement.

## 3. Pointer Arbitration And Capture Need To Become A Service

[`VRCursorHandlerImpl`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/gui/VRCursorHandlerImpl.java)
maintains state per hand, ray-tests every visible cursor-capable overlay, chooses
the closest hit, reports focus-change events, and keeps a forced-focus surface
during dragging. Two-hand overlays can display and accept both pointers while
still identifying one active pointer.

Mclone's current controller-to-panel math is clean and tested, but
`xr_menu_pointer_hit_from_controllers` chooses the right controller first and
then the left, and `McloneSceneHost` owns one `menu_pointer_down` bit. That is
adequate for one panel and one active pointer, but it will not compose with a
keyboard, wrist UI, movable diagnostics, or overlapping surfaces.

Before adding a second interactive XR surface, extract a neutral resolver that:

1. assigns stable pointer IDs, normally one per tracked hand;
2. ray-tests all eligible surfaces and chooses the nearest hit;
3. tracks hover and press capture separately per pointer;
4. retains capture until release, cancellation, surface removal, or tracking
   loss;
5. delivers the same move/down/up intents already consumed by `GameUiHost`;
6. emits optional haptic effects through the shared client-experience effect
   boundary.

This preserves one UI behavior surface across mouse, touch, synthetic XR, and
real XR. It also gives the planned automated XR click smoke a reusable harness
instead of another menu-specific path.

## 4. The Virtual Keyboard Is The Most Direct Product Lesson

Visor's
[`VROverlayKeyboard`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/gui/overlays/builtin/keyboard/VROverlayKeyboard.java)
is an ordinary high-priority overlay that can attach to a screen, appear when
text entry needs it, accept two cursors, and be dragged or resized. Its layout
model includes shift layers and multiple language layouts.

This maps directly to mclone's current connect/world-name text-entry gap. The
right shared-first implementation is:

- a focused text-editing model in `mclone-ui`;
- neutral text intents such as insert text, backspace, delete, move selection,
  accept, and cancel;
- platform adapters for desktop/web/Android keyboard, IME, and clipboard;
- an XR virtual-keyboard surface that emits the same text intents;
- attachment to a focused field/screen, with explicit show/hide and focus-loss
  policy.

Do not copy Visor's GLFW-key-code emulation. Mclone needs Unicode text and IME
semantics, not a virtual physical keyboard pretending to be GLFW. The virtual
keyboard should be one input projection over the same text model used by flat
clients.

## 5. Settings Metadata And Presets Are Useful, Reflection Is Not

Visor annotates static settings fields with category, key, and widget metadata,
then reflects over them to create a settings catalog. It also has registered
built-in and custom presets, including overlay-layout presets. This makes
settings discoverable and lets a preset represent a ready-to-play experience
rather than a bag of unrelated toggles.

Mclone's typed `GameUiRenderState` and `GameUiAction` model is safer than mutable
static fields. Preserve it. The transferable idea is a typed setting descriptor
containing:

- stable ID and category;
- label/help/localization keys;
- value kind, bounds, and step;
- profile/capability availability;
- apply timing: immediate, staged, or restart/rebuild required;
- serialization version and default;
- optional preset participation.

This could reduce the hand-written repetition in the options layouts while
keeping all mutation routed through shared actions and effect/completion policy.
Presets become valuable after the shared preferences store exists: comfort,
seated/standing, child-friendly, performance, and XR HUD placement are more
useful product concepts than exposing every low-level toggle.

## 6. Input Actions Are Contextual And Device Profiles Are Separate

Visor's
[`VRActionSet`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/input/action/VRActionSet.java)
groups logical actions, selects one active set by enabled/can-activate priority,
and stores bindings per OpenXR interaction profile. Left-handedness is applied
while reading the physical profile. UI mouse-like actions, gameplay actions,
and key actions therefore do not have to share one unconditional binding map.

Mclone already has neutral `InputBindingAction` values and XR controller
snapshots, but several XR button choices remain constants and direct scene
logic. A future binding slice should preserve this routing:

```text
platform/OpenXR component state
  -> physical interaction profile + handedness
  -> active input context (gameplay, UI, text entry, debug)
  -> neutral mclone-input intent
  -> shared client-experience/scene policy
```

A composable context stack may fit mclone better than Visor's exactly-one-set
rule, but the separation of context, logical action, and physical binding is
worth adopting. Haptics should return as an effect; neither `mclone-ui` nor the
client-experience core should call an OpenXR haptic API.

## 7. Render Work Must Say "Once Per Frame" Or "Per View"

Visor explicitly separates a decorator's once-per-game-frame
`updateRenderState()` from its per-render-pass methods. It also names stages
after solid geometry, after translucency, and after the world, and classifies
depth overlays separately from HUD overlays. That vocabulary makes hand,
cursor, world UI, and screen-effect ordering reviewable.

This is especially relevant to mclone's XR render-path guardrail:

- compute UI models, visibility, content revisions, and input once per frame;
- compute view-dependent matrices for each eye/layer;
- render every XR-visible surface in both the per-eye and multiview paths;
- make depth-tested versus always-on-top composition explicit;
- classify effects by render stage instead of relying on call order in one
  large frame function.

Do **not** copy Visor's `VRRenderPass.worldUpdater() == EYE_LEFT` convention.
That is a sequential stereo compatibility shortcut. Mclone's shared frame state
must not depend on which eye happened to render first, and the full-frame
multiview path must remain first-class.

## 8. Lifecycle State Is A Capability Fact, Not An App Boolean

Visor exposes `OFF -> INITIALIZED -> ACTIVE -> FOCUSED` separately from a play
policy (`DISABLED`, `ENABLED`, `WORLD_ONLY`, `ALWAYS_ACTIVE`). Cursor processing
requires focus, activation/deactivation has explicit transitions, and failures
tear down the session and fall back to non-VR behavior.

Mclone should retain exclusive low-level OpenXR sequencing in
`mclone-xr-host::OpenXrFrameDriver`. The transferable lesson is to project
neutral lifecycle facts into the shared experience:

- runtime/session available;
- session active or visible;
- input focused;
- tracking quality/capabilities;
- recoverable versus terminal error.

Scene/UI policy can then pause interaction, clear pointer capture, present a
status panel, or fall back safely without acquiring or ending OpenXR frames
itself.

## 9. Addon Registries Suggest Internal Extension Points

Visor components share stable IDs, owners, enabled state, and sometimes
priority. Registries cover tasks, input action sets, decorators, body types,
game and hand effects, item poses, overlays, templates, and settings presets.
This is cleaner than having every feature inject more conditionals into one
orchestrator.

For mclone, the near-term use is internal registration/composition, not a public
binary plugin system. Good candidates are typed registries for transient visual
effects, UI surface definitions, player presentation/body variants, and input
contexts. Prefer enums or stable string/newtype IDs, deterministic ordering,
explicit conflicts, and capability filtering.

Avoid Visor's less portable details:

- global `ClientContext` and `VisorAPI.client()` service access;
- constructors that perform config-file I/O;
- runtime reflection and annotation scanning;
- `instanceof` chains that choose option screens;
- API interfaces containing Minecraft `Screen`/`RenderTarget` types;
- silent component replacement as the default conflict policy.

Those shapes conflict with mclone's sans-I/O client-experience core and its
host-neutral shared-crate boundaries.

## 10. Remote XR Pose Is Independent Of Local XR Rendering

Visor negotiates a versioned channel, sends an initial remote-player snapshot,
then replicates pose continuously and sends relatively static state only when
it changes: body type, handedness, rotation, world scale, calibrated height,
gun angle, and whether an overlay is focused. The server relays pose only to
connections tracking that player. A non-VR local client may still render remote
VR players.

This is a useful future shape for mclone multiplayer:

- capability/version negotiation separate from transport connection;
- a compact, sequenced XR pose stream separate from durable player appearance;
- server interest filtering;
- client interpolation and tracking-loss policy;
- flat-client rendering of remote XR avatars;
- server validation for XR-originated gameplay actions rather than trusting
  controller poses as authority.

Do not port Visor's packets directly. Mclone's protocol must define timestamp,
sequence, quantization, loss, interpolation, and anti-cheat/validation semantics
for its own authoritative model.

## Recommended Mclone Follow-Ups

| Priority | Slice | Why Visor changes the recommendation |
|---|---|---|
| P0 | Shared text editing plus XR virtual keyboard | Visor demonstrates that the keyboard can be a normal attachable surface; this closes a current connect/world UI blocker |
| P1 | Extract pointer identity, focus, nearest-surface arbitration, and capture | Required before keyboard plus menu or wrist UI can coexist safely |
| P1 | Add an explicit world-UI composition policy | Current `WorldGuiPanel` is always on top; Visor shows the practical need for depth and HUD classes |
| P1 | Generalize menu/diagnostic geometry into surface definitions and instances | Avoid another scene-local special case when chat, keyboard, inventory, or status panels arrive |
| P2 | Move XR physical bindings behind shared input contexts | Replaces hard-coded scene button choices without leaking OpenXR into shared policy |
| P2 | Add typed settings descriptors and transactional presets | Reduces manual UI repetition while preserving `GameUiAction` and sans-I/O rules |
| P3 | Add negotiated remote-XR pose replication | Valuable after the shared remote-player model and protocol cadence are ready |

The P0/P1 work should be one continuing XR UI concern, not an invitation to
build a general addon platform first.

## Validation Scenarios To Preserve Or Add

Visor's lack of source tests is a reason to make mclone's adoption stronger:

- synthetic stereo: two overlapping panels choose the nearest hit;
- pointer capture survives leaving bounds and ends on release/tracking loss;
- right and left pointers keep independent hover/capture state;
- analog trigger hysteresis does not double-click;
- Unicode virtual-keyboard input reaches the same text model as native input;
- a depth panel is occluded while a HUD panel remains visible, in both per-eye
  and full-frame multiview captures;
- UI content updates once per frame while per-view matrices differ correctly;
- focus/session loss clears interaction and shows a recoverable state;
- flat clients can decode and interpolate a remote XR pose without owning an
  OpenXR session.

The existing headset-free XR-emulation lane is the natural first gate for most
of these. Real desktop/Quest headset validation remains necessary for comfort,
tracking loss, haptics, and text-entry ergonomics.

## Suggested Reading Order

1. [`README.md`](../../reference/Visor/README.md) and
   [`settings.gradle`](../../reference/Visor/settings.gradle) for project
   posture and module boundaries.
2. [`ComponentRegistries.java`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/common/addon/ComponentRegistries.java)
   and [`ComponentRegistry.java`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/common/addon/component/ComponentRegistry.java).
3. [`VROverlay.java`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/gui/overlays/VROverlay.java),
   [`VROverlayTemplate.java`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/gui/overlays/VROverlayTemplate.java),
   and [`VROverlayPose.java`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/gui/overlays/VROverlayPose.java).
4. [`VRCursorHandlerImpl.java`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/gui/VRCursorHandlerImpl.java)
   and [`VROverlayManagerImpl.java`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/gui/VROverlayManagerImpl.java).
5. [`VROverlayScreen.java`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/gui/overlays/framework/VROverlayScreen.java)
   for the screen-to-spatial-surface adapter, drag, resize, and cursor behavior.
6. [`VROverlayKeyboard.java`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/gui/overlays/builtin/keyboard/VROverlayKeyboard.java)
   and [`KeyboardLayout.java`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/gui/overlays/builtin/keyboard/KeyboardLayout.java).
7. [`VRActionSet.java`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/input/action/VRActionSet.java)
   and [`VRInputManagerImpl.java`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/input/VRInputManagerImpl.java).
8. [`RenderPipelineStage.java`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/render/RenderPipelineStage.java),
   [`VRDecorator.java`](../../reference/Visor/visor-api/src/main/java/org/vmstudio/visor/api/client/render/decoration/VRDecorator.java),
   and [`DecorationRendererImpl.java`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/render/decoration/DecorationRendererImpl.java).
9. [`VisorState.java`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/VisorState.java)
   for lifecycle and error fallback.
10. [`ClientNetworking.java`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/client/network/ClientNetworking.java)
    and [`ServerNetworking.java`](../../reference/Visor/visor-core/src/main/java/org/vmstudio/visor/core/server/network/ServerNetworking.java)
    for remote pose/state replication.

For the corresponding current mclone seams, start with
[`client-experience-architecture.md`](../client-experience-architecture.md),
[`mclone-ui/src/v2.rs`](../../native/crates/mclone-ui/src/v2.rs),
[`mclone-scene/src/ui_panels.rs`](../../native/crates/mclone-scene/src/ui_panels.rs),
[`mclone-render/src/gui.rs`](../../native/crates/mclone-render/src/gui.rs), and
[`mclone-input/src/lib.rs`](../../native/crates/mclone-input/src/lib.rs).

## Open Questions

- How comfortable are Visor's default panel distance, scale, hand attachment,
  and two-hand keyboard on actual headsets compared with mclone's current menu?
- How stable is the addon API across Visor's pre-1.0 releases?
- What are Visor's CPU/GPU costs for multiple independently rendered overlay
  targets, especially with shaders?
- Which features work on OpenXR runtimes other than SteamVR in practice?
- Does Visor have unpublished/manual test coverage beyond its release build?
- Which ideas remain after comparing the same flows with current Vivecraft on a
  headset?

These require runtime/headset testing or maintainer evidence. They should not be
answered by source inspection alone.
