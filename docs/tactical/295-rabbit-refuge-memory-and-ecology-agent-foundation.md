# Tactical 295: Rabbit Refuge Memory and Ecology Agent Foundation

Status: complete 2026-08-14; exact public desktop and phone acceptance passed

Topic: `wildlife-ecology-state-model`

Topic: `rabbit-burrow-ecology`

## Instruction Synthesis

Move the accepted rabbit chapter toward one defensible wildlife state model
without making every future species a warren variant or building a general
actor platform. Persist animals and their bounded knowledge, treat unloaded
world state as unavailable rather than absent, make warrens reusable and
replaceable current shelter rather than permanent family owners, and let a
rabbit needing refuge reuse a reachable mouth before excavating one suitable
cell. Establish only the common knowledge, availability, decision scheduling,
and work-budget machinery that rabbits concretely consume; use deer and fox as
later contrasting extraction points. Preserve the accepted garden, pathing,
hiding, breeding, separation, collapse, persistence, showcase, and
cross-platform behavior.

## Diagnosis

- Rabbit save state carries one optional warren ID but no durable locator or
  bounded alternative knowledge. The runtime position cache exists only while
  that exact mouth entity is loaded.
- `home == None` is both an ecological fact and the founder state-machine
  trigger, so every unhomed rabbit immediately searches for a dig site rather
  than foraging, discovering existing capacity, or waiting until shelter is
  needed.
- A warren persists permanent `residents`, and every entity tick reconstructs
  those slots by scanning currently loaded rabbit mobs. An unloaded animal can
  therefore look absent to a loaded mouth; capacity and dispersal depend on
  loaded-set accidents rather than one authoritative fact.
- Collapse notifies loaded rabbits, while an absent animal can retain a stale
  home ID without enough locator/version information to distinguish
  unavailable from authoritatively gone later.
- Rabbit behavior, candidate scanning, up to eight immediate A* attempts,
  action continuation, and slow timers live in one per-tick function. There is
  no explicit due-decision boundary or shared work admission.
- Species-wide flock/herd/rabbit preparation and dense contact work have no
  bounded stress contract. A large player-bred population must defer idle
  thinking fairly rather than create an unbounded server-tick spike.

## Binding Model

### Shared ecology primitives

Add a narrow server-internal ecology module with pure, host-neutral data and
tests for:

- a stable world-fact locator containing durable ID, last-known block/position,
  and optional revision;
- bounded known-place memory with observation time, familiarity/confidence,
  deterministic replacement, confirmation, and invalidation;
- `Available`, `CurrentlyUnavailable`, `ConfirmedUnsuitable`, and
  `ConfirmedGone` resolution outcomes;
- decision deadlines, urgent wake reasons, and attempt generations that make
  deferred results stale-safe; and
- deterministic work-unit admission with per-class limits, rotating fairness,
  deferral diagnostics, and no wall-clock-dependent ecological ordering.

Do not add an `Animal`/`HomeAnimal` inheritance hierarchy, a scripting system,
an unconstrained fact database, or a public cross-crate ecology API. Rabbit
policy remains an explicit species module. The shared module becomes public or
crate-independent only after deer or fox proves a real second consumer.

### Rabbit record and warren ownership

- Replace one permanent `home` with at most three persisted known-refuge
  memories. Each carries the mouth ID, last-known position, last confirmation,
  and familiarity. Preserve an optional current `sheltered_in` relationship
  only while entering, underground, or emerging.
- Migrate the prior record format. A legacy home may begin as an unresolved
  locator and gain its position when that mouth is authoritatively observed;
  migration must not invent destruction or a replacement.
- A warren persists identity, capacity, condition, disturbance, damage, and
  last use. Permanent family membership is removed. Parent/child identity
  remains exclusively on rabbits.
- Current occupancy is derived from co-located rabbits whose
  `sheltered_in` names the mouth. Entering and breeding reserve capacity
  through serialized host mutation; a foraging rabbit does not reserve a
  lifetime slot.
- Because a sheltered rabbit is positioned at its mouth, occupant and mouth
  records remain in the same entity chunk. Hydration reconciles the complete
  entity-chunk record after load rather than inferring absence from a partial
  callback order.
- A fully loaded entity chunk that authoritatively lacks the remembered mouth
  may confirm it gone. An inactive/unloaded chunk only yields currently
  unavailable. No query force-loads terrain to settle memory.

### Needs-driven refuge choice

Retain `RabbitBehavior` as the authoritative action/animation phase rather than
the entire decision model. Split rabbit work into urgent reaction, continuous
action, and due decision.

When safety or rest requires shelter:

1. score reachable remembered mouths;
2. query nearby active mouths with current capacity and remember a selected
   candidate;
3. excavate exactly one existing habitat-qualified dirt/grass cell only when
   no adequate reachable refuge exists and the rabbit's bounded dig cooldown,
   local-density, danger, and need rules permit it; or
4. flee/use local cover/reconsider later when neither entry nor excavation is
   currently safe.

An unhomed rabbit may forage normally. A mature disperser does not instantly
dig on release. Using or creating a local mouth does not prove an unavailable
familiar mouth was destroyed. Repeated use increases familiarity; collapse or
complete active evidence removes the exact memory. Existing off-axis gate,
carrot raid, temptation, entry/hiding/emergence, family growth, separation,
and damage behavior remain ordinary shared gameplay.

### Cadence and overload

- Urgent threat, damage, interaction, path-block change, and habitat collapse
  wake immediately.
- Movement, collision, path following, entry, digging, and emergence continue
  at the gameplay entity cadence while active.
- New refuge/forage decisions are stable-ID staggered and consume explicit
  decision, habitat-query, and path-request work units. Missing a due idle
  decision produces one later current decision, never catch-up replay.
- Budget exhaustion defers lower-priority idle work before player-observable or
  safety work. A rotating fair start identity prevents starvation.
- Replace any rabbit-wide or warren-wide full scans on the hot path with
  chunk/cell-indexed lookup or one bounded pass reused for the tick. Dense
  rabbit separation must query spatial buckets rather than all pairs.
- Add diagnostics for active/due/admitted/deferred rabbit decisions, habitat
  candidates, path requests, neighbor candidates, and oldest work debt.

The general thousand-animal benchmark may use chickens because player breeding
can exceed natural caps. It must prove bounded per-tick admitted decision/path
work, fair eventual decisions, and nonquadratic neighbor candidate growth. It
does not need to render one thousand actors or change natural mob caps.

## Persistence and Compatibility

- Bump the entity-chunk record codec with explicit migration from the current
  rabbit and burrow payloads. Old permanent residents become memory/occupancy
  inputs only when the co-located saved rabbits prove current shelter; they do
  not remain a second membership authority.
- Ambient pose, intent, memory familiarity, and decision deadlines retain the
  existing coalesced dirty-entity checkpoint policy. Materialization remains
  logically persistent immediately without synchronous per-tick storage.
- Dig, raid, birth, collapse, pickup, and death already dirty all affected
  records. A general multi-record crash-atomic batch remains a separately
  bounded persistence tactical; do not disguise it as part of the memory
  refactor. Record the residual mixed-checkpoint risk explicitly.
- Keep transient showcase worlds write-free. No showcase branch may enter
  memory, decisions, occupancy, budgets, pathing, or persistence.

## Validation and Acceptance

Focused pure and server integration tests must prove:

1. bounded deterministic memory replacement, confirmation, and exact
   invalidation;
2. unavailable, unsuitable, and gone outcomes never alias;
3. legacy rabbit/warren records migrate and round-trip with stable identity;
4. arbitrary rabbit/mouth load order cannot drop a memory, fabricate
   destruction, or overbook current occupancy;
5. an unavailable familiar mouth remains known while the rabbit chooses a
   reachable existing mouth with capacity;
6. a shelter-needing rabbit digs only after no reachable capacity exists,
   while an ordinary unhomed forager does not immediately excavate;
7. collapse invalidates the exact loaded mouth, releases sheltered rabbits,
   and preserves their identity, lineage, and other refuge knowledge;
8. garden raids, open-gate routing, feeding/breeding, invisibility,
   same-identity emergence, and separation remain accepted;
9. deterministic budget exhaustion defers idle work fairly, admits urgent
   work, rejects stale attempts, and bounds the dense-population fixture; and
10. unload/reload plus SQLite and IndexedDB codec paths preserve the new facts.

Advance the existing data-only `rabbit-burrow` showcase only as needed to make
reuse versus excavation observable. Its bounded desktop and phone gate must
observe one rabbit use an existing non-primary mouth before any permitted new
dig, retain actual garden navigation and shelter identity outcomes, and leave
all browser world-record stores empty. Inspect native flat/stereo and headed
WebGPU desktop/phone pixels. Push the exact accepted revision, let the normal
deploy path publish it, rerun deployed desktop/phone behavior and storage
gates, inspect public pixels, and share the fresh URL.

## Non-Goals

- deer bedding, herd memory, fox predation, denning, population summaries, or
  offscreen catch-up;
- player-walkable tunnels, linked mouths, voxel cave-ins, traps, or burrow
  repair;
- a general behavior-tree language, ECS rewrite, new engine crate, or public
  mod API;
- lower-fidelity movement through unloaded terrain; and
- the cross-record atomic checkpoint batch tracked by the ecology topic.

## Execution Record

Commits `98c36dc8` and `17166e55` land the implementation and showcase proof;
the preceding `eff9f619` records the architecture and tactical.

- `mclone-server::ecology` now owns the private pure locator, known-place,
  availability, decision-generation, and deterministic work-budget
  primitives. Rabbit is the only consumer; no public generic animal API was
  introduced.
- Entity persistence is version 11. Rabbits own up to three refuge memories
  and an optional current shelter; mouths no longer persist residents. Version
  10 migration, arbitrary entity-record load order, exact collapse
  invalidation, current occupancy, and SQLite-compatible roundtrip are covered.
- Rabbit decisions are need-driven and stable-ID staggered. Fixed 64 decision,
  32 habitat-query, and 32 path-request units per tick defer idle work without
  catch-up. Spatial refuge cells and bounded neighbor buckets replace broad
  hot-path candidate scans.
- The full `cargo test -p mclone-server --lib` lane passed 708 tests, including
  unavailable memory, alternate reuse, no overbooking, delayed one-cell dig,
  pathing, hiding, collapse, and the 1,000-rabbit bounded/fair fixture.
- `rabbit-burrow` revision 4 compiles six saved entities: four rabbits and two
  typed reusable mouths. Native flat and XR-emulated stereo captures passed.
- Local headed WebGPU desktop and phone gates both observed the founder use the
  alternate mouth with all thirteen pre-dig samples fixed at two mouths. Both
  then recorded two carrot raids, hide/return, zero overlap streak, exact
  three-hit primary collapse, all four rabbit identities after collapse, the
  surviving alternate mouth, and zero records in all eight IndexedDB world
  stores.

Exact pushed revision
`df837bf06ae268eb12fb7fc563ed2c2aaf06dc1a` deployed through the normal
after-main-push route as asset version
`df837bf06ae2-20260814130525` and Cloudflare Worker version
`89e80b4e-75b4-4f9d-b52c-6d150f95589d`. Public desktop and phone gates both
repeated alternate-refuge approach, deep hiding, and return without adding a
third mouth; two real carrot raids; zero sustained overlap; exact primary-mouth
collapse after three ordinary attacks; all four rabbit identities surviving;
and the alternate mouth remaining. Every browser world-record store remained
empty in both lanes.

The clean first-frame desktop and phone captures were inspected at the recipe's
seed `17507` and entry camera. Their SHA-256 digests are
`3df1c05815b1b609af8f3e6f832c740808924059e99ef59f27ba97bf648bc5cb`
and
`866cbed8a4be978c199df3aaf6abe2149e92412d2b9ea23ed165ee00718a564c`.
The fresh transient review URL is
`https://mclone.kzahel.com/app.html?showcase=rabbit-burrow`.
