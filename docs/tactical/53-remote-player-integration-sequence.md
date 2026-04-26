# Tactical 53 - Remote Player Integration Sequence

Status: implementation sequence complete; follow-up movement polish remains.

This tactical coordinates the cross-cutting work needed before `movementMode=player` becomes real grounded player movement in both browser singleplayer and remote multiplayer. It exists because the next work spans transport, join/session semantics, and movement integration; doing those in the wrong order would bake temporary debug shortcuts into the player simulation.

## Goal

Land a basic but structurally correct remote/player integration path:

1. remote clients use a persistent WebSocket message channel instead of HTTP polling
2. joining a world allocates or resumes a named player slot instead of treating the session as the player
3. player mode uses shared host-authoritative movement physics, client prediction, and collision

This is not a gameplay feature bundle. It is the ordering contract for the next implementation tacticals.

## Current State

Already landed:

- `R0` through `R8`: authoritative host/client boundary, worker singleplayer, dedicated Node host, remote clients, protocol hardening, authoritative player command queue, and debug browser authority integration
- `ClientRuntime0` through `ClientRuntime6`: `IntegratedServer`, `ClientRuntime`, `ClientWorld`, prediction views, presentation state, and entity interpolation boundary
- `Movement0` through `Movement2`: shared movement body, fixed command stream, prediction core, host command queue, and ack snapshots
- `Movement3`: live host player ticks now use the shared movement body, host collision reads, explicit missing-collision handling, browser prediction/reconcile, and player-mode input semantics

Landed through this sequence:

- remote dedicated browser play uses a persistent WebSocket message channel by default
- server-originated session, player, chunk, light, and entity updates are pushed into the client drain queue
- reusable remote message serialization lives outside the HTTP envelope module
- browser joins carry a default player profile, and remote host sessions now point at separate player slots
- local worker singleplayer and remote WebSocket clients share the same command/snapshot movement path
- debug player mode emits local wish axes plus jump/crouch/sprint buttons; `moveY` no longer means flight in player mode
- browser debug player prediction uses `ClientRuntime.getPredictionService()` over `ClientWorldPredictionView`
- player camera position is derived from predicted or authoritative movement body position plus eye height

Still temporary:

- HTTP polling remains as a non-default compatibility adapter and test surface
- `open_world` still folds world open/create and session join together
- profile-based persisted player-slot recovery is not implemented yet
- player spawn height is still provisional; surface-aware spawn/respawn is a follow-up movement slice

## Required Order

### 1. R9 WebSocket Message Channel

Owner doc: [`R9-websocket-message-channel.md`](R9-websocket-message-channel.md).

Status: done.

Do this first because grounded movement will depend on frequent player-state acks, server-pushed chunk/collision facts, and entity updates. Those should not be paced by HTTP polling once we start depending on prediction/reconciliation behavior.

Required outcomes landed:

- remote browser clients use WebSocket by default
- `ClientRuntime.drainTransportUpdates()` still exists as a local hydrate/drain point, but remote networking no longer performs a `poll_world_updates` request
- server-originated session, player, chunk, light, and entity updates can be pushed
- message serialization is split from HTTP-specific request/response envelopes
- HTTP polling is isolated as a non-default compatibility fallback

Do not add grounded movement physics in this slice.

### 2. Join And Player Slot Semantics

Status: done.

Required outcomes landed:

- `open_world` remains the transitional join command, but it now carries an optional `playerProfile`
- browser joins send the default `Player` profile
- the remote host allocates a separate player slot id and keeps it stable across session resume
- `sessionId` remains a connection/resume handle and no longer doubles as the player slot
- `ClientSessionState` exposes `playerProfile` alongside session, player, save, revision, and interest facts
- profile-based persisted-slot recovery is documented as future work instead of inferred from a session id

Do not add inventory, authentication, permissions, or full persisted player data in this slice.

### 3. Basic Player Movement Integration

Owner doc: [`Movement3-basic-player-movement-integration.md`](Movement3-basic-player-movement-integration.md).

Status: done.

This replaced the compatibility fly path without reopening transport or identity work.

Required outcomes landed:

- host player ticks use `simulatePlayerMoveCommand(...)` / `simulateMovementStep(...)` instead of the legacy `nextPositionForCommandStep(...)` fly integrator
- host movement reads collision through an authoritative `CollisionWorld`
- missing collision data is explicit and does not silently consume commands as successful movement
- debug player input emits local `wishX/wishZ` plus jump/crouch/sprint button bits
- `moveY` stops being flight in player mode
- browser player prediction uses `ClientRuntime.getPredictionService()` over `ClientWorldPredictionView`
- camera position is derived from predicted/authoritative body position plus eye height
- local worker singleplayer and remote WebSocket clients use the same movement command/snapshot path

Non-goals:

- vanilla-perfect movement constants
- non-full collision shapes
- liquids/swimming/ladders
- crouch body resize
- entity collision
- inventory/game mode persistence

## Cross-Track Invariants

- Logical host/client messages remain the authority model. WebSocket is a carrier, not a gameplay protocol.
- `sessionId` and `playerId` are separate concepts everywhere movement, inventory, or persistence can observe them.
- Movement integration must use fixed command quanta, not render frame delta or transport cadence.
- Client prediction reads only bounded `ClientWorld` views; it must not read host internals.
- The renderer consumes presentation state and camera facts; it must not own movement truth or collision caches.
- Missing chunk/collision data must stay visible to prediction/reconciliation diagnostics.

## Suggested Validation Gates

After R9:

- done: WebSocket coverage alongside `test/runtime/remote-world-transport.test.ts`
- done: `pnpm typecheck`
- done: `pnpm test:browser`
- done: `git diff --check`
- not run in R9: `pnpm test:browser:integration`; run this in the player-slot cleanup if debug player-state assumptions change

After join/player-slot cleanup:

- done: remote two-client test proves distinct sessions can join one world with distinct player ids
- done: remote two-client test proves distinct join profiles are preserved
- done: resume tests prove the same session resumes the same player id
- done: browser smoke no longer assumes `playerId === sessionId`
- done: `pnpm typecheck`
- done: `pnpm test -- test/runtime/remote-world-transport.test.ts`
- done: `pnpm test:browser`
- done: `pnpm test:browser:integration`
- done: `git diff --check`

After movement integration:

- done: `pnpm test -- test/runtime/player-loop.test.ts test/renderer/debug-player-controls.test.ts`
- done: `pnpm test -- test/runtime/movement/movement-step.test.ts test/runtime/movement/movement-command.test.ts test/runtime/movement/movement-predictor.test.ts test/runtime/client-prediction-service.test.ts test/runtime/generated-world-boundary.test.ts test/runtime/generated-world-host-scheduler.test.ts`
- done: `pnpm test -- test/runtime/remote-world-transport.test.ts`
- done: `pnpm typecheck`
- done: `pnpm test:browser:integration`
- done: `pnpm test:browser`
- done: `git diff --check`

## Done When

- The default remote path is WebSocket-backed.
- Joining a world creates or resumes a named player slot instead of using the session id as the player id.
- Player mode in `debug.html` no longer behaves as free flight.
- Host and client prediction use the same command records, movement body shape, and collision facts.
- Singleplayer and remote clients share the same `ClientRuntime` / `ClientWorld` / `PredictionService` path for movement.
