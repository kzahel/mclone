# Movement2 - Authoritative host command integration

Standing after [`Movement1-command-stream-and-local-prediction.md`](Movement1-command-stream-and-local-prediction.md). `Movement1` landed command records, fixed command quanta, and local snap/replay prediction. This slice migrates the live host/session path away from a mutable latest-input slot and toward host-owned command queues with authoritative ack snapshots.

## Goal

Make sequenced movement commands the logical host/player contract without committing to a final network transport.

At the end of `Movement2`, `mclone` should:

- enqueue `set_player_input` records by sequence on the authoritative host
- drain ordered commands on player ticks and ack only consumed commands
- avoid phantom movement on ticks with no queued command
- carry command timing fields (`commandQuantumUs`, `stepCount`) through the existing protocol shape
- publish authoritative player snapshots with enough movement-body state for later replay/reconciliation
- keep current local worker and remote HTTP transports as adapters, not as the movement model
- preserve the current debug free-fly browser control behavior until the grounded/collision body is deliberately wired into the live player path

## Status

Implemented in this slice. The host/client boundary now treats player input as queued command records, while the browser debug control path emits fixed-quantum commands through the existing transport adapters.

## Transport Rule

Movement correctness is defined by the logical command stream:

```text
client samples input -> emits sequenced command records -> host queues by sequence
host drains commands -> simulates fixed command quanta -> snapshots body + ack sequence
client reconciles from ack snapshots -> replays unacked commands
```

HTTP polling is only one delivery adapter for those records. Movement2 must not depend on request cadence, poll cadence, or "latest input" behavior. WebSocket/WebTransport/WebRTC work belongs to later runtime slices and should carry the same logical command/snapshot records.

Keep these lane assumptions from [`../protocol.md`](../protocol.md):

- reliable ordered lane: session, chunk interest, chunk snapshots/deltas, interactions, inventory, errors, world/session state
- realtime superseding lane: movement command bundles and player/entity snapshots
- bulk/binary lane: packed chunk/light payloads that can use binary framing while remaining logically reliable

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | `PlayerInputCommand` protocol fields | optional command timing, button, edge-button, physics, and collision revision facts |
| 2 | Host command queue helpers | per-player ordered queue, stale/duplicate rejection, command-compatible adapter from existing browser input |
| 3 | Authoritative player ticking | drain queued commands and update ack only when commands are consumed |
| 4 | Snapshot body facts | position, velocity, bounds, movement mode, revision facts, command quantum, last processed command |
| 5 | Browser debug adapter | fixed command clock and FIFO send queue over current `set_player_input` |
| 6 | Runtime tests | multi-command drain, no empty-queue movement, fixed-quanta stepping, local/remote transport regression |
| 7 | Docs | protocol transport lanes and movement-specific delivery rules |

Do not add:

- WebSocket, WebRTC, or WebTransport implementation
- correction smoothing or interpolation buffers
- unreliable/lossy command delivery semantics
- final grounded FPS collision in the live browser path
- NPC locomotion bridge

## Compatibility Choice

The live debug browser path is still a free-fly camera/player adapter. Movement2 converts that input into command records, but it does not switch the browser player to the `Movement0` gravity/collision body yet. That avoids mixing a protocol migration with a gameplay/control migration.

The snapshot now includes a movement-body-shaped record in `ClientPlayerState.movementBody`. In this slice it is a compatibility body (`mode: "flying"`) derived from the authoritative command-drained position and velocity. Later slices can replace the compatibility simulator with the shared `Movement0` body while keeping the ack/snapshot contract.

## Architectural Constraints

- Ack sequence advances on command consumption, not command receipt.
- A host tick can drain more than one command and simulate each command's fixed quanta.
- A host tick with no queued commands must not reapply the previous input.
- `commandQuantumUs * stepCount` is the movement time source for command simulation, not render frame time, request time, or poll time.
- Duplicate or already-acked commands are ignored.
- Processing budgets cap catch-up work per player tick.
- Transports may bundle commands, but they must preserve per-command sequence and timing facts.
- Remote HTTP tests validate the adapter only; they do not define the movement network model.

## Validation

Minimum:

- `pnpm test -- test/runtime/player-loop.test.ts test/runtime/generated-world-boundary.test.ts test/runtime/generated-world-host-scheduler.test.ts test/runtime/remote-world-transport.test.ts test/renderer/debug-player-controls.test.ts`
- `pnpm test -- test/runtime/movement/movement-step.test.ts test/runtime/movement/movement-command.test.ts test/runtime/movement/movement-predictor.test.ts`
- `pnpm typecheck`
- `git diff --check`

## Done When

- `set_player_input` is no longer implemented as a latest-input slot.
- Local and remote hosts both enqueue and drain ordered movement commands.
- Authoritative snapshots ack the last processed command and include movement-body restart facts.
- Existing browser control behavior still works over the current transports.
- Docs state that Movement2 is transport-agnostic and does not choose HTTP polling as the movement model.

## Next Step

`Movement3-interpolation-and-correction-smoothing.md`: add local correction offsets, remote interpolation buffers, and artificial latency/jitter/loss tests against an in-memory transport so the command/replay model is proven before any push or lossy realtime transport work.
