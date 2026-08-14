# Rabbit Burrow Ecology

Topic: `rabbit-burrow-ecology`

Status: **live shared implementation complete under Tacticals
[`292`](../tactical/292-rabbit-burrow-ecology.md) and
[`293`](../tactical/293-rabbit-navigation-and-burrow-visibility.md), including
the shared A* routing and deep-burrow visibility correction, with Tactical
[`294`](../tactical/294-rabbit-warren-lifecycle-and-separation.md) locally
accepted for shelter cadence, soft separation, disturbance, collapse,
resident-safe resettlement, and full-warren dispersal on 2026-08-13. Revision
3 has exact public desktop/phone acceptance. Tactical
[`295`](../tactical/295-rabbit-refuge-memory-and-ecology-agent-foundation.md)
lands the 2026-08-14 evolution to bounded rabbit-owned refuge memories,
replaceable current shelter, derived occupancy, needs-driven reuse, and
budgeted decisions. Revision 4 has native, local Web, and exact public
desktop/phone acceptance.**

## Purpose

This topic owns the continuing contract where rabbits connect natural banks,
persistent creature homes, carrots, gardens, enclosure, family growth, and
player observation. It is the first ecology chapter in which a creature makes
a small authoritative terrain edit and the first animal-pressure consumer of
the functional kitchen garden.

The selected product shape is a hybrid burrow:

```text
one genuine shallow excavated cell + semantic mouth/threshold
  -> compact persistent warren and resident identities
  -> visible emergence, retreat, feeding, and breeding
  -> no implied player-walkable tunnel network
```

The bounded implementation and evidence record live in Tactical 292. General
creature admission and persistence live in
[`habitat-driven-creature-ecology.md`](habitat-driven-creature-ecology.md),
garden mechanics in
[`functional-kitchen-gardens.md`](functional-kitchen-gardens.md), and review
recipe guardrails in [`playable-showcases.md`](playable-showcases.md).

## Binding Decisions

- A completed burrow physically removes exactly one validated dirt/grass bank
  cell. The entrance is not merely a dark decal or a raised rigid track.
- The authored `rabbit_burrow` world prop supplies a recessed dark inset,
  irregular earth lip, roots, and threshold within that cell. Geometry is
  presentation; server state owns identity, capacity, residents, and use.
- The deeper warren is inaccessible compact simulation state. Player crawling,
  tunnel graphs, general cave carving, and voxel cave-ins remain later systems
  that require their own design. The semantic mouth itself takes ordinary
  player damage and can collapse without pretending the compact warren is a
  traversable cave.
- Individual rabbits become durable when materialized. Going underground
  hides the same saved entity; it does not despawn and reroll an individual.
- Housed rabbits take identity-staggered short rests during active hours, so
  visible entry, true deep hiding, and same-identity emergence are observable
  without freezing a review scene at night.
- Natural Mclone founder rabbits must visibly qualify and dig a site. A
  showcase may start from the resulting ordinary saved fact but cannot be the
  only producer.
- World time governs emergence opportunity, with dawn/dusk bias rather than
  continuous wandering.
- Real carrots are temptation, breeding food, and raid targets. One feeding
  consumes one item; two fed compatible adults and one safe warren with space
  are required for one persistent kit.
- A completed raid mutates one real mature carrot growth stage through shared
  crop state. Cooldowns and path reachability bound damage.
- Closed fences and gates exclude rabbits through ordinary collision/path
  facts; open gates are traversable. No garden/showcase coordinate exceptions
  are permitted.
- Visible rabbits use bounded mutual soft pushes resolved through ordinary
  block collision. Contact at a mouth remains possible, but sustained
  same-space occupation is not normal behavior.
- A hit alarms a mouth and flushes deep residents. Three prompt hits collapse
  it; residents survive with their identities and family links, lose the
  invalid home, flee, and reuse ordinary habitat qualification and excavation
  to resettle.
- At capacity, a stable mature descendant with recorded parents disperses into
  the same unhomed founder loop. Founders and kits are not selected merely to
  create room.
- Terrain-surface decals remain a separate general concern. Loose-earth decal
  blending may improve the mouth later but cannot become a rabbit-specific
  renderer.
- Java 1.17.1 `Rabbit` remains the source reference for recognizable hop,
  temptation, avoidance, breeding, persistence, and garden-raid semantics.
  Persistent excavated warrens are an explicit Mclone-profile extension.

## Acceptance Themes

- **Terrain truth:** a burrow reads as a recess because the bank is actually
  opened, while its dry support, roof, rear, and approach are validated.
- **Identity truth:** a family and its home survive unload/reload without
  population replacement or duplicate births.
- **Motion truth:** rabbits hop with useful displacement, stop turning in
  place, enter and emerge through their actual mouth, and retreat under danger.
- **Garden truth:** fence/gate state changes reachable crops, a raid visibly
  changes a carrot, feeding visibly consumes inventory, and a kit is durable.
- **Review truth:** the public tiny save composes ordinary facts, passes a
  behavioral window, and never writes a browser world record.

## Current Implementation

Tactical 292 has landed the complete first chapter:

- a habitat-qualified Mclone founder creates one persistent rabbit only where
  a dry supported dirt/grass bank, forage, cover, movement room, and occupancy
  checks pass;
- the founder visibly digs and authoritatively removes one terrain cell, then
  a semantic mouth occupies that honest recess and owns compact persistent
  warren capacity/resident state;
- individual rabbits retain identity, parentage, life stage, health,
  underground state, love/breeding state, raid cooldown, and home across
  entity-chunk persistence;
- world-time emergence, collision-aware hop/flee movement, home retreat,
  hidden sheltering, forage reconsideration, and named animations create a
  legible daily loop;
- closed fences and gates protect live mature carrots, while an open gate lets
  rabbits reach and decrement one real crop age under cooldown;
- every habitat intent now accepts only a complete route from the shared
  `GroundPathNavigation` A* owner and follows its intermediate waypoints, so
  unreachable carrots are rejected and newly opened off-axis gates are
  reconsidered after the ordinary block update;
- the visible entry animation remains at the mouth, but the same alive saved
  rabbit leaves client tracking in the deeper `Underground` state and is
  snapshotted again with the same entity and persistent identity on emergence;
- a held carrot tempts rabbits; using one on a reachable adult consumes the
  item and two compatible fed residents can create one durable smaller kit;
- a harvested raid target cancels on that block-change tick instead of letting
  a rabbit finish a chew epoch at air;
- active-hour rests now exercise the full mouth-entry, untracked underground,
  and same-ID emergence loop; visible pairs receive deterministic soft
  separation without being pushed through fences or banks;
- ordinary cross-platform attacks alarm a mouth, and the third prompt hit
  collapses it. Loaded residents are flushed alive, retain identity and family
  truth, lose the invalid home, and can excavate replacement mouths. Persisted
  damage is covered by current-format roundtrip and legacy-format migration;
- full warrens release a stable mature offspring with recorded parents through
  the ordinary founder path;
- six normal evidence paths drive field notes, and real rabbit state emits
  bounded spatial thump and dig cues from the first-party CC0 sound bank; and
- the deny-unknown-fields `rabbit-burrow` recipe composes these facts without
  behavior scripts or showcase-specific game logic.

The original implementation revision `3a950022` passed native flat/stereo,
local desktop/phone, and public desktop/phone review. Tactical 293 then showed
that its near-line gate proved collision but did not force A* waypoint use.
Correction commits `b0f7c28b` and `9a9ec120` route live rabbits through the
shared navigator and move revision 2's gate off-axis. Local desktop and phone
acceptance observed all four rabbits travel roughly 24–30 blocks, create a
second ordinary excavation, and mutate two protected carrots only after the
gate opened; all browser world stores remained empty. Feeding remains covered
by the focused authoritative interaction test and the original public
`12 -> 11` receipt rather than being coupled to this autonomous-route gate.
Exact pushed revision `d466a763` repeated those outcomes on deployed desktop
and phone under Cloudflare Worker version
`61ee5243-c972-4f2a-8dcb-7d1a175fe0e9`. The temporary review link is
`https://mclone.kzahel.com/app.html?showcase=rabbit-burrow`.

Tactical 294 revision 3 local acceptance then observed every initial rabbit ID
hide and return, no sustained near-zero pair overlap, the protected-garden
route and two real raids, three normal attacks that removed the exact initial
mouth, survival of all initial residents, and ordinary replacement mouths in
both desktop and phone lanes. All browser world-record stores remained empty.
Exact pushed revision `013d9303` repeated those results on public desktop and
phone under Cloudflare Worker version
`7c6e4167-517c-435f-ad23-8203b0bd3048`.

Tactical 295 removes permanent warren resident authority. Each rabbit now
persists up to three refuge memories with stable ID, last-known position,
optional revision, confirmation time, and familiarity, plus a current
`sheltered_in` relationship only during actual shelter phases. Version 10
records migrate into version 11 without interpreting an unloaded mouth as
destroyed. Current occupancy and navigation reservations are derived from
rabbit facts, and collapse invalidates only the exact mouth while preserving
lineage and other knowledge.

An active unhomed rabbit forages instead of immediately excavating. Rest or
safety need first resolves remembered active capacity, then discovers a nearby
active mouth, and only then considers one-cell excavation under cooldown and
local-density bounds. Stable decision deadlines and fixed per-tick decision,
habitat-query, and path-request units defer idle thought without catch-up;
16-block mouth lookup and bounded neighbor buckets keep dense populations from
becoming quadratic. The full 708-test server suite includes a 1,000-rabbit
fairness and work-bound fixture.

Revision 4's data-only tiny save starts with two ordinary mouths. One familiar
mouth is temporarily full with a truly underground kit, so the independent
founder must discover, approach, enter, disappear into, and return from the
second mouth while the burrow count remains exactly two. Local desktop and
phone gates also preserved four carrots while closed, opened the off-axis gate
through their ordinary controls, recorded two raids, found no sustained
overlap, collapsed the exact primary mouth after three hits, retained every
rabbit identity, kept the alternate mouth, and left all eight browser
world-record stores empty. Exact pushed revision `df837bf0` repeated the full
receipt on public desktop and phone as asset version
`df837bf06ae2-20260814130525` under Cloudflare Worker version
`89e80b4e-75b4-4f9d-b52c-6d150f95589d`. The inspected clean first-frame
desktop and phone digests are
`3df1c05815b1b609af8f3e6f832c740808924059e99ef59f27ba97bf648bc5cb`
and
`866cbed8a4be978c199df3aaf6abe2149e92412d2b9ea23ed165ee00718a564c`.
The transient review link remains
`https://mclone.kzahel.com/app.html?showcase=rabbit-burrow`.

## Deliberate Later Work

- player-accessible tunnels, expanding warrens, multiple linked entrances,
  voxel cave-ins, repair, trapping, relocation, artificial burrow boxes, predators,
  and population-summary simulation;
- rabbit variants, fur/resource loops, taming, richer genetics, disease,
  seasons, temperature, crop economics, and cooking;
- subtle loose-earth terrain decals after the general surface-trace owner
  exists; and
- broader small-animal navigation only when rabbits expose a reusable need.

## Refuge Model Boundary

The current implementation proves durable excavation, identity, capacity,
hiding, destruction, reuse, and resettlement without making a `home`
relationship the template for deer beds, broad territories, nomadic animals,
or every other spatial behavior. The shared direction lives in
[`wildlife-ecology-state-model.md`](wildlife-ecology-state-model.md).

For rabbits specifically, the landed shape is:

- each animal retaining scored knowledge of one or more refuge candidates
  instead of requiring one universally reachable permanent home;
- a warren remaining a durable, reusable feature with capacity and condition,
  but not owning the rabbit's identity or entire behavior lifecycle;
- an unloaded remembered mouth producing "no current path" rather than
  "missing," so the rabbit can choose a local alternative without falsely
  deleting or replacing distant truth;
- a tired or threatened rabbit first trying a suitable reachable known mouth,
  then a locally discovered mouth with capacity, and finally excavating one
  qualifying dirt/grass cell when necessary; and
- cooldown, habitat suitability, local density, reuse, and collapse bounding
  new excavation.

Bounded social sharing, energy cost, and eventual abandonment remain possible
later refinements rather than requirements hidden inside the first foundation.

This retains the readable one-cell mouth and compact hidden interior. It does
not require underground tunnel simulation. It turns inaccessible shelter into
a behavioral decision—search, follow, hide locally, or dig—instead of a
persistence race or state-machine failure.

## Code and Content Map

- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Rabbit.java`
- `tools/asset-lab/examples/rabbit/figure.ts`
- `tools/asset-lab/props/rabbit_burrow/figure.ts`
- `native/crates/mclone-protocol/src/ecology.rs`
- `native/crates/mclone-server/src/entity/mob/`
- `native/crates/mclone-server/src/farming.rs`
- `native/crates/mclone-server/src/playable_showcase.rs`
- `assets/mclone/showcases/rabbit-burrow.showcase.json`

## Recommended Next Direction

Use deer as the contrasting replaceable-site and herd-memory proof before this
vocabulary is declared a general ecology framework. Fox can then pressure
rabbit knowledge through prey observation, interruptible pursuit, and
life-stage-relevant den use without turning warrens into universal homes.
