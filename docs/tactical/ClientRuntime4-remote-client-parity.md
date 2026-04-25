# ClientRuntime4 - Remote client parity

Standing after [`ClientRuntime3-integrated-server-flow.md`](ClientRuntime3-integrated-server-flow.md), which made browser worker singleplayer explicitly join a local `IntegratedServer` facade while preserving the shared client runtime and client-world hydration path.

Status: **done**.

Landed result: remote HTTP clients now have explicit close behavior and focused parity coverage proving remote state is visible through `ClientRuntime.publishPresentationState()` and `ClientWorld` views. Reconnect/session restoration remains owned by `RemoteWorldTransport`, while hydration still flows through the shared `ClientWorld`.

## Goal

Prove and tighten remote HTTP clients so they use the same `ClientRuntime` / `ClientWorld` path as browser integrated-server clients, with transport differences confined to carriers and reconnect/session restoration.

Remote clients should not grow their own presentation state, replica mutation, chunk-interest semantics, or player-state polling model. The remote path is a dedicated-host transport adapter over the same logical records.

## Source Review

Re-read the current remote and shared client paths before changing code:

- `src/runtime/transport/remote-world-transport.ts`
- `src/runtime/node/generated-world-http-server.ts`
- `src/runtime/client/client-runtime.ts`
- `src/runtime/client/client-world.ts`
- `src/renderer/scene-setup.ts`
- `test/runtime/remote-world-transport.test.ts`
- `test/browser/smoke.test.ts`

Vanilla ownership reminder:

- `ClientPacketListener` hydrates `ClientLevel` from server packets regardless of whether the server is local or remote.
- The remote/dedicated distinction changes connection/session transport, not client replica ownership.

## Scope

| # | Work | Expected result |
|---|---|---|
| 1 | Audit remote-specific client assumptions | identify any direct `RemoteWorldClient` behavior that bypasses `ClientRuntime` / `ClientWorld` |
| 2 | Normalize lifecycle and close behavior | remote client close is explicit even if it only drops local sinks/session handles for now |
| 3 | Keep reconnect/resume transport-owned | session restoration remains inside `RemoteWorldTransport`, with hydrated messages still applied by `ClientWorld` |
| 4 | Tighten tests | remote unit tests assert state is visible through `ClientRuntime.publishPresentationState()` and `ClientWorld` views |
| 5 | Preserve two-client browser smoke | remote browser path still uses the same scene/runtime flow as worker singleplayer |

## Do Not Add

- WebSocket/WebRTC/WebTransport
- push updates
- new movement semantics or prediction
- client-side generation fallback
- presentation or renderer-specific remote state
- dedicated-host scheduling changes

## Validation

- `pnpm typecheck`
- focused remote runtime tests
- `pnpm test:browser`
- `pnpm test:browser:integration` if touching presentation-state consumption, polling cadence, player state, or chunk-interest movement
- `git diff --check`

## Done When

- Remote HTTP and worker integrated-server clients both enter through `ClientRuntime` and hydrate the same `ClientWorld` implementation.
- Remote reconnect/resume remains a transport concern and does not mutate client replica state directly.
- Presentation and debug code do not branch on remote-vs-worker client state access.
- Existing two-client remote browser smoke remains green.

## Next Step

`ClientRuntime5-prediction-service-scaffold.md`: attach prediction-service APIs to explicit `ClientWorldPredictionView` facts without changing movement physics.
