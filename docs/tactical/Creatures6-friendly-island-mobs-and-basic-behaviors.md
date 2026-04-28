# Creatures6 - Friendly island mobs and basic behaviors

Status: in progress; pig-first render/spawn/movement slice landed.

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
| Chicken behavior baseline, flap state, egg timer | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Chicken.java` |
| Passive animal base class | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Animal.java` |
| Ageable mob base behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/AgeableMob.java` |

## Landed Scope

- Added vanilla-shaped pig render/model/texture coverage through `PigModel`, `PigRenderer`, `ModelLayers.PIG`, and renderable snapshot hydration.
- Registered pig passive movement attributes from vanilla: max health `10.0`, movement speed `0.25`.
- Registered pig `WaterAvoidingRandomStrollGoal` through the shared generated-mob stack at vanilla priority `6`.
- Expanded `SmallIslandWorldGenerator` starter entities near the spawn chunk from cows only to cows plus pigs in separate visible groups.
- Kept the island mobs on the authoritative generated-entity lifecycle with normal host publication, client hydration, and `GroundPathNavigation` movement.
- Kept these as debug starter entities, not a replacement for generation-time original mobs or live natural spawning.
- Added unit/runtime coverage for pig render batches, texture path resolution, island starter placement, host publication, and shared passive-mob navigation.

## Remaining Scope

- Add vanilla-shaped render/model/texture coverage for sheep and chicken before spawning them in the island preset.
- Register sheep and chicken basic passive movement attributes/goals through the shared generated-mob stack.
- Add sheep default color data/render coloring follow-through and chicken flap/egg-timer state placeholders.
- Expand `SmallIslandWorldGenerator` starter entities to include sheep and chickens in separate small groups.
- Add host publication and client hydration tests for sheep/chicken starter entity types.

## Behavioral Order

1. Spawn/render only, with static authoritative lifecycle.
2. Shared random stroll through `GroundPathNavigation`.
3. Passive look goals and idle animation state.
4. Animal-specific details: sheep color/eat grass/shear hooks, chicken flap and egg timer, pig saddle/boost hooks when items/interactions exist.
5. Natural-spawn integration after mob caps, player-distance eligibility, despawn, and persistence are in place.

## Out Of Scope

- Breeding, food items, drops, damage/death, milking, shearing interactions, saddles, riding, eggs as item entities, sounds, and loot tables.
- Exact natural spawn scheduling or mob caps.
- Hostile mobs and water creatures.
- Pig saddle render layer and boost/riding logic until saddle data and interactions exist.

## Validation

```bash
pnpm test -- test/worldgen/levelgen/demo-world-generators.test.ts test/runtime/generated-world-host-entities.test.ts test/world/entity/cow-ai.test.ts test/renderer/entity/player-renderer.test.ts
pnpm --silent typecheck
npx -y deno@2.7.13 run --unstable-webgpu --allow-read=. --allow-write=/tmp ./scripts/deno-entity-render-smoke.ts
```

The Deno entity smoke writes `/tmp/mclone-deno-entity-render-smoke.png`. Use a browser probe again when sheep/chicken are visible in the full island scene.

## Next Step

Implement sheep next: port sheep model/renderer/color handling, add island sheep starters, then reuse the pig runtime/render coverage pattern before moving to chicken.
