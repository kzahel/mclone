# ClientRuntime3 - Integrated server flow

Standing after [`ClientRuntime2-presentation-thread-boundary.md`](ClientRuntime2-presentation-thread-boundary.md), which made browser UI/render consume `ClientRuntime` lifecycle methods and presentation state instead of raw transport client APIs.

Status: **done**.

Landed result: browser worker singleplayer now goes through `createBrowserIntegratedServer()`, which wraps the worker transport in the shared `IntegratedServer` facade. The facade exposes created/running/paused/closed lifecycle state, rejects use after close, and close propagates through `ClientRuntime` to the worker transport.

## Goal

Make browser singleplayer visibly shaped like a client joining a local integrated server.

The local browser path should explicitly create an `IntegratedServer` facade, connect the client runtime through the same logical host/client messages, and expose lifecycle semantics for ready/running/paused/closed. This is still a boundary slice; it must not change worldgen, movement, transport protocol, or host scheduling behavior.

## Source Review

Re-read these vanilla ownership points before changing browser singleplayer flow:

- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java`
  - `doLoadLevel(...)`: creates `IntegratedServer`, waits for readiness, starts a memory channel, then lets normal client login hydrate `ClientLevel`.
  - `clearLevel(...)`: disconnects the client and waits for the integrated server to shut down.
- `reference/minecraft-1.17.1/src/net/minecraft/client/server/IntegratedServer.java`
  - integrated server is not a different simulation authority; it is a local server with pause/LAN/lifecycle behavior.

`mclone` diverges only in carriers: browser singleplayer uses a worker and `postMessage` instead of Java memory connections.

## Scope

| # | Work | Expected result |
|---|---|---|
| 1 | Add explicit browser integrated-server factory | worker singleplayer creates an `IntegratedServer` facade instead of directly constructing a worker transport in scene setup |
| 2 | Add lifecycle naming | integrated server reports created/running/paused/closed state and rejects use after close |
| 3 | Add close/pause/resume hooks | local runtime can close its worker transport; pause/resume are explicit lifecycle state hooks without new simulation semantics |
| 4 | Preserve protocol path | client runtime still hydrates `ClientWorld` from logical host messages |
| 5 | Add focused tests | cover integrated-server lifecycle and worker transport close behavior |

## Do Not Add

- new transport protocol or push transport
- movement prediction, smoothing, or physics changes
- host tick scheduling changes
- client-side worldgen, decoration, or seed fallback
- renderer/GPU/meshing behavior changes
- LAN publishing or multiplayer discovery

## Validation

- `pnpm typecheck`
- focused runtime tests for integrated-server lifecycle / worker close behavior
- `pnpm test:browser`
- `pnpm test:browser:integration`
- `git diff --check`

`pnpm test:browser:integration` remains required because this slice touches browser singleplayer bootstrap and runtime lifecycle plumbing used by debug free-cam.

## Done When

- [x] Browser worker singleplayer is created through an `IntegratedServer` facade.
- [x] `ClientRuntime` can close the local runtime path and drop render-world sink ownership.
- [x] Integrated-server lifecycle state is explicit and tested.
- [x] Remote HTTP remains a dedicated-host transport path, not an integrated-server path.
- [x] Current browser smoke and debug free-cam integration pass.

## Validation Run

- `pnpm -s vitest run test/runtime/integrated-server.test.ts test/runtime/worker-world-transport.test.ts`
- `pnpm typecheck`
- `pnpm test:browser`
- `pnpm test:browser:integration`
- `git diff --check`

## Next Step

`ClientRuntime4-remote-client-parity.md`: prove the remote HTTP client and worker integrated-server client stay on the same `ClientRuntime`/`ClientWorld` path and remove any remaining remote-specific client assumptions.
