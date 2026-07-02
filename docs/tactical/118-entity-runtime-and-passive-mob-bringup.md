# 118 - Entity Runtime And Passive Mob Bringup

## Goal

Bring the current starter passive entity path toward the durable entity
architecture in [`../entity-architecture.md`](../entity-architecture.md),
without turning `mclone-server/src/entities.rs` into a God module.

This tactical is a first implementation tracker. It is not an all-creatures
scope. The first visible behavior target is cow; chicken follows once the shared
entity/mob/goal skeleton is in place.

Workstream: native Rust shared server/runtime, desktop validation first.

Status: Slices 0-4 landed. Slice 5 asset promotion is landed, and tactical
[`126`](126-chicken-runtime-behavior.md) landed the first chicken runtime chunk:
supported passive goals, species flap/egg state, and airborne glide damping.
Slice 5A landed shared block collision, the debug passive showcase toggle, and
first server mob gravity/collision. Slice 5B landed server-owned
`GroundPathNavigation`, a terrain-MVP `WalkNodeEvaluator` subset, and
collision-aware `LandRandomPos` / `DefaultRandomPos` target selection. Slice 5C
landed the immediate path service boundary and heap-backed A* core. Slice 5D
landed one-block step-up path expansion and the first `JumpControl` scaffold.
Slice 5E landed mob movement attribute facts for navigation and jumping. Slice
5F landed Minecraft-shaped navigation recompute and timeout state. Slice 5G
landed block-change path recompute triggers and path-trim hooks. The starter
passive path is now an explicit debug passive showcase, enabled by default,
while natural spawning remains live-disabled. Slice 8 landed a bounded
farm-animal biome/placement dry run so biome tables, on-ground animal
predicates, and cow/chicken AABB collision are no longer missing subsystems;
brightness, gamerules, despawn, persistence, and the live executor remain
required before natural spawning creates mobs.

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
- Tactical [`126`](126-chicken-runtime-behavior.md) added chicken species state,
  supported passive goals, Java-shaped airborne glide damping, and an egg timer
  hook without item-entity side effects.

Still pending:

- Protocol/client presentation review for any chicken-specific animation data
  that cannot be derived from ordinary entity movement.
- Egg item entities, egg sound, persisted `EggLayTime`, and
  chicken-jockey/passenger behavior.

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
- Full Java `AttributeMap` / modifier / effect ownership for mutable movement,
  follow range, step, and jump values.
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

## Slice 5E - Mob Attribute Facts For Navigation And Jumping (Landed)

Purpose: move the movement facts used by pathfinding and jump movement into a
shared mob attribute owner before adding more animals or path scheduling
policy.

Implementation sketch:

- Read Java `LivingEntity`, `Mob`, `Cow`, and `Chicken` for the default
  `maxUpStep`, jump impulse, follow range, and species movement speeds.
- Add `entity/mob/attributes.rs` as the narrow current owner for movement
  speed, follow range, `maxUpStep`, and jump power.
- Keep this intentionally smaller than Java's full `AttributeMap` while
  preserving the future route for mutable modifiers and effects.
- Pass follow range and step height through `GroundPathNavigation` and
  `PathRequest` instead of keeping navigation-local defaults.
- Pass movement speed, step height, and jump power from mob attributes into
  `MoveControl` / `JumpControl` application.

Done when:

- Cow and chicken derive movement speeds from metadata and share Java default
  follow range, `maxUpStep`, and jump impulse values.
- Path search bounds continue to derive from follow range.
- Step-up expansion uses the mob's step-height fact.
- Jump impulse application uses the mob's jump-power fact.

Landed notes:

- Added `entity/mob/attributes.rs` with cow/chicken movement speeds plus Java
  defaults for follow range `16.0`, `maxUpStep` `0.6`, and jump power `0.42`.
- Replaced local mob movement constants in `MobRuntimeState` /
  `MobGoalContext` with `MobAttributes`.
- Routed follow range and `maxUpStep` through `GroundPathNavigation` and
  `PathRequest`, keeping the path service boundary ready for future budgeted
  scheduling.
- Routed movement speed, `maxUpStep`, and jump power into `MoveControl` and
  `JumpControl` application.
- Added tests for cow/chicken attribute facts and cow runtime attribute
  accessors.
- Verified with `cargo test --manifest-path native/Cargo.toml -p
  mclone-server entity::mob`, `cargo test --manifest-path native/Cargo.toml
  -p mclone-server`, and the full `cargo test --manifest-path
  native/Cargo.toml` workspace gate.

## Slice 5F - Navigation Recompute And Timeout State (Landed)

Purpose: add the Minecraft-shaped `PathNavigation` bookkeeping that keeps path
following bounded and gives future budgeted/deferred pathfinding a stable
state-machine boundary.

Implementation sketch:

- Read Java `PathNavigation` and `GroundPathNavigation` for target/reach
  storage, delayed recomputation, stuck detection, cached-node timeout, and
  airborne waypoint advancement.
- Keep path execution immediate and host-thread local for this slice.
- Store `targetPos`, `reachRange`, delayed recompute state, and timeout state
  inside `GroundPathNavigation`.
- Let navigation tick rebuild delayed paths through the existing path service
  boundary using mob attributes and pathfinding malus from `MobGoalContext`.
- Count timeout elapsed time from deterministic navigation ticks instead of
  wall-clock time; this intentionally preserves scheduling predictability while
  keeping the Java timeout state shape.

Done when:

- `recomputePath()` requests inside the 20-tick Java gate are delayed and
  consumed by later navigation ticks.
- Stalled mobs can stop a path through cached-node timeout before the older
  100-tick distance stuck check is the only escape.
- Falling mobs can advance past a waypoint when vertically above it in the
  same block column, matching the Java `PathNavigation.tick()` edge case.

Landed notes:

- Added target/reach, delayed recompute, timeout cached-node, timeout timer,
  and timeout limit fields to `GroundPathNavigation`.
- Added deterministic tick-time timeout accounting using 50 ms navigation
  ticks rather than wall clock `Util.getMillis()`.
- Routed mob height, follow range, `maxUpStep`, movement speed, and malus into
  `GroundPathNavigation::tick(...)` so delayed recompute can rebuild paths
  without storing world state in the navigation object.
- Added the Java falling-past-waypoint advancement path for non-ground ticks.
- Added a direction-based next-node shortcut for the current walkable path
  subset.
- Added focused tests for delayed recomputation, cached-node timeout, airborne
  waypoint advancement, and path node introspection.
- Verified with `cargo test --manifest-path native/Cargo.toml -p
  mclone-server entity::mob::navigation`, `cargo test --manifest-path
  native/Cargo.toml -p mclone-server`, and the full `cargo test
  --manifest-path native/Cargo.toml` workspace gate.

## Slice 5G - Block-Change Recompute And Path Trim Hooks (Landed)

Purpose: connect server block mutations to mob path invalidation without
letting block or scheduler code own AI policy.

Implementation sketch:

- Read Java `PathNavigation.recomputePath(BlockPos)` and `trimPath()`.
- Add a navigation method that checks whether a changed block is close enough
  to the remaining path before marking delayed recompute.
- Route direct simulation block changes, falling-block moves, and fluid-tick
  block mutations into `ServerEntityStore` before entity ticks.
- Keep actual path rebuilding inside `GroundPathNavigation::tick(...)` through
  the existing path service boundary.
- Add `GroundPath` node replacement/truncation primitives and a Java-shaped
  `trimPath()` hook. Terrain MVP has no cauldron facts yet, so cauldron trim
  behavior remains a data follow-up.

Done when:

- A far block change does not mark a mob path for recompute.
- A near block change along the remaining path marks the path for delayed
  recompute.
- Fluid-tick mutations expose changed positions to entity navigation.
- Path trim tests cover the cauldron node-raise rule independent of missing
  cauldron block ids.

Landed notes:

- Added `GroundPathNavigation::recompute_path_around(...)` using Java's
  midpoint/radius shape from `PathNavigation.recomputePath(BlockPos)`.
- Added entity-store block-change fan-out and integrated-server notifications
  for direct block changes and fluid-tick mutated positions.
- Extended fluid ticking to return deduplicated mutated block positions
  alongside existing metrics/events.
- Added `GroundPath` node replacement and truncation helpers plus the
  Java-shaped `trim_path` hook.
- Added focused tests for affected-path recompute, remaining-path distance,
  store-level block-change notification, and trim node raising.
- Verified with focused navigation/store tests, `cargo test --manifest-path
  native/Cargo.toml -p mclone-server`, and the full `cargo test
  --manifest-path native/Cargo.toml` workspace gate.

## Slice 6 - Spawning Skeleton, Not Full Natural Spawning

Purpose: prepare the non-optional spawning boundary without pretending live
natural spawning is done.

Status: initial scaffold landed; live natural spawn attempts remain disabled
until the required world/player/chunk inputs are present.

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

Landed notes:

- Added `entity::spawning` modules for mob category facts, biome spawn table
  facts, placement facts, spawn state/caps, and a natural spawn planner.
- Ported Java 1.17.1 `MobCategory` constants, the `NaturalSpawner` `17 * 17`
  mob-cap denominator, and the `CREATURE` 400-tick natural-spawn cadence.
- Recorded vanilla farm-animal spawn table facts, including unsupported
  sheep/pig entries so future passive mobs have the correct table shape.
- Recorded cow/chicken on-ground animal placement facts without implementing
  live placement, brightness, or collision predicates yet.
- Kept live natural spawning blocked by default. The planner reports missing
  blockers for required inputs that are not yet supplied by later slices.
- Verified the scaffold with focused `mclone-server` `entity::spawning` unit
  tests, full `cargo test --manifest-path native/Cargo.toml`, and
  `pnpm native:web:build`.

## Slice 7 - Natural Spawn Input Diagnostics

Purpose: wire the first real natural-spawn inputs into the server report without
creating mobs yet.

Status: landed; diagnostics now supply player-distance spawnable chunks and
live category counts, while live attempts remain disabled.

Implementation sketch:

- Mirror Java's `FixedPlayerDistanceChunkTracker(8)` shape as a `17 * 17`
  player-distance chunk set around accepted player positions.
- Track the subset of entity-ticking chunks that are also close enough to a
  player for natural spawn attempts (`ChunkMap.noPlayersCloseForSpawning`
  style 128-block center distance).
- Count live non-`MISC` entity categories from the authoritative entity store.
- Feed those inputs into the natural-spawn planner and expose a compact
  `NaturalSpawningDiagnostics` report on simulation ticks and runner
  diagnostics.
- Keep live spawning disabled and keep the remaining blockers visible.

Done when:

- A server tick report can show the current player-distance spawnable chunk
  count and eligible entity-ticking spawn chunks.
- A server tick report can show current `CREATURE` count, cap, cadence status,
  and "would attempt if enabled" status.
- The planner no longer treats player-distance chunks or live category counts
  as missing, but still blocks on biome/placement/brightness/collision/gamerule,
  despawn, and persistence inputs.

Landed notes:

- Added Java-shaped natural-spawn chunk diagnostics using the 8-chunk fixed
  player-distance tracker radius and the 128-block close-player filter.
- Added entity-store category counting so debug cows/chickens count toward the
  live `CREATURE` cap and item entities do not.
- Added `NaturalSpawningDiagnostics` to `ServerSimulationTickReport` and
  `ServerRunnerTickDiagnostics`.
- Verified that a dedicated-player simulation tick reports 289
  player-distance chunks, two debug `CREATURE` entities, cap 10, live attempts
  disabled, and the remaining blocker count.
- Verified with `cargo test --manifest-path native/Cargo.toml -p mclone-server
  natural_spawn`, `cargo test --manifest-path native/Cargo.toml -p
  mclone-server`, full `cargo test --manifest-path native/Cargo.toml`, and
  `pnpm native:web:build`.

## Slice 8 - Farm Animal Biome And Placement Dry Run

Purpose: wire the next non-optional natural-spawn predicates into diagnostics
without creating mobs yet.

Status: landed; live natural spawn attempts remain disabled.

Implementation sketch:

- Encode the 1.17.1 farm-animal biome membership for layered overworld biomes
  whose `VanillaBiomes` builder path calls `farmAnimals(...)`, directly or via
  `plainsSpawns(...)` / `defaultSpawns(...)`.
- Implement the cow/chicken natural placement predicate behind the existing
  `placements.rs` boundary:
  - `ON_GROUND`
  - `MOTION_BLOCKING_NO_LEAVES`
  - valid floor and empty feet/head blocks
  - grass block below
  - raw brightness greater than 8
  - entity AABB no-collision using shared block collision shapes
- Add a bounded dry-run diagnostic over eligible entity-ticking chunks. The
  diagnostic samples deterministic surface columns and reports candidate,
  biome-blocked, floor/space/collision-blocked, and missing-brightness counts.
- Keep brightness honest: until the server exposes a raw-brightness sampler,
  otherwise-valid candidates report `MissingBrightness` rather than assuming
  full daylight.

Done when:

- The planner no longer treats biome spawn tables, placement predicates, or
  collision checks as missing for the farm-animal path.
- The server tick report exposes dry-run candidate/blocker counts.
- Live spawning is still blocked by brightness, gamerules/server flags,
  despawn, and persistence, and no entities are created by this dry-run.

Landed notes:

- Added farm-animal biome membership tests for plains/forest/savanna/taiga
  positives and desert/jungle/ocean/badlands negatives.
- Added `check_farm_animal_natural_spawn(...)` with Java-shaped failure
  reasons and cow/chicken dimensions from shared entity metadata.
- Added `dry_run_creature_spawn_eligibility(...)` with a fixed per-tick chunk
  cap and sampled `MOTION_BLOCKING_NO_LEAVES` surface columns.
- Threaded dry-run counters into `NaturalSpawningDiagnostics`.
- The integrated diagnostics now clear biome/placement/collision blockers and
  keep brightness/gamerules/despawn/persistence as the remaining blockers.

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
- Live natural spawning for `CREATURE` through the `entity::spawning` planner
  once raw brightness checks, gamerules/server flags, despawn, persistence, and
  the live spawn executor are wired.
- Despawn rules for passive animals and later hostile mobs.
- Sounds for passive mobs.
- Data watcher / tracked data equivalent for richer entity presentation.
- Web/Android/XR screenshot coverage after desktop validation.
