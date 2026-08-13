# Rabbit Burrow Ecology

Topic: `rabbit-burrow-ecology`

Status: live shared implementation completed under Tactical
[`292`](../tactical/292-rabbit-burrow-ecology.md), with the A* routing and deep
burrow tracking correction active under Tactical
[`293`](../tactical/293-rabbit-navigation-and-burrow-visibility.md) on
2026-08-13.

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
  tunnel graphs, general cave carving, and collapse are later systems that
  require their own design.
- Individual rabbits become durable when materialized. Going underground
  hides the same saved entity; it does not despawn and reroll an individual.
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
- a held carrot tempts rabbits; using one on a reachable adult consumes the
  item and two compatible fed residents can create one durable smaller kit;
- six normal evidence paths drive field notes, and real rabbit state emits
  bounded spatial thump and dig cues from the first-party CC0 sound bank; and
- the deny-unknown-fields `rabbit-burrow` recipe composes these facts without
  behavior scripts or showcase-specific game logic.

Exact implementation revision `3a950022` passed native flat/stereo, local
desktop/phone, and public desktop/phone review. The public gates observed four
rabbits travel roughly 21–30 blocks, a second ordinary excavation, two
gate-dependent carrot raids, feeding consumption `12 -> 11`, and zero records
in all browser world stores. The temporary review link is
`https://mclone.kzahel.com/app.html?showcase=rabbit-burrow`.

## Deliberate Later Work

- player-accessible tunnels, expanding warrens, multiple linked entrances,
  collapse, repair, trapping, relocation, artificial burrow boxes, predators,
  and population-summary simulation;
- rabbit variants, fur/resource loops, taming, richer genetics, disease,
  seasons, temperature, crop economics, and cooking;
- subtle loose-earth terrain decals after the general surface-trace owner
  exists; and
- broader small-animal navigation only when rabbits expose a reusable need.

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

Human review should now judge the rabbit hop/entry animation, whether the mouth
reads as an actual shallow excavation, and whether garden pressure, retreat,
and family life stay interesting outside the bounded receipt. Address any
observed motion or legibility defect in the shared live system before adding
more species. If this chapter holds up, the next ecology slice should deepen a
reusable stewardship consequence—food/cooking, crop yield, protected habitat,
or predator pressure—rather than immediately add another decorative animal.
