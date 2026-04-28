# Creatures10 - Sheep eat block goal

Status: sheep grass-eating baseline landed.

This tactical records the first passive-mob behavior that mutates world blocks. Sheep keep using the shared generated-mob movement, push, and passive look stacks, but now also run the vanilla `EatBlockGoal` and publish the resulting animation/state through the normal authoritative entity path.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Sheep goal registration, eating animation, sheared state, and `ate()` side effect | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Sheep.java` |
| Grass/grass-block eating goal, timer, mob-griefing gate, and block mutation | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/EatBlockGoal.java` |
| Mob AI tick ordering and `customServerAiStep()` hook | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java` |
| Living entity server/client AI tick shape | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` |
| Sheep head eating animation formulas | `reference/minecraft-1.17.1/src/net/minecraft/client/model/SheepModel.java` |
| Sheep fur layer visibility and tint inputs | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/layers/SheepFurLayer.java` |

## Vanilla Model

- `Sheep.registerGoals()` creates one `EatBlockGoal` and installs it at priority `5`, before random stroll priority `6`, look-at-player priority `7`, and random-look-around priority `8`.
- `EatBlockGoal` carries `MOVE`, `LOOK`, and `JUMP` flags, so eating stops movement/look/jump goals while active.
- `canUse()` has a vanilla random gate: babies try `1/50`, adults try `1/1000`.
- The goal can eat short grass at the sheep block position or a `grass_block` directly below the sheep.
- `start()` sets the timer to `40`, broadcasts entity event `10`, and stops navigation.
- `tick()` decrements the timer and mutates the world when the timer reaches `4`.
- With `mobGriefing` enabled, short grass is destroyed without drops; a grass block below the sheep becomes dirt. Either successful case calls `Sheep.ate()`.
- `Sheep.customServerAiStep()` copies `eatBlockGoal.getEatAnimationTick()` into sheep runtime state.
- Vanilla client event `10` also sets the client-side animation timer to `40`; the renderer derives head Y offset and head pitch from that timer.

## Landed Scope

- Added a shared `EatBlockGoal` translation with the vanilla timer, flags, random gate, short-grass target, grass-block-below target, and mutation tick.
- Registered the goal for generated sheep at vanilla priority `5`.
- Extended `MobAiLevel` with block default-state lookup, `mobGriefing`, `destroyBlock`, and `setBlock` hooks needed by entity goals that mutate blocks.
- Bridged generated-world hosts so sheep grass eating mutates authority chunks, marks block changes, and publishes dirty chunk snapshots even when liquid simulation is disabled.
- Hydrated and published sheep `EatAnimationTick` and `Sheared` data through generated entity snapshots.
- Ported the sheep eating head-position and head-angle formulas into the renderable sheep adapter.
- Added focused unit/runtime coverage for grass-block-to-dirt mutation, short-grass destruction, sheared-state reset, renderer animation formulas, and dirty chunk publication.

## Runtime Divergence

- Game-rule storage is not modeled yet; the host bridge currently returns vanilla's default `mobGriefing=true`.
- Vanilla broadcasts entity event `10` and lets the client run a local 40-tick eating animation. The current generated-entity path publishes the authoritative server tick value in snapshots instead.
- `Sheep.ate()` currently unshears generated sheep. Vanilla also grows babies by `60` seconds; generated age mutation is deferred until age/growth state becomes mutable.
- Block break particles, level event `2001`, `GameEvent.EAT`, eating sounds, wool regrowth persistence, and shearing/drop interactions remain deferred.

## Deferred Scope

- Game-rule storage and synchronization.
- Entity event protocol for animation triggers.
- Baby growth / age mutation and persistence.
- Sheep shearing interaction, wool item drops, shearing sound, and persistence-backed sheared state.
- Block break particles, game events, and sound publication.

## Validation

```bash
pnpm test -- test/world/entity/cow-ai.test.ts test/renderer/entity/player-renderer.test.ts test/world/level/pathfinder/walk-node-pathfinder.test.ts test/runtime/generated-world-host-entities.test.ts
pnpm --silent typecheck
npx -y deno@2.7.13 run --unstable-webgpu --allow-read=. --allow-write=/tmp ./scripts/deno-entity-render-smoke.ts
```

The Deno entity smoke writes `/tmp/mclone-deno-entity-render-smoke.png`.

## Next Step

`Creatures11` pivots to breadth first: mooshroom, rabbit, and wolf baseline render/runtime support before adding the shared item/entity interaction path.
