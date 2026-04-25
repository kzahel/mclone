# ClientRuntime5 - Prediction service scaffold

Standing after [`ClientRuntime4-remote-client-parity.md`](ClientRuntime4-remote-client-parity.md), which proved remote HTTP clients and browser worker singleplayer both hydrate the same `ClientRuntime` / `ClientWorld` path.

Status: **done**.

Landed result: `ClientRuntime` now exposes a `PredictionService` seeded from authoritative movement facts in `ClientWorld`. `ClientWorldPredictionView` carries bounded prediction inputs: collision view construction, entity snapshots, authoritative local movement state, and movement/collision revision facts. The prediction service consumes those view facts without changing movement physics, transport cadence, or host authority.

## Goal

Attach the existing movement predictor to the Client Runtime architecture without making prediction own host state, renderer state, or new movement semantics.

Prediction must read from `ClientWorldPredictionView`:

- authoritative local movement body and acknowledged command sequence
- movement physics and collision revision facts
- bounded collision queries over hydrated client chunks
- entity snapshots for future prediction/interpolation consumers

## Source Review

Reviewed current scaffold and ownership surfaces:

- `src/runtime/client/client-runtime.ts`
- `src/runtime/client/client-world.ts`
- `src/runtime/client/prediction-service.ts`
- `src/runtime/movement/movement-predictor.ts`
- `src/runtime/movement/movement-command.ts`
- `src/runtime/session/player-loop.ts`
- `test/runtime/movement/movement-predictor.test.ts`
- `test/runtime/render-world-update-sink.test.ts`

## Scope

| # | Work | Result |
|---|---|---|
| 1 | Expose prediction service from `ClientRuntime` | `WorldClientRuntimeFacade.getPredictionService()` lazily seeds a service from hydrated client-world movement facts |
| 2 | Expand prediction view facts | `ClientWorldPredictionView` now exposes authoritative movement state and entity snapshots in addition to collision and revisions |
| 3 | Keep reconcile inputs view-owned | `PredictionService` derives current movement/collision revision options from the supplied client-world prediction view |
| 4 | Keep host command surface unchanged | `sendPlayerCommand(...)` remains the host command submission API; no protocol or physics changes |
| 5 | Add focused tests | unit coverage proves runtime seeding and prediction-view reconcile facts |

## Do Not Add

- new movement physics, smoothing, or correction presentation
- client-authoritative movement consequences
- renderer-thread replay buffers
- client-side generation fallback for collision
- transport protocol changes or push transport
- NPC AI behavior

## Architecture Divergence Review

Vanilla singleplayer and multiplayer both hydrate a client-side `ClientLevel`/`ClientChunkCache` from server packets, while local prediction and presentation read client-side state. `mclone` keeps that ownership split but adapts the carrier/runtime shape: browser singleplayer and remote HTTP both hydrate `ClientWorld`, and `PredictionService` reads only bounded client-world views. This divergence is narrow and helps future parity because prediction no longer needs to know whether facts came from a worker integrated server or a dedicated HTTP host.

## Validation Run

- `pnpm -s vitest run test/runtime/client-prediction-service.test.ts test/runtime/render-world-update-sink.test.ts test/runtime/movement/movement-predictor.test.ts`
- `pnpm typecheck`
- `pnpm test:browser`
- `pnpm test:browser:integration`
- `git diff --check`

## Done When

- [x] `ClientRuntime` exposes a prediction-service API.
- [x] Prediction seeding comes from hydrated `ClientWorld` movement facts.
- [x] Prediction/reconcile code reads collision and revision facts from `ClientWorldPredictionView`.
- [x] Host command submission remains unchanged.
- [x] No movement physics or transport behavior changes were introduced.

## Next Step

`ClientRuntime6-entity-interpolation-and-ai-bridge.md`: define client entity interpolation buffers and NPC presentation hooks while keeping AI authority on the host.
