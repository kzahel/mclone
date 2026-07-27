# Universe Product Shell

Topic: `universe-product-shell`

Status: **concept and architecture direction recorded 2026-07-26; shared
terrain representation direction clarified 2026-07-27; implementation has not
started.** The current shipped behavior remains the session-free title shell
from
[`client-entry-lifecycle.md`](client-entry-lifecycle.md), the bounded
active-plus-optional-standby scene from
[`embedded-worlds.md`](embedded-worlds.md), and the separate Terrain Lab and
World Explorer products. This topic gives those existing flows a prospective
first-class product model without selecting a new crate or replacing their
current owners.

## Scope

This topic owns the product-level idea of **Universe**: the context around
realms, dimensions, previews, and live play. Universe can present the main
menu, recent destinations, generation and inspection surfaces, detached
terrain views, and transitions into or out of an authoritative scene.

It records:

- how existing menu, catalog, Explorer, overview, preview, warm-world, and
  session flows can become one navigation model;
- the distinction between a realm, dimension, preview representation, and
  authority role;
- the intended continuum from catalog-scale inspection to embodied play;
- startup, resume, detach, and failure principles;
- the relationship between Universe and the current authored lobby;
- the blessed role of web Labs as rapid workbenches; and
- the smallest architectural fitness test that could validate the direction.

It does not yet own an implementation tactical, a `mclone-universe` crate, a
new scene host, a universal editor framework, or an arbitrary N-world live
runtime.

## Product Thesis

Universe is not necessarily a new engine subsystem. It is a first-class name
for a product flow that already exists in fragments:

```text
catalog -> preview -> observe -> play
             ^          |        |
             `-- detach / zoom out'
```

The player starts in a product context rather than in an implicit simulated
world. On a flat display, that context may appear as a terrain atlas, recent
worlds, and ordinary panels. In XR it may be spatial: dismissing the menu can
leave the player outside the represented worlds, able to inspect or approach
them. Neither presentation changes the identity or authority model.

Universe is logically available even while a live world fills the
presentation. It need not remain fully rendered, GPU-resident, or ticking
behind that world. An explicit preference or launch intent may enter a live
destination immediately without first drawing a Universe frame. Returning to
Universe reconstructs or reveals the surrounding product context as required
by the platform and resource policy.

The main menu is therefore a panel or focus within Universe, not necessarily
the only root scene. Closing the panel reveals the current Universe
presentation. Entering a world selects a more authoritative representation;
leaving or zooming out selects a less authoritative one.

## Vocabulary

The product name must not blur the engine's existing identity model:

| Term | Meaning |
|---|---|
| **Universe** | The product shell and navigation context spanning available destinations and representations. It is not a persistence key or server simulation. |
| **Realm** | One durable save/universe owned by one `RealmServer`, including realm-scoped players, state, and a dimension registry. |
| **Dimension** | One terrain/entity/tick namespace within a realm, identified by `DimensionKey`. |
| **World** | User-facing language that may refer to a realm, a dimension, or a destination. Persistent contracts must use the more precise identity. |
| **Destination** | A resolvable realm/server plus dimension and optional region/view recipe. It is an intent, not authority to choose a safe player pose. |
| **Representation** | The data currently used to show a destination: metadata, procedural LOD, bounded exact observation, or a live scene. |
| **Authority role** | No authority, observer/admin authority, or ordinary physical-player authority. |
| **Workbench** | A developer or authoring interface such as Terrain Lab, Texture Lab, Asset Lab, or Structure Lab. It is not automatically part of the player shell. |

“Universe Explorer” may remain a product title even though the product can
show multiple technical realms. Engine APIs should continue to use `RealmId`,
`DimensionKey`, `WorldInstanceId`, source revisions, and other precise
identities rather than introduce a vague `UniverseId`.

## Existing Flows, Reframed

Universe encodes existing or already-directed behavior rather than requiring
every part to be invented:

| Existing surface | Current owner and truth | Universe interpretation |
|---|---|---|
| Session-free title and settings | `mclone-app-runtime`, `mclone-scene`, and `mclone-ui` | Menu focus inside the product shell |
| Local-world catalog and remote join | shared client-experience/catalog policy plus platform storage/transport adapters | Available realm/server destinations |
| World Explorer and Terrain Lab navigation | `mclone-view-control` and `mclone-terrain-view` | Detached, non-authoritative terrain inspection |
| Tabletop or god-scale view | `mclone-scene` policy direction over the active world | Live observation at a broader scale |
| Lobby diorama and retained destination | one active and one optional standby `DrawableWorldSlot` | Bounded live preview and activation evidence |
| Realm dimension transfer | `RealmServer` and client/session contracts | Live movement between dimensions without inventing another realm |
| Explicit startup intents | `ClientEntryIntent` and shared entry resolution | Initial Universe focus or immediate destination activation |
| Terrain, texture, asset, and structure Labs | specialized web workbenches over shared or generated engine facts | Rapid capability ideation and inspection |

This reframing does not make every row share one renderer, runtime, UI toolkit,
or physical Worker. It supplies common product language for selecting and
moving between them.

## Shared Terrain Representation Direction

Universe should converge on **one shared terrain-view engine with multiple
hosts and truth-aware sources**, not on parallel Explorer, Lab, game, and
Universe compositors:

```text
detached canonical source --.
live authoritative source ---+--> shared terrain-view engine
bounded observer source -----'      clipmap and residency
                                     exact/procedural coverage and frontier
                                     representation ownership arbitration
                                     immutable prepared terrain frame
                                                  |
                       .--------------------------+------------------.
                       v                          v                  v
               World Explorer               game scene          Universe
              lightweight orbit host    embodied/live host    overview host
```

“One engine” means one logical owner for terrain representation,
exact/procedural composition, and its prepared render products. It does not
mean one universal world runtime, one physical Worker, or one permanently
shared WGPU allocation.

The shared `mclone-terrain-view` direction should own:

- procedural clipmap scheduling, residency, admission, and diagnostics;
- exact-painted coverage, frontier, collar, and composition-mask policy;
- bounded-representation ownership arbitration, including natural-feature
  replacement;
- source, generation, freshness, topology, and provenance facts needed to
  reject stale or semantically incompatible products; and
- immutable prepared terrain-frame products consumed by target-neutral render
  paths.

Narrow source adapters should retain the facts that genuinely differ:

- a **detached canonical source** may regenerate untouched terrain from a
  qualified seed/profile recipe and must report that it is
  non-authoritative;
- a **live authoritative source** adapts the client replica's drawable
  sections, edits, readiness, and revisions without asking the terrain-view
  engine to own simulation, persistence, or networking; and
- a future **bounded observer source** may consume server-published exact or
  summary facts without requiring disclosure of private generator inputs.

Hosts likewise retain view and lifecycle policy. World Explorer supplies a
small orbit/map host and canonical source. `mclone-scene` supplies embodied,
tabletop, stereo, and XR views plus the authoritative source adapter. A
Universe surface may select a detached or observer-backed source and must
present that truth honestly.

The current `TerrainRuntimeExactRenderer` is therefore a transitional proof
boundary, not a second permanent terrain system. It combines canonical exact
generation, proof residency, coverage publication, and exact drawing because
World Explorer was the first composition host. Full-game adoption should
decompose that boundary: retain canonical production as the detached adapter,
reuse its general admission and draw preparation where applicable, and have
both Explorer and the live scene publish through the same terrain-view
coordinator. Do not force the game through a canonical generator, and do not
make Explorer instantiate `McloneSceneHost`, an integrated server, persistence,
or network authority merely to claim runtime reuse.

This boundary should stay narrower than a generic world-runtime trait.
Acquiring authoritative facts and presenting terrain are related but distinct
responsibilities. Sharing the latter is the architectural requirement; safely
sharing GPU allocations between detached and live representations remains a
measured optimization question.

## Representation And Authority Continuum

A visually continuous zoom may cross discrete data and authority boundaries:

| Product view | Data source | Simulation relationship | Authority |
|---|---|---|---|
| Embodied play | Exact live client replica | Active player session | Physical player |
| Live god/overview view | Exact live replica plus admitted distant representation | Same active session or an explicit observer/admin session | Observer/admin or player, according to mode |
| Bounded live preview | Exact observer publication for a selected region | Active but bounded remote/local observation | Read-only observer |
| Detached terrain preview | Procedural LOD, cached summaries, and optionally bounded stored exact facts | No required live simulation | None |
| Catalog/Universe overview | Metadata, saved thumbnails, curated descriptors, or coarse previews | No required live simulation | None |

The visual transition may be smooth; the semantic transition must not be
implicit. A live god camera and a detached LOD preview can look similar while
having very different edit visibility, freshness, cost, and permissions.
Diagnostics and player-facing state must be able to say which representation
is being shown.

Zooming out from a live world does not by itself decide whether the underlying
session:

- remains live under reduced presentation and simulation interest;
- changes from player to bounded observer;
- pauses because it is a local single-player realm;
- persists and shuts down;
- disconnects from a remote host; or
- stays warm as the scene's one allowed standby.

That is explicit product/session policy. It must account for local versus
remote authority, multiplayer consequences, persistence, latency, and resource
budgets.

## Destination And Preview Truth

A future destination value will likely need some combination of:

- local catalog identity or remote endpoint/realm identity;
- `DimensionKey`;
- generator/profile and source revision when legitimately available;
- selected region or horizontal focus;
- view scale, orientation, and presentation mode;
- requested authority role; and
- freshness or cached-preview provenance.

This list is a contract inventory, not a locked Rust struct. Existing
`SessionStartRequest`, catalog descriptors, realm/dimension identities,
scenario intents, observer bounds, and world-view state should be reused or
composed before adding another parallel destination DTO.

A destination recipe is never an authoritative spawn. Entering live play
still requires the server or integrated authority to admit the player, choose
or validate the dimension, load collision-ready exact terrain, and return a
safe position.

Preview truth must remain explicit:

- untouched local procedural terrain may be reconstructed from a qualified
  generator identity;
- edited terrain requires exact stored/live facts or an honest indication that
  the detached LOD omits edits;
- a remote host may keep its seed and generator inputs private;
- remote preview may therefore be server-published, observer-backed, curated,
  cached, unavailable, or stale; and
- no cached or procedural view may silently present itself as current
  authoritative state.

## Startup, Resume, And Failure

The implemented product currently defaults to the title and starts a session
only from an explicit host-neutral request. Universe does not silently change
that behavior. A future tactical may generalize the shared entry vocabulary so
the ordinary root becomes Universe with an initial focus.

Prospective startup choices include:

- open Universe with its menu focused;
- restore the last detached Universe/terrain view;
- explicitly rejoin the last live realm and representation;
- open a deep-linked destination or scenario; or
- enter a platform-neutral automation target.

“Rejoin last world” or “restore god view” must be an explicit shared
preference, not a desktop-, browser-, Android-, or XR-specific default. A
stale, inaccessible, incompatible, or failed destination falls back to
Universe with a useful status. It must not silently create a replacement
realm.

The last view and the last live session are different facts. Restoring a
detached view may need only identity and camera state; rejoining live play
requires storage or network access, authority, compatibility, and safe
arrival.

## Relationship To The Authored Lobby

The current lobby is valuable implementation evidence:

- a session-free title can launch a scenario;
- a transient protected realm and a persistent destination can coexist;
- one bounded destination can warm and render as a diorama;
- the physical player can transfer only after authoritative and drawable
  readiness; and
- complete slots can exchange without reconstructing the selected runtime.

It is also deliberately limited. It uses an authoritative authored world as
the surrounding context, retains at most one standby, and expresses navigation
through one in-world presentation.

Universe generalizes the surrounding product context without requiring it to
be an authoritative voxel level. The lobby may remain an authored scenario,
theme, tutorial, test fixture, or optional spatial presentation. This topic
does not delete it or make Universe responsible for its world content.

## Web Labs As Blessed Workbenches

Terrain Lab, Texture Lab, Asset Lab, and Structure Lab demonstrate that small
web interfaces are unusually effective ideation and review surfaces. Rapidly
assembled panels, controls, comparisons, charts, and URL-addressed state are a
feature of those tools, not accidental debt.

The useful rule is:

> Web workbenches may own interface experimentation; they do not become the
> owners of engine meaning.

A specialized Lab may own:

- React/DOM layout, panels, tabs, docking, and responsive behavior;
- rapid control instantiation and developer-facing labels;
- charts, comparisons, inspection tables, and capture affordances;
- URL encoding and local transient view state;
- browser capability presentation; and
- explicitly local authoring mechanics such as invoking a checked tool
  pipeline, when that Lab's scope permits mutation.

Shared Rust, validated generated artifacts, or the appropriate engine owner
must own:

- generator, asset, material, and structure semantics;
- validation and canonical identity;
- scheduling, cache validity, source revisions, and comparison truth;
- runtime behavior and persistence consequences;
- authority and permission decisions; and
- any capability promoted into the cross-platform player product.

An experiment-only control may begin as Lab-local state so iteration remains
fast. Before another host or the live game depends on its meaning, the
semantic value and validation must move into a typed shared contract. Universe
may later expose mature inspection capabilities in a developer/editor profile,
but it must not embed whole React applications or require DOM UI on native and
XR targets.

No universal Workbench framework is selected here. The Labs should remain
specialized while their common needs are merely visual resemblance. Shared
control schemas, docking models, or plugin systems should be extracted only
from repeated concrete pressure.

## Ownership Direction

The current owners remain valid:

- `mclone-app-runtime` owns entry intent, catalog/session-start policy,
  preferences, activity demand, and product-level effects that do not require
  a live renderer;
- `mclone-scene` owns the live scene/session, active physical authority,
  authoritative exact-source adaptation, live frame orchestration, UI
  assembly, and its bounded optional standby;
- `mclone-server` owns realms, dimensions, players, observers, transfer,
  persistence, and authoritative readiness;
- `mclone-terrain-view` owns the shared terrain representation engine:
  procedural products, streaming coordination, exact/procedural coverage and
  composition, bounded representation arbitration, and prepared terrain-frame
  contracts used by detached and live hosts;
- `mclone-view-control` owns platform-neutral map/orbit/focus/zoom semantics;
- `mclone-ui` owns cross-platform player-facing UI models;
- render crates own target-neutral rendering contracts and mono, stereo, and
  multiview execution; and
- app/platform code owns window, canvas, Android, OpenXR, storage, transport,
  Worker/thread, and presentation mechanics.

Universe may initially be only a product model implemented by extending these
owners. Do not create a `UniverseController`, `mclone-universe` crate, or new
app hierarchy until concrete state or dependencies have no coherent existing
owner. Conversely, do not force a real cross-scene composition owner into an
unrelated crate merely to avoid a new name.

The standalone World Explorer should remain a small dependency-firewall and
profiling proof. It and `mclone-scene` should be peer hosts of the same shared
terrain-view engine, with different source and lifecycle adapters. A future
Universe entry point should consume those services rather than slowly
importing the full game into that leaf app or creating another composition
implementation.

## Invariants

1. Universe is product composition, not world authority or a global current
   realm/dimension singleton.
2. At most one session owns the physical player's authority unless a future
   multiplayer design explicitly changes that rule.
3. A visually smooth transition never hides an authority, freshness, source,
   or persistence boundary.
4. Procedural and cached previews cannot claim to include authoritative edits
   they do not possess.
5. Remote previews do not require disclosure of private seeds or generator
   inputs.
6. Showing many destinations does not require keeping many
   `DrawableWorldSlot`s, servers, client replicas, or renderer shells live.
7. The current scene's active-plus-optional-standby bound remains until a
   measured product need justifies broader live residency.
8. Product meaning, startup preference, and transitions are shared across
   flat, touch, controller, XR, native, and browser hosts.
9. Universe presentation must not assume every dimension is a spherical
   planet. Finite, unbounded, periodic, or custom-topology dimensions retain
   their real topology.
10. Every world-space Universe or preview presentation must support mono,
    per-eye, and full-frame multiview where applicable.
11. The Labs remain free to optimize for developer speed; promotion into the
    player product crosses an explicit shared-contract boundary.
12. A fresh entry point may compose public shared owners but may not copy
    policy from `mclone-native-client` or make one platform the product owner.
13. Explorer, live scene, and future Universe terrain views share one logical
    terrain representation/composition engine. Source authority and host
    lifecycle remain explicit adapters rather than reasons to fork that
    engine.

## Architectural Fitness Experiment

The most useful first experiment is not a wholesale client rewrite. It is a
fresh composition proof that asks whether the current shared owners can
reconstruct an existing journey without importing app-local policy.

A bounded proof could:

1. inventory the present title, catalog, direct-start, Explorer, overview,
   preview, activation, dimension-transfer, and return paths;
2. define a shared representation/transition vocabulary by composing existing
   identity and request types;
3. construct a session-free shell through public shared APIs;
4. open one local realm/dimension through the shared terrain-view engine with
   a detached canonical source;
5. turn an explicitly selected region into the existing authoritative
   session-start and safe-arrival path, then adapt the live replica into that
   same terrain-view engine;
6. detach or return to the same Universe destination by changing the explicit
   representation/source role rather than treating LOD as live truth; and
7. prove the state machine and shared composition boundary through a cheap
   offscreen or desktop lane before adding browser, Android, and XR
   presentation receipts.

The controller/model must be shared from the start even if desktop or
offscreen is the first mechanical proof. Success means the new composition is
boring: existing engine services assemble cleanly, platform apps stay thin,
and the experiment identifies rather than copies any remaining app-local
ownership.

## Staged Direction

### Stage 0: Architecture Audit

- Map current nouns, identities, owners, startup paths, transitions, and
  resource lifecycles.
- Identify where “title,” “scenario,” “world,” “dimension,” “preview,”
  “overview,” and “session” currently overlap or disagree.
- Decide the smallest representation and transition vocabulary.
- Record current direct-path and session-free performance baselines.

### Stage 1: Universe Entry Model

- Extend existing entry/client-experience policy where it fits.
- Keep the implemented title behavior compatible until a deliberate product
  switch.
- Represent menu focus separately from destination/session state.
- Add no new renderer or live-world registry.

### Stage 2: Detached Destination

- Select one local catalog realm/dimension.
- Open the shared procedural terrain view with qualified source identity.
- Preserve the standalone Explorer as an independent consumer.
- State honestly whether stored edits are represented.

### Stage 3: Enter And Detach

- Resolve a selected region through the existing authoritative session-start
  and safe-arrival path.
- Distinguish live god/overview from detached LOD.
- Return to Universe with stable destination and view identity.
- Measure teardown, warm retention, GPU rebuild, and first-use latency before
  choosing a seamless resource-transfer design.

### Stage 4: Resume And Failure Policy

- Add explicit restore-view and rejoin-live preferences.
- Prove local, remote, missing, stale, incompatible, and failed destinations.
- Fall back to Universe with status and without creating replacement content.

### Stage 5: Cross-Platform Presentation

- Give flat, touch, controller, and XR hosts suitable presentations over the
  same product state.
- In XR, validate scale, comfort, world-space UI, per-eye correctness, and
  multiview.
- Keep platform-specific lifecycle and API mechanics outside product policy.

### Stage 6: Optional Workbench Surfaces

- Promote only mature Lab semantics through typed shared inspection contracts.
- Consider developer/editor panels inside Universe without making them
  required player UI.
- Extract common Lab UI machinery only after repeated concrete use warrants
  it.

Each stage should become its own bounded tactical or be split further after
the Stage 0 audit. This topic is not an instruction to implement all stages as
one campaign.

## Open Questions

- Is Universe merely a state within the current client-experience controller,
  or does cross-renderer composition eventually justify a focused shared
  owner?
- What is the smallest destination locator that composes existing catalog,
  realm, dimension, session, and view types without duplicating them?
- When does zooming out keep a live session, demote it to an observer, pause a
  local realm, or tear it down?
- How should edited local terrain contribute to detached LOD truth?
- Which cached or server-published preview formats are useful for private
  remote realms?
- Can detached and live terrain share WGPU resources safely enough to matter,
  or is bounded regeneration simpler?
- How are multiple dimensions of one realm arranged without falsely implying
  spherical planets or physical distance between namespaces?
- Which parts of the current title menu become Universe panels, and which
  remain presentation-specific?
- Does the authored lobby remain an optional product destination after
  Universe becomes useful?
- Which Lab inspection capabilities deserve promotion into a cross-platform
  developer mode?

## Related Documents

- [`client-entry-lifecycle.md`](client-entry-lifecycle.md) — implemented
  menu-first entry, explicit launch intent, activity demand, and future resume
  policy boundary.
- [`realm-dimension-runtime.md`](realm-dimension-runtime.md) — authoritative
  realm, dimension, player, observer, and transfer identities.
- [`embedded-worlds.md`](embedded-worlds.md) — retained previews, authored
  lobby, warm activation, and bounded two-world scene evidence.
- [`world-view-navigation.md`](world-view-navigation.md) — shared
  map/orbit/zoom semantics and detached-to-authoritative arrival direction.
- [`tabletop-overview-mode.md`](tabletop-overview-mode.md) — live active-world
  overview, god-scale navigation, and authority boundaries.
- [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md) — reusable
  fixed-budget LOD terrain and vegetation streaming.
- [`gpu-procedural-terrain.md`](gpu-procedural-terrain.md) — Terrain Lab,
  shared terrain-view engine, and Lab-versus-game reuse.
- [`texture-material-profiles.md`](texture-material-profiles.md) — Texture Lab
  and Terrain Lab material/profile semantics.
- [`structure-lab.md`](structure-lab.md) and
  [`animal-catalogue.md`](animal-catalogue.md) — specialized web catalogue and
  workbench precedents.
- [`platform-host-boundary.md`](platform-host-boundary.md) — shared product
  meaning versus autonomous platform and browser mechanics.
- [`../native-engine-architecture.md`](../native-engine-architecture.md) —
  current shared scene, app-runtime, renderer, and platform ownership.
