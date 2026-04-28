# Creatures6 - Friendly island mobs and basic behaviors

Status: starter island farm-animal baseline landed; animal-specific gameplay remains follow-up work.

This tactical follows the `CreatureMovement` stack. It expands the small island debug preset from cows only to the common passive farm-animal set while keeping behavior and rendering tied to the authoritative entity lifecycle.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Farm animal biome weights | `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` |
| Cow behavior baseline | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Cow.java` |
| Pig behavior baseline | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Pig.java` |
| Pig model and renderer | `reference/minecraft-1.17.1/src/net/minecraft/client/model/PigModel.java`, `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/PigRenderer.java` |
| Quadruped model geometry | `reference/minecraft-1.17.1/src/net/minecraft/client/model/QuadrupedModel.java` |
| Entity type dimensions | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/EntityType.java` |
| Sheep behavior baseline, color data, shearing/eating hooks | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Sheep.java` |
| Sheep model, renderer, and fur layer | `reference/minecraft-1.17.1/src/net/minecraft/client/model/SheepModel.java`, `SheepFurModel.java`, `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/SheepRenderer.java`, `layers/SheepFurLayer.java` |
| Sheep wool tint model | `reference/minecraft-1.17.1/src/net/minecraft/world/item/DyeColor.java`, `reference/minecraft-1.17.1/src/net/minecraft/client/model/ColorableAgeableListModel.java` |
| Chicken behavior baseline, flap state, egg timer | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Chicken.java` |
| Chicken model and renderer | `reference/minecraft-1.17.1/src/net/minecraft/client/model/ChickenModel.java`, `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/ChickenRenderer.java` |
| Passive animal base class | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Animal.java` |
| Ageable mob base behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/AgeableMob.java` |

## Landed Scope

- Added vanilla-shaped pig render/model/texture coverage through `PigModel`, `PigRenderer`, `ModelLayers.PIG`, and renderable snapshot hydration.
- Registered pig passive movement attributes from vanilla: max health `10.0`, movement speed `0.25`.
- Registered pig `WaterAvoidingRandomStrollGoal` through the shared generated-mob stack at vanilla priority `6`.
- Added vanilla-shaped sheep body and fur render/model/texture coverage through `SheepModel`, `SheepFurModel`, `SheepRenderer`, `ModelLayers.SHEEP`, `ModelLayers.SHEEP_FUR`, and renderable snapshot hydration.
- Registered sheep passive movement attributes from vanilla: max health `8.0`, movement speed `0.23`.
- Registered sheep `WaterAvoidingRandomStrollGoal` through the shared generated-mob stack at vanilla priority `6`; `EatBlockGoal`/eating animation later land in `Creatures10`.
- Hydrated sheep `Color` data for all generated sheep construction paths and rendered the wool fur layer with vanilla sheep tint math.
- Added vanilla-shaped chicken render/model/texture coverage through `ChickenModel`, `ChickenRenderer`, `ModelLayers.CHICKEN`, and renderable snapshot hydration.
- Registered chicken passive movement attributes from vanilla: max health `4.0`, movement speed `0.25`.
- Registered chicken `WaterAvoidingRandomStrollGoal` through the shared generated-mob stack at vanilla priority `5`, and set its water pathfinding malus to `0.0` like vanilla.
- Hydrated chicken `Flap`, `FlapSpeed`, `OFlap`, `OFlapSpeed`, `Flapping`, `EggLayTime`, and `IsChickenJockey` data for generated chicken construction paths; live flap/egg ticking lands in `Creatures9`.
- Expanded `SmallIslandWorldGenerator` starter entities near the spawn chunk from cows only to cows, pigs, sheep, and chickens in separate visible groups.
- Kept the island mobs on the authoritative generated-entity lifecycle with normal host publication, client hydration, and `GroundPathNavigation` movement.
- Kept these as debug starter entities, not a replacement for generation-time original mobs or live natural spawning.
- Added unit/runtime coverage for pig/sheep/chicken render batches, texture path resolution, island starter placement, host publication, type-specific data hydration, and shared passive-mob navigation.

## Deferred Scope

- Add egg item spawning once item entities exist.
- Port sheep shearing interactions, wool drops, and persistence-backed sheared state.
- Port pig saddle/boost/riding hooks once items, interactions, and passenger entities exist.
- Add breeding, food interactions, drops, sounds, damage/death, and loot tables through later entity/gameplay tacticals.
- Replace debug starter entities with vanilla generation-time and live natural-spawn coverage when mob caps, despawn, player-distance eligibility, and persistence are in place.

## Behavioral Order

1. Spawn/render only, with static authoritative lifecycle.
2. Shared random stroll through `GroundPathNavigation`.
3. Done in `Creatures8`: passive look goals and idle head/body rotation state.
4. Animal-specific details: done for sheep grass eating in `Creatures10` and chicken live flap/egg timer in `Creatures9`; sheep shearing, egg item drops, and pig saddle/boost hooks wait for items/interactions.
5. Natural-spawn integration after mob caps, player-distance eligibility, despawn, and persistence are in place.

## Out Of Scope

- Breeding, food items, drops, damage/death, milking, shearing interactions, saddles, riding, eggs as item entities, sounds, and loot tables.
- Exact natural spawn scheduling or mob caps.
- Hostile mobs and water creatures.
- Pig saddle render layer and boost/riding logic until saddle data and interactions exist.
- Sheep shearing interactions, sheared-state persistence beyond in-memory snapshots, and the `jeb_` color cycle until names/interactions are modeled.

## Validation

```bash
pnpm test -- test/worldgen/levelgen/demo-world-generators.test.ts test/runtime/generated-world-host-entities.test.ts test/world/entity/cow-ai.test.ts test/renderer/entity/player-renderer.test.ts
pnpm --silent typecheck
npx -y deno@2.7.13 run --unstable-webgpu --allow-read=. --allow-write=/tmp ./scripts/deno-entity-render-smoke.ts
```

The Deno entity smoke writes `/tmp/mclone-deno-entity-render-smoke.png`. Use a browser probe again when validating the full island scene.

## Next Step

Continue with the shared item/entity interaction foundation needed by sheep shearing, chicken egg items, cow milking, and pig saddle/riding hooks.
