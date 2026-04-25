# Player Movement And Netcode

Architecture sketch for high-rate FPS-style player movement, prediction, reconciliation, and interpolation in `mclone`.

This document is intentionally not a vanilla movement parity plan. Minecraft Java 1.17.1 is still the reference for entity lifecycle, storage, ticking eligibility, and many mob systems, but player movement is a deliberate gameplay/runtime divergence. The target here is responsive Quake/Overwatch-style local movement on top of an authoritative host, while keeping Minecraft-style world, block, entity, and AI systems able to run at lower rates.

## Goals

- Local player movement should feel responsive at high-refresh display rates.
- The host remains authoritative for player body state, collision, block interaction, and gameplay consequences.
- Prediction and reconciliation should be first-class, not a late transport patch.
- Client and server movement must integrate the same command timeline.
- NPC AI and world simulation can run at Minecraft-like rates without forcing player physics down to 20 Hz.
- The shared movement/collision core should be reusable by players, mobs, items, and simple physics entities.

## Non-Goals

- Do not port Minecraft's 20 TPS `Player` movement model class-for-class.
- Do not let render frame delta become authoritative movement delta.
- Do not make the client send final positions as trusted truth.
- Do not require every mob AI decision, path search, block tick, or liquid tick to run at player movement frequency.
- Do not start with full rollback for all world entities. Local player prediction plus remote interpolation is the first target.

## Implementation Constraints

These constraints should survive the tactical breakdown. They are the parts most likely to cause subtle movement jank if they are postponed or treated as transport details.

- Use one shared movement body/core for host authority, local prediction, and future NPC body stepping. Controllers may differ; the body simulation should not.
- Keep vanilla source review mandatory for the lower movement/collision concepts: `Entity.move(...)`, `LivingEntity.travel(...)`, player hooks in `Player`, and mob intent production through `MoveControl`/navigation. Do not copy vanilla's high-level 20 TPS client-position-packet shape for the custom FPS path.
- Movement simulation takes an explicit fixed `dt` or integer command quantum. Browser frame time, host tick time, and transport poll cadence must not be used directly as body integration time.
- Store and process ordered command records. A "latest input" slot is acceptable only as a temporary compatibility shim before the command stream lands, not as the long-term player movement model.
- A host tick at 60 Hz may drain two 120 Hz commands and run two `1/120` movement steps. It should not replace them with one `1/60` step if the client predicted two smaller steps.
- Long or irregular client frames must be converted into bounded fixed quanta, preserving button edges where possible. One large variable-dt command is a misprediction source.
- Reconciliation mutates simulation truth by snap-and-replay. Interpolation, smoothing, and camera/viewmodel easing are presentation layers over that truth.
- NPC AI/pathfinding can run at lower rates and feed intent into the movement body. Collision-relevant NPC bodies may still need higher-rate stepping near players.
- Dynamic collision inputs need revision facts once block edits, non-full shapes, liquids, or entity collisions enter prediction. Revision mismatch should be diagnosable, not hidden as generic floating-point drift.
- The protocol should evolve `set_player_input` toward sequenced movement commands with acknowledgements, while preserving the existing host/client authority boundary.

## Prior Art

Minecraft 1.17.1 source is useful for the shared lower movement layers:

| Concern | Source |
|---|---|
| Shared collision/resolution | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |
| Shared living movement modes | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` |
| Player-specific travel/flying/swimming hooks | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/player/Player.java` |
| Client-local movement and movement packet emission | `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java` |
| Server-side movement validation/correction | `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java` |
| Mob AI producing movement intent | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java`, `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/MoveControl.java`, `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/PathNavigation.java` |

`tilefun` is useful prior art for the command/prediction side:

| Concern | Source |
|---|---|
| Prediction/replay/reconcile implementation | `/Users/kgraehl/code/tilefun/src/client/PlayerPredictor.ts` |
| Shared movement params, bounded substeps, physics revision | `/Users/kgraehl/code/tilefun/src/physics/PlayerMovement.ts` |
| Shared server/client simulation adapters | `/Users/kgraehl/code/tilefun/src/physics/SimulationEnvironment.ts` |
| Server-side input queue draining and AI/physics phases | `/Users/kgraehl/code/tilefun/src/server/Realm.ts` |
| Input queue and no-phantom-movement lessons | `/Users/kgraehl/code/tilefun/docs/hard-won-knowledge.md` |
| Drift-reduction plan and parity boundaries | `/Users/kgraehl/code/tilefun/docs/archive/physics-sim-drift-reduction-plan.md` |
| Netcode parity scenarios | `/Users/kgraehl/code/tilefun/src/server/NetcodeParityBaseline.test.ts`, `/Users/kgraehl/code/tilefun/src/server/InputQueuePrediction.test.ts` |
| Transport/channel direction | `/Users/kgraehl/code/tilefun/docs/NETWORK-ARCHITECTURE.md` |

The main `tilefun` invariant to carry forward is:

```text
The server must advance player movement from the same ordered command records the client predicted.
```

Do not collapse multiple client samples into one mutable "latest input" slot, and do not let the server move a predicted player extra times from wall-clock ticks that had no corresponding command.

## Minecraft Comparison

Vanilla has two useful ideas but the wrong high-level shape for this target.

Useful:

- `Entity.move(...)` is a shared collision/resolution function. Players and mobs both rely on the same lower movement machinery.
- `LivingEntity.travel(...)` is a shared living-body movement layer for ground, air, water, lava, ladders, gravity, and friction.
- Mobs differ mostly in how they produce intent: goals/path navigation/`MoveControl` eventually set speed, strafe, forward, and jump intent.
- Remote entities are interpolated from server updates using `lerpTo(...)`.

Poor fit:

- Vanilla player networking is mostly client-simulated position packets plus server validation/correction.
- Vanilla assumes a 20 TPS game loop and does not model a fixed high-rate input command stream.
- It does not provide the shooter-style "server acks command N, client rewinds to server state and replays N+1..." model we want.

`mclone` should keep the lower shared movement concept, but replace the player command/protocol shape.

## Clock Model

Use separate clocks. Do not let one clock impersonate another.

| Clock | Typical Rate | Owner | Purpose |
|---|---:|---|---|
| Render frame | display rate, variable | browser main thread | draw predicted/interpolated presentation |
| Input sampling | every render frame / input event | browser main thread | collect raw button, mouse, gamepad state |
| Command clock | fixed, e.g. 120 Hz or 128 Hz | client and host | quantized player movement commands |
| Player physics | command quantum, possibly substepped | host and predictor | authoritative/predicted body integration |
| Snapshot publish | 20-60 Hz | host | authoritative states to clients |
| NPC AI/path decisions | 5-20 Hz | host | update mob intent, goals, navigation |
| NPC/body movement | variable by importance | host | high-rate nearby, lower-rate distant |
| World/block/liquid ticks | Minecraft-like schedulers | host | block, liquid, random, spawning systems |

A 240 Hz display should not create 240 Hz movement truth unless the host also consumes 240 Hz commands. It can still render at 240 Hz using the latest predicted state, view smoothing, weapon/viewmodel smoothing, and interpolation.

## Command Stream

The client emits sequenced movement commands over a fixed command timeline. Commands should represent integer physics quanta, not arbitrary frame deltas.

Suggested logical shape:

```ts
interface PlayerMoveCommand {
  readonly type: "player_move_command";
  readonly playerId: string;
  readonly sequence: number;
  readonly clientTimeUs: number;
  readonly commandQuantumUs: number;
  readonly stepCount: number;
  readonly buttons: number;
  readonly edgeButtons: number;
  readonly wishX: number;
  readonly wishZ: number;
  readonly yaw: number;
  readonly pitch: number;
  readonly physicsRevision: number;
  readonly collisionRevision?: number;
}
```

Rules:

- `sequence` is strictly increasing per player.
- `commandQuantumUs` is fixed while a movement profile is active.
- `stepCount` is an integer count of `commandQuantumUs` slices.
- Large frame spans are split or capped before becoming commands.
- `edgeButtons` carries events like jump-pressed that can be lost if only held state is sampled.
- Analog inputs and yaw/pitch are quantized deterministically enough for replay.
- Runtime-tunable movement params carry a `physicsRevision`.
- Collision-affecting world edits should eventually carry a chunk/collision revision so misprediction can be diagnosed instead of guessed.

### Framerate Dips

If a browser frame arrives after 40 ms, the client should not emit one 40 ms physics step. It should convert that span into fixed command quanta, for example five 8.333 ms commands at 120 Hz, or one command with `stepCount = 5`.

Button events that happen inside that long frame should be timestamped where possible and assigned to the correct command window. Gamepad input is only known at poll time, so it has lower input fidelity during dips, but once sampled it still becomes deterministic command data.

## Host Processing

The host's world tick and the player's command stream are related but not identical.

If the host publishes snapshots at 60 Hz and player commands run at 120 Hz, each host tick usually drains two movement commands per player and runs two `1/120` movement steps. If a packet arrives late, the next host tick can drain more queued commands subject to a budget cap.

Server-side rules:

- Queue all valid commands by sequence.
- Process commands in order.
- Drop or reject duplicates and impossible sequence jumps according to an explicit policy.
- Advance predicted player body state only from commands, plus intentional non-input forces such as knockback, gravity continuation, or conveyors.
- Cap catch-up work per host tick to prevent speedhack or CPU starvation.
- Ack the last processed command sequence in authoritative snapshots.
- Include enough body state to restart replay: position, velocity, grounded state, movement mode, jump/crouch state, and relevant timers.

Do not process "latest input" once per server tick. That loses commands when the client produces two samples between server ticks.

## Prediction And Reconciliation

Client predictor state:

- predicted local player body
- ring buffer of sent commands
- previous predicted pose for render smoothing
- last authoritative state received
- last acknowledged sequence
- correction diagnostics

On local command creation:

1. Store command in the ring buffer.
2. Simulate the predicted body for the command's fixed quanta.
3. Send the command to the host.

On authoritative snapshot:

1. Read `lastProcessedCommandSeq`.
2. Snap the prediction state to the authoritative body state.
3. Drop commands with `sequence <= lastProcessedCommandSeq`.
4. Replay remaining commands in order.
5. Compute correction diagnostics.
6. Smooth presentation if the correction is small; hard snap if the correction is too large or caused by teleport/world change.

The simulation state should snap/replay exactly. Visual smoothing is a presentation offset layered on top, not a different simulation truth.

Diagnostics should classify common drift causes:

- replay backlog
- command dt mismatch
- physics revision mismatch
- collision/world revision mismatch
- grounded/movement-mode flip
- velocity drift
- jump-state mismatch
- quantization-like tiny error

`tilefun`'s `PlayerPredictor` diagnostics are a good model for this.

## Interpolation

Use separate paths for local and remote presentation.

### Local Player

Render the predicted body immediately. Apply small correction smoothing as a visual offset after reconciliation. Camera yaw/pitch can update immediately from local input for feel even if body commands are fixed-rate.

### Remote Players And NPCs

Remote entities should render from a buffered snapshot timeline:

```text
renderTime = estimatedServerTime - interpolationDelay
```

Then interpolate between the two surrounding authoritative snapshots. Brief extrapolation is acceptable when packets are late, but should be bounded and corrected by the next snapshot.

Remote interpolation should not reuse the local player's replay buffer. Remote players and NPCs are presentation consumers of authoritative snapshots unless we later add explicit remote prediction for special interactions.

## Shared Movement Core

Keep the movement body simulation separate from input and AI controllers.

Suggested split:

```ts
interface MovementBody {
  position: Vec3;
  velocity: Vec3;
  bounds: Aabb;
  onGround: boolean;
  mode: "ground" | "air" | "water" | "flying";
}

interface MovementIntent {
  wishDir: Vec3;
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
  dt: number,
  params: MovementPhysicsParams,
): MovementStepResult;
```

Controllers produce `MovementIntent`:

- local player input controller
- server player command controller
- NPC path/AI controller
- simple scripted entity controller

The shared movement core owns:

- acceleration/friction/air control
- jump and crouch body transitions
- gravity and movement modes
- AABB collision against block shapes
- stepping and grounding
- water/lava hooks
- collision result flags

For parity-sensitive Minecraft mobs, later slices can still use vanilla-shaped AI and navigation to produce intent. They do not require vanilla player movement.

## NPC And AI Rates

NPCs should not force player movement down to 20 Hz.

Recommended model:

```text
AI/path update at 5-20 Hz -> latest desired movement intent
nearby NPC body movement at high-rate movement quantum
far NPC body movement at lower rate or frozen
client renders all non-local entities through snapshot interpolation
```

Nearby hostile or collision-relevant mobs may need high-rate body stepping so they interact cleanly with players. Distant passive mobs can use lower-rate body updates or stay frozen according to chunk/entity activity rules.

This matches Minecraft's entity-ticking distinction in spirit: activity is controlled by host-side eligibility, not renderer demand.

## Collision World And Revisions

Prediction exactness depends on client and host seeing compatible collision inputs.

Risky inputs:

- block edits and delayed chunk updates
- doors/trapdoors/fences/slabs/stairs with non-full collision shapes
- liquids and swimming transitions
- entity collisions
- piston-like moving collision, if added later
- chunk boundary load gaps

First slice should keep collision scope narrow: solid full-block AABBs, generated terrain chunks known to both sides, simple step/ground behavior, no dynamic block shape changes during prediction unless revisioned.

Later, include revision facts in snapshots and commands:

- `physicsRevision`: movement constants/profile
- `chunkCollisionRevision`: collision data for relevant chunk set
- `entityCollisionRevision`: dynamic colliders if used for prediction

When revisions differ, the predictor should expect correction rather than trying to hide it as a numerical drift bug.

## Protocol Direction

`set_player_input` should evolve from "latest input intent" toward sequenced command records. Polling can carry these messages initially, but high-rate movement likely wants a push-capable transport later.

Transport rule: movement commands are a logical sequenced stream, not an HTTP polling behavior. The host must enqueue command records by sequence and ack the last processed command in authoritative player snapshots. It must not interpret "one request" or "one poll" as "one movement step", and it must not keep a mutable latest-input slot as the authority model.

Delivery rules:

- command messages may be bundled, but each command keeps its own sequence, `commandQuantumUs`, `stepCount`, button/edge bits, yaw/pitch, and revision facts
- duplicates and already-acked commands are ignored; sequence gaps remain queued/backlogged policy decisions, not permission to simulate stale input
- host snapshots include the last processed command sequence and enough movement body state to restart replay
- polling, local worker `postMessage`, WebSocket, WebTransport, and future WebRTC adapters all carry the same logical command/snapshot records
- lossy realtime lanes, if added later, must resend recent unacked commands or bundles until acked and keep reliable ordered lanes for control, chunk, and interaction state
- HTTP polling may be used for Movement2 validation, but it is not the network model

Logical host-to-client snapshots should include:

- server tick/time
- authoritative local player movement body
- `lastProcessedCommandSeq`
- physics/collision revision facts
- remote entity snapshots/deltas

Wire details can differ by transport. The logical command/replay model should not depend on HTTP polling.

## Testing Strategy

Use deterministic tests before browser feel tests.

Core tests:

- one command per host tick stays exact
- two commands before one host tick are both processed in order
- zero-command host tick does not accidentally consume stale input
- command rate higher than host tick rate stays exact after replay
- low host snapshot rate with high command rate reconciles without drift
- quick-tap jump is preserved through `edgeButtons`
- held jump landing does not diverge after replay
- collision edge / step-up / chunk-boundary fixtures stay deterministic
- physics revision changes are classified and corrected
- delayed ack leaves temporary correction but settles after backlog drains

Browser tests:

- local prediction remains responsive under artificial latency/jitter/loss
- render at 120/144/240 Hz does not change authoritative movement distance
- intentional correction smoothing does not create visible rubber-banding for small errors
- large corrections snap instead of sliding through terrain

## Suggested Planning Slices

Use the player movement/netcode arc in [`tactical/README.md`](./tactical/README.md) as the tactical index. Draft the next slice in detail only when the previous one has taught us enough.

1. **[`Movement0: shared movement body and collision world`](./tactical/Movement0-shared-movement-body-and-collision.md)** - landed
   - AABB body, velocity, grounded state, full-block collision, stepping, fixed-step simulation.

2. **[`Movement1: command stream and local prediction`](./tactical/Movement1-command-stream-and-local-prediction.md)** - landed
   - Sequenced commands, fixed command quanta, ring buffer, local replay predictor, deterministic unit tests.

3. **Movement2: authoritative host integration** - next
   - Host command queue, command processing budgets, authoritative snapshots with ack sequence, browser control path migration.

4. **Movement3: interpolation and correction smoothing**
   - Local visual correction offset, remote entity interpolation buffers, latency/jitter debug controls.

5. **Movement4: richer collision and world interaction**
   - Non-full block shapes, liquids, crouch body shape, block edit collision revisions, chunk boundary policy.

6. **Movement5: NPC locomotion bridge**
   - Low-rate AI intent feeding shared movement bodies, entity activity tiers, nearby high-rate body stepping.
