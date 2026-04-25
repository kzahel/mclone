# ClientRuntime0 - Integrated server and client world boundary

Standing after the vanilla client-replica/networking research in [`../minecraft-client-replica-research.md`](../minecraft-client-replica-research.md). This is now the next architecture tactical before any new movement, NPC/AI, or transport work.

## Goal

Create a clear runtime ownership model for:

- `IntegratedServer` / authoritative local host
- dedicated/headless server sharing the same authority core
- `ClientRuntime`
- `ClientWorld`
- prediction-service boundary
- render/presentation/UI thread

This slice should be mostly boundary and naming work. It should prepare the codebase for proper implementation without sneaking in new movement physics, AI behavior, or transport semantics.

## Problem

The current runtime stack already has a useful authoritative host, worker transport, remote HTTP path, render-world worker, movement command queues, entity snapshots, lighting, and liquid simulation. But the shape is still too implementation-accidental:

- "host/client" exists, but there is no vanilla-shaped `IntegratedServer` concept for browser singleplayer.
- The renderer/render-world path has client chunk caches, but there is not yet a clearly named `ClientWorld` replica as the client-side source of visible world facts.
- The movement predictor exists as a narrow service, but future prediction needs a bounded view over a richer client replica.
- NPC/AI presentation, remote player interpolation, block-action speculation, lighting, fluids, and movement all need the same client-world ownership answer.
- Network transport choices should remain adapters over logical records, not define the runtime model.

## Source Review

Use these vanilla references before touching architecture:

- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java` integrated-server bootstrap and memory connection
- `reference/minecraft-1.17.1/src/net/minecraft/client/server/IntegratedServer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java`

The target is not a class-for-class Java thread model port. The target is a faithful ownership split adapted to browser workers, WebGPU, Node, and our logical protocol.

## Target Terms

| Term | Meaning |
|---|---|
| `IntegratedServer` | browser-singleplayer authoritative server facade; wraps the same host authority core as dedicated server, but is created and owned by the local game session |
| `DedicatedServer` / headless host | Node-backed authoritative server using the same authority core and protocol semantics |
| `ClientRuntime` | client session owner: transport adapter, protocol application, `ClientWorld`, prediction/interpolation services, presentation-state publication |
| `ClientWorld` | client replica of visible/interested world facts: chunks, block/fluid states, block entities, entities, light/render facts, revisions, and speculative overlays |
| `PredictionService` | high-rate command/replay/reconcile service over bounded `ClientWorld` collision/entity views |
| Presentation/UI thread | input sampling, pointer lock, UI, GPU resources, draw submission, compact presentation-state consumption |

## Ownership Rules

- Singleplayer and multiplayer clients must hydrate `ClientWorld` from the same logical protocol records.
- The presentation/UI thread must not own raw chunk sections, collision caches, replay buffers, or host objects.
- `ClientWorld` is a client replica, not a server clone. It does not run worldgen, persistence, block ticks, liquid ticks, spawning authority, NPC AI authority, or host scheduling.
- Host authority remains the only owner of gameplay consequences.
- Prediction reads bounded collision/entity views from `ClientWorld`; it never reads `IntegratedServer` or dedicated-host internals.
- Render-world/mesh workers produce derived render products. Meshes are never collision truth.
- Transport adapters carry logical records. HTTP polling, worker `postMessage`, WebSocket, or future WebRTC/WebTransport do not define gameplay semantics.

## Clock-Rate Safeguards

`ClientRuntime0` should protect future high-rate FPS movement without implementing it.

Keep these clocks separate in names, interfaces, and docs:

- host world/block/entity tick
- player command clock
- player physics step / fixed command quantum
- snapshot publish cadence
- transport poll/send/push cadence
- render frame / presentation interpolation cadence

Safeguards:

- `ClientWorld` stores replica facts and revisions; it does not decide movement step rate.
- `PredictionService` is the future owner of fixed-quantum replay over bounded `ClientWorld` views.
- UI/render may sample input every frame, but render frame delta must not become authoritative movement `dt`.
- Host authority may drain multiple command records inside one lower-rate world/network tick.
- Protocol records must keep command sequence, command quantum, step count, ack sequence, and revision facts explicit.
- Avoid generic API names that imply one global tick drives everything. Prefer names that reveal the clock, such as `applyWorldUpdate`, `publishPresentationState`, `advanceCommandReplay`, or `drainMovementCommands`.

## Scope

Implement or document enough to make the boundary concrete:

| # | Work | Expected result |
|---|---|---|
| 1 | Runtime inventory | map current `WorldHost`, `WorldClient`, transports, render-world cache, movement predictor, entity snapshots, light/liquid flows to the target terms |
| 2 | Naming plan | decide whether to introduce new classes now or create aliases/facades over existing host/client types first |
| 3 | `IntegratedServer` facade | define the browser-singleplayer authoritative server wrapper and how it differs from the dedicated/headless host only by adapter/ownership |
| 4 | `ClientWorld` contract | define the client-replica facts it owns and what APIs expose render, interpolation, and prediction views |
| 5 | `ClientRuntime` contract | define protocol application, session ownership, transport adapter boundary, and presentation-state output |
| 6 | Prediction boundary | define how existing movement command/replay code will eventually attach without owning the whole client world |
| 7 | Presentation boundary | document or test that UI/render consumes presentation state and mesh handles, not host/client-world mutable internals |
| 8 | Migration notes | list exact follow-up slices needed to move existing code without breaking browser smoke, remote host smoke, lighting, liquids, or entity publication |
| 9 | Clock vocabulary | define names and interfaces that keep world ticks, command quanta, snapshot cadence, transport cadence, and render frames separate |

## Runtime Inventory

Current code maps to the target ownership terms like this:

| Current code | Target term | Boundary note |
|---|---|---|
| `GeneratedWorldHost` | shared authoritative host core | owns worldgen, persistence, liquid ticks, entity snapshots, light publication, session/player state, and pending host-to-client updates |
| `generated-world-worker.ts` + `WorkerWorldTransport` | browser `IntegratedServer` adapter | browser singleplayer authority currently lives in a worker and speaks the same logical `WorldHost` messages as other transports |
| `headless-generated-world-host.ts` + `generated-world-http-server.ts` | dedicated/headless host adapter | Node entry points reuse `createGeneratedWorldHostForRequest(...)`; remote HTTP is a transport adapter, not a separate authority model |
| `WorldHost` | authoritative host protocol surface | open/create world, set chunk interest, accept player commands, and publish updates through logical records |
| `TransportWorldClient`, `WorkerWorldClient`, `RemoteWorldClient` | current `ClientRuntime` implementation shape | owns protocol application, session/player/entity caches, polling, and chunk-update routing, but is still named after transport |
| `ClientChunkCache` | current `ClientWorld` fact cache and render view | stores visible chunk snapshots, block/fluid states through block states, biome containers, light data, and render-level queries |
| `render-world-worker.ts` | render-world/client-world derived worker | owns meshing input cache and produces derived render products; meshes remain outside collision truth |
| `LocalWorldTransport`, `WorkerWorldTransport`, `RemoteWorldTransport` | transport adapters | carry logical records over direct calls, `postMessage`, or HTTP without owning gameplay semantics |
| `runtime/movement/*` | paused prediction-service implementation parts | command clocks, command buffers, and predictor exist, but future client prediction must read bounded `ClientWorld` views |
| lighting worker/client and liquid host code | host-owned simulation plus client-facing facts | authoritative outputs flow through chunk snapshots/deltas; no client fluid prediction is introduced here |

## Naming Plan

`ClientRuntime0` introduces facade names before moving boot flow:

| Name | First implementation step |
|---|---|
| `IntegratedServer` | `src/runtime/host/integrated-server.ts` wraps any `WorldHost` and labels browser-singleplayer authority without changing the shared host core |
| dedicated/headless host | `DedicatedServerHost` is a type alias for the same `WorldHost` protocol surface |
| `ClientRuntime` | `src/runtime/client/client-runtime.ts` wraps existing `WorldClient` calls with owner-oriented method names |
| `ClientWorld` | `src/runtime/client/client-world.ts` exposes a replica facade over current session/player/entity/chunk/light facts |
| `PredictionService` | `src/runtime/client/prediction-service.ts` wraps the existing `PlayerMovementPredictor` behind client-world prediction views |
| presentation state | `ClientRuntime.publishPresentationState()` returns session/player/entity/perf state without exposing host objects |

This is intentionally a facade step. It does not yet replace the browser bootstrap, move chunk hydration, or make `ClientChunkCache` the final client-world implementation. It gives the next slice stable names to wire through existing code.

## Boundary Contracts

`IntegratedServer` and dedicated/headless hosts both implement `WorldHost`; the distinction is ownership and adapter placement. Browser singleplayer creates and owns an integrated host worker, while the Node path owns a headless host behind HTTP. Both keep the same authority core and message semantics.

`ClientRuntime` owns session orchestration and protocol application. Its method names keep clocks visible:

| Method | Clock or cadence |
|---|---|
| `setChunkInterest(...)` | client interest changes, currently view-shaped |
| `sendPlayerCommand(...)` | player command clock |
| `drainTransportUpdates()` | transport poll/push cadence |
| `publishPresentationState()` | render/presentation sampling cadence |

`ClientWorld` owns replica facts and exposes views:

| View | Purpose |
|---|---|
| render view | transitional access to the current `ClientChunkCache` for render-world and meshing consumers |
| prediction view | bounded `CollisionWorld` construction plus movement/collision revision facts |
| revision facts | session, local-player, movement-physics, and collision revisions that future reconciliation can compare |

`PredictionService` is deliberately smaller than a movement system. It advances or reconciles existing command replay against a `ClientWorldPredictionView`; it does not own worldgen, transport polling, render frames, NPC AI, or fluid prediction.

## Architecture Divergence Review

Vanilla Java starts an `IntegratedServer`, waits for readiness, opens a memory channel, and lets the normal client login path hydrate `ClientLevel` and `ClientChunkCache`. `mclone` keeps that ownership split but diverges in carriers: browser singleplayer uses workers and `postMessage`; remote play uses HTTP polling today; Node uses a headless process. The divergence is limited to runtime adapters and facade names. Simulation/content parity stays in the shared host core, and future parity work can still follow the vanilla host-to-client fact flow.

## Migration Notes

Follow-up slices should move one boundary at a time:

1. `ClientRuntime1`: make `ClientWorld` the single hydration target for chunks, light, fluids, entities, revisions, and speculative overlays. Keep worker and remote clients on the same path.
2. `ClientRuntime2`: make UI/render consume presentation state plus render-world mesh handles, not mutable host or raw client-world internals.
3. `ClientRuntime3`: wire browser singleplayer bootstrap through the `IntegratedServer` facade and local transport naming.
4. `ClientRuntime4`: prove remote HTTP clients use the same `ClientRuntime`/`ClientWorld` path as singleplayer.
5. `ClientRuntime5`: attach prediction to explicit `ClientWorldPredictionView` collision/entity facts without changing movement physics.
6. `ClientRuntime6`: add entity interpolation and NPC presentation hooks while keeping AI authority on the host.

## Do Not Add

- new movement physics or correction smoothing
- NPC AI behavior
- WebSocket/WebRTC/WebTransport implementation
- new fluid prediction
- renderer feature work
- broad file moves without a clear adapter/facade path

## Validation

Minimum validation for this tactical:

- architecture docs name the runtime owners consistently
- tactical README pauses movement work and points to the client runtime arc
- clock/rate safeguards are recorded in active architecture docs, not only in paused movement notes
- existing browser singleplayer, remote host, entity, lighting, and liquid docs still have a clear owner after the rename/facade plan
- type-level interfaces or facade names are small enough that the next slice can implement them without a rewrite
- `pnpm typecheck` if code changes land
- `git diff --check`

If this slice stays docs-only, `git diff --check` is enough.

## Done When

- `IntegratedServer`, dedicated host, `ClientRuntime`, `ClientWorld`, `PredictionService`, and presentation/UI thread mean specific things in the repo.
- Movement, NPC/AI, and network follow-up work all point at the same client-world architecture instead of each inventing a partial model.
- The next tactical can safely implement the first facade/contracts without deciding movement physics, WebRTC, or AI scheduling.

## Next Step

`ClientRuntime1-client-world-replica-hydration.md`: introduce the first concrete `ClientWorld` facade over existing chunk/light/entity/session data and make singleplayer and remote clients hydrate it through the same protocol-facing path.
