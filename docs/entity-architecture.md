# Entity Architecture

Durable architecture notes for the native Rust entity, mob, spawning, and
presentation stack.

This document is intentionally stricter than a roadmap. The systems below are
not optional later polish. They are the foundation that every entity feature has
to fit through, even when a first slice implements only one cow or one chicken.
A feature may leave behavior stubbed, but it should not bypass the ownership,
chunk-status, tracking, persistence, or shared module boundaries.

For vanilla behavior, read the Minecraft Java 1.17.1 source before porting a
class or system. Useful entry points include:

| Concern | Vanilla source |
|---|---|
| Server entity tick ownership | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java` |
| Chunk tick, natural spawning, and chunk activity | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java` |
| Chunk ticket distances and natural-spawn tracker | `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java` |
| Full chunk status | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java` |
| Entity section manager | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/PersistentEntitySectionManager.java` |
| Entity ticking set | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntityTickList.java` |
| Entity base behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |
| Mob AI hook surface | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java` |
| Passive animal base behavior | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Animal.java` |
| Goal selector and goals | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/` |
| Navigation and pathfinding | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/`, `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/` |
| Natural spawning | `reference/minecraft-1.17.1/src/net/minecraft/world/level/NaturalSpawner.java` |
| Spawn placement registry | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/SpawnPlacements.java` |
| Entity type metadata | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/EntityType.java` |

## Hard Requirements

Entity truth is host-owned. Local singleplayer, dedicated server, browser
singleplayer, remote clients, Android, and XR all consume the same authoritative
state through shared host/client contracts. The renderer and platform apps can
present entities; they cannot own spawning, ticking, despawning, or persistence.

Chunk status controls entity activity. An entity being visible to a client is
not enough to tick it. An entity being loaded from storage is not enough to tick
it. Ordinary mobs tick only when their owning section is in an
`ENTITY_TICKING` chunk. This is required for correctness and server
performance.

Do not grow a giant `entities.rs` module. The current starter entity path may
act as a temporary facade, but new behavior should move toward explicit
submodules for state, section storage, visibility, ticking, tracking, mobs,
goals, navigation, spawning, and persistence.

Do not add a platform-local gameplay implementation unless the behavior is
genuinely platform-specific. Desktop may validate first, but shared behavior
belongs in shared crates and server/runtime modules.

Do not model creatures as decorative render assets. A cow or chicken placed in
the world is an entity with authoritative identity, chunk/section ownership,
tracking, tick eligibility, and eventual persistence.

## Target Module Shape

The exact file names can evolve, but the boundaries should stay recognizable.

```text
native/crates/mclone-server/src/entity/
  mod.rs                 facade: EntityRuntime, tick entrypoints, host integration
  state.rs               authoritative entity state, identity, pose, dimensions, flags
  section_storage.rs     chunk/section ownership, movement between sections, AABB queries
  visibility.rs          FullChunkStatus -> hidden/tracked/ticking mapping
  tick_list.rs           stable copy-on-write ordinary entity tick set
  tracking.rs            per-player snapshot/update/remove routing
  metadata.rs            entity type, category, dimensions, tracking range, attributes key
  persistence.rs         logical entity chunk records, dirty/unload/save rules
  mob/
    mod.rs               Mob/LivingEntity-shaped runtime fields and tick hooks
    attributes.rs        health, movement speed, pathfinding malus, follow range
    controls.rs          MoveControl, LookControl, JumpControl
    navigation.rs        GroundPathNavigation and path ownership
    goals/
      mod.rs             Goal, WrappedGoal, GoalSelector
      random_stroll.rs
      look.rs
      float.rs
      panic.rs
    animals/
      cow.rs
      chicken.rs
  spawning/
    mod.rs               live and generation-time spawn entrypoints
    mob_category.rs
    biome_tables.rs
    placements.rs
    spawn_state.rs       category counts, caps, spawnable chunks
    natural.rs           NaturalSpawner port
```

Shared protocol facts remain in `mclone-protocol`. Client interpolation and
actor presentation remain in `mclone-client` / `mclone-render-session`.
Renderer geometry, figure compilation, texture atlases, and animation sampling
remain in `mclone-render` / `mclone-assets`. Those crates consume entity facts;
they do not decide entity lifecycle.

## Runtime Ownership

The authoritative host owns entity mutation in this order:

1. Chunk tickets and full chunk statuses are updated by the scheduler.
2. Entity visibility is reconciled from chunk/section status.
3. Ticking entities are selected from the entity tick list.
4. Entity ticks run through `Entity` / `LivingEntity` / `Mob` hooks.
5. Mob goals, controls, navigation, collision, and side effects mutate
   authoritative entity/world state.
6. Tracking publishes snapshots, updates, and removals to clients.
7. Dirty persistence state is retained for save/unload.

This order is more important than individual file names. If a feature needs to
move an entity, spawn an item, play a sound, mutate a block, or remove itself,
the mutation starts in host-owned state and presentation follows.

## Chunk Status And Ticking

Use the existing native chunk-ticket backbone:

- `mclone-server::distance_manager` owns ticket positions and active levels.
- `mclone-server::holder` owns `ChunkHolder` and `FullChunkStatus`.
- `mclone-server::scheduler` reports `block_ticking_chunks` and
  `entity_ticking_chunks`.

Entity activity should follow the vanilla-shaped mapping:

| Full chunk status | Entity visibility | Ordinary entity ticking |
|---|---|---|
| `Inaccessible` | hidden | no |
| `Border` | tracked/queryable | no |
| `Ticking` | tracked/queryable | no |
| `EntityTicking` | tracked/queryable | yes |

AI goals must not perform their own player-distance chunk gating. A goal can
ask whether it can run for the current mob state, but the entity runtime decides
whether the mob receives a tick at all.

## Tracking And Protocol

Tracking is separate from ticking. A tracked entity can be visible and
queryable while not ticking. This matters for loaded boundary chunks, remote
clients, and future persistence.

Entity protocol messages should stay separate from chunk block snapshots:

- `entity_snapshot`: first visible state for a tracked entity
- `entity_update`: authoritative position/rotation/tracked-data delta
- `entity_remove`: explicit untrack or removal

Chunk mesh rebuilds should not be required because a mob moved. Entity
tracking should be filterable per session by chunk interest, tracking range,
and future revision policy.

## Mob AI Stack

The AI stack should mirror vanilla layering:

- `GoalSelector` owns priority, flags, replacement, start/stop, and running
  goal ticks.
- Individual goals decide `canUse`, `canContinueToUse`, `start`, `stop`, and
  `tick`.
- Controls (`MoveControl`, `LookControl`, `JumpControl`) turn intent into
  per-tick body/look/jump changes.
- Navigation owns paths and waypoints, not the goals themselves.
- World queries, pathfinding, collision, and lighting are shared services
  consumed by goals/navigation/spawning.

For the first passive animals, it is acceptable to port a narrow goal subset:
random stroll, look at player, and random look around. It is not acceptable to
hard-code animal movement directly in the renderer or app shell.

## Spawning And Despawn

Spawning is its own subsystem, not a side effect of rendering assets.

Generation-time original mobs and live natural spawning are distinct paths.
Natural spawning depends on player-distance spawnable chunks, mob category
caps, category counts, biome spawn lists, gamerules/server flags, light,
collision, placement predicates, player/spawn distance exclusions, and chunk
entity-ticking status.

Despawn rules are also foundational. Passive animals usually feel persistent
because vanilla `Animal.removeWhenFarAway(...)` returns false, but killed
animals are not seed-respawned decorations. Replacement animals arrive only
through normal spawning rules.

Do not add a feature that assumes killed generation-time animals can be restored
from world seed. Once an entity is inserted into runtime state, it is normal
persisted entity state.

## Persistence

Entity records are separate logical records from block chunk snapshots. The
physical storage adapter may differ from Java's `entities/*.mca` files, but the
logical model should stay chunk-addressed and section-owned while loaded.

Minimum durable facts:

- runtime id and UUID-equivalent stable identity
- entity type
- position, rotation, velocity when simulated
- owning chunk/section
- alive/removed state and removal reason
- persistence-required/custom-persistence flags
- type/category metadata needed for caps and despawn
- type-specific data for rendered or simulated species

Persistence can be staged after initial cow/chicken behavior, but new entity
code should carry state in a shape that can be serialized without renderer
handles or platform objects.

## Asset-Lab Figure Integration

Asset-lab animals are presentation assets. Runtime integration still starts
from authoritative entity type and state.

The integration path should be:

1. Export/register the figure asset in the shared asset registry.
2. Map an authoritative `EntityKind` plus optional type-specific presentation
   data to an `ActorPresentation`.
3. Let `mclone-render-session` convert that presentation to `ActorInstance`.
4. Let `mclone-render` draw the figure/animation using shared actor resources.

No asset-lab example should become a private desktop-only spawn path. If an
animal is visible in desktop flat, the same authoritative state should be able
to reach web, Android, and XR once those lanes adopt the presentation mapping.

## First Passive-Mob Direction

Use cow to harden the reusable entity/mob skeleton because the current native
runtime already has a starter cow and cow rendering path. Then use chicken as
the first species-specific follow-up because Java `Chicken` exercises the same
passive goal stack plus compact unique state: flap, flap speed, fall-slowing
delta movement, egg timer, chicken-jockey flag, and no fall damage.

The narrow first target is not "all animals." It is:

- a non-God-module entity runtime split
- status-gated entity ticking
- a shared goal-selector foundation
- cow random-stroll/look behavior through shared mob modules
- chicken asset registration and species-specific state after the cow skeleton
  is proven
