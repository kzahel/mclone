# Bot0 - Headless client runtime

Standing after the landed `ClientRuntime0` through `ClientRuntime6` arc, `R9` WebSocket transport, and `Movement3` player command integration. The browser client already joins a local or dedicated authority through `ClientRuntime` and sends sequenced player input. This slice gives bots the same client path without a renderer.

Status: **done**.

Landed result: `src/runtime/bot/` now exposes a renderer-free bot runtime and client-runtime bootstrap. `pnpm bot:client` launches a Node bot against a dedicated WebSocket server, joins as a named player, keeps chunk interest centered on the bot player, and emits sequenced movement commands through the same `ClientRuntime` / `ClientWorld` / `set_player_input` path as browser clients.

## Goal

Create a reusable headless bot client that can join a generated-world authority, hydrate a `ClientWorld`, and submit player movement commands through the same client/runtime/protocol path as a browser player.

At the end of `Bot0`, `mclone` should have:

- a Node CLI entrypoint for launching one bot against a dedicated server
- a reusable bot runtime that can also run in tests against local or fake transports
- no renderer, mesh worker, browser DOM, or direct host-world access in the bot path
- a minimal controller that can stand, look around, and optionally walk in a scripted pattern
- enough logging/status output to prove the bot is connected, receiving chunks, and advancing player state

## Intent

The bot is a fake human client, not a server-side NPC. It should use the same shape as the browser multiplayer client:

```text
DedicatedServer / IntegratedServer
  -> WorldTransport
  -> WorldClient
  -> ClientRuntime
  -> ClientWorld replica
  -> BotRuntime
  -> BotController
  -> set_player_input
```

The bot may inspect memory directly, but only through hydrated client-owned state such as `ClientWorld`, `ClientChunkCache`, player snapshots, entity snapshots, and prediction/collision views. It must not read `GeneratedWorldHost` internals or run worldgen to fill missing data.

## Source Review

Review these current project files before implementation:

| Concern | Source |
|---|---|
| Runtime facade and presentation state | `src/runtime/client/client-runtime.ts` |
| Client replica, chunks, entities, prediction view | `src/runtime/client/client-world.ts` |
| Logical client protocol | `src/runtime/protocol/world-client.ts`, `src/runtime/protocol/world-messages.ts` |
| Local and remote transports | `src/runtime/transport/local-world-transport.ts`, `src/runtime/transport/remote-world-transport.ts` |
| Dedicated server entrypoint/config shape | `src/runtime/node/generated-world-http-server.ts` |
| Player command clock and command conversion | `src/runtime/movement/movement-command-clock.ts`, `src/runtime/session/player-loop.ts` |
| Browser player input semantics | `src/renderer/debug/debug-player-controls.ts`, `src/renderer/gui/gpu-world-runtime.ts` |

Useful vanilla source orientation, not a direct port target:

| Concern | Vanilla source |
|---|---|
| Client local player packet/input role | `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java` |
| Client world replica role | `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java` |
| Client packet listener hydration | `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java` |

## Scope

| # | Work | Expected result |
|---|---|---|
| 1 | Bot runtime module | done - `src/runtime/bot/` has `BotRuntime`, lifecycle options, controller interface, and close semantics |
| 2 | Client bootstrap helper | done - `createBotClientRuntime(...)` builds a headless `ClientRuntime` from an injected `WorldTransport` and a `ClientChunkCache` level factory |
| 3 | Node CLI entrypoint | done - `src/runtime/node/bot-client.ts` plus `pnpm bot:client` accept URL, name, seed, preset, view radius, tick rate, controller, and max ticks |
| 4 | Transport selection | done - the CLI uses `RemoteWorldWebSocketTransport`; runtime tests inject a fake transport |
| 5 | Minimal controller | done - idle and wander controllers emit valid sequenced `set_player_input` commands without gameplay goals |
| 6 | Chunk interest loop | done - chunk interest stays centered on the bot's authoritative player position |
| 7 | Runtime tests | done - tests cover open/join, session/player state hydration, chunk-interest submission, command sequencing, CLI parsing, and close cleanup |

## CLI Shape

Target command:

```bash
pnpm bot:client -- --url ws://127.0.0.1:4173/api/world/socket --name ViewBot --seed 12345 --preset default --radius 2 --controller wander
```

The first implementation can require a dedicated server already running through:

```bash
pnpm host:dedicated
```

Do not make the bot process implicitly start or own the dedicated server in `Bot0`. Test harnesses can still construct local transports directly.

## Architecture Divergence Review

Minecraft does not have a separate "bot client" product path in vanilla. The closest shape is a normal remote client: local input produces movement packets, the server remains authoritative, and the client world is a hydrated replica.

The `mclone` divergence is a new Node-side client entrypoint with no renderer. This is a good fit because:

- Node bots cannot depend on browser DOM/WebGPU/pointer-lock APIs
- testing benefits from the same reusable connection/runtime code
- the authority boundary stays identical to multiplayer browser clients
- future bot goals can be tested without spawning browser pages

The divergence is narrow: only input policy and presentation are replaced. World authority, movement commands, chunk interest, session identity, and client-world hydration remain the existing protocol model.

## Do Not Add

- pathfinding, block scanning, or goal selection beyond a trivial scripted controller
- block interaction, mining, placing, inventory, chat, or combat
- server-side NPC state or host-owned AI
- renderer, mesh worker, GPU resources, DOM, or screenshot output
- client-side worldgen fallback for missing chunks
- vanilla mob AI or `Mob`/`PathNavigation` ports

## Validation

Completed:

```bash
pnpm test -- test/runtime/bot-runtime.test.ts
pnpm test -- test/runtime/remote-world-transport.test.ts
pnpm typecheck
git diff --check
```

The first `remote-world-transport` run hit sandbox `listen EPERM` on `127.0.0.1`; rerunning the same test with local socket binding allowed passed.

Useful integration gate when the dedicated server path is touched:

```bash
pnpm test -- test/runtime/dedicated-server-startup.test.ts
```

Browser validation is not required for `Bot0` unless shared client-runtime or transport behavior changes.

## Done When

- [x] `pnpm bot:client` can connect to a running dedicated server and join as a named player profile.
- [x] The bot hydrates `ClientWorld` through `ClientRuntime`, not host internals.
- [x] Chunk interest follows the bot player position.
- [x] The bot emits valid movement commands with monotonically increasing sequences.
- [x] Tests can run the same bot runtime against an injected transport without launching a browser.
- [x] Closing the bot releases transport/client resources cleanly.

## Next Step

[`Bot1-client-world-observation-and-navigation.md`](Bot1-client-world-observation-and-navigation.md): add observation and path-planning helpers over hydrated client-world facts so bot behavior and headless tests can query nearby blocks and reachable surfaces without duplicating world access code.
