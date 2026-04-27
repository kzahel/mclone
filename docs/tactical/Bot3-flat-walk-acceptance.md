# Bot3 - Flat walk acceptance

Status: done.

This slice adds the smallest useful bot behavior for movement acceptance: choose one nearby point on a flat world, walk there using normal player input commands, and assert that the authoritative remote host accepts enough commands for the bot to arrive.

## Why

The good-view behavior proves bot-side observation and path selection, but it is too complex for diagnosing basic remote movement. A flat-grass point walk gives a lower-noise failure signal for rubber-banding symptoms:

- if commands are rejected, the acknowledged input sequence does not advance
- if collision data is missing, the player does not settle on the flat surface
- if authoritative state is not delivered back, the bot never observes arrival
- if steering is wrong, the final distance to the target stays large

## Landed

- `WalkToPointBotController` in `src/runtime/bot/bot-point-walk-goal.ts`
- `--goal walk-to-point` in `pnpm bot:client`
- `test/runtime/bot-flat-walk-integration.test.ts`

The acceptance test opens a `flat_grass` world in two ways:

- in-memory remote service through `RemoteWorldTransport`
- real localhost `GeneratedWorldHttpServer` through `RemoteWorldWebSocketTransport`

Both cases boot a real `BotRuntime`, wait for the center chunk, advance the authoritative host tick, drain client updates, and confirm the bot reaches its deterministic nearby target.

## Validation

Completed:

```bash
pnpm test -- test/runtime/bot-flat-walk-integration.test.ts
```

The sandboxed run hits `listen EPERM` for the WebSocket case on hosts that disallow localhost binding. Rerun the same command with approval; the WebSocket lane passed under that path.

Run with related bot coverage:

```bash
pnpm test -- test/runtime/bot-flat-walk-integration.test.ts test/runtime/bot-runtime.test.ts test/runtime/bot-good-view-goal.test.ts
pnpm typecheck
git diff --check
```

## Next Step

Use this test as the baseline for any movement-authority debugging. If rubber-banding still appears manually, run `--goal walk-to-point` against a long-lived dedicated server and compare command acknowledgements, player-state revisions, and final authoritative position against this passing WebSocket acceptance path.
