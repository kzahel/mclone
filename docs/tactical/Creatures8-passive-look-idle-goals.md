# Creatures8 - Passive look and idle goals

Status: shared passive look/idle baseline landed.

This tactical records the vanilla source shape behind passive mob head motion. The goal is reusable entity AI plumbing, not cow-specific behavior: cows, pigs, sheep, and chickens should all share the same look control, player-look goal, random-look goal, eye-height facts, and authoritative snapshot publication.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Mob AI tick ordering and head rotation defaults | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java` |
| Shared look control | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/LookControl.java` |
| Player look goal | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/LookAtPlayerGoal.java` |
| Random idle look goal | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/RandomLookAroundGoal.java` |
| Cow passive goal order and eye height | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Cow.java` |
| Pig passive goal order | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Pig.java` |
| Sheep passive goal order and eye height | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Sheep.java` |
| Chicken passive goal order, eye height, and future flap tick hook | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Chicken.java` |
| Base entity eye height | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |

## Vanilla Model

- `Mob.serverAiStep()` ticks goals, navigation, custom mob AI, then controls in move/look/jump order.
- `LookControl` stores a wanted look target, rotates `yHeadRot` toward that target by `getHeadRotSpeed()` degrees per tick, rotates pitch by `getMaxHeadXRot()`, returns the head toward `yBodyRot` when idle, and clamps head rotation to `getMaxHeadYRot()` while navigating.
- `LookAtPlayerGoal` has `Goal.Flag.LOOK`, rolls probability `0.02`, chooses the nearest player within `6.0` blocks for these passive animals, runs for `40 + random.nextInt(40)` ticks, and points the mob's `LookControl` at the player's eye position.
- `RandomLookAroundGoal` has `Goal.Flag.MOVE` and `Goal.Flag.LOOK`, rolls probability `0.02`, picks a random horizontal unit direction, runs for `20 + random.nextInt(20)` ticks, and points the mob at that idle target.
- Adult cow eye height is `1.3`, sheep eye height is `0.95 * height`, chicken eye height is `0.92 * height`, and pig uses the base `height * 0.85`.
- Vanilla passive goal priorities used here are:
  - cow/chicken: stroll `5`, look at player `6`, random look `7`
  - pig/sheep: stroll `6`, look at player `7`, random look `8`

## Landed Scope

- Added shared `LookControl` with vanilla method names and rotation math.
- Added shared `LookAtPlayerGoal` and `RandomLookAroundGoal` for generated passive mobs.
- Added the passive goal priorities above for cows, pigs, sheep, and chickens.
- Added generated-mob head/body/pitch rotation fields (`YBodyRot`, `YBodyRotO`, `YHeadRot`, `YHeadRotO`, `XRotO`) to authoritative entity snapshot data.
- Updated renderable mob hydration to consume those rotation fields so vanilla-shaped entity renderers can animate head/body separation.
- Added host-supplied nearest-local-player look targeting through `MobAiLevel`, using the current authoritative player movement body and standing eye height.
- Added vanilla standing eye heights for cow, sheep, chicken, and the base pig fallback.
- Added focused unit coverage for eye heights, look control rotation, player-look acquisition, pathfinder interface compatibility, and renderer hydration of head/body rotation data.

## Runtime Divergence

Vanilla `LookAtPlayerGoal` depends on `TargetingConditions`, `EntitySelector.notRiding(...)`, full level entity queries, and concrete `Player`/`LivingEntity` instances. `mclone` does not yet model local and remote players as ordinary server-side player entities in the generated entity runtime. This slice adds a narrow `MobAiLevel.getNearestPlayer(...)` bridge that exposes the authoritative local player as a look target.

The bridge preserves the future parity path:

- the goal still asks the level for a nearest player at the mob eye position and range
- the look target exposes entity-like position, eye height, and alive state
- the bridge can be replaced by a real server entity query once players enter the same entity system

The snapshot rotation fields are also a protocol bridge. Vanilla tracks living-entity body/head rotation through entity state and network packets; this project currently publishes generic `EntitySnapshot.data`. The fields use vanilla names so they can move to a more exact protocol representation later without changing AI ownership.

## Deferred Scope

- Full `TargetingConditions` parity, including visibility, spectator, riding, predicate, and non-player living-entity targeting.
- Remote player look targets in the dedicated/HTTP multiplayer path after remote players share the same entity query model.
- Full `LivingEntity` rotation/body animation update loop beyond the current generated-mob head/body fields.
- Animal-specific passive behaviors beyond `Creatures9`: sheep grass eating, pig saddle/boost hooks, cow milking/interactions, breeding, food, sounds, damage/death, and loot.
- Item entity spawning for eggs, drops, and interaction results.

## Validation

```bash
pnpm test -- test/world/entity/cow-ai.test.ts test/world/level/pathfinder/walk-node-pathfinder.test.ts test/renderer/entity/player-renderer.test.ts
pnpm --silent typecheck
```

## Next Step

Chicken live flap and egg-timer ticking is now tracked in `Creatures9`. Start `Creatures10` with sheep `EatBlockGoal`, grass mutation, and eating head animation.
