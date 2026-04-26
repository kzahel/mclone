# Movement3 - Basic player movement integration

Standing after [`Movement2-authoritative-host-command-integration.md`](Movement2-authoritative-host-command-integration.md) and the Tactical 53 R9/player-slot prerequisites. `Movement2` made movement a sequenced command stream, but the live browser player path still used a compatibility fly body. This slice connects the existing shared movement core to the live host, client prediction, and debug player input path.

## Goal

Replace the compatibility fly integrator with host-authoritative shared movement physics while preserving the existing logical command/snapshot protocol.

At the end of `Movement3`, `mclone` should:

- simulate live player commands through the shared movement body and fixed command quanta
- read host collision from authoritative generated chunks in local worker singleplayer
- read remote host collision from pushed packed chunk snapshots in the dedicated Node service
- leave commands queued and unacked when required collision chunks are missing
- keep browser remote transport and player-slot semantics unchanged
- feed debug player-mode input as local wish axes plus jump/crouch/sprint buttons
- run browser prediction/reconciliation through `ClientRuntime.getPredictionService()`
- derive the player camera from movement body position plus eye height

## Status

Implemented in this slice. Local worker singleplayer and remote WebSocket clients now use the same command records, movement body snapshots, collision availability checks, and prediction service path for basic player movement.

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | `runtime/session/player-loop` | command queue drains through `simulatePlayerMoveCommand(...)`, snapshots the real movement body, and preserves unprocessed commands on missing collision |
| 2 | `runtime/host/generated-world-host` | local generated-world host exposes a collision world backed by loaded authority chunks |
| 3 | `runtime/node/generated-world-http-server` | remote service keeps a server-side collision cache hydrated from authoritative chunk snapshots and unloads |
| 4 | `renderer/debug/debug-player-controls` | player mode emits local wish axes and button bits; free camera keeps the old camera-space flight command helper |
| 5 | `renderer/debug/debug-free-cam` | debug player mode reconciles and advances prediction through `ClientRuntime` / `PredictionService` |
| 6 | Runtime tests | player queue, remote transport, generated-world boundary, movement core, prediction service, and debug input coverage |

Do not add:

- vanilla-perfect movement constants
- non-full collision shapes
- liquids, swimming, ladders, or climbables
- crouch body resizing
- entity collision
- inventory, game mode, or persisted player data
- profile-based persisted-slot recovery

## Compatibility Choices

`moveY` remains in `PlayerInputCommand` for protocol compatibility, but player mode no longer treats it as flight. The debug free camera has a separate helper that preserves the old camera-space movement behavior.

The default player spawn Y is still provisional. This avoids mixing movement integration with surface-aware spawn/respawn selection; the follow-up should replace the fixed height with an authoritative spawn placement path before longer-lived player persistence depends on it.

The remote Node service builds its collision cache from the same pushed packed snapshots that clients receive. That keeps collision availability tied to server-originated world facts instead of introducing a gameplay-specific network path.

## Validation

Minimum:

- `pnpm test -- test/runtime/player-loop.test.ts test/renderer/debug-player-controls.test.ts`
- `pnpm test -- test/runtime/movement/movement-step.test.ts test/runtime/movement/movement-command.test.ts test/runtime/movement/movement-predictor.test.ts test/runtime/client-prediction-service.test.ts test/runtime/generated-world-boundary.test.ts test/runtime/generated-world-host-scheduler.test.ts`
- `pnpm test -- test/runtime/remote-world-transport.test.ts`
- `pnpm typecheck`
- `pnpm test:browser`
- `pnpm test:browser:integration`
- `git diff --check`

## Done When

- Host player ticks no longer call the compatibility fly integrator.
- Missing collision data prevents command acknowledgment instead of producing silent movement.
- Local worker and remote WebSocket clients share the same movement command/snapshot path.
- Debug player input and camera prediction no longer behave as free flight.
- The remaining player-slot identity shortcut is limited to the documented persisted-profile recovery follow-up.

## Next Step

Add a movement polish slice for authoritative spawn/respawn placement and collision fidelity. Start by replacing the provisional fixed Y spawn with surface-aware placement from generated world facts, then widen collision behavior beyond the current full-block-only baseline where it affects basic play.
