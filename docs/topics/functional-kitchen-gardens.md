# Functional Kitchen Gardens

Topic: `functional-kitchen-gardens`

Status: **active under Tactical
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
  harvest, direct seed item, and the future rabbit attraction/breeding/raid
  resource.
- Rabbits follow the garden foundation. Their burrows, feeding, crop damage,
  and breeding must consume live carrots and real enclosure/navigation facts.

## Current Contract

Tactical 291 is responsible for the first complete chapter:

- widen the compact raw block-state lane so new exact property families do not
  alias or compress mechanics;
- implement oak fences and gates through shared state, asset, physical,
  interaction, navigation, audio, persistence, and placement paths;
- generalize the accepted wheat implementation and add carrots;
- author and promote one source-first kitchen-garden recipe;
- replace the intro homestead's placeholder flower-bed garden with the working
  recipe through ordinary starter-content materialization; and
- provide a bounded transient desktop/phone review save backed entirely by
  those live producers.

## Deliberate Later Work

- rabbits, burrows, crop attraction, crop raids, breeding, and garden escape;
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

After Tactical 291 is accepted, promote rabbits as the next contrasting
creature chapter: substrate-qualified burrows, dawn/dusk emergence, wary
foraging, carrot attraction, bounded crop raids, fence/gate-aware escape and
exclusion, breeding with real food, persistent family/burrow state, and field
notes based on ordinary evidence. Decide whether burrow mouths need a terrain
decal/surface-patch owner before adding raised track-like geometry.

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

