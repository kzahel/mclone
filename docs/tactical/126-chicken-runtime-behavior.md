# 126 - Chicken Runtime Behavior

## Goal

Finish chicken as the first compact species-specific passive mob on top of the
shared entity runtime from [`118`](118-entity-runtime-and-passive-mob-bringup.md).

This tactical is not a general item, sound, breeding, or passenger system. It
uses Java 1.17.1 `Chicken` as the reference for the behavior that belongs in
the chicken runtime now, and leaves broader systems behind explicit hooks.

Workstream: native Rust shared server/client/render runtime, desktop validation
first.

## Current Baseline

- `EntityKind::Chicken` exists in protocol and server metadata.
- The debug passive showcase spawns chicken near the player by default.
- Chicken uses the promoted first-party `mclone:chicken` figure.
- Chicken has Java-shaped dimensions, eye height, movement speed, and water
  pathfinding malus.
- Chunk A landed supported passive goals, species flap/egg state, and
  Java-shaped airborne glide damping. Egg item entities and sounds remain
  deferred behind the future item/sound boundaries.
- Chunk B landed the first visible chicken wing pose through client-derived
  presentation data and the shared actor render path. No chicken protocol
  metadata is needed for the current behavior slice.

## Reference Shape

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Chicken.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Cow.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/WaterAvoidingRandomStrollGoal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/LookAtPlayerGoal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/RandomLookAroundGoal.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/ChickenRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/model/ChickenModel.java`

The Java chicken behavior that matters for this phase:

- registers the same supported passive goal tail as cow:
  `WaterAvoidingRandomStrollGoal`, `LookAtPlayerGoal`, and
  `RandomLookAroundGoal`
- sets water pathfinding malus to `0.0`
- stores `flap`, `flapSpeed`, `oFlapSpeed`, `oFlap`, `flapping`, `nextFlap`,
  `eggTime`, and `isChickenJockey`
- dampens downward velocity while airborne
- decrements and resets the egg timer on the authoritative side
- suppresses normal fall damage
- derives visible wing rotation from interpolated `flap` / `flapSpeed` in
  `ChickenRenderer.getBob()` and applies opposite Z rotations to `rightWing`
  and `leftWing` in `ChickenModel.setupAnim()`

## Chunk A - Supported Goals And Species Tick

Purpose: make chicken dynamic through the existing mob stack without inventing
new systems.

Implementation sketch:

- Add a species-state module under `entity/mob/`.
- Register the supported passive goal tail for chicken.
- Initialize chicken `egg_time` with Java's `random.nextInt(6000) + 6000`
  shape.
- Tick Java-shaped flap/flapping state after shared mob movement.
- Damp airborne downward velocity by `0.6`.
- Decrement/reset `egg_time` and count pending egg-lay events, but do not spawn
  item entities yet.

Done when:

- Chicken has the supported passive goals and can stroll/look through the shared
  goal/navigation/control stack.
- Chicken falling velocity is damped relative to cow.
- Chicken egg timer state advances only while entity-ticking and resets through
  the Java range.
- Tests cover the runtime state without adding protocol or item-entity
  contracts.

Status: landed.

Landed notes:

- Added `entity/mob/species.rs` with compact `MobSpeciesState` and
  `ChickenRuntimeState`.
- Registered chicken for the currently supported passive animal goal tail:
  `WaterAvoidingRandomStrollGoal`, `LookAtPlayerGoal`, and
  `RandomLookAroundGoal`.
- Initialized `egg_time` with Java's `random.nextInt(6000) + 6000` shape.
- Ticked flap/flapping state after shared mob movement and damped airborne
  downward velocity by `0.6`.
- Counted pending egg-lay events and reset `egg_time` without spawning item
  entities yet.
- Verified with `cargo test --manifest-path native/Cargo.toml -p
  mclone-server chicken` and `cargo test --manifest-path native/Cargo.toml -p
  mclone-server`.
- Verified with the full `cargo test --manifest-path native/Cargo.toml`
  workspace gate.

## Chunk B - Presentation Data Review

Purpose: decide what chicken-specific animation state needs to cross the
server/client boundary.

Implementation sketch:

- Audit whether client-side animation can derive enough from ordinary entity
  position/yaw/on-ground updates.
- If flap state needs protocol, add species presentation data behind an entity
  metadata/update shape rather than one-off chicken messages.
- Keep non-box primitive rendering and higher-fidelity chicken animation as
  renderer follow-ups unless required for visible correctness.

Done when:

- The tactical records whether flap data stays server-local or becomes tracked
  data.
- Any protocol addition has round-trip tests, or the tactical explicitly records
  why no protocol addition is needed yet.

Status: landed for the current presentation scope.

Landed notes:

- Audited the Java client render shape. The current native slice keeps the
  authoritative flap and egg timer state server-local; the client derives the
  visible wing pose from ordinary entity `on_ground` and vertical motion during
  interpolation.
- Added `ActorPresentation::chicken_wing_flap_radians` as local presentation
  data, mapped through `mclone-render-session` into `ActorInstance`.
- Applied the pose in `mclone-render` as additive opposite Z rotations on
  Asset Lab figure parts named `wing_l` and `wing_r`, after walk-clip sampling.
- Updated the native actor walk review capture path so chickens render with a
  visible wing pose for screenshot validation.
- Verified with focused client/session/render/native-client tests and the
  affected crate gate:
  ```bash
  cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-render-session -p mclone-render -p mclone-native-client
  ```
- Verified with the full `cargo test --manifest-path native/Cargo.toml`
  workspace gate.

Remaining presentation follow-ups:

- If later parity needs exact server flap phase, expose species animation data
  through a general entity metadata/update path rather than a chicken-specific
  protocol message.
- Higher-fidelity flap cadence, sound/game-event routing, and egg item side
  effects remain separate chunks behind their shared systems.

## Chunk C - Egg Item Side Effects

Purpose: turn the egg timer hook into real gameplay once item entities exist.

Implementation sketch:

- Add or reuse a shared item-entity/drop boundary.
- Drain chicken egg-lay events from the authoritative mob tick.
- Spawn an egg item entity and later route the chicken egg sound event.

Done when:

- A mature chicken can visibly produce an egg item without embedding item-drop
  policy in chicken ticking.

Status: blocked on item entities / sound event boundaries.

## Chunk D - Persistence, Jockey, And Despawn

Purpose: finish the rest of Java `Chicken` state once the surrounding systems
exist.

Implementation sketch:

- Persist `IsChickenJockey` and `EggLayTime`.
- Route chicken-jockey passenger behavior through the eventual passenger stack.
- Apply jockey-specific far-away removal once despawn rules exist.
- Suppress fall damage once damage/fall-distance systems exist.

Status: blocked on persistence, passenger, despawn, and damage systems.

## Validation

Minimum gates for code slices:

- focused `mclone-server` chicken/entity tests
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- focused `mclone-client`, `mclone-render-session`, `mclone-render`, and
  `mclone-native-client` tests for client/render presentation slices
- full `cargo test --manifest-path native/Cargo.toml` for committed runtime
  changes

Visible validation follows tactical `118`: desktop first, with screenshots under
`/tmp` for renderer-facing changes.
