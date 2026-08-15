# Entity Architecture

Durable architecture notes for the Rust entity, mob, spawning, and presentation
stack.

This document is intentionally stricter than a roadmap. The systems below are
not optional later polish. They are the foundation that every entity feature has
to fit through, even when a first slice implements only one cow or one chicken.
A feature may leave behavior stubbed, but it should not bypass the ownership,
chunk-status, tracking, persistence, or shared module boundaries.

The Minecraft Java 1.17.1 source remains comparative ownership research. Read
it only when a scoped task asks about Minecraft or modifies retained
reference-shaped code; original entity work follows Mclone's ecology and
gameplay topics. Useful reference entry points include:

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
genuinely platform-specific. Shared behavior belongs in shared crates and
server/runtime modules, and validation follows the affected contracts.

Do not model creatures as decorative render assets. A cow, chicken, or mallard
placed in the world is an entity with authoritative identity, chunk/section
ownership, tracking, tick eligibility, and eventual persistence.

## Current Status

As of 2026-08-11, the native runtime has both an explicit debug passive
showcase path and a bounded live natural-spawn path. The showcase remains
enabled by default through shared startup/server options
(`--debug-passive-showcase true|false`, `debugPassiveShowcase=true|false`).
It deliberately places registered passive mobs near the initial safe spawn so
new animal assets are visible while the entity stack is being built; it is not
natural spawning.

Natural spawning currently covers the `CREATURE` cadence/cap, implemented
cow/chicken entries, and Mclone-only mallard wetland flocks. Candidate chunks
must be entity-ticking and in the player-distance set. Surface blocks,
brightness, collision, and habitat biome for cow/chicken all come from the
same published generated chunk; the mallard wetland sample reads its canonical
block state from that chunk instead.
Persistent worlds wait for entity-chunk load completion and save a naturally
spawned animal immediately; null-store worlds label the same runtime path
volatile and discard those animals at full unload. Tactical
[`277`](tactical/277-habitat-driven-creature-ecology-foundation.md) and
[`topics/habitat-driven-creature-ecology.md`](topics/habitat-driven-creature-ecology.md)
own the foundation, first original ecology loop, and continuing product
direction.

The first shared ground/collision scaffold is also in place:

- `mclone-blocks` owns terrain-MVP block facts, outline/collision shapes, and
  Java-shaped AABB movement clipping below both client and server.
- Local player collision and server passive-mob movement consume that shared
  block collision path.
- Passive mobs carry vertical delta movement, resolve movement against server
  world blocks, derive `on_ground` from vertical collision, and fall when they
  leave support.
- Server mobs now carry a `GroundPathNavigation` owner. Passive stroll goals
  choose targets through collision-aware `LandRandomPos` / `DefaultRandomPos`
  helpers, reject unsupported and water/malus targets, and let navigation feed
  `MoveControl` waypoints.
- The native `WalkNodeEvaluator` subset classifies terrain-MVP blocks as
  open, walkable, blocked, water, or lava using shared block facts and
  collision shapes.
- `GroundPathNavigation` now routes through an immediate host-thread path
  service and heap-backed A* `PathFinder`. The first `WalkNodeEvaluator`
  neighbor generator supports body clearance, cardinal/diagonal ground
  neighbors, one-block drops, malus filtering, per-search node bounds, reach
  range, and best-partial-path fallback.
- Ground navigation also has a first one-block step-up and `JumpControl`
  scaffold: the evaluator can emit elevated ground nodes with headroom, and
  server mob movement can request and apply the default Java jump impulse to
  follow those waypoints.
- Simple Java movement attribute facts now live in `mob::attributes`.
  Movement speed, follow range, `maxUpStep`, and jump power flow from mob
  runtime state into navigation, path requests, `MoveControl`, and
  `JumpControl` instead of being local pathfinding constants.
- Server mob ground travel now uses the first Java-shaped
  `LivingEntity.travel(...)` subset: `MoveControl` sets yaw and speed intent,
  forward input is damped before travel, horizontal delta movement carries
  across ticks, shared block friction/speed/jump facts feed travel and jump
  impulses, and Java's stepped-collision candidate is tried before accepting a
  clipped horizontal move. The terrain-MVP block facts currently cover Java
  defaults plus generated ice/packed-ice friction.
- `GroundPathNavigation` now owns Java-shaped target/reach bookkeeping,
  delayed path recomputation gating, cached-node timeout detection, and the
  falling-past-waypoint advance path. Timeout elapsed time is counted from
  deterministic navigation ticks rather than wall clock time so host-thread
  execution remains reproducible and later scheduling budgets can degrade
  behavior without depending on frame duration.
- Server block mutations now notify mob navigation before entity ticks. The
  notification uses Java's `PathNavigation.recomputePath(BlockPos)` shape:
  changed blocks only mark paths for recompute when they are near the remaining
  path, and the actual rebuild still flows through navigation and the path
  service. Java `trimPath()` scaffolding is also present, with cauldron
  behavior waiting on cauldron block facts.
- Chicken and mallard now have compact server species state behind the shared
  mob runtime. Chicken's supported passive goal tail, flap/egg timer, and
  airborne downward damping follow the current Java-shaped path. The mallard
  owns durable egg, feather, call, age, and parentage timers/state; shallow
  water adds species-scoped buoyancy/paddling and flock steering before the
  shared passive goal tail. A separate persistent mallard-nest entity owns its
  egg, parents, incubation, habitat/attendance checks, and single hatch.
  Chicken egg sounds and jockey passenger behavior remain follow-ups.
- Passive mob head yaw and body yaw are tracked separately inside the server
  mob runtime. Until a head-yaw presentation lane exists, look goals must not
  fake visibility by mutating authoritative body yaw; body yaw remains the
  locomotion-facing yaw sent through the current entity update field.

The implementation still lacks several useful movement and presentation
capabilities: fluid and climbable travel, richer attributes/effects, broader
block-path facts, and tracked head/body-yaw presentation. Prior Java comparison
gaps are evidence, not a checklist; natural passive movement is complete when
Mclone's intended creatures and environments are covered.

Pathfinding should keep the useful neutral layering already proven in the
engine. Goals should ask navigation to move;
navigation should ask a path service for a path; `PathFinder` and
`NodeEvaluator` should own the search and terrain graph. The first native path
service may execute synchronously on the host thread with bounded Mclone
limits, but the boundary must remain explicit so later slices can add fixed
node budgets, wall-clock budgets, priorities, deferred results, or worker
execution without rewriting goals.

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
    attributes.rs        movement speed, follow range, maxUpStep, jump power
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
    natural.rs           retained Java-shaped spawning plus profile policy
```

Shared protocol facts remain in `mclone-protocol`. Client interpolation and
actor presentation remain in `mclone-client` / `mclone-render-session`.
Renderer geometry, figure compilation, texture atlases, and animation sampling
remain in `mclone-render` / `mclone-assets`. Those crates consume entity facts;
they do not decide entity lifecycle.

Shared terrain block facts and AABB collision semantics live below both client
and server in `mclone-blocks`. Do not duplicate block collision shape or
movement clipping logic in a client-only or server-only module.

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

Entity activity currently follows this shared mapping:

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

The AI stack uses this layered Mclone contract:

- `GoalSelector` owns priority, flags, replacement, start/stop, and running
  goal ticks.
- Individual goals decide `canUse`, `canContinueToUse`, `start`, `stop`, and
  `tick`.
- Controls (`MoveControl`, `LookControl`, `JumpControl`) turn intent into
  per-tick body/look/jump changes.
- Navigation owns paths and waypoints, not the goals themselves.
- A path service owns path request execution. Today this can be immediate and
  host-thread local; future budgeted/deferred execution should fit behind the
  same request/result contract.
- `PathFinder` owns A* search and per-search bounds.
- `NodeEvaluator` owns start/goal/neighbor generation and terrain path types.
- World queries, pathfinding, collision, and lighting are shared services
  consumed by goals/navigation/spawning.

For the first passive animals, implement only the narrow behavior subset the
creatures need, such as random stroll, looking at a player, and idle looking.
It is not acceptable to hard-code animal movement directly in the renderer or
app shell.

## Spawning And Despawn

Spawning is its own subsystem, not a side effect of rendering assets.

Generation-time original mobs and live natural spawning are distinct paths.
Mclone population and spawning depend on habitat, durable population policy,
player/session interest, light/collision/placement facts, and chunk activity as
defined by the original ecology topics.

The current cow/chicken slice plus Mclone mallard flock path implement part of
that shape. Missing Java group geometry, species, categories, and gamerules are
not product gaps by themselves. Habitat lookup must use the active generated
chunk payload; it must not query a seed-only biome source belonging to another
profile.

Lifecycle/removal rules are also foundational. Materialized Mclone animals are
durable world state rather than seed-respawned decorations; replacement
population follows the documented ecology policy.

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

Chunk-addressed entity records are live for memory, SQLite, and browser
persistence backends. Runtime IDs are reconstructed while
`EntityPersistentId`, pose/motion, and the implemented
cow/chicken/mallard/nest/item payload survive hydration, including mallard
parentage, growth, cooldowns, and nest incubation. Natural animals and placed
nests join that ordinary record immediately in a persistent world; transient
stores use a separately marked volatile set. Mallard observations live in the
separate per-player record, while short-lived tracks remain explicitly
per-world presentation state. Future subtype state must remain serializable
without renderer handles or platform objects.

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
