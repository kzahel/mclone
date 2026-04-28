# Creatures9 - Chicken live flap and egg timer

Status: chicken flap and egg-timer data ticking landed.

This tactical records the first animal-specific generated-mob behavior after the shared movement, push, and passive look stacks. The goal is not a renderer-only wing animation; the authoritative generated chicken should tick vanilla-shaped chicken state and publish it through the normal entity snapshot/update data path.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Chicken flap state, egg timer, fall damage, save keys, and chicken-jockey gate | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Chicken.java` |
| Living entity tick ordering around `aiStep()` and `serverAiStep()` | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` |
| Mob `aiStep()` and `serverAiStep()` split | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java` |
| Chicken renderer wing bob input | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/ChickenRenderer.java` |
| Chicken model wing rotation input | `reference/minecraft-1.17.1/src/net/minecraft/client/model/ChickenModel.java` |

## Vanilla Model

- `Chicken.aiStep()` runs after `super.aiStep()`, so chicken-specific flap and egg logic follows the normal mob AI/travel/push work.
- Each tick stores previous flap fields, adjusts `flapSpeed` by `-0.3` on ground or `+1.2` while airborne, clamps that speed to `[0, 1]`, restores `flapping` to at least `1.0` while airborne, decays `flapping` by `0.9`, then advances `flap` by `flapping * 2.0`.
- While airborne and falling, vanilla damps Y `deltaMovement` by `0.6`; that remains deferred until full `deltaMovement`/`LivingEntity.travel(...)` parity lands.
- Adult, alive, non-jockey chickens decrement `eggTime`; when it reaches zero, vanilla plays `CHICKEN_EGG`, spawns an egg item, and resets `eggTime` to `random.nextInt(6000) + 6000`.
- Baby chickens and chicken jockeys do not decrement `eggTime` because the vanilla condition short-circuits before `--eggTime`.

## Landed Scope

- Hydrated generated chicken runtime state from existing `Flap`, `FlapSpeed`, `OFlap`, `OFlapSpeed`, `Flapping`, `EggLayTime`, and `IsChickenJockey` snapshot data.
- Ticked the vanilla flap/flap-speed/flapping formulas after generated-mob movement.
- Ticked adult non-jockey egg timers and reset them to the vanilla `6000..11999` range when they expire.
- Kept baby and chicken-jockey egg timers from decrementing.
- Published live chicken fields through `GeneratedMobEntity.getSnapshotData()`, so host entity updates and the existing chicken renderer consume authoritative state.
- Added focused unit coverage for live flap state, egg timer decrement/reset, and chicken-jockey gating.

## Runtime Divergence

Egg item spawning and egg sounds are intentionally deferred. Vanilla calls `playSound(SoundEvents.CHICKEN_EGG, ...)` and `spawnAtLocation(Items.EGG)` when the timer expires. `mclone` does not yet have item entities, item stacks, sound events, or loot/drop publication on the generated entity path. This slice resets the timer at the correct point and leaves a code callout where item and sound side effects should attach later.

The fall-slowing `deltaMovement.y *= 0.6` branch is also deferred because generated mobs still use the current path-following movement bridge rather than full `LivingEntity.travel(...)`.

## Deferred Scope

- Egg item entity creation and authoritative item pickup/drop lifecycle.
- Chicken egg sound publication.
- Full `deltaMovement`, falling, gravity, fall-damage immunity, and `LivingEntity.travel(...)` integration.
- Chicken breeding, food/tempt interactions, jockey spawning/riding behavior, sounds beyond egg laying, damage/death, and loot.

## Validation

```bash
pnpm test -- test/world/entity/cow-ai.test.ts test/renderer/entity/player-renderer.test.ts
pnpm --silent typecheck
```

## Next Step

Start `Creatures10` with sheep `EatBlockGoal`, grass mutation, and eating head animation. That is the next visible animal-specific behavior that exercises both server-side world mutation and renderer state.
