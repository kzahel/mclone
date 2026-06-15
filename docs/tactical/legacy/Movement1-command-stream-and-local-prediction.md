# Movement1 - Command stream and local prediction

Standing after [`Movement0-shared-movement-body-and-collision.md`](Movement0-shared-movement-body-and-collision.md). `Movement0` landed the shared fixed-step body and full-block collision core. This slice adds the local command timeline and replay predictor that will later feed the authoritative host integration in `Movement2`.

## Goal

Add a deterministic player movement command layer without changing the live browser control path or `set_player_input` protocol yet.

At the end of `Movement1`, `mclone` should be able to:

- represent player movement as sequenced fixed-quantum command records
- convert command records into `MovementIntent`
- simulate command batches through the `Movement0` step function
- store unacknowledged commands in a bounded ring buffer
- snap prediction to authoritative state and replay unacknowledged commands
- report useful reconciliation diagnostics for the next protocol slice

## Status

Done. Landed as a runtime movement command/prediction foundation. The live `set_player_input` protocol and browser control path are intentionally unchanged.

## Implementation Status

Landed shape:

- `movement-command.ts` defines `PlayerMoveCommand`, command button bits, command validation, command-to-intent conversion, and command/batch simulation through `simulateMovementStep(...)`.
- `movement-command-clock.ts` converts elapsed microseconds into bounded fixed command step counts without creating variable-dt movement commands.
- `movement-command-buffer.ts` provides bounded ordered unacknowledged-command storage with ack dropping.
- `movement-predictor.ts` provides `PlayerMovementPredictor`, authoritative snap/replay reconciliation, and diagnostics for replay backlog, command dt mismatch, physics/collision revision mismatch, position/velocity error, grounded flip, jump-state mismatch, and missing collision data.
- `test/runtime/movement/movement-command.test.ts` covers command clock splitting, fixed-quanta command simulation, quick jump edge buttons, zero-command no-op behavior, ordered batches, and ring-buffer ack behavior.
- `test/runtime/movement/movement-predictor.test.ts` covers all-acked prediction parity, delayed ack replay, high command rate inside a lower-rate server tick, quick jump replay, and diagnostic classification.
- Protocol migration remains out of scope for this slice.

## Reference / Prior Art

Use the durable command/prediction architecture in [`../player-movement-netcode.md`](../player-movement-netcode.md).

Read these before implementing:

| Source | Why it matters |
|---|---|
| `src/runtime/movement/` | `Movement0` fixed-step body and collision core |
| `/Users/kgraehl/code/tilefun/src/client/PlayerPredictor.ts` | prior snap/replay predictor and diagnostics |
| `/Users/kgraehl/code/tilefun/src/server/InputQueuePrediction.test.ts` | input queue invariant: process every command, no stale latest-input slot |
| `/Users/kgraehl/code/tilefun/src/server/NetcodeParityBaseline.test.ts` | jitter, delayed ack, quick jump, and command-rate scenarios |
| `/Users/kgraehl/code/tilefun/docs/hard-won-knowledge.md` | no input overwrite and no phantom movement lessons |

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | Command shape | `PlayerMoveCommand` with sequence, fixed quantum, `stepCount`, buttons, wish axes, yaw/pitch, revision facts |
| 2 | Command clock helpers | convert elapsed time into bounded fixed command step counts without variable-dt movement |
| 3 | Command simulation | run one command or an ordered command batch through `simulateMovementStep(...)` |
| 4 | Ring buffer | bounded unacknowledged command storage with ack/drop and ordered replay |
| 5 | Predictor | local apply, authoritative snap, drop acked commands, replay unacked commands |
| 6 | Diagnostics | replay count/range, correction error, resim error, revision/dt mismatch tags |
| 7 | Unit tests | command-rate, jitter/drain, snap/replay, quick jump edge, delayed ack, and no variable-dt cases |

Do not add:

- live `set_player_input` protocol migration
- host command queue integration
- browser camera/control migration
- remote interpolation
- correction smoothing
- non-full block collision or collision revision enforcement beyond diagnostic tags

## Architectural Constraints

- Commands are ordered records. Do not introduce or depend on a mutable latest-input slot.
- `commandQuantumUs` and `stepCount` are the only movement time source in this layer.
- A command with `stepCount = 2` must simulate two fixed steps, not one doubled step.
- Snap/replay mutates simulation truth. Any smoothing belongs to a later presentation/integration slice.
- The predictor must keep enough diagnostics to distinguish replay backlog, command dt mismatch, physics revision mismatch, collision revision mismatch, position error, velocity error, grounded flip, jump-state mismatch, and missing collision data.
- This slice may define a protocol-ready command shape, but it must not wire that shape into the live host/client message model yet.

## Validation

Minimum:

- `pnpm test -- test/runtime/movement/movement-command.test.ts test/runtime/movement/movement-predictor.test.ts`
- `pnpm typecheck`
- `git diff --check`

Run the existing `Movement0` movement-step test too if command simulation touches the step API.

Landed validation:

- `pnpm test -- test/runtime/movement/movement-step.test.ts test/runtime/movement/movement-command.test.ts test/runtime/movement/movement-predictor.test.ts`
- `pnpm typecheck`

## Done When

- Command records can drive the same body simulation on both "client" and "server" test paths.
- Local prediction matches server command-drain results when all commands are acknowledged.
- Delayed acknowledgements snap to authoritative state and replay unacked commands in order.
- Quick jump taps survive via `edgeButtons`.
- Docs mark `Movement1` done and point to `Movement2-authoritative-host-command-integration.md` as next.

## Next Step

`Movement2-authoritative-host-command-integration.md`: migrate the current simple player/session loop toward host-owned command queues, acked movement snapshots, and a compatibility path from existing browser controls to command records.
