# Creatures4 - Cow baseline lifecycle

Status: baseline landed; first cow tick/wander foundation continues in [`Creatures5-cow-tick-wander-foundation.md`](Creatures5-cow-tick-wander-foundation.md).

This tactical follows the landed entity/runtime foundation in [`Entities0-runtime-entity-foundation.md`](Entities0-runtime-entity-foundation.md), generated passive spawning in [`Creatures1-generation-passive-spawning.md`](Creatures1-generation-passive-spawning.md), host entity publication in [`Creatures2-host-entity-publication.md`](Creatures2-host-entity-publication.md), and the vanilla-shaped renderer foundation in [`EntityRender0-vanilla-entity-renderer-foundation.md`](EntityRender0-vanilla-entity-renderer-foundation.md).

The goal is to make `minecraft:cow` the first end-to-end passive mob implemented on the real entity architecture. Cow is the right first target because it uses the common farm-animal spawn table and ordinary animal goals without sheep wool/eat-block state, pig saddle/rider state, or chicken egg/flap/fall behavior.

## Goal

Implement the first cow baseline without shortcuts:

- generated cows are authoritative host entities, not renderer decorations
- cow lifecycle uses the existing `PersistentEntitySectionManager` path
- clients receive cow add/update/remove facts through the entity protocol
- rendering consumes client presentation state through `EntityRenderDispatcher`
- later cow wandering ports vanilla mob tick/goal behavior instead of inventing a local movement rule

This slice should make cow behavior a reusable path for sheep, pig, chicken, and remote player entities.

## Landed Baseline

The stationary cow baseline is now implemented:

- committed vanilla fixture for seed `12345`, chunk `(2, -18)`, containing `minecraft:cow`
- fixture-backed generation tests covering sheep and cow through `NaturalSpawner.spawnMobsForChunkGeneration(...)`
- generated-world host publication for cow snapshots through the host-owned `EntityRuntime`
- `entity_update` and `entity_remove` protocol messages beside `entity_snapshot`
- client hydration for entity add/update/remove lifecycle facts
- explicit remove/untrack messages on chunk interest unload and dropped remote player slots
- vanilla-shaped `QuadrupedModel`, `CowModel`, `CowRenderer`, `ModelLayers.COW`, and cow texture loading
- Deno entity render smoke updated to draw both a remote player and adult cow with extracted vanilla textures

Deferred from this baseline:

- generated cow movement/ticking
- cow attributes, health, sounds, interactions, breeding, drops, and persistence
- vanilla goal execution, including `WaterAvoidingRandomStrollGoal`
- live natural spawning and mob caps

## Vanilla Source Review

Read these files before writing or changing implementation code. Simulation/content behavior is a direct port unless explicitly documented as a runtime carrier divergence.

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Cow.java` | cow goals, attributes, dimensions, sounds, offspring, eye height |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Animal.java` | passive spawn predicate, persistence/despawn behavior, breeding hooks, animal base rules |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/AgeableMob.java` | age state, baby/adult dimensions and save fields |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/PathfinderMob.java` | path navigation and movement-controller ownership |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java` | despawn, goal ticking, move/look/jump controls, persistence flags |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` | living tick, movement, health, pose, interpolation fields |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` | identity, position, bounds, section move callbacks, removal, save/load |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/EntityType.java` | cow dimensions, tracking metadata, construction defaults |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/GoalSelector.java` | goal priority and tick scheduling |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/FloatGoal.java` | priority 0 cow goal |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/PanicGoal.java` | priority 1 cow goal, deferred until damage/threat exists |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/BreedGoal.java` | priority 2 cow goal, deferred until interaction/breeding exists |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/TemptGoal.java` | priority 3 cow goal, deferred until held items/player interactions exist |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/FollowParentGoal.java` | priority 4 cow goal, deferred until baby cows exist |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/WaterAvoidingRandomStrollGoal.java` | first wandering goal to port after stationary lifecycle works |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/LookAtPlayerGoal.java` | priority 6 cow goal, depends on nearby player/entity queries |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/RandomLookAroundGoal.java` | simple idle look goal |
| `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/CowRenderer.java` | renderer lookup, texture, model layer use |
| `reference/minecraft-1.17.1/src/net/minecraft/client/model/CowModel.java` | vanilla cow geometry and render state |

Related architecture docs:

- [`../entities.md`](../entities.md)
- [`../creatures.md`](../creatures.md)
- [`../minecraft-client-replica-research.md`](../minecraft-client-replica-research.md)
- [`../protocol.md`](../protocol.md)

## Implementation Order

### 1. Cow Spawning And Stationary Lifecycle

Status: landed.

Use existing generation-time passive spawning first. The first acceptance target is not live natural spawning; it is generated cows becoming durable host entities and visible client entities when their chunk is tracked.

Required work:

- add or identify an oracle fixture where generation-time `minecraft:cow` appears
- assert cow type, position, rotation, dimensions, age, and basic data through the existing creature fixture path
- ensure generated cow records enter `EntityRuntime.addWorldGenChunkEntities(...)`
- keep entity storage separate from block chunk snapshots
- keep cows stationary in this first step if ticking behavior is not ported yet

Do not add renderer-local cow placement, hand-authored test cows in the browser, or seed respawn of killed cows.

### 2. Entity Lifecycle Protocol

Status: baseline landed for add/update/remove message shape and client hydration. `Creatures5` adds tick context and first moving-entity routing; per-session revision remains deferred.

Before cow movement, fix the protocol gap exposed by remote players and generated mobs.

Required work:

- add tick context to authoritative entity state before movement deltas depend on ordering; per-session revision remains follow-up
- add explicit entity removal/untrack messages instead of relying only on `chunk_unload`
- add entity update messages for position, rotation, chunk/section ownership, and tracked data
- make per-session entity tracking decide when a client receives baseline, update, and remove messages
- keep `entity_snapshot` as a baseline message and add delta/removal semantics beside it
- preserve `player_state` as the owning-player reconciliation stream; remote players and cows should share the normal tracked-entity stream for other clients

This is required for cows because a moving cow can cross chunk boundaries or leave client interest without forcing chunk mesh messages.

### 3. Cow Rendering

Status: landed for neutral adult cows.

Render cow snapshots through the vanilla-shaped renderer stack landed by `EntityRender0`.

Required work:

- port `CowModel` geometry and `CowRenderer` lookup far enough for neutral adult cows
- bind the extracted vanilla cow texture through entity `RenderType`
- adapt cow presentation state by `typeId: "minecraft:cow"`, not by ad hoc metadata
- validate with the smallest browser or Deno visual lane that shows a generated cow in-world

Rendering is visual-only. It must not own cow lifecycle, spawning, movement, or despawn.

### 4. Cow Tick Foundation

Status: next.

After stationary cows render and lifecycle is correct, start the simulation port needed for wandering.

Required work:

- introduce vanilla-shaped passive mob state only where needed by cow
- port attribute defaults for cow: max health `10.0`, movement speed `0.2`
- port base age/adult state enough to preserve adult cow behavior
- port enough of `Mob` / `PathfinderMob` tick ownership for goals to run in `ENTITY_TICKING` chunks only
- keep ordinary cows non-despawning at distance through `Animal.removeWhenFarAway(...) == false`
- save/load the state introduced in this slice through entity records

Do not collapse this into one synthetic `Cow.tick()` that moves randomly. If the vanilla stack is too large for one pass, land the base classes in narrow steps but keep names and control flow aligned with the Java sources.

### 5. Cow Wandering

Port cow wandering through the vanilla goal system rather than a one-off random walk.

First behavior target:

- `GoalSelector`
- `WaterAvoidingRandomStrollGoal`
- movement/look control pieces needed by that goal
- block collision checks through authoritative host world data
- entity section move callbacks on every authoritative position change
- `entity_update` publication as cows move
- client interpolation from authoritative updates

Follow-up behavior targets after wandering:

- `RandomLookAroundGoal`
- `LookAtPlayerGoal`
- `FloatGoal`
- interaction-dependent goals only when player inventory/item interaction exists
- breeding, panic, drops, sounds, particles, and damage after the underlying systems exist

## Explicit Non-Goals

- live natural spawning through `NaturalSpawner.spawnForChunk(...)`
- mob caps and player-distance natural-spawn eligibility
- sheep, pig, chicken behavior beyond sharing infrastructure
- combat, damage, drops, sounds, particles, breeding, milking, or food interactions
- full pathfinding parity beyond the pieces needed for first cow wandering
- client-owned AI or prediction
- renderer-owned entity truth
- spawning cows as block decorations or mesh data

## Verification

Data/runtime validation:

```bash
pnpm test -- test/worldgen/levelgen/creature-generation.test.ts test/oracle/committed-creature-fixture.test.ts test/runtime/generated-world-host-entities.test.ts test/runtime/client-entity-interpolation-service.test.ts test/runtime/world-message-queue.test.ts test/runtime/remote-player-snapshots.test.ts test/renderer/entity/player-renderer.test.ts
pnpm typecheck
npx -y deno@2.7.13 run --unstable-webgpu --allow-read=. --allow-write=/tmp ./scripts/deno-entity-render-smoke.ts
git diff --check
```

Add focused tests as behavior lands:

- cow fixture comparison for generated entity facts
- entity add/update/remove protocol hydration in `ClientWorld`
- entity section move and chunk crossing during cow wandering
- explicit removal/untrack when a cow leaves client interest
- stationary cow persistence/load roundtrip once entity persistence is added

Pixel validation:

- for rendering changes, run the smallest Deno/browser lane that reaches cow rendering
- save screenshots to `/tmp`
- inspect the image before continuing
- use browser GPU validation only after `pnpm host:check` confirms the lane is viable on the host

## Done Criteria

Stationary cow baseline is done when:

- generated `minecraft:cow` fixtures are covered by a Java oracle or committed vanilla fixture - done
- cow entities enter host-owned entity runtime through the generation sink - done
- clients receive cow baselines through the protocol and remove/untrack cows explicitly - done
- the browser/Deno renderer can draw an adult cow through `EntityRenderDispatcher` - done
- no cow truth is stored in chunk mesh or renderer state - done

Cow wandering follow-up is done when:

- cow movement is produced by vanilla-shaped entity tick/goals in entity-ticking chunks
- cow section/chunk ownership updates through the entity manager as it moves
- clients receive tick-context entity updates and interpolate cow movement
- tests cover movement, update publication, and client removal when interest changes

## Next

After cow baseline and first wandering are stable, repeat the same path for:

1. sheep, adding wool color/sheared/eat-block state
2. pig, adding saddle/rider-related tracked data only when interaction/riding systems exist
3. chicken, adding flap/fall/egg state

Live natural spawning should still wait until cow-style ticking, mob counting, player-distance eligibility, despawn, and entity persistence are solid enough to preserve vanilla semantics.
