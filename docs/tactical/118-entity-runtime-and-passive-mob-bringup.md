# 118 - Entity Runtime And Passive Mob Bringup

## Goal

Bring the current starter passive entity path toward the durable entity
architecture in [`../entity-architecture.md`](../entity-architecture.md),
without turning `mclone-server/src/entities.rs` into a God module.

This tactical is a first implementation tracker. It is not an all-creatures
scope. The first visible behavior target is cow; chicken follows once the shared
entity/mob/goal skeleton is in place.

Workstream: native Rust shared server/runtime, desktop validation first.

Status: Slices 0-4 landed. The starter passive entity path is split under
`native/crates/mclone-server/src/entity/`, and entity visibility/tick-list
boundaries, passive cow/chicken metadata, a standalone goal selector, and a
first cow passive AI/control path now exist before chicken-specific work.

## Non-Negotiable Constraints

- Entity lifecycle stays authoritative-host owned.
- Entity ticking is driven by chunk `FullChunkStatus` / `ENTITY_TICKING`, not
  by renderer visibility or ad hoc player-distance checks inside AI goals.
- Tracking and ticking remain separate concepts.
- Entity updates remain protocol messages separate from chunk snapshots.
- New behavior must move toward shared modules under `mclone-server`, not app
  crates.
- Asset-lab animals are presentation assets; they do not own spawning, ticking,
  despawn, or persistence.
- Reference Minecraft Java 1.17.1 source must be read before porting vanilla
  behavior.

## Reference Files

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/PersistentEntitySectionManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntityTickList.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Animal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Cow.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Chicken.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/Goal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/WrappedGoal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/GoalSelector.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/RandomStrollGoal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/WaterAvoidingRandomStrollGoal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/LookAtPlayerGoal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/RandomLookAroundGoal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/MoveControl.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/LookControl.java`

Existing native modules to consume rather than duplicate:

- `native/crates/mclone-server/src/distance_manager.rs`
- `native/crates/mclone-server/src/holder.rs`
- `native/crates/mclone-server/src/scheduler.rs`
- `native/crates/mclone-server/src/integrated.rs`
- `native/crates/mclone-server/src/entity/mod.rs`
- `native/crates/mclone-protocol/src/lib.rs`
- `native/crates/mclone-client/src/actor.rs`
- `native/crates/mclone-render-session/src/lib.rs`
- `native/crates/mclone-render/src/asset_lab_figure.rs`
- `native/crates/mclone-render/src/actor_assets.rs`

## Current Baseline

The current native code has a narrow server-owned passive entity path:

- `EntityKind` includes cow and chicken in protocol.
- The server has a starter passive cow path.
- Entity snapshots/updates/removes already flow to clients.
- Entity ticks currently advance age only in `entity_ticking_chunks`.
- Cow has a real render model path; chicken currently maps to a placeholder.
- Asset-lab animal examples exist under `tools/asset-lab/examples/`, but only
  player/upright-bear figure JSON is registered as runtime first-party figure
  assets today.

This tactical should preserve working behavior while splitting ownership and
adding reusable foundations.

## Slice 0 - Module Split Without Behavior Change (Landed)

Purpose: stop the current entity path from becoming a larger staging file.

Implementation sketch:

- Create `native/crates/mclone-server/src/entity/`.
- Move plain state/snapshot/update helpers into `entity/state.rs`.
- Move per-player entity routing into `entity/tracking.rs`.
- Move status-gated tick entrypoint into `entity/runtime.rs` or `entity/mod.rs`.
- Keep old public integration points stable through a facade while
  `integrated.rs` is migrated.
- Keep tests equivalent to the existing starter-passive and tracking tests.

Done when:

- Existing starter cow snapshot/update/remove tests still pass.
- `entities.rs` is deleted or reduced to a compatibility facade.
- No AI, spawning, or asset work is mixed into this slice.

Landed notes:

- Added `entity/mod.rs` as the server entity facade.
- Moved authoritative state and snapshot/update conversion into
  `entity/state.rs`.
- Moved starter passive storage, id allocation, and stationary ticking into
  `entity/store.rs`.
- Moved per-player snapshot/update/remove routing into `entity/tracking.rs`.
- Deleted the old `mclone-server/src/entities.rs` staging module.
- Preserved the existing starter cow and entity tracking behavior.
- Verified with `cargo test --manifest-path native/Cargo.toml -p mclone-server`.

## Slice 1 - Visibility And Tick-List Foundation (Landed)

Purpose: make chunk-status-driven entity activity explicit before adding real
AI.

Implementation sketch:

- Add `entity/visibility.rs` with a Java-shaped mapping from
  `FullChunkStatus` to entity visibility: hidden, tracked, ticking.
- Add or isolate `entity/tick_list.rs` for stable entity iteration semantics.
- Reconcile runtime entity ticking from scheduler-provided
  `entity_ticking_chunks`.
- Add tests for:
  - entity in `Border` is tracked but not ticking
  - entity in `Ticking` is tracked but not ticking
  - entity in `EntityTicking` advances age/tick state
  - demotion removes ordinary mobs from the tick set without removing stored
    entity state

Done when:

- No entity tick path performs its own duplicate player-distance chunk check.
- The runtime exposes diagnostics for stored/tracked/ticking entity counts.

Landed notes:

- Added `entity/visibility.rs` with a Java-shaped `FullChunkStatus` to
  `Hidden` / `Tracked` / `Ticking` mapping.
- Added `entity/tick_list.rs` as the explicit ordinary entity ticking set.
- `ServerEntityStore::tick_stationary(...)` now reconciles the tick list from
  scheduler-provided `entity_ticking_chunks` before advancing entity age.
- Added store/tracking diagnostics for stored, tracked, observer-pair, and
  ticking entity counts.
- Added tests for visibility mapping, tick-list reconciliation, chunk demotion,
  and diagnostics.

## Slice 2 - Metadata And Passive Mob Runtime State (Landed)

Purpose: give cow/chicken reusable type metadata instead of ad hoc dimensions
and behavior branches.

Implementation sketch:

- Add `entity/metadata.rs` for type facts:
  - entity kind
  - category
  - dimensions
  - standing eye height
  - movement speed
  - tracking range placeholder
- Add `entity/mob/` for shared mob state:
  - no-action time
  - on-ground state
  - yaw/body/head rotations as needed by presentation
  - pathfinding malus table
  - deterministic per-entity runtime random source
- Seed cow metadata from Java `Cow`.
- Seed chicken metadata from Java `Chicken`, but do not tick chicken-specific
  flap/egg behavior until Slice 5.

Done when:

- Starter cow dimensions come from metadata, not local constants.
- Chicken dimensions can be snapshotted from metadata even if still spawned
  only in tests.

Landed notes:

- Added `entity/metadata.rs` with Java 1.17.1 cow/chicken category,
  dimensions, standing eye height, movement speed, and client tracking range
  facts.
- Added `entity/mob/` runtime state for no-action time, ground state,
  body/head yaw, pathfinding malus, movement speed, and deterministic
  per-entity random source.
- Starter cow construction now flows through metadata-driven passive mob
  insertion instead of local width/height constants.
- Chicken can be inserted in tests through the same passive mob path and
  snapshotted with metadata dimensions.
- Added tests for cow/chicken metadata, chicken water pathfinding malus,
  deterministic mob random seeding, mob rotation sync, and passive snapshots.

## Slice 3 - Goal Selector Foundation (Landed)

Purpose: port the shared AI scheduling surface before adding more species.

Implementation sketch:

- Add `entity/mob/goals/`:
  - `Goal`
  - `WrappedGoal`
  - `GoalSelector`
  - goal flags: move, look, jump, target
- Preserve Java semantics:
  - priority ordering
  - interruptibility
  - locked flags
  - disabled flags
  - cleanup/update/tick phases
- Add synthetic tests for:
  - lower priority number replaces a running interruptible goal
  - locked flags prevent incompatible goals
  - stop/start/tick calls happen in the expected order
  - disabled flags stop running goals and prevent new starts

Done when:

- The selector is unit-tested independently of cow/chicken.
- No animal-specific logic is embedded in the selector.

Landed notes:

- Added `entity/mob/goals/` with `Goal`, `WrappedGoal`, `GoalSelector`, and
  `Move` / `Look` / `Jump` / `Target` control flags.
- Preserved the Java cleanup, update, and tick phase ordering.
- Preserved Java-style replacement semantics: lower numeric priority can
  replace a running goal only when the current lock holder is interruptible.
- Added disabled control flag handling that stops running goals and prevents
  new starts.
- Added synthetic selector tests independent of cow/chicken for priority
  replacement, non-interruptible locks, same-priority blocking, phase order,
  disabled flags, and independent flag coexistence.

## Slice 4 - Cow Baseline Behavior (Landed)

Purpose: prove the reusable mob stack on the existing starter cow before adding
species-specific chicken state.

Implementation sketch:

- Read Java `Cow`, `Animal`, `Mob`, `RandomStrollGoal`,
  `WaterAvoidingRandomStrollGoal`, `LookAtPlayerGoal`, and
  `RandomLookAroundGoal`.
- Add narrow shared goals:
  - water-avoiding random stroll
  - look at nearest player
  - random look around
- Add enough `MoveControl` / `LookControl` scaffolding for visible cow behavior.
- Keep pathfinding shallow if needed, but route movement intent through the
  shared mob/navigation boundary instead of direct cow-specific movement.
- Publish authoritative `entity_update` when position/rotation changes.

Deferred from this slice:

- breeding
- tempting
- panic/damage
- full `LivingEntity.travel(...)`
- natural spawning
- persistence

Done when:

- Starter cow only ticks in `ENTITY_TICKING` chunks.
- Cow can choose idle/look/stroll goals through `GoalSelector`.
- Client interpolation sees authoritative cow updates.
- A desktop headless screenshot or small movement smoke captures the cow after
  behavior is active.

Landed notes:

- Converted `GoalSelector` to a typed-context selector so real goals can read
  mob/player/control context without embedding animal logic in the selector.
- Added `entity/mob/control.rs` with narrow `MoveControl` and `LookControl`
  scaffolding based on Java `MoveControl` / `LookControl` rotation semantics.
- Added `entity/mob/goals/passive.rs` with cow's first passive goal subset:
  water-avoiding random stroll, look at nearest player, and random look around
  at the Java cow priorities 5/6/7.
- Threaded nearby player positions into entity ticking from `IntegratedServer`
  while keeping ticking gated by scheduler-provided `ENTITY_TICKING` chunks.
- Kept full `PathNavigation`, collision-aware `LandRandomPos`, and
  `LivingEntity.travel(...)` out of this slice; the temporary stroll target is
  local and deterministic but still flows through the shared move-control
  boundary.
- Added tests for selector context use, move/look controls, look-at-player
  target application, cow goal registration, visibility-gated ticking, and a
  small server-side cow movement smoke.
- Verified with `cargo test --manifest-path native/Cargo.toml -p mclone-server`.

## Slice 5 - Chicken Asset And Species State

Purpose: use chicken to prove asset-lab figure promotion and compact
type-specific ticking.

Implementation sketch:

- Export/register `tools/asset-lab/examples/chicken/figure.ts` as a first-party
  runtime actor figure.
- Add a chicken figure id/path to `mclone-assets`.
- Load the chicken figure in `mclone-render` actor assets.
- Map authoritative `EntityKind::Chicken` to the chicken figure in
  `mclone-render-session`.
- Add chicken-specific state:
  - `flap`
  - `flap_speed`
  - previous flap fields needed for interpolation/animation
  - `flapping`
  - `egg_time`
  - `is_chicken_jockey`
- Port the Java `Chicken.aiStep()` state updates that do not require item
  entities or sounds.

Deferred from this slice:

- egg item entity spawning
- chicken egg sound
- chicken jockey passenger behavior
- no-fall-damage side effects beyond stored flags
- full falling `deltaMovement` behavior if the shared movement stack is not
  ready

Done when:

- A spawned chicken uses the asset-lab figure instead of the placeholder.
- Chicken flap/egg timer state advances only while entity-ticking.
- Entity protocol can carry any chicken presentation data required by the
  renderer, or the deferred data gap is documented explicitly.
- Desktop screenshot validation shows the chicken figure in-world.

## Slice 6 - Spawning Skeleton, Not Full Natural Spawning

Purpose: prepare the non-optional spawning boundary without pretending live
natural spawning is done.

Implementation sketch:

- Add `entity/spawning/` modules:
  - `mob_category.rs`
  - `biome_tables.rs`
  - `placements.rs`
  - `spawn_state.rs`
  - `natural.rs` placeholder
- Port passive category metadata and farm-animal spawn table facts.
- Add unit tests for mob-cap formula and `CREATURE` 400-tick interval gating.
- Keep live spawn attempts disabled unless all required world/player/chunk
  inputs are present.

Done when:

- Future natural spawning has a home.
- Cow/chicken behavior does not need to know about spawn caps or player-distance
  spawnable chunks.
- The tactical clearly lists remaining blockers for real natural spawning:
  player-distance spawnable chunk tracker, live category counts, placement
  predicates, brightness checks, collision checks, gamerules/server flags,
  despawn, and persistence.

## Validation

Minimum gates for code slices:

- `cargo test --manifest-path native/Cargo.toml`
- targeted server entity tests for each slice
- protocol round-trip tests when adding entity data
- desktop validation first for visible behavior

When a slice produces pixels, save screenshots under `/tmp`, inspect them, and
do not write screenshots into the repo.

Suggested visible checks:

- starter cow in an entity-ticking chunk
- same cow no longer ticking after chunk demotion
- cow stroll/look update path visible in a short smoke
- chicken figure review sheet or in-world screenshot after registration

## Open Follow-Ups

- Durable entity persistence adapter and save/unload dirtying.
- Full `Entity.move(...)` / `LivingEntity.travel(...)` parity.
- Real pathfinding over loaded world collision.
- Live natural spawning for `CREATURE`.
- Despawn rules for passive animals and later hostile mobs.
- Item entities and chicken egg side effects.
- Sounds for passive mobs.
- Data watcher / tracked data equivalent for richer entity presentation.
- Web/Android/XR screenshot coverage after desktop validation.
