# D6 - Authoritative host scheduler

Standing after [`D5-transport-measurement-and-push-sab-decision.md`](D5-transport-measurement-and-push-sab-decision.md) and the durable scheduler note in [`../authoritative-host-scheduling.md`](../authoritative-host-scheduling.md).

`D5` proved the traversal harness and added the first coarse report. Manual walking in local browser-worker mode exposed a more specific gap: movement can stutter even at low view distance because chunk-interest work still runs as a blocking host command. This D6 addresses that scheduling model before push transport, `SharedArrayBuffer`, or render-world subworkers.

## Goal

Make authoritative player/session work responsive while chunk loading, generation, persistence, and snapshot assembly are active.

The target behavior is:

- `set_player_input` remains cheap while chunks are loading/generating
- `poll_world_updates` remains cheap while chunks are loading/generating
- `player_state.tick` delivery does not pause behind chunk jobs
- `set_chunk_view` acknowledges interest promptly
- chunk snapshots arrive later as server-originated updates
- local browser worker and dedicated Node host keep the same logical protocol
- renderer ownership and packed chunk facts do not change

## Vanilla baseline

Read these before changing scheduler shape:

- `reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java`
- `reference/minecraft-1.17.1/src/net/minecraft/util/thread/ProcessorMailbox.java`

Relevant vanilla shape:

- player movement is processed by the server tick authority path
- crossing chunk sections updates tickets/tracking
- chunk holders advance asynchronously through status futures
- chunk load/generation/light work is prioritized by distance/ticket level
- chunks are sent to players when ready, not synchronously returned by movement

We should not transliterate the JVM executor model class-for-class, but the host scheduler must preserve that separation.

## Current mismatch

Local browser singleplayer currently uses:

```text
main thread -> WorkerWorldTransport -> generated-world worker -> GeneratedWorldHost
```

Problems:

- `GeneratedWorldHost.setChunkView(...)` preloads, generates, mutates, persists, evicts, and snapshots the whole view before returning.
- `WorkerWorldTransport` processes one command through the same host instance, so a long `set_chunk_view` delays later input/poll commands.
- `GeneratedWorldRemoteService.runWorldOperation(...)` serializes chunk-view, input, and poll work through one per-world operation queue on the Node remote path.
- `debug-free-cam` currently awaits `setChunkView(...)` inside the visual tick, compounding the scheduler issue on the client side.

The first D6 slice should fix the host scheduler direction first. A later small client-loop follow-up may still be needed, but it should not substitute for server-side responsiveness.

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Tactical + tests | scheduler invariant is documented and pinned by tests before broad refactor |
| 2 | Local worker host scheduler | browser singleplayer can acknowledge chunk interest promptly and stream snapshots through `poll_world_updates` |
| 3 | Player/session responsiveness | input and poll commands are not delayed by queued chunk work beyond a single chunk/job quantum |
| 4 | Chunk job records | chunk work is tracked as host-owned jobs keyed by chunk and current interest generation |
| 5 | Snapshot publication | snapshots/unloads are queued as host-originated updates after chunk jobs complete |
| 6 | Remote Node follow-through | dedicated host shares the same scheduler semantics instead of serializing all session commands behind aggregate chunk-view work |
| 7 | D5 harness update | report input/poll latency and `player_state.tick` gaps while chunk jobs are active |
| 8 | Visual validation | local debug walking at low view distance no longer freezes movement because chunk view work is pending |

## Explicit non-goals

- no client-side prediction
- no client-authoritative movement
- no push transport
- no `SharedArrayBuffer`
- no render-world subworkers
- no chunk data-model change
- no renderer ownership regression
- no vanilla movement/collision parity work
- no worldgen parity changes

## Implementation sequence

1. Draft the tactical and commit it.

2. Add scheduler-invariant tests around the existing host.

   The tests should demonstrate the current desired semantics even before the full worker pool exists:

   - `set_chunk_view` can return without chunk snapshots in the same response when async scheduling is enabled
   - `poll_world_updates` later delivers chunk snapshots
   - a later chunk-view generation supersedes stale work
   - `set_player_input` is serviced while chunk work is queued

3. Add a browser-worker scheduler mode to `GeneratedWorldHost`.

   First slice can be cooperative, not parallel:

   - acknowledge interest immediately
   - process chunk jobs one at a time
   - yield between chunk jobs so queued input/poll commands run
   - enqueue snapshots to `pendingMessages`

   This does not solve a single very expensive chunk generation quantum, but it removes the current "whole view blocks as one command" behavior and gives D5 something sharper to measure.

4. Keep the existing synchronous path available temporarily.

   Some Node/headless tests currently expect chunk snapshots directly from `setChunkView`. Preserve that mode until the remote service and tests are migrated.

5. Update local browser worker creation to use scheduler mode.

   Browser singleplayer is the user-visible stutter path for `debug.html?viewDistance=1`, so it should get the first scheduler slice.

6. Update tests that should use the new semantics.

   Avoid weakening direct host tests accidentally. Split tests by scheduler mode where needed.

7. Extend D5 metrics.

   Record:

   - `set_player_input` latency during active chunk jobs
   - `poll_world_updates` latency during active chunk jobs
   - `player_state.tick` gaps
   - chunk-view ack latency separately from chunk snapshot completion

8. Migrate remote Node service.

   Once the single-session local worker path is stable, make the dedicated remote service treat `set_chunk_view` as interest acknowledgement and stream chunks through pending updates. Remove the per-world operation queue from input/poll where it blocks behind chunk jobs.

9. Add worker-thread/job-pool follow-through if measurement requires it.

   If cooperative chunk quanta are still too large, split chunk load/generation/snapshot encode into browser worker sub-jobs and Node `worker_threads`, keeping authority decisions in the host/session lane.

## First-slice acceptance

The first implementation slice is done when:

- scheduler-mode `set_chunk_view` returns a `session_state` acknowledgement before chunk snapshots are complete
- `poll_world_updates` receives the chunk snapshots as they are built
- `set_player_input` can complete while chunk jobs are still pending
- stale chunk jobs from an older view do not publish snapshots outside the current interest set
- default tests still pass through the preserved synchronous path
- browser worker mode uses scheduler mode
- `pnpm typecheck` and targeted runtime tests pass

## D6 completion acceptance

D6 is complete when:

- local browser worker and remote Node host share interest-ack-plus-streaming semantics
- player/session commands are not serialized behind chunk generation/loading/snapshot jobs
- D5 harness reports bounded input/poll latency and bounded `player_state.tick` gaps while crossing chunk boundaries
- low-view-distance manual walking does not freeze movement waiting for chunk-view completion
- chunk snapshots still use the packed D3/D4 data model
- renderer render-world ownership remains unchanged
- `pnpm typecheck`, `pnpm test`, `pnpm test:browser`, and the D5 harness pass

## Next

Implement the first scheduler-mode slice in `GeneratedWorldHost` and the local browser worker transport. Do not start push transport, `SharedArrayBuffer`, render-world subworkers, or client-side prediction as part of this tactical.
