# Bot2 - Good view goal

Standing after [`Bot1-client-world-observation-and-navigation.md`](Bot1-client-world-observation-and-navigation.md). Bots can join as headless clients, inspect hydrated client-world facts, identify standable surfaces, and steer toward path waypoints. This slice gives the first bot a personality-shaped objective: it likes finding a good view.

Status: done.

Landed result: `src/runtime/bot/bot-good-view-goal.ts` now exposes a reusable `BotGoal`-backed good-view policy. `pnpm bot:client -- --goal good-view` launches a renderer-free client that scores loaded standable surfaces, requires loaded visibility/path data, walks to a reachable high target through normal movement commands, stops on arrival, and reports wait/failure states for missing chunks, unavailable targets, and stuck movement.

## Goal

Implement the first meaningful bot behavior: a "good view" controller that scans the loaded client-world replica, chooses a reachable high vantage point, walks toward it through normal player commands, and idles/looks around when it arrives.

At the end of `Bot2`, a launched bot should feel like a simple client-controlled player with a goal:

- it joins a dedicated server as a named player
- it requests enough chunk interest to inspect nearby terrain
- it scores nearby loaded standable surfaces
- it picks a reachable high point with a broad surrounding view
- it walks there using the shared movement command path
- it reports what target it selected, why, and whether it arrived or got stuck

## Behavior Sketch

The first behavior does not need visual perception. "Good view" is a world-data score over loaded chunks:

```text
candidate score =
  height score
  + local openness score
  + horizon/sky exposure score
  - distance cost
  - path risk/stuck penalty
```

Simple first approximations are enough:

- height score: prefer higher standable Y positions
- openness score: count nearby air/empty headroom samples around the candidate
- horizon score: cast a small number of horizontal/diagonal block rays through loaded data and count how far they remain unobstructed
- distance cost: avoid choosing far targets unless the view improvement is meaningful
- reachability: require a path from `Bot1` before committing

The target should be re-evaluated when the client-world revision changes, the bot arrives, the path fails, or the selected target leaves loaded interest.

## Source Review

Review these current project files before implementation:

| Concern | Source |
|---|---|
| Bot foundation | `docs/tactical/Bot0-headless-client-runtime.md`, `docs/tactical/Bot1-client-world-observation-and-navigation.md` |
| Client runtime and world facts | `src/runtime/client/client-runtime.ts`, `src/runtime/client/client-world.ts` |
| Movement command clock | `src/runtime/movement/movement-command-clock.ts` |
| Player snapshots and command records | `src/runtime/protocol/world-messages.ts` |
| Browser movement input semantics | `src/renderer/debug/debug-player-controls.ts` |
| Dedicated server and WebSocket runtime | `src/runtime/node/generated-world-http-server.ts`, `src/runtime/transport/remote-world-transport.ts` |

Useful vanilla source orientation:

| Concern | Vanilla source |
|---|---|
| Client local player movement input | `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java` |
| Mob goal lifecycle concepts | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/goal/Goal.java` |

This is not a vanilla mob-goal port. The bot is a client-controlled player policy.

## Scope

| # | Work | Expected result |
|---|---|---|
| 1 | Goal interface | done - `BotGoal` lifecycle covers evaluate, activate, tick, complete/fail, and explain |
| 2 | Good-view scorer | done - scores height, openness, horizon exposure, distance, and path cost from loaded client-world facts |
| 3 | Target selection | done - selects reachable loaded targets, rate-limits rescans, and keeps targets stable without meaningful score gain |
| 4 | Path following | done - uses `Bot1` path planning and waypoint steering to emit normal player input commands |
| 5 | Arrival behavior | done - stops near the target and rotates slowly in place |
| 6 | Stuck handling | done - reports missing chunks, unavailable targets, blocked searches, and lack of progress |
| 7 | CLI mode | done - `--goal good-view` selects the first non-idle goal policy; `--controller idle|wander` remains a compatibility alias |
| 8 | Tests | done - scoring/selection, target stability, path-following, arrival, missing data, stuck handling, and CLI parsing are covered |

## Target Selection Rules

The behavior should make conservative choices:

- only evaluate surfaces in currently loaded chunks
- require loaded data for all sampled visibility rays
- reject candidates without enough headroom for the player body
- reject candidates without a path from the current bot position
- prefer nearby high points over distant marginally higher points
- keep the current target if it remains valid and still scores close to the best candidate
- rate-limit rescans so a busy chunk stream does not make the bot jitter

## Runtime Output

The CLI should print concise state transitions, for example:

```text
bot ViewBot joined save generated-12345-default as player player-...
bot ViewBot loaded 25 chunks near 0,0
bot ViewBot selected good-view target x=42 y=91 z=-18 score=137.4 path=38
bot ViewBot arrived at target x=42 y=91 z=-18
```

Avoid per-tick spam by default. A later debug flag can expose detailed scoring and path diagnostics.

## Architecture Divergence Review

This goal is deliberately original gameplay/test behavior. Vanilla mob goals run on the server and own mob AI authority. A bot client should instead behave like a remote human: it observes a client-world replica and submits normal player inputs to an authoritative host.

This makes future parity work easier because:

- host-owned mobs and client-owned bots remain separate concepts
- tests exercise the real multiplayer client path
- movement, chunk interest, prediction, and session handling all use existing runtime contracts
- no renderer-specific data is required for nonvisual bot perception

The divergence would become harmful if the bot started reading host internals, simulating chunks locally, or becoming the basis for vanilla mob AI. Keep those boundaries explicit.

## Do Not Add

- mining, building, block interaction, inventory, or chat
- server-owned NPCs or mob AI
- renderer/camera screenshot perception
- full ray-traced visibility or expensive global search
- offloaded worker pools or persistent map databases
- combat or entity targeting
- worldgen fallback for unloaded chunks

## Validation

Minimum:

```bash
pnpm test -- test/runtime/bot-good-view-goal.test.ts test/runtime/bot-navigation.test.ts test/runtime/bot-runtime.test.ts
pnpm test -- test/runtime/remote-world-transport.test.ts test/runtime/player-loop.test.ts
pnpm typecheck
git diff --check
```

Completed:

- `pnpm test -- test/runtime/bot-good-view-goal.test.ts test/runtime/bot-navigation.test.ts test/runtime/bot-runtime.test.ts`
- `pnpm test -- test/runtime/remote-world-transport.test.ts test/runtime/player-loop.test.ts`
  - sandboxed run hit `listen EPERM` on localhost WebSocket binding
  - rerun with command approval passed
- `pnpm typecheck`
- `git diff --check`

Useful manual smoke with a running dedicated server:

```bash
pnpm host:dedicated
pnpm bot:client -- --url ws://127.0.0.1:4173/api/world/socket --name ViewBot --goal good-view --seed 12345 --radius 3
```

If browser player movement or shared client-runtime code changes during this slice, also run:

```bash
pnpm test:browser:integration
```

## Done When

- [x] `--goal good-view` launches a bot that joins a dedicated server and selects a reachable high viewpoint from loaded client-world facts.
- [x] Target scoring is deterministic in unit tests.
- [x] The bot follows a path by sending normal player input commands.
- [x] Missing chunks and blocked paths produce explicit wait/failure states.
- [x] The bot can arrive, stop, and look around without spamming commands or retargeting every tick.
- [x] The behavior remains outside host authority and renderer code.

## Next Step

Recommended next slice: add a patrol-between-viewpoints goal on top of good-view selection. It should temporarily blacklist the current arrived target, choose another reachable viewpoint, and keep moving for a long-running dedicated-server smoke without requiring new block interaction or inventory protocol.
