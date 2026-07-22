# Local Couch Multiplayer And Multi-Presentation

Topic: `local-couch-multiplayer`

Status: accepted product and architecture direction; preliminary foundation
completed on 2026-07-21 through
[`Tactical 215`](../tactical/215-preliminary-couch-readiness.md). Mclone should
retain a credible path from one local participant to 1-4 local participants,
including console-style split screen, useful single-player auxiliary panes,
helper/builder participation, and a mixed XR-plus-flat session.
[`Tactical 216`](../tactical/216-controller-input-foundation.md) subsequently
completed shared semantic controller sessions, controller-complete UI,
desktop/browser/Android collectors, XR convergence, and automated platform
gates; real-device acceptance remains open. Active
[`Tactical 217`](../tactical/217-local-participant-foundation.md) owns the
judgment-free participant/input/client/layout foundation while stopping before
product couch enablement and unresolved profile/cache policy.

This topic owns local participant cardinality, participant-to-input assignment,
participant-versus-view separation, shared split-screen presentation policy,
and the research/validation path toward couch multiplayer. It does not replace
[`controller-input.md`](controller-input.md), which owns physical controllers
and semantic actions; [`multiplayer-networking.md`](multiplayer-networking.md),
which owns ordinary client streams and remote transports; or
[`realm-dimension-runtime.md`](realm-dimension-runtime.md), which owns
authoritative players, observers, dimensions, and interest aggregation.

## Product Decision

Local multiplayer is a product capability worth preserving before a console
port exists. A docked Steam Deck, living-room SteamOS PC, future Steam-focused
box, and a possible console port all benefit from controller-first couch play.
The internal cardinality is **1-4 local participants**, even if the first
shippable slice supports only two.

Local participation and presentation are separate axes:

- one participant may own one ordinary flat view;
- one participant may own multiple views, such as a main view plus a minimap,
  debug camera, or building-plan view;
- several participants may share one flat surface through viewport rectangles;
- one participant may consume an XR stereo/multiview surface while another
  consumes a flat window or television view; and
- a helper participant may use a pointer, plan view, or optional focus pane
  without becoming a conventional first-person player.

A presentation view never manufactures player authority. An auxiliary camera
can observe an already resident area without adding another player. If it needs
distant world facts, it must request explicit observer interest or use an
ordinary participant stream rather than causing a renderer to load or simulate
world state implicitly.

## Accepted Concept Model

Exact Rust names can adjust during implementation, but these identities and
ownership boundaries are required:

```text
physical input sources
  InputSourceId -> optional LocalParticipantId assignment
                              |
                              v
local participant group (1-4 humans/roles on this device)
  participant A -> identity + logical client endpoint + private owner state
  participant B -> identity + logical client endpoint + private owner state
                              |
                              v
one authoritative RealmServer (integrated or remote dedicated)
  ordinary players + observers + unioned authoritative interest

shared client presentation resources for one realm/dimension/resource epoch
  terrain/assets/actors/compile/upload residency
                              |
                              v
presentation views
  participant camera -> flat viewport or XR stereo surface
  auxiliary camera   -> flat viewport/PIP/plan/minimap
```

The durable distinctions are:

- **Local participant**: one human/device-side role with assigned input,
  profile or guest identity, UI context, and optional authoritative avatar.
- **Logical client endpoint**: the ordered owner-specific command/update
  stream for an ordinary player or observer. Full local players remain
  ordinary realm players rather than privileged integrated-server actors.
- **Presentation view**: camera, projection, viewport, per-view effects, and
  HUD policy. A view may belong to a participant or be auxiliary.
- **Presentation surface**: a flat window/canvas/swapchain image or an OpenXR
  compositor surface. A flat surface may contain multiple viewport rectangles;
  an XR surface contains the per-eye/multiview topology for one tracked
  participant.
- **Interest source**: a server player or explicit observer that contributes
  residency and, where authorized, simulation interest. A camera is not
  automatically an interest source.

Do not encode local participant count as a fixed pair. Bounded collections may
cap at four, while two-player layouts and validation remain the first product
target.

## Hard Architecture Invariants

1. Starting couch play does not create a second world or second integrated
   server. All local participants join one ordinary realm authority. The same
   participant group may later connect to a remote dedicated realm using the
   ordinary remote session lifecycle.
2. Full local players are ordinary realm players with distinct identities,
   owner-only state, commands, persistence, disconnect, and resume behavior.
   Local transport may avoid byte encoding but may not bypass logical join and
   publication semantics.
3. Do not clone `McloneSceneHost` once per viewport. World/asset/GPU resources
   that are semantically identical must have a sharing and deduplication path.
4. Do not reinterpret XR `Stereo([view; 2])` as two local players. XR eyes
   share one participant pose and compositor contract; split-screen cameras
   belong to independent participants or auxiliary views.
5. Runtime/update/compile/upload preparation occurs once for a shared frame
   plan, then each admitted view performs its own culling and rendering into
   its target rectangle or XR layer.
6. A unioned resident set is not submitted wholesale in every pane. Frustum,
   occlusion, distance, per-view effects, and HUD assembly remain per view.
7. Real couch players are not required to remain tethered. A helper role may
   deliberately use a leash, recall, or pointer model, but that must not become
   an authority restriction on full local players.
8. Keyboard/mouse is not structurally player one. A controller-only title,
   join, world selection, play, pause, and recovery path is required for
   living-room and console use.
9. Product behavior stays shared. Desktop, web, Android, XR, Steam Input, and
   a future console adapter collect devices and own surfaces; they do not own
   separate participant, split-layout, streaming, or helper gameplay policy.
10. Every enabled world-space feature remains correct in mono, each flat
    viewport, XR per-eye, and full-frame multiview paths where applicable.

## Current Mclone State

The preliminary preservation foundation now spans authority, input, and
presentation:

- `RealmServer` starts without an implicit player and already supports
  ordinary concurrent players, per-player publication queues, source-owned
  player/observer views, and unioned residency/simulation ticketing.
- local integrated play joins one ordinary player through `LocalRealmSession`;
  TCP and WebSocket hosts use the same player authority.
- a focused realm smoke proves one, two, and four co-located/separated
  player/observer interest sources union and remove without stealing another
  source's tickets;
- `mclone-input` owns opaque session-local source IDs, neutral descriptors, a
  normalized standard-gamepad snapshot, and deterministic four-seat scripted
  assignment with disconnect clearing and bounded reconnect reservation;
- the shared controller pipeline now also owns semantic per-source actions,
  controller-complete menu navigation, preferences, and physical collectors
  for desktop, browser, Android, and both XR hosts, with hardware acceptance
  still outstanding;
- renderer uniform identity admits four neutral presentation views while typed
  stereo-eye identity remains XR-only; and
- the scene admits one through four independent flat targets with one shared
  preparation, and the inspected offscreen proof renders first-person plus
  detached plan cameras in horizontal and vertical compositions.

The client/presentation side remains singleton in the places couch play must
change:

- one `DrawableWorldSlot` retains one camera, interaction controller, and
  player model;
- the preliminary multi-flat entry still targets complete textures rather than
  viewport rectangles on one presentation surface, and rejects simultaneous
  retained embedded-world preview composition;
- `LocalIntegratedSceneRuntime` owns one `SingleViewRuntime` and one
  `IntegratedRunnerConnection`;
- the local profile layer exposes one installation/browser-origin profile,
  not a local participant/profile group;
- flat screen-space HUD/menu state is assembled for one mono view; and
- product hosts still arbitrate multiple ordinary controllers into one local
  player's semantic input session rather than assigning them to participants.

These are explicit refactor points, not permission to stage the feature in
`mclone-native-client`.

## Player Streams And Shared Client Resources

The correctness baseline is one ordinary logical client endpoint per full
participant. Owner-only health, inventory, statistics, life state, camera
corrections, capability negotiation, and disconnect reasons cannot be merged
into an anonymous device-global stream.

That baseline does not require permanently duplicating all client resources.
The intended presentation target is:

```text
participant-private                 shared when coherent
-------------------                 --------------------
identity and owner state            block/chunk snapshots
command sequencing                  terrain mesh resources
camera and interaction              asset catalog and pipelines
inventory/HUD/menu focus            actor presentation resources
life/death/respawn UI               compile/upload coordination
input context and prompts           world/environment presentation facts
```

An early proof may retain separate CPU replicas for correctness. The follow-up
must measure overlapping duplicate snapshots, meshes, and GPU allocations and
introduce a device-local participant-session aggregator or ref-counted shared
cache where useful. Such optimization stays behind ordinary per-player stream
semantics; it must not invent a couch-only authoritative server.

For one dimension, let each admitted view or client interest source request a
resident region `R_i`. The target drawable residency is conceptually:

```text
U = union(R_0, R_1, ... R_n)
```

Each view then derives its own visible set from `U`. Overlapping views reuse
one compatible mesh/resource; separated players enlarge `U` and therefore
increase generation, publication, compile, upload, traversal, actor, and GPU
residency cost. Per-participant render distance, Far LOD policy, and adaptive
render scale may bound cost, but must be explicit and observable.

Preparation budgets require fairness. Participant zero cannot permanently
consume every compile/upload grant while another pane remains empty. Prefer a
shared priority calculation with per-interest-source aging and overlap reuse,
then record per-participant readiness and cost in diagnostics.

## Bedrock Split-Screen Evidence And Research Gap

Bedrock is useful product evidence but not a public implementation oracle.
RenderDragon and Bedrock's local-client internals are closed.

Primary public facts as of 2026-07-21:

- [Minecraft Help documents console split screen](https://help.minecraft.net/hc/en-us/articles/37122759127821-Play-Minecraft-Bedrock-Edition-Split-Screen)
  for up to four players on Xbox, PlayStation, and Switch, with a controller
  and console user selection for each added player. Switch supports four while
  recommending two.
- [Bedrock's camera documentation](https://learn.microsoft.com/en-us/minecraft/creator/documents/camerasystem/cameracommandintroduction?view=minecraft-bedrock-stable#the-camera-command-and-split-screen-gameplay)
  says split-screen participants retain separate cameras and separate
  targetable player entities, and a camera command may affect them
  independently. It also says Bedrock has no plan to combine those views or
  expose multiple split-screen-like views for one player, so mclone's
  auxiliary-pane direction is a deliberate product extension rather than a
  Bedrock parity claim.
- The same camera documentation warns that a free camera far from its player
  cannot independently make distant chunks load; renderable chunks still
  depend on player proximity and configured render distance.
- [Microsoft's render/simulation-distance guide](https://learn.microsoft.com/en-us/minecraft/creator/documents/simulationrenderdistanceguide?view=minecraft-bedrock-stable)
  distinguishes client render distance from costlier authoritative simulation
  distance and describes distance around a player or camera, but does not
  describe split-screen cache ownership.
- Mojang's [GDC 2026 RenderDragon presentation](https://media.gdcvault.com/gdc2026/Slides/Fairfield_AJ_ModernizingTheRenderingOfMinecraft.pdf)
  says terrain chunks are queued for assembly as the player moves, use pooled
  vertex/index ranges, and rely on render distance and scalable quality across
  hardware. It does not disclose split-screen residency, mesh reuse, or cull
  submission structure.

No primary source found in this investigation answers whether Bedrock:

- maintains one unioned client chunk/mesh cache for all local players;
- retains separate client replicas and duplicate overlapping render caches;
- shares GPU terrain while retaining participant-private CPU replicas; or
- applies additional split-screen-specific distance, quality, or scheduling
  policy beyond ordinary per-view culling.

Do not convert the likely need for a union of resident facts into a claim
about Bedrock internals. Use a controlled Bedrock study to collect observable
behavior:

| Players | Positions | Camera directions | Primary cost isolated |
| --- | --- | --- | --- |
| 1 | baseline | fixed | one-view residency and draw |
| 2/4 | co-located | aligned | additional viewport/HUD submission |
| 2/4 | co-located | opposing | per-view culling and visible-union draw |
| 2/4 | separated beyond overlap | fixed | additional residency, assembly, and simulation interest |

The study should use one pinned Bedrock version/platform, fixed seed or flat
world, explicit low and high render/simulation distances, chunk-border
landmarks, and reproducible player poses. Record:

- visible radius and fog/pop behavior in every pane;
- frame pacing or platform performance telemetry where available;
- time for each pane to become drawable after join, teleport, and separation;
- whether deterministic simulation canaries continue around every separated
  player;
- behavior when players reconverge, including whether terrain appears already
  resident; and
- any automatic resolution, graphics-feature, or distance changes as players
  join and leave.

Comparing co-located aligned, co-located opposing, and far-separated cases can
separate some draw/culling cost from added resident-world cost. It still cannot
prove private cache ownership without memory/GPU instrumentation, so label any
conclusion from pixels and frame timing as inference.

## Single-Player Auxiliary Split Mode

The first rendered proof should not wait for a second authoritative player.
One participant can deliberately request two flat presentation views on one
surface in vertical or horizontal layout. Useful panes include:

- an overhead or angled minimap/world camera;
- a renderer/traversal/streaming debug camera or detailed diagnostic pane;
- a building elevation, layer, or plan view;
- a ghost-plan progress view showing missing, correct, and conflicting blocks;
- a fixed security/review camera near the active player; or
- a low-resolution focus/PIP view used by a helper.

This is both a product feature and an architecture test. It exercises viewport
and scissor rectangles, per-view aspect/projection, separate depth, view-local
effects, per-pane UI policy, shared frame preparation, and per-view culling
without conflating a view with a player connection.

A purely textual debug pane is useful but does not prove the second world-view
path. The acceptance lane should include at least one independently posed world
camera. Keep the ordinary one-view path direct: no auxiliary view means no
second depth target, second cull, second render pass, or auxiliary interest.

An auxiliary camera near the player may consume already resident facts. A
distant plan/security view needs an explicit bounded observer source and is
subject to server capability and budget policy, especially on a remote realm.

## Helper And Builder Participation

A helper is an explicit local participant role, not a hidden second controller
feeding player one's action frame. Candidate interaction models include a
screen-space pointer, a small world-space helper entity, direct control from a
stable third-person/top-down focus view, or switching between those modes.

A builder helper can contribute meaningfully without conventional first-person
navigation:

- select or align a building plan;
- inspect layers/elevations and mark the next required block;
- render missing/correct/conflicting-block overlays;
- fetch or carry a bounded material selection;
- place or remove explicitly permitted plan blocks;
- ping blocks, hazards, entities, and points of interest; and
- switch between player, helper, overhead, and plan perspectives.

Edge arrows, occlusion outlines, a bounded leash, and recall are appropriate
for this helper role. They are not substitutes for a view when the helper is
expected to navigate independently off screen.

Any helper action that mutates blocks, inventories, entities, statistics, or
progress is an authoritative server command with an explicit actor, material
source, permission, range, rate, and failure result. Plan overlays and pings
may be presentation-only. The exact authority representation—ordinary player
avatar, dedicated helper entity, or role-scoped command capability—remains an
open product/gameplay decision and must be settled before mutation lands.

Building-plan world visuals must support every enabled mono/per-eye/multiview
path. A helper feature cannot introduce a flat-only overlay that disappears in
XR or a per-eye implementation that disappears in XR multiview.

## Mixed XR And Flat Couch Play

Participant identity must not imply presentation topology. A valuable mixed
configuration is:

```text
participant A: tracked OpenXR input -> stereo/multiview headset surface
participant B: ordinary gamepad     -> mono flat window/TV surface
shared realm: two ordinary players and unioned authoritative interest
```

Related configurations include an XR player plus a flat helper/builder, a
flat player using the window while the headset receives a spectator view, or
an XR player whose desktop surface shows an auxiliary plan/debug view instead
of a simple eye mirror.

OpenXR sequencing remains exclusively owned by
`mclone-xr-host::OpenXrFrameDriver`. A mixed host may coordinate an additional
flat target and event pump, but it must not move wait/begin/end/swapchain
ownership into `mclone-scene`. Shared scene preparation supplies participant
and auxiliary views to the appropriate surfaces; each surface retains its own
acquire/present mechanics and may have a different cadence.

The initial mixed target is one headset participant plus one flat participant
or auxiliary view. Multiple locally attached headsets are outside the current
scope. Spatial audio listener/mix policy for simultaneous XR and flat players
is unresolved and must be explicit rather than silently following the last
camera rendered.

Avoid naming this product role merely `Companion`: desktop XR already uses
that word for its non-gameplay companion window. Prefer helper, sidekick,
builder, auxiliary view, or a specific in-world role name.

## Input Assignment And Join Policy

The device contract remains owned by
[`controller-input.md`](controller-input.md). Couch play consumes its
session-local `InputSourceId`, descriptors, semantic action frames, hotplug
lifecycle, and controller-family prompt facts.

Accepted assignment policy:

- one physical input source is assigned to at most one local participant;
- keyboard and mouse normally form one composite source assignment;
- an unassigned gamepad's meaningful confirm/join edge may request the next
  available participant slot;
- browser/controller indices, GilRs IDs, Android device IDs, OpenXR paths, and
  Steam handles are never persisted as participant identity;
- disconnect clears held state and starts a bounded reconnect grace period;
- reconnect does not silently steal another participant's source;
- each participant owns independent input context, active prompt origin, menu
  focus, and rebinding projection; and
- tracked XR sources and an ordinary gamepad can be assigned to different
  participants in a mixed host.

The first player may join with a controller and complete the entire flow
without keyboard/mouse. Platform account selection and console user handles
remain adapter facts that resolve to shared local profile/session intents.

## Profiles, Persistence, UI, And Lifecycle

One installation profile is insufficient for durable couch play. The future
local profile owner must support a bounded participant/profile selection:

- durable local profiles with distinct stable UUIDs;
- session guests with collision-free identities and an explicit persistence
  policy;
- later Steam/console-user association supplied by the platform adapter; and
- duplicate-live-identity rejection before joining the realm.

Each ordinary participant's realm record remains keyed by that profile UUID.
Player two is not saved under player one's identity, and leaving one
participant must not tear down the other participants or the integrated realm.

UI must consume a viewport rectangle and safe area, not assume full-surface
dimensions. Participant-scoped state includes HUD, hotbar/inventory, prompts,
death/respawn, selected camera, and focus. Device-global overlays include
join/leave, controller loss, account/profile selection, and any menu that
deliberately pauses or covers the whole presentation.

Pause policy depends on host mode. A purely local integrated session may pause
the realm only through an explicit group/global action; a remote realm and any
session with non-local peers continue ticking. Inventory or a participant menu
must not accidentally freeze every player because one pane opened UI.

## Presentation And Performance Policy

Flat layout is data. Support horizontal and vertical two-pane arrangements
before choosing a default; retain arbitrary rectangles for 3/4-player grids,
PIP, auxiliary plans, and platform safe areas. Every pane receives its own
aspect-correct projection and depth target. HUD scale and content may simplify
per pane, but gameplay facts cannot disappear silently.

Performance controls may include:

- per-view render scale and upscaling;
- per-participant near render distance and Far LOD detail;
- shared overlap-aware compile/upload admission;
- per-view actor, particle, shadow, and post-effect quality tiers;
- bounded auxiliary-view refresh rates when their product contract permits;
  and
- automatic quality selection based on participant count, with visible and
  inspectable effective settings.

Do not use culling as a synonym for residency. Per-view culling controls what
is submitted from resident data. Separated players can still grow authoritative
interest, client snapshots, mesh assembly, and GPU memory even if every pane
culls perfectly.

## Implementation Direction

The sequence is intentionally staged so each milestone is useful on its own:

1. **Complete the controller foundation.** Completed by Tactical 216 through
   semantic action state, look-rate semantics, UI navigation, lifecycle
   clearing, physical collectors, XR convergence, preferences, and automated
   validation. Real-device acceptance remains an explicit hardware gate.
2. **Promote the flat-view proof to surface layout.** View and participant are
   already separate, and the horizontal/vertical auxiliary proof prepares once
   and renders twice without changing mono. Add viewport/scissor rectangles,
   safe areas, per-pane HUD policy, and retained-preview composition only when
   a product or diagnostic consumer needs them.
3. **Participantize singleton client-experience state.** Active in Tactical
   217: introduce a bounded local participant group, first migrate the current
   camera/interaction/player-model envelope at cardinality one, and integrate
   scripted source assignment with participant-local semantic input. Durable
   profile and unresolved global UI policy remain outside that tactical.
4. **Add a local client group.** Join two distinct ordinary identities to one
   integrated realm through independent logical endpoints; prove owner-only
   updates, overlapping and separated interest, join/leave, save/reopen, death,
   and respawn headlessly before rendering them as a product.
5. **Ship two-player flat split screen.** Add device assignment, layout,
   per-pane HUD/inventory, controller-loss UI, shared render resources, fair
   preparation, and colocated/separated performance evidence on desktop
   SteamOS-shaped hardware.
6. **Exercise all targets and cardinality four.** Adopt browser and Android
   collectors/surfaces, prove 1-4 participant collections and layouts, and
   validate real-device budgets. Console-specific account/suspend adapters
   remain future platform work over the same contracts.
7. **Add role and mixed-presentation products.** Build helper/builder authority
   only after its gameplay contract is selected; prove one XR participant plus
   one flat participant/auxiliary view without forking server or scene policy.

Open a bounded tactical for the first active slice rather than attempting this
entire sequence in one campaign.

## Validation Contract

Shared contract tests must cover:

- participant collections of sizes 1-4 without fixed-pair assumptions;
- source assignment, join edges, disconnect/reconnect, and no cross-player
  held-state leakage;
- distinct local identities and owner-only update routing;
- ordinary integrated and remote join/leave semantics for each participant;
- player/observer interest union and removal without stealing another source's
  tickets;
- fair preparation when participant regions overlap and when they are far
  apart; and
- participant-independent presentation views, including one participant with
  two views.

Rendered acceptance must begin with the single-player auxiliary proof and
inspect screenshots at the first drawable milestone:

- horizontal and vertical layouts with aspect-correct independent cameras;
- separate depth/effects and correct viewport/scissor containment;
- per-pane HUD policy without cross-pane clipping;
- one-view pixels and resource/cost behavior unchanged when auxiliary views
  are absent;
- two-player colocated, opposing-view, and separated captures;
- 3/4-player layout and reduced-pane UI; and
- flat plus synthetic/real XR, including full-frame multiview for enabled
  world-space helper/plan visuals.

Performance evidence must report participant count, view count, overlap,
resident/visible section counts, compile/upload work, GPU memory where
available, CPU/GPU frame splits, per-view render scale/distance, and readiness
fairness. Compare at least one view, two co-located aligned views, two
co-located opposing views, and two separated views.

Platform evidence eventually includes desktop/Linux controller-first couch
play, browser multi-gamepad join and visibility clearing, Android controller
and lifecycle behavior, Steam Deck docked/suspend behavior, and mixed desktop
OpenXR plus flat input/presentation. A future console port adds platform-user,
safe-area, suspend/resume, entitlement, and certification evidence without
changing shared participant semantics.

## Open Decisions

- Exact shared-client resource ownership while preserving participant-private
  ordered streams and owner state.
- Default two-player orientation and whether layout can change live.
- Per-participant versus group/global inventory and pause presentation.
- Guest profile durability and later platform-account association.
- Audio listener and mix policy for flat split screen and mixed XR+flat.
- Helper authority representation and material/progress ownership.
- Whether an auxiliary plan/minimap can request a remote observer capability,
  and the server admission/budget contract if it can.
- Measured Bedrock split-screen residency/cache behavior beyond the public
  evidence recorded above.

## Code And Documentation Map

- [`../../native/crates/mclone-scene/src/lib.rs`](../../native/crates/mclone-scene/src/lib.rs)
  and [`../../native/crates/mclone-scene/src/mono.rs`](../../native/crates/mclone-scene/src/mono.rs)
  — current one-slot player/camera/UI ownership and mono frame path.
- [`../../native/crates/mclone-app-runtime/src/native_service_assembly.rs`](../../native/crates/mclone-app-runtime/src/native_service_assembly.rs)
  and [`../../native/crates/mclone-app-runtime/src/client_connection.rs`](../../native/crates/mclone-app-runtime/src/client_connection.rs)
  — current single local client runtime/connection and ordinary connection
  contract.
- [`../../native/crates/mclone-server/src/integrated.rs`](../../native/crates/mclone-server/src/integrated.rs)
  and [`../../native/crates/mclone-server/src/player_chunk_tracking.rs`](../../native/crates/mclone-server/src/player_chunk_tracking.rs)
  — ordinary realm players/observers and authoritative interest aggregation.
- [`../../native/crates/mclone-input/src/lib.rs`](../../native/crates/mclone-input/src/lib.rs)
  — current gamepad/input contracts; target design lives in
  [`controller-input.md`](controller-input.md).
- [`../../native/crates/mclone-app-runtime/src/local_profile.rs`](../../native/crates/mclone-app-runtime/src/local_profile.rs)
  — current single installation/browser-origin profile owner.
- [`../native-engine-architecture.md`](../native-engine-architecture.md) and
  [`../client-experience-architecture.md`](../client-experience-architecture.md)
  — controlling shared scene/app/platform ownership.
- [`multiplayer-networking.md`](multiplayer-networking.md) and
  [`realm-dimension-runtime.md`](realm-dimension-runtime.md) — ordinary client
  streams, realm players, observers, persistence, and interest.
- [`embedded-worlds.md`](embedded-worlds.md) — related but distinct retained
  world composition, observer, and multi-view resource evidence.
