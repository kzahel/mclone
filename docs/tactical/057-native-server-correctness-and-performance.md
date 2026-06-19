# 057: Native Server Correctness And Performance

Status: proposed parent.

## Purpose

Create a focused native server workstream for Java-shaped correctness,
multiplayer behavior, performance, and module boundaries.

This is not a request to make the server more complicated for its own sake. The
goal is to prevent the native server from turning into one giant file or one
giant "server manager" abstraction while we add real multiplayer semantics.
When a Java server concept already has a mature boundary, use that as the
default Rust module boundary unless native/web constraints give us a concrete
reason to diverge.

## Current Problem

The latest dedicated-player work split movement, inventory, teleport ack, reach
checks, and spawn sync by `ServerPlayerId`, but chunk interest remains
single-view shaped:

- `IntegratedServer::set_chunk_view_for_target(...)` knows which player sent the
  command.
- It still calls `ChunkScheduler::apply_interest(view)`.
- `ChunkScheduler::apply_interest(...)` calls
  `ChunkDistanceManager::set_player_view(view)`.
- `ChunkDistanceManager` currently stores one global `player_ticket_positions`
  set.

That means multiple dedicated clients can have distinct player state while still
overwriting each other's chunk view. This is both a correctness issue and a
performance issue: we cannot reason about server residency, publication, unloads,
or client view-distance caps unless player tracking and ticket aggregation are
separate concepts.

## Reference Shape

Read these Java 1.17.1 files before changing this lane:

- `reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/players/PlayerList.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java`

Important reference facts:

- `MinecraftServer` owns levels and the `PlayerList`; it is the process-level
  tick/orchestration root.
- `ServerLevel` owns level simulation and exposes the level's
  `ServerChunkCache`.
- `ServerChunkCache` is the level-facing chunk source/cache and delegates chunk
  map work to `ChunkMap`.
- `ChunkMap` owns updating/visible holder maps, pending unloads, player chunk
  tracking, view-distance clamping, entity tracking, and packet fan-out.
- `DistanceManager` owns ticket storage, propagated ticket levels, player ticket
  tracking, and queue-level updates. It has player awareness for tickets, but it
  is not the whole networking fan-out layer.
- `ChunkMap.setViewDistance(...)` clamps the server view distance to Java's
  `3..33` shaped range and updates player tickets/tracking.
- `ChunkMap.updatePlayerStatus(...)` and `ChunkMap.move(...)` maintain
  per-player chunk tracking separately from holder residency.
- `ChunkMap.updateChunkTracking(...)` sends chunk load/unload effects to the
  affected player; loading/unloading for one player is distinct from whether a
  holder remains resident because another player or forced ticket needs it.
- `PlayerList.placeNewPlayer(...)` connects a `ServerPlayer` to one
  `ServerGamePacketListenerImpl`; this is the player/session boundary the native
  dedicated path is now starting to mirror.

## Native Boundary Direction

Keep the native server split into small Rust modules that map to those reference
concepts:

- `players.rs`: connected server-player ids and per-player gameplay state that
  belongs to the player, not the socket.
- `player.rs`: Java-shaped movement/teleport state and movement packet
  validation.
- `inventory.rs`: player inventory/hotbar state.
- `distance_manager.rs`: chunk ticket storage, ticket propagation, and
  ticket-level math. It should not become the packet fan-out layer.
- `holder.rs`: per-chunk holder state, status slots, residency, and dirty facts.
- `scheduler.rs`: current native owner for chunk scheduling, worldgen/light
  publication, pending unload draining, and simulation-facing chunk lanes. This
  module is already large enough; new player-view logic should not simply be
  appended here.
- new `player_chunk_tracking.rs` or `chunk_tracking.rs`: per-player accepted
  chunk views, view-distance clamping, visible-set deltas, and aggregate player
  ticket positions.
- new `update_routing.rs` or `outbox.rs` if needed: per-player pending
  `ServerUpdate` queues/fan-out so dedicated sessions receive only updates they
  should observe.
- future `server_level.rs` / `server_chunk_cache.rs` may become useful once
  integrated and dedicated runtime orchestration need a cleaner split. Do not
  introduce them until they remove real coupling.

Avoid a giant `network`, `server`, `runtime`, or `manager` module. If a file is
starting to own unrelated concepts, split by the closest Java reference
boundary first.

## First Implementation Slice

Implement Java-shaped player chunk tracking and server-capped view distance:

1. Add a focused player chunk tracking module.
2. Store each connected player's requested and accepted `ChunkView`.
3. Clamp client-requested view/tracking radii through server policy.
4. Maintain each player's visible chunk set.
5. Compute added/removed chunks for one player's view changes.
6. Build the aggregate union of all player visible/ticket chunks.
7. Replace the single `player_ticket_positions` ownership in
   `ChunkDistanceManager` with an aggregate player-ticket input.
8. Emit unloads to a player when the chunk leaves that player's view, even if the
   chunk remains resident for another player.
9. Send chunk snapshots/deltas only to players whose accepted visible set
   contains the affected chunk.
10. Keep the local integrated single-player path as a thin caller through the
   same machinery.

Server policy should respect client preference within server limits:

- If the client asks for less than the server maximum, use the client value.
- If the client asks for more than the server maximum, clamp to the server
  maximum.
- Keep Java's `MIN_VIEW_DISTANCE = 3` and `MAX_VIEW_DISTANCE = 33` shape in mind,
  but choose a native default/cap that is practical for current performance.
- Do not let protocol-provided radii directly allocate unbounded server work.

## Correctness Invariants

- Player A moving or changing view distance must not unload chunks still visible
  to Player B.
- Player A must receive chunk unloads for chunks leaving A's accepted view, even
  if those chunks remain loaded for Player B.
- Player A must not receive snapshots or block deltas outside A's accepted view.
- The server uses accepted/clamped view settings for tickets and publication,
  not raw client requests.
- Forced, light, post-teleport, and future non-player tickets continue to keep
  chunks resident independently of client visibility.
- Block/fluid simulation should operate on server residency/ticking lanes, not
  on one player's visible set.
- Dedicated and integrated paths should share the same server logic; only the
  transport/session plumbing should differ.

## Performance Focus

Performance work should be tied to observable server costs, not broad rewrites:

- View changes should diff old/new visible sets instead of rebuilding and
  re-sending everything.
- Ticket reconciliation should only run when the aggregate ticket set changes.
- Publication should be budgeted and per-recipient where needed.
- Large view distances should have explicit counts in diagnostics:
  player-visible chunks, aggregate player-ticket chunks, active ticket chunks,
  pending publications, pending unloads, and per-player outbound queue depth.
- Avoid cloning full chunk snapshots per recipient if several players need the
  same newly published chunk.
- Preserve WASM compatibility for shared server crates; native dedicated IO can
  remain platform-specific.

## Non-Goals For This Parent

- Do not replace the permissive movement authority model with FPS-style
  prediction/reconciliation here. That remains tracked by
  `056-native-player-movement-networking.md`.
- Do not port entities, mobs, scoreboard, chat, commands, dimensions, or Anvil
  storage as part of the first server correctness slice.
- Do not introduce a full async runtime just to model Java's Netty/processor
  graph. Current blocking socket workers plus a serialized server owner remain
  acceptable until profiling or correctness says otherwise.
- Do not perform move-only refactors and behavior changes in the same commit
  unless the behavior change is tiny and required to preserve tests.

## Validation

Use these lanes for every implementation slice in this parent:

- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo test --manifest-path native/Cargo.toml -p mclone-dedicated-server`
- `cargo test --manifest-path native/Cargo.toml`
- `pnpm --silent native:movement:smoke`
- `pnpm --silent native:timedemo:smoke`
- `pnpm --silent native:web:build`
- `pnpm --silent native:web:smoke`

Add focused tests for the first slice:

- two players with disjoint views keep both view regions ticketed
- overlapping views remove only the chunks no remaining player needs
- one player's smaller view distance receives fewer snapshots than another's
  larger accepted view
- server clamps a too-large client view request
- block delta routing sends updates only to players tracking the changed chunk
- disconnect removes that player's contribution to aggregate player tickets

## Follow-Up Queue

Landed:

1. Per-player chunk tracking, accepted view policy, aggregate player tickets,
   and per-player update routing.
2. Dedicated multiplayer diagnostics for per-player visible chunks, aggregate
   ticket chunks, and outbound queue depth.
3. A native TCP multi-client smoke covering disjoint views, per-client snapshots,
   and per-client unloads.
4. The TCP smoke now also covers spawn teleport acknowledgements, movement near
   a loaded block, non-overlapping block-delta isolation, and overlapping
   block-delta fan-out to both clients.

Next correctness steps, before more optimization work:

1. Extend the dedicated gameplay TCP smoke beyond break commands:
   - place blocks through `UseItemOn`
   - assert per-player inventory/selected-slot state over TCP
   - add a negative reach check over TCP so far-away interactions stay rejected
2. Add remote-player state publication once dedicated clients have independent
   chunk views and gameplay commands:
   - publish enough remote player position/state for clients to observe each
     other
   - keep this separate from FPS-style prediction/reconciliation
3. Harden the dedicated protocol/connection edge:
   - version mismatch behavior
   - clearer disconnect/error reporting
   - reconnect/resync behavior when the client cache is stale
4. Split or rename scheduler-facing modules only where these correctness slices
   expose a real boundary. Likely candidates are `player_chunk_tracking.rs` and
   an update fan-out/outbox module.
5. Defer release-mode server movement/view-distance perf, publication clone
   reduction, and per-recipient publication budgets until the multi-player
   semantics above are protected by TCP smokes.
6. Consider a Java-shaped `server_level.rs` / `server_chunk_cache.rs` split only
   after chunk tracking and update routing make the current `IntegratedServer`
   responsibilities clearly too broad.
