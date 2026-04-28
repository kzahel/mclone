# Creatures6 - Friendly island mobs and basic behaviors

Status: planned.

This tactical follows the `CreatureMovement` stack. It expands the small island debug preset from cows only to the common passive farm-animal set while keeping behavior and rendering tied to the authoritative entity lifecycle.

## Vanilla Sources To Read

| Concern | Source |
|---|---|
| Farm animal biome weights | `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` |
| Cow behavior baseline | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Cow.java` |
| Pig behavior baseline | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Pig.java` |
| Sheep behavior baseline, color data, shearing/eating hooks | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Sheep.java` |
| Chicken behavior baseline, flap state, egg timer | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Chicken.java` |
| Passive animal base class | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Animal.java` |
| Ageable mob base behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/AgeableMob.java` |

## Scope

- Add vanilla-shaped render/model/texture coverage for pig, sheep, and chicken before spawning them in the island preset.
- Register basic passive goals through the shared generated-mob stack: random stroll, look-around/look-at-player when available, water avoidance, and movement attributes.
- Add sheep default color data, chicken flap/egg-timer state placeholders, and pig/sheep/chicken vanilla dimensions and attributes.
- Expand `SmallIslandWorldGenerator` starter entities near the spawn chunk to include cows, pigs, sheep, and chickens in separate small groups.
- Keep these as debug starter entities, not a replacement for generation-time original mobs or live natural spawning.
- Add host publication and client hydration tests for all starter entity types.

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

## Validation

```bash
pnpm test -- test/worldgen/levelgen/demo-world-generators.test.ts test/runtime/generated-world-host-entities.test.ts test/world/entity
pnpm --silent typecheck
pnpm probe:browser -- test/browser/probes/<smallest-entity-probe>.probe.ts
```

Use the browser probe once pig/sheep/chicken renderers are wired, and save screenshots to `/tmp`.

## Next Step

After `CreatureMovement3`, implement render/model coverage for one additional farm animal first, then add that type to the island preset and reuse the same tests for the remaining passive animals.
