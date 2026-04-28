# Movement0 - Shared movement body and collision

Standing after the durable movement architecture in [`../player-movement-netcode.md`](../player-movement-netcode.md), the runtime boundary in [`../architecture.md`](../architecture.md), and the entity foundation in [`Entities0-runtime-entity-foundation.md`](Entities0-runtime-entity-foundation.md). This is the first player movement/netcode tactical. It builds a deterministic shared movement body and collision step before command prediction, replay, reconciliation, interpolation, or NPC AI integration.

## Goal

Add the first shared movement/collision core that can later be used by:

- host-authoritative local player movement
- client-side local player prediction
- server-side replay of sequenced movement commands
- future NPC/body locomotion fed by lower-rate AI intent

At the end of `Movement0`, `mclone` should have a fixed-step movement body that can move through solid full-block terrain, stop on collisions, track velocity and grounded state, and run deterministic unit tests. It should not yet change the network protocol or the live browser control path.

## Status

Done. Landed as the first shared movement/collision foundation. The live protocol and browser control path are intentionally unchanged.

## Implementation Status

Landed shape:

- `src/runtime/movement/` contains movement body, intent, params, collision-world adapter, fixed-step simulation, and a barrel export.
- `src/world/phys/vec3.ts` and `src/world/phys/aabb.ts` now include the vanilla-shaped vector and box helpers needed by collision and movement.
- `test/runtime/movement/movement-step.test.ts` covers grounded movement, gravity/landing, wall collision, edge grounding, jump-held behavior, low step-up, missing collision data, deterministic repeatability, and zero-`dt` no-op behavior.
- The first collision scope started as solid full-block AABBs only. [`BlockCollision0-render-occlusion-vs-collision-shapes.md`](BlockCollision0-render-occlusion-vs-collision-shapes.md) now supplies minimal block collision shapes for leaves, plants, and slabs without making this slice responsible for full vanilla shape parity. Liquids, crouch resize, dynamic collision revisions, entity collision, command prediction, and protocol migration remain out of scope here.

## Reference Source

Read these before implementing:

| Source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` | shared `move(...)`, collision flags, step-up behavior, on-ground updates |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` | travel modes, gravity, friction, jump/fall behavior |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/player/Player.java` | player-specific hooks to understand and mostly avoid in this first custom movement slice |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/MoverType.java` | movement cause classification |
| `reference/minecraft-1.17.1/src/net/minecraft/world/phys/AABB.java` | vanilla box math and inflate/move/intersect semantics |
| `reference/minecraft-1.17.1/src/net/minecraft/world/phys/shapes/Shapes.java` | collision clipping helpers for future non-full shapes |
| `reference/minecraft-1.17.1/src/net/minecraft/world/phys/shapes/VoxelShape.java` | future collision shape model |
| `reference/minecraft-1.17.1/src/net/minecraft/world/phys/shapes/CollisionContext.java` | future entity-aware collision context |
| `/Users/kgraehl/code/tilefun/src/physics/PlayerMovement.ts` | prior bounded-substep and shared movement parameter work |
| `/Users/kgraehl/code/tilefun/src/physics/SimulationEnvironment.ts` | prior host/client simulation adapter shape |
| `/Users/kgraehl/code/tilefun/docs/archive/physics-sim-drift-reduction-plan.md` | prior drift-boundary and revision lessons |

The `tilefun` files are prior art, not port targets. Vanilla source is the port/reference source for shared movement and collision concepts.

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | Movement body types | `MovementBody`, `MovementIntent`, movement mode, and `MovementStepResult` records that carry enough state for future replay |
| 2 | Collision world adapter | a small interface for authoritative block collision reads, separate from renderer caches |
| 3 | Fixed-step movement function | `simulateMovementStep(...)` taking explicit fixed `dt` or command quantum and movement params |
| 4 | Full-block AABB collision | solid block collision/clipping against known terrain, initially full cubes only |
| 5 | Ground/air basics | gravity, horizontal acceleration, friction, jump impulse, and simple air control |
| 6 | Grounding and step-up | output collision flags, `onGround`, blocked-axis facts, and a conservative step-up threshold |
| 7 | Deterministic tests | fixtures for movement, gravity, collision, step-up, and repeatability under fixed quanta |

Do not add:

- sequenced movement commands
- prediction, replay, reconciliation, or correction smoothing
- `set_player_input` protocol changes
- remote interpolation buffers
- non-full block collision shapes
- liquids, swimming, ladders, elytra, crouch body resize, or entity collision
- NPC AI, pathfinding, `MoveControl`, or navigation
- a browser control-path migration

## Architectural Constraints

`Movement0` should be shaped so `Movement1` can add command prediction without replacing the core:

- The movement step takes explicit fixed time. Do not integrate from `requestAnimationFrame` delta, host wall-clock delta, or transport poll cadence.
- The intent shape should be command-compatible even if tests feed synthetic intent directly.
- The simulation should not depend on "latest input" state. That can exist outside this module as a temporary caller behavior only.
- The host should eventually move players by draining command records through this function, not by running arbitrary per-host-tick displacement.
- The result must include enough replay state: position, velocity, bounds, `onGround`, movement mode, jump-related state, and collision flags.
- Collision reads go through an adapter over authoritative world data. Do not let the movement core read renderer chunk caches.
- Keep full-block collision narrow and explicit. Dynamic/non-full collision needs revision facts before it participates in prediction.

## Recommended File Layout

Exact names can follow local style during implementation, but keep vanilla-grep-friendly names where practical.

```text
src/runtime/movement/
  movement-body.ts
  movement-intent.ts
  movement-params.ts
  movement-step.ts
  collision-world.ts

src/world/phys/
  aabb.ts
```

If an equivalent `AABB`/physics helper already exists, extend it instead of adding a duplicate. If the codebase already has a stronger `src/world/entity/` home for movement by the time this lands, use that, but keep the movement core independent from protocol, DOM, workers, and rendering.

Suggested tests:

```text
src/runtime/movement/
  movement-step.test.ts
  collision-world.test.ts
```

## Behavioral Requirements

### Fixed-Step Integration

The public step API should make fixed time unavoidable:

```ts
interface MovementBody {
  position: Vec3;
  velocity: Vec3;
  bounds: Aabb;
  onGround: boolean;
  mode: "ground" | "air" | "water" | "flying";
}

interface MovementIntent {
  wishX: number;
  wishZ: number;
  jump: boolean;
  crouch: boolean;
  sprint: boolean;
  yaw: number;
  pitch: number;
}

function simulateMovementStep(
  body: MovementBody,
  intent: MovementIntent,
  world: CollisionWorld,
  fixedDtSeconds: number,
  params: MovementPhysicsParams,
): MovementStepResult;
```

The first implementation can use copies or immutable return values if that makes tests clearer. Later performance work can choose mutation if profiling justifies it.

### Collision World

Start with full-block terrain collision:

- query solid blocks overlapping the swept body bounds
- clip movement on each axis deterministically
- update position and velocity consistently after blocked movement
- report which axes collided
- treat missing/unloaded collision data explicitly instead of silently passing through

Do not try to solve stairs, fences, slabs, trapdoors, liquids, or entity collision in this slice.

### Grounding

The step result should make grounding observable:

- standing on a block sets `onGround`
- walking off an edge clears `onGround`
- jumping clears `onGround` and applies exactly one jump impulse
- hitting a ceiling clears or clamps upward velocity
- horizontal collision does not masquerade as grounded state

### Step-Up

Implement a conservative first step-up path only if the vanilla source review and tests keep it small. The goal is a usable Minecraft-like body over blocky terrain, not full shape parity.

The first tests should cover:

- flat ground motion
- stopping against a wall
- landing on a floor
- ceiling collision
- walking off a ledge
- stepping up one low block if included in the slice

If step-up threatens the slice size, leave a documented `TODO` and keep the rest deterministic.

## Validation

Minimum:

- `pnpm test -- <movement test target>`
- `pnpm typecheck`
- `git diff --check`

Add targeted browser smoke only if this slice touches the live control path. It should not need to.

Tests should prove:

- the same body, intent, world, params, and fixed `dt` produce the same result every run
- fixed smaller steps are the planned authority path, even if a larger single step currently differs
- no movement occurs from stale intent when no step is requested
- full-block collisions stop the body without burying it in terrain
- `onGround` changes only from vertical collision/grounding logic

Landed validation:

- `pnpm test -- test/runtime/movement/movement-step.test.ts`
- `pnpm typecheck`

## Done When

- The shared movement body and collision-world adapter exist behind runtime/simulation modules, not renderer modules.
- Unit tests cover movement, collision, jump/grounding, and deterministic fixed-step repeatability.
- The current protocol and browser control path are unchanged.
- The design leaves a direct path for `Movement1` to feed sequenced commands into the same step function.

## Next Step

`Movement1-command-stream-and-local-prediction.md` landed the command stream, and [`BlockCollision0-render-occlusion-vs-collision-shapes.md`](BlockCollision0-render-occlusion-vs-collision-shapes.md) now separates render occlusion from collision shape facts. Deeper player collision polish should build on that shared shape path.
