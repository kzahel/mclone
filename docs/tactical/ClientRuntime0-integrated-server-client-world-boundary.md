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
