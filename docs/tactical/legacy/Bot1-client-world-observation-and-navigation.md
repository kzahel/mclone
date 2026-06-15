# Bot1 - Client-world observation and navigation

Standing after [`Bot0-headless-client-runtime.md`](Bot0-headless-client-runtime.md). `Bot0` gives bots the same client runtime and command path as a browser player. This slice makes the bot able to inspect its client-world replica in reusable, testable ways.

Status: **done**.

Landed result: `src/runtime/bot/` now has reusable client-world observation, safe loaded/missing block queries, standable-surface scanning with a revision-aware spatial cache, a bounded coarse path planner, and waypoint steering that produces normal bot movement intent. These utilities operate on hydrated `ClientWorld` facts and do not read host internals or run worldgen.

## Goal

Build the shared observation and navigation layer that bot behaviors and headless client tests can reuse.

At the end of `Bot1`, a bot should be able to:

- describe its own session, player body, chunk interest, and loaded-chunk coverage
- query nearby block states from `ClientWorld` without accessing host internals
- identify simple standable surfaces in loaded chunks
- detect missing world data explicitly instead of pretending unloaded chunks are air
- build a coarse path over currently hydrated, full-block-collision terrain
- convert a path waypoint into movement intent for the existing `set_player_input` command stream

## Intent

This is the "memory inspection" layer. It should be direct and efficient, but still client-owned:

```text
ClientWorld
  -> BotObservation
  -> BotSpatialIndex / standable-surface scan
  -> BotNavigator
  -> BotController
  -> player input commands
```

The bot is allowed to inspect hydrated chunk/block/entity data directly. It is not allowed to invent authority for missing chunks, generate chunks from seed, or ask the server for private world internals.

## Source Review

Review these current project files before implementation:

| Concern | Source |
|---|---|
| Client world query surface | `src/runtime/client/client-world.ts` |
| Client chunk cache and block access | `src/world/level/client-chunk-cache.ts` |
| Chunk snapshot section layout | `src/world/level/chunk-snapshot.ts`, `src/world/level/packed-chunk-snapshot.ts` |
| Collision query contract | `src/runtime/movement/collision-world.ts` |
| Movement command and intent shape | `src/runtime/movement/movement-command.ts`, `src/runtime/movement/movement-intent.ts` |
| Player body snapshots | `src/runtime/protocol/world-messages.ts` |
| Existing movement tests | `test/runtime/movement/movement-step.test.ts`, `test/runtime/player-loop.test.ts` |

Useful vanilla source orientation:

| Concern | Vanilla source |
|---|---|
| Mob navigation concepts | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/navigation/PathNavigation.java` |
| Ground path node concepts | `reference/minecraft-1.17.1/src/net/minecraft/world/level/pathfinder/WalkNodeEvaluator.java` |
| Mob movement control shape | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/ai/control/MoveControl.java` |

This slice should not port those vanilla classes. They are reference material for later host-owned mob AI. A player-like bot can start with a project-specific coarse planner over the client replica.

## Scope

| # | Work | Expected result |
|---|---|---|
| 1 | Observation snapshot | done - `BotObservation` carries session state, player state, loaded chunks, entities, and revision facts |
| 2 | Block query helpers | done - safe block lookup APIs return `loaded`/`missing` results for world positions |
| 3 | Surface scanning | done - standable surfaces are found within bounded X/Z/Y windows using hydrated block states and full-block collision assumptions |
| 4 | Spatial cache | done - `BotSpatialIndex` caches scans by bounds, revision facts, and loaded chunk set |
| 5 | Coarse path planner | done - bounded A* over standable cells returns loaded path, missing data, or blocked failure states |
| 6 | Waypoint steering | done - waypoint steering emits yaw and local `moveZ` intent without bypassing command sequencing |
| 7 | Tests | done - unit coverage proves block lookup, missing-data handling, standable-surface detection, path success/failure, and steering output |

## Observation Rules

Block and surface queries must distinguish:

| Result | Meaning |
|---|---|
| `loaded` | The relevant chunk/snapshot is present and the query is based on client-replica facts |
| `missing` | One or more required chunks are absent; the caller should wait, change interest, or choose another goal |
| `blocked` | Loaded data says the target is not standable or the path cannot pass |

Do not treat missing chunks as air or solid. That will hide transport/chunk-interest bugs and make bots look smarter than the client data actually allows.

## Navigation Baseline

The first planner should be deliberately modest:

- X/Z grid over block centers
- integer Y surface cells found by the scanner
- one-block horizontal moves
- allow stepping up only where current movement/collision support can handle it
- allow dropping only within a small bounded height difference
- reject water/lava/unknown non-solid terrain until movement supports it intentionally
- cap search radius and expanded nodes to keep tests and bot ticks bounded

This planner is for player-like bot goals, not vanilla mob parity. Later, if host-owned mobs need vanilla pathfinding, that work should use the reference sources and a separate authority-owned AI tactical.

## Do Not Add

- a long-lived global world database outside `ClientWorld`
- seed/worldgen fallback for unloaded chunks
- mining, block placing, inventory, item use, or interaction commands
- liquid/swimming/ladders/crouch-specific navigation
- renderer visibility, ray-cast pixels, screenshots, or camera frustum logic
- full vanilla `PathNavigation`, `NodeEvaluator`, or mob goals

## Validation

Completed:

```bash
pnpm test -- test/runtime/bot-observation.test.ts test/runtime/bot-navigation.test.ts
pnpm test -- test/runtime/movement/movement-command.test.ts test/runtime/player-loop.test.ts
pnpm typecheck
git diff --check
```

Also reran `test/runtime/bot-runtime.test.ts` with the Bot1 tests because `BotRuntime.observe()` now delegates to the shared observation helper.

If the implementation changes shared `ClientWorld` or `ClientChunkCache` public surfaces, also run:

```bash
pnpm test -- test/runtime/client-prediction-service.test.ts test/runtime/render-world-update-sink.test.ts
pnpm test:browser
```

## Done When

- [x] Bot code can inspect loaded client-world block/entity/player facts without host access.
- [x] Missing chunks are surfaced as explicit query failures.
- [x] Standable-surface scanning works over hydrated chunk snapshots.
- [x] A bounded path planner can produce a path across simple loaded terrain.
- [x] A steering helper can turn a path waypoint into normal player input commands.
- [x] Headless tests and future bot behaviors can share the same observation/navigation APIs.

## Next Step

[`Bot2-good-view-goal.md`](Bot2-good-view-goal.md): implement the first meaningful bot behavior, a "good view" goal that chooses a reachable high vantage point from loaded world facts and walks there like a player.
