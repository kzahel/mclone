# 118 - Entity Runtime And Passive Mob Bringup

## Goal

Bring the current starter passive entity path toward the durable entity
architecture in [`../entity-architecture.md`](../entity-architecture.md),
without turning `mclone-server/src/entities.rs` into a God module.

This tactical is a first implementation tracker. It is not an all-creatures
scope. The first visible behavior target is cow; chicken follows once the shared
entity/mob/goal skeleton is in place.

Workstream: native Rust shared server/runtime, desktop validation first.

Status: Slices 0-4 landed. Slice 5 asset promotion is landed, with
chicken-specific ticking still pending. Slice 5A landed shared block collision,
the debug passive showcase toggle, and first server mob gravity/collision.
Slice 5B landed server-owned `GroundPathNavigation`, a terrain-MVP
`WalkNodeEvaluator` subset, and collision-aware `LandRandomPos` /
`DefaultRandomPos` target selection. Slice 5C landed the immediate path service
boundary and heap-backed A* core. Slice 5D landed one-block step-up path
expansion and the first `JumpControl` scaffold. The starter passive path is now
an explicit debug passive showcase, enabled by default, while natural spawning
remains future work.

Pathfinding direction: preserve the Minecraft layering (`Goal` ->
`PathNavigation` -> path service -> `PathFinder` / `NodeEvaluator` ->
`MoveControl`) while keeping execution policy replaceable. The immediate
native path service should start synchronous and host-thread local with
Minecraft-shaped bounds. Later performance slices can add fixed node budgets,
wall-time budgets, priorities, deferred results, or worker execution behind the
same path request/result boundary.

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
- `native/crates/mclone-blocks/src/lib.rs`
- `native/crates/mclone-protocol/src/lib.rs`
- `native/crates/mclone-client/src/actor.rs`
- `native/crates/mclone-render-session/src/lib.rs`
- `native/crates/mclone-render/src/asset_lab_figure.rs`
- `native/crates/mclone-render/src/actor_assets.rs`

## Current Baseline

The current native code has a narrow server-owned passive entity path:

- `EntityKind` includes cow and chicken in protocol.
- The server has an explicit debug passive showcase path, enabled by default,
  that spawns registered passive mobs near the first safe spawn.
- Entity snapshots/updates/removes already flow to clients.
- Entity ticks currently advance age only in `entity_ticking_chunks`.
- Cow has the vanilla-shaped model path; chicken now maps to the promoted
  asset-lab figure.
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

Partial landed notes:

- Exported `tools/asset-lab/examples/chicken/figure.ts` to
  `assets/mclone/figures/chicken.figure.json`.
- Added `mclone:chicken` figure id/path registration in `mclone-assets`.
- Extended the Rust asset-lab figure compiler to accept exported
  `sphere` / `capsule` / `cylinder` primitives as cuboid bounds for the current
  actor renderer. True non-box primitive mesh rendering remains follow-up work.
- Added chicken to the first-party actor figure set loaded by `mclone-render`.
- Mapped authoritative `EntityKind::Chicken` presentations to the chicken
  figure in `mclone-render-session`, with entity dimensions preserved.
- Added actor review coverage so the first-party review sheet includes chicken.
- Verified with
  `cargo test --manifest-path native/Cargo.toml -p mclone-assets -p mclone-render -p mclone-render-session`.
- Verified with `cargo test --manifest-path native/Cargo.toml`.
- Rendered and inspected `/tmp/mclone-chicken-actor-review.png` with 3 views,
  9 actors, and 9 drawn actors.

Still pending:

- Chicken-specific flap/egg/jockey runtime state.
- Java `Chicken.aiStep()` ticking for state that does not require item entities
  or sounds.
- Protocol/client presentation review for any chicken-specific animation data
  that cannot be derived from ordinary entity movement.

## Slice 5A - Shared Ground Collision And Debug Passive Showcase (Landed)

Purpose: make the always-nearby animal behavior explicit and remove the first
floating-mob shortcut without pretending full path navigation is done.

Implementation sketch:

- Introduce a shared block/collision owner below both client and server.
- Route local player AABB movement and server mob AABB movement through the
  same collision clipping primitive.
- Replace the implicit single starter cow with a named debug passive showcase
  setting that defaults on and can be disabled for spawn-parity testing.
- Spawn every registered passive mob kind in the showcase path so future
  animals have one metadata registration point.
- Add server mob vertical delta movement and gravity/drag so mobs fall when
  unsupported and derive `on_ground` from collision.

Landed notes:

- Added `native/crates/mclone-blocks/` for terrain-MVP block facts,
  outline/collision shapes, and Java-shaped AABB movement clipping.
- Replaced the client-local block facts/shapes implementation with
  `mclone-blocks` re-exports and routed local player movement through the
  shared collision primitive.
- Added `--debug-passive-showcase true|false` and
  `debugPassiveShowcase=true|false` to shared startup options, defaulting to
  `true`.
- Replaced the implicit starter cow with `PASSIVE_MOB_KINDS`-driven debug
  passive showcase spawning. Cow and chicken now spawn near the initial safe
  spawn when enabled.
- Passed scheduler-backed block lookups into entity ticks and added server mob
  gravity/collision movement using entity dimensions and shared AABB clipping.
- Added tests for enabled/disabled showcase behavior and unsupported passive
  mobs falling instead of preserving their original spawn Y.
- Verified with `cargo test --manifest-path native/Cargo.toml -p mclone-server debug_passive_showcase`.
- Verified full native workspace compile with
  `cargo test --manifest-path native/Cargo.toml --workspace --no-run`.

Still pending:

- Full A* `PathFinder` and `WalkNodeEvaluator` neighbor expansion over loaded
  world collision.
- One-block step-up / `maxUpStep`, jump control, timeout-based stuck
  detection, and `LivingEntity.travel(...)` parity.
- A visible desktop capture of cow/chicken walking over uneven terrain after
  path navigation lands.

## Slice 5B - Ground Navigation And Land Random Targets (Landed)

Purpose: move passive stroll behavior through a shared navigation owner and
terrain-aware random target selection instead of direct raw `MoveControl`
targets.

Implementation sketch:

- Add `entity/mob/navigation/` with path ownership, ground navigation, a
  terrain-MVP `WalkNodeEvaluator` subset, and random land target helpers.
- Give `MobGoalContext` a short-lived world block lookup so goals can perform
  Java-shaped terrain checks during `canUse`.
- Port the `RandomStrollGoal` / `WaterAvoidingRandomStrollGoal` path boundary:
  goals choose a target, navigation owns progress, and `MoveControl` receives
  per-tick waypoints.
- Reject unsupported random land targets so passive mobs do not intentionally
  stroll toward floating air.
- Keep full A* pathfinding and step-up out of this slice, but preserve their
  module boundary.

Landed notes:

- Added `navigation/mod.rs`, `path.rs`, `walk_node_evaluator.rs`, and
  `random_pos.rs` under `entity/mob/`.
- Converted the mob goal selector to use `MobGoalContext<'_>` directly so
  goal `canUse` checks can query the active server terrain lookup without
  storing platform or scheduler state in goals.
- Added `GroundPathNavigation` path state, waypoint advancement, Java-shaped
  passive waypoint centering, stable-destination checks, and 100-tick
  distance-based stuck detection.
- Added a terrain-MVP `WalkNodeEvaluator` subset for open, walkable, blocked,
  water, and lava path types backed by `mclone-blocks`.
- Added collision-aware `LandRandomPos` / `DefaultRandomPos` helpers that use
  Java random-direction attempts, stable floor checks, water rejection, and
  pathfinding malus checks.
- Updated cow random stroll to call those helpers and continue while
  navigation is in progress.
- Added a lower-floor regression proving navigation plus server collision lets
  a cow descend to a lower supported block instead of preserving its old Y.
- Verified with `cargo test --manifest-path native/Cargo.toml -p mclone-server`.

Still pending:

- Full Java `WalkNodeEvaluator.getNeighbors(...)` parity for doors, fences,
  rails, trapdoors, water, fall-depth limits, and collision-cache checks.
- Attribute/metadata ownership for `maxUpStep` and jump modifiers instead of
  local mob constants.
- Timeout-cached-node stuck detection and path recomputation timing.
- `LivingEntity.travel(...)` parity for friction, fluids, ladders, and jump
  movement.
- Visible desktop capture of showcase animals on uneven terrain.

## Slice 5C - Immediate Path Service And A* Core (Landed)

Purpose: add the Minecraft-shaped path service and A* core without committing
to synchronous unbounded pathfinding as the permanent execution policy.

Implementation sketch:

- Add `PathService` request/result structs under `entity/mob/navigation/`.
- Add `PathFinder` with A* search, open-set heap, per-search visited-node cap,
  reach range, and best-partial-path fallback.
- Expand `WalkNodeEvaluator` from classification-only into the first
  terrain-MVP neighbor generator.
- Route `GroundPathNavigation::createPath` through the path service instead of
  constructing a one-node path directly.
- Keep path execution immediate for this slice; do not add worker threads or
  deferred results yet.

Done when:

- Navigation can receive a multi-node path through the same boundary future
  budgeted pathfinding will use.
- Passive goals remain unaware of pathfinding execution policy.
- Tests cover bounded search, obstacle routing, target fallback, and unchanged
  lower-floor descent behavior.

Landed notes:

- Added `path_service.rs` with an immediate host-thread path service and
  `PathRequest` boundary. This is intentionally synchronous today, but goals
  and navigation now depend on request/result shape rather than a direct
  pathfinder call.
- Added `path_finder.rs` with heap-backed A* search, Java-shaped heuristic
  fudge, follow-range-derived visited-node cap, reach range, and best partial
  path fallback.
- Expanded `GroundPath` to track reachability and node lists.
- Expanded `WalkNodeEvaluator` from classification-only into the first
  terrain-MVP neighbor generator with start-node selection, body clearance,
  cardinal/diagonal neighbors, one-block drops, stable-floor checks, and malus
  filtering.
- Routed `GroundPathNavigation::create_path` through the path service using
  authoritative mob width/height and pathfinding malus from `MobGoalContext`.
- Updated the lower-floor descent regression for Java-shaped reach range: the
  cow may stop adjacent to the requested block, but must still descend through
  server collision once support is gone.
- Verified with `cargo test --manifest-path native/Cargo.toml -p mclone-server
  entity::mob::navigation` and `cargo test --manifest-path native/Cargo.toml
  -p mclone-server entity::mob`, plus the full `cargo test --manifest-path
  native/Cargo.toml` workspace gate.

## Slice 5D - One-Block Step-Up And Jump Control (Landed)

Purpose: let ground navigation intentionally plan and physically follow a
simple one-block ledge without bypassing the Minecraft-shaped
`NodeEvaluator` / `MoveControl` / `JumpControl` layering.

Implementation sketch:

- Extend the terrain-MVP `WalkNodeEvaluator` neighbor generator to try one
  accepted node above the horizontal candidate before falling back to a drop.
- Preserve Java's floor-height guard for step-up candidates: reject elevated
  nodes whose floor is more than 1.125 blocks above the current floor.
- Keep body-clearance and stable-floor checks on elevated nodes.
- Add `JumpControl` as a one-tick pulse control and let `MoveControl` request
  a jump when the wanted waypoint is above `maxUpStep` and close horizontally.
- Apply the default Java `LivingEntity` jump impulse in server mob movement
  before collision clipping.

Done when:

- `PathFinder` can return a path containing an elevated one-block waypoint.
- A passive cow can navigate onto a one-block ledge through navigation,
  move-control jump request, jump-control pulse, gravity, and server collision.
- Two-block columns still route around rather than becoming invalid step-up
  shortcuts.

Landed notes:

- Added `WalkNodeEvaluator` step-up expansion with headroom and floor-height
  checks, plus diagonal validation that rejects diagonals relying on stepped-up
  cardinal neighbors.
- Refactored `MoveControl` toward the Java shape by separating wanted
  position/speed from operation state and adding a `Jumping` operation.
- Added `JumpControl` and wired it through `MobRuntimeState` /
  `MobGoalContext`.
- Applied the default `0.42` jump impulse when jump control pulses while the
  mob is on ground.
- Added tests for elevated neighbor generation, low-ceiling rejection,
  one-block ledge pathfinding, two-block obstacle routing, one-shot jump
  pulses, move-control jump requests, and cow movement onto a ledge.
- Verified with `cargo test --manifest-path native/Cargo.toml -p mclone-server
  entity::mob`, `cargo test --manifest-path native/Cargo.toml -p
  mclone-server`, and the full `cargo test --manifest-path native/Cargo.toml`
  workspace gate.

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

- debug passive showcase cow/chicken in an entity-ticking chunk
- same showcase entities no longer ticking after chunk demotion
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
