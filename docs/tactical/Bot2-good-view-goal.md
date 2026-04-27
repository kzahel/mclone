# Bot2 - Good view goal

Standing after [`Bot1-client-world-observation-and-navigation.md`](Bot1-client-world-observation-and-navigation.md). Bots can join as headless clients, inspect hydrated client-world facts, identify standable surfaces, and steer toward path waypoints. This slice gives the first bot a personality-shaped objective: it likes finding a good view.

Status: planned.

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
| 1 | Goal interface | Add a small `BotGoal` lifecycle: evaluate, activate, tick, complete/fail, and explain current state |
| 2 | Good-view scorer | Score loaded standable surfaces by height, openness, simple horizon exposure, distance, and reachability |
| 3 | Target selection | Pick the best reachable candidate, keep the selected target stable until arrival/failure or a major world revision |
| 4 | Path following | Use `Bot1` navigation and waypoint steering to walk toward the target through sequenced player input commands |
| 5 | Arrival behavior | Stop near the target, rotate slowly, and keep chunk interest centered on the player |
| 6 | Stuck handling | Detect lack of progress, blocked paths, missing chunks, or command ack stalls and select a new target or wait |
| 7 | CLI mode | Add `--goal good-view` as the first non-idle bot mode |
| 8 | Tests | Cover scoring, target selection, stable retargeting, path-following decisions, arrival, and stuck/failure reporting |

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

- `--goal good-view` launches a bot that joins a dedicated server and selects a reachable high viewpoint from loaded client-world facts.
- Target scoring is deterministic in unit tests.
- The bot follows a path by sending normal player input commands.
- Missing chunks and blocked paths produce explicit wait/failure states.
- The bot can arrive, stop, and look around without spamming commands or retargeting every tick.
- The behavior remains outside host authority and renderer code.

## Next Step

After `Bot2`, choose the next bot behavior based on what is most useful for testing: patrol between viewpoints, follow another player, inspect nearest generated entity, or wait for block interaction commands before adding mining/building goals.
