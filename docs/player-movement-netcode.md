# Player Movement And Netcode Notes

Status: **historical server-authoritative alternative, superseded for current
planning by the accepted permissive client-authority direction in
[`topics/client-prediction.md`](topics/client-prediction.md)**.

The old movement/netcode sketch was useful for recording command-stream and prediction constraints, but it assumed too much about where the client prediction world lives. The client runtime architecture arc in [`tactical/README.md`](./tactical/README.md) has now landed the needed `IntegratedServer`, `ClientRuntime`, `ClientWorld`, `PredictionService`, and presentation ownership surfaces.

Do not resurrect the old `Movement3+` tactical direction from this document.
The command transport, host replay, acknowledgement, and correction design
below is retained as research only. It requires an explicit product-direction
change before becoming roadmap work.

## Current Accepted Constraints

The current target is high-rate FPS-style local movement with a cooperative
client trust boundary:

- local movement should feel responsive at high-refresh display rates;
- the client owns routine player body movement and publishes resulting poses;
- the server rejects malformed or non-finite poses, clamps world bounds, and
  honors server-directed teleport acknowledgement, but does not replay
  movement;
- the server remains authoritative for block interaction, inventory, health,
  entities, world mutation, persistence, and other gameplay consequences;
- render delta, packet cadence, and host world tick cadence must not become
  movement integration `dt`;
- NPC AI/pathfinding may run at Minecraft-like rates without forcing player
  physics down to 20 Hz;
- integrated single-player must not duplicate the client's movement and
  collision computation.

## What Was Wrong Or Premature

The previous plan framed the next slice as "client prediction runtime ownership". That was too narrow.

The client needs a broader vanilla-shaped replica:

- visible/interested chunks
- block and fluid states
- block entities
- client entity replicas
- client light/render facts
- interpolation state
- speculative action overlays
- a bounded prediction view over those facts

That replica is not a full server. It should not own worldgen, persistence, block ticks, liquid ticks, spawning authority, NPC AI authority, or host scheduling. But it is also not just a collision cache for movement.

The architectural dependency is now:

```text
IntegratedServer / DedicatedServer world authority
  -> logical protocol messages
  -> ClientRuntime + ClientWorld replica
  -> PredictionService and interpolation views
  -> Presentation/UI/render thread
```

Movement prediction should be a service inside or adjacent to `ClientRuntime`. It should read from `ClientWorld` through explicit collision/entity views. It should not read authoritative host internals, and the UI/render thread should not own raw chunk/collision/replay state.

## Vanilla Guidance

Minecraft Java 1.17.1 is still the reference for lower movement and entity concepts:

| Concern | Source |
|---|---|
| Shared block collision, occlusion, and shape semantics | `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/state/BlockBehaviour.java`, `reference/minecraft-1.17.1/src/net/minecraft/world/level/CollisionGetter.java`, `reference/minecraft-1.17.1/src/net/minecraft/world/level/CollisionSpliterator.java` |
| Shared collision/resolution | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` |
| Shared living movement modes | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java` |
| Player-specific travel/flying/swimming hooks | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/player/Player.java` |
| Client local movement and vanilla movement packet emission | `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java` |
| Server movement validation/correction | `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java` |
| Mob AI producing movement intent | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java`, `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/MoveControl.java`, `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/PathNavigation.java` |

Vanilla should not be copied for the high-level FPS player protocol:

- vanilla local player networking is mostly client-simulated position packets plus server validation/correction
- vanilla assumes a 20 TPS game loop
- vanilla does not implement shooter-style command ack, rewind, and replay

Vanilla should inform the client replica topology:

- singleplayer uses an integrated server plus client world over a memory connection
- the client owns a `ClientLevel`, client chunk cache, client entities, and client light engine
- the client does not run worldgen, persistence, block ticks, or liquid ticks
- remote entities are interpolated from authoritative updates
- fluids are server-authoritative and arrive as block/fluid state changes

Block collision facts are shared below both player and mob movement. `noOcclusion()` is a render/light concept, not a movement concept; `noCollission()` is the block-property path that removes collision. [`tactical/BlockCollision0-render-occlusion-vs-collision-shapes.md`](./tactical/BlockCollision0-render-occlusion-vs-collision-shapes.md) lands the first shared collision-shape split.

## Conditional Tilefun Guidance

`tilefun` remains useful prior art if authoritative command replay is ever
explicitly reopened:

| Concern | Source |
|---|---|
| Prediction/replay/reconcile implementation | `/Users/kgraehl/code/tilefun/src/client/PlayerPredictor.ts` |
| Shared movement params, bounded substeps, physics revision | `/Users/kgraehl/code/tilefun/src/physics/PlayerMovement.ts` |
| Shared server/client simulation adapters | `/Users/kgraehl/code/tilefun/src/physics/SimulationEnvironment.ts` |
| Server-side input queue draining and AI/physics phases | `/Users/kgraehl/code/tilefun/src/server/Realm.ts` |
| Input queue and no-phantom-movement lessons | `/Users/kgraehl/code/tilefun/docs/hard-won-knowledge.md` |
| Netcode parity scenarios | `/Users/kgraehl/code/tilefun/src/server/NetcodeParityBaseline.test.ts`, `/Users/kgraehl/code/tilefun/src/server/InputQueuePrediction.test.ts` |

The main invariant for that dormant alternative would be:

```text
An authoritative server must advance player movement from the same ordered
command records the client predicted.
```

Do not collapse multiple client samples into one mutable "latest input" slot, and do not let the server move a predicted player extra times from wall-clock ticks that had no corresponding command.

## Archived Authoritative Netcode Shape

One coherent command shape, if that product direction is ever reopened, is:

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

Conditional rules for that alternative:

- `sequence` is strictly increasing per player
- `commandQuantumUs` is fixed while a movement profile is active
- long or irregular client frames are converted into bounded fixed quanta
- button edges must survive frame dips where possible
- host snapshots ack the last processed command sequence
- authoritative snapshots include enough body state to restart replay
- polling, local worker `postMessage`, WebSocket, WebTransport, and future WebRTC adapters all carry the same logical records if they are used

The scene-owned 60 Hz clock implements fixed quanta, bounded catch-up, edge
retention, sequenced local semantic command recording, and presentation
interpolation. Those commands intentionally remain local. The host does not
drain them, corrections do not replay them, and neither behavior is missing
work under the accepted authority policy.

## Reopening This Alternative

The earlier architectural prerequisites are now present:

- an `IntegratedServer` / authoritative host facade for browser singleplayer
- a `ClientRuntime` that consumes the same logical protocol in singleplayer and multiplayer
- a `ClientWorld` replica that owns visible chunk, light, fluid, block-entity, and entity facts
- a presentation boundary where UI/render receives compact presentation state instead of raw world facts
- a prediction-service API that can read a bounded collision/entity view without host access
- tests or probes proving singleplayer and remote clients use the same client-world hydration path

Tactical 221 records the completed local input/movement implementation. There
is no next command-transport tactical. Reopening server-authoritative movement
requires an explicit product decision and a fresh tactical; these historical
notes can then inform it without making the old shape mandatory.
