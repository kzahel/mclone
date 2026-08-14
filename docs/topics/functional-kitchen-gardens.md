# Functional Kitchen Gardens

Topic: `functional-kitchen-gardens`

Status: **live shared implementation and exact public desktop/phone
acceptance complete under Tactical
[`291`](../tactical/291-functional-kitchen-garden-foundation.md) on
2026-08-13.**

## Purpose

This topic owns the continuing gameplay contract for small working gardens:
physical boundaries, usable entrances, irrigated mixed crop beds, harvest and
renewal, source-first garden composition, and their placement in ordinary
settlements. It connects the crop foundation to future animal pressure and
husbandry without making any creature the only reason gardens function.

The intended loop is:

```text
choose and shape a garden place
  -> enclose it with connected fences and a gate
  -> irrigate and plant useful crops
  -> enter, tend, harvest, collect, and replant
  -> animals later create attraction, protection, and stewardship choices
  -> garden needs improve structures, terrain, and ecology together
```

Wheat's first crop mechanics remain documented in
[`wheat-farming.md`](wheat-farming.md). Settlement composition remains in
[`starter-farmstead-settlement.md`](starter-farmstead-settlement.md). This
topic owns their intersection as a repeatable gameplay place.

## Binding Direction

- A garden is ordinary live state, never a decorative-only structure or
  showcase script.
- Fence geometry, collision, selection, connectivity, gate state, navigation,
  and persistence must agree.
- Crop breadth extends one shared crop specification and random-tick path;
  distinct crops keep distinct planting items, silhouettes, loot, and later
  ecology relationships.
- Structure Lab is the source pipeline for intentional garden composition.
  Runtime projection may adapt that composition to terrain, but may not keep a
  visually unrelated duplicate recipe.
- The intro homestead is the first ordinary generated consumer. Pure reference
  Overworld output remains unchanged.
- Carrots precede potatoes because they are already useful as food-shaped
  harvest, direct seed item, and the live rabbit attraction/breeding/raid
  resource.
- The rabbit chapter consumes the garden foundation. Its burrows, feeding,
  crop damage, and breeding use live carrots and real enclosure/navigation
  facts.

## Current Contract

Tactical 291 has landed the first complete chapter:

- the compact live raw-state lane is `u16`, with little-endian native/Web
  worker transport and above-255 persistence and mesh coverage;
- oak fences own exact four-way connections and five-box geometry, while oak
  gates own facing/open/powered/in-wall state, normal placement/use, 1.5-block
  closed collision, open traversal, path semantics, wood feedback, inventory,
  and persistence;
- wheat and carrots share `CropKind` support, hydration-density growth,
  support cleanup, harvest, and item-drop decisions while retaining distinct
  states, silhouettes, planting items, and loot;
- `farmstead-kitchen-garden-v1` is a promoted 13×2×13 Structure Lab source
  with 253 blocks, a connected 35-fence/one-gate boundary, central irrigation,
  and mixed hydrated crop beds;
- intro-homestead `garden-v1` projects that same canonical record onto its
  surveyed grade and remains ordinary editable/persisted starter content; and
- `kitchen-garden` revision 1 is a bounded data-only review save whose typed
  evidence points back to those live placement, farming, and homestead
  producers;
- Tactical 292 now consumes those same facts in ordinary rabbit ecology:
  closed boundaries protect mature carrots, open gates expose them to bounded
  raids, and the selected carried carrot automatically tempts persistent
  rabbits without use while an intentional use feeds them for breeding.
- Tactical 293 closes the navigation side of that agreement: rabbits use the
  shared A* ground path, reject partial routes, and replan after an ordinary
  nearby gate edit. Its off-axis review garden cannot pass by steering toward
  the carrot through the fence.

The local desktop and phone gates prove gate opening/traversal/closing and
restored collision, automatic crop growth, mature carrot removal, generic
moving world drops, proximity pickup, age-zero replanting, real keyboard/touch
controls, and zero browser persistence. The gate exposed and fixed a shared
tall-collision lookup defect rather than compensating in the showcase.

Implementation commits are `1e68d96d`, `f1c78ef1`, `e3b65ba2`, `1dd5687a`,
`bee025f2`, and `e5b8d2f9`. Exact local and public captures, digests, commands,
and deployment receipt live in Tactical 291.

## Deliberate Later Work

- richer animal-pressure defenses, crop loss/recovery balance, and garden
  stewardship beyond the first rabbit loop;
- potatoes, beetroot, crop rotation, fertility, pests, disease, and seasons;
- crafting and acquisition loops for fences, gates, tools, and seeds;
- food, hunger, cooking, storage, trade, and villager farming;
- redstone-powered gates, leads, fence water-restoration interaction, and a
  broader wall/door family; and
- large generated fields, machinery, irrigation engineering, and unloaded
  crop catch-up.

These should deepen the same garden rather than create alternate fixture-only
versions of its mechanics.

## Acceptance Themes

- **Object language:** enclosure, entrance, crop stage, removal, loot, and new
  planting read from geometry and motion without mandatory text.
- **Mechanical agreement:** pixels, raycast, collision, navigation, server
  state, client replica, and persistence report the same open/closed boundary.
- **Renewability:** harvest goes into the world, pickup is physical, and one
  collected carrot can be replanted.
- **Place continuity:** the standalone source recipe, ordinary homestead, and
  transient review scene share content and gameplay producers.
- **Future ecology:** the resulting facts are sufficient inputs for a rabbit
  chapter without rabbit-specific fence or carrot exceptions.

## Recommended Next Work

Tactical [`292`](../tactical/292-rabbit-burrow-ecology.md) completes the first
animal-pressure consumer of this foundation. Human play should now tune how
often raids create an interesting enclosure/stewardship choice without making
small gardens futile. Potatoes remain useful later crop breadth, but food,
cooking, storage, or crop-yield consequences may deepen the existing garden
more than another mechanically equivalent plant.

## Code And Documentation Map

- `native/crates/mclone-worldgen/src/block.rs`: compact live block identities
- `native/crates/mclone-server/src/farming.rs`: reusable crop decisions
- `native/crates/mclone-server/src/placement.rs`: block placement semantics
- `native/crates/mclone-blocks/src/block_shapes.rs`: exact block shapes
- `native/crates/mclone-server/src/homestead_landscape.rs`: terrain-matched
  intro landscape pieces
- `tools/structure-lab/`: source-first garden authoring and previews
- `assets/mclone/showcases/`: bounded review-save composition
- [`wheat-farming.md`](wheat-farming.md): first crop and field foundation
- [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md):
  settlement dashboard
- [`habitat-driven-creature-ecology.md`](habitat-driven-creature-ecology.md):
  terrain/creature mechanics cycle
- [`playable-showcases.md`](playable-showcases.md): review-save guardrails
- [`rabbit-burrow-ecology.md`](rabbit-burrow-ecology.md): rabbit/garden
  pressure and persistent hybrid-warren contract
