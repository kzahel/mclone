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

## Landed

### First scheduler-mode slice

- Added a cooperative `GeneratedWorldHost` chunk-view scheduling mode.
- Kept the synchronous host path as the default for existing direct and Node remote behavior.
- Changed browser worker singleplayer host creation to use cooperative scheduling.
- Split chunk-view interest updates from chunk generation in `GeneratedRenderLevel`.
- In cooperative mode, `set_chunk_view` acknowledges session interest before snapshots are ready.
- In cooperative mode, chunk snapshots are queued through `poll_world_updates` as chunk jobs complete.
- Stale chunk jobs are guarded by a chunk-view generation and current interest check before publication.
- Added targeted runtime coverage for prompt acknowledgement, input service during queued chunk work, streamed snapshots, and stale snapshot suppression.

### Remote service and measurement slice

- Changed the dedicated Node remote service to create its shared generated host in cooperative chunk-view mode.
- Kept `set_chunk_view` as an interest acknowledgement path: immediate responses now carry session/unload messages, not chunk snapshots.
- Routed host-originated chunk snapshots into per-session pending queues for delivery through `poll_world_updates`.
- Moved cached visible chunk resync for ordinary chunk-view changes out of `set_chunk_view` responses and into poll delivery.
- Removed the per-world operation queue from `set_player_input` and `poll_world_updates`.
- Added a guarded shared host-drain path so concurrent session polls do not double-drain host pending messages.
- Added browser-side D5 fetch timing so ack/input/poll latency is measured from the client, not from Playwright's Node-side network events.
- Increased visual boot chunk-ring polling to allow cooperative remote startup to stream a full view instead of returning it synchronously.

Measured D5 traversal after this slice:

| Metric | Observed |
|---|---:|
| traversal duration | `30043 ms` |
| chunk-view changes | `10` |
| `set_chunk_view` ack p50 / p95 / max during traversal | `1.8 ms` / `10.0 ms` / `10.0 ms` |
| `set_player_input` latency during traversal | `1.1 ms` |
| `poll_world_updates` p50 / p95 / p99 / max during traversal | `0.9 ms` / `11.2 ms` / `256.9 ms` / `291.3 ms` |
| `poll_world_updates` with chunks p50 / p95 / max | `7.4 ms` / `256.9 ms` / `274.2 ms` |
| player tick response gap p50 / p95 / p99 / max during traversal | `56.4 ms` / `69.1 ms` / `309.2 ms` / `340.5 ms` |
| frame gap p95 / p99 / max | `10.3 ms` / `10.4 ms` / `10.7 ms` |
| long tasks | `0` |

The browser frame path is smooth, and chunk-view acknowledgement is no longer the blocking path. The remaining D6 risk is the single chunk-generation/snapshot quantum: poll responses that include chunks still have p99/max spikes around `250-290 ms`, and player tick delivery shows matching p99 gaps.

### Poll payload split slice

- Added a shared world-host message drain helper that prefers session/player/control messages over bulk `chunk_snapshot` messages when a poll response is capped.
- Capped remote browser poll responses at two messages by default.
- Capped browser-worker poll responses at four messages in renderer scene setup so live chunk ingestion is split without regressing parity-test startup.
- Reused the packed chunk snapshot built for persistence as the streamed snapshot, removing the duplicate pack pass from cooperative chunk jobs.
- Inserted a yield between chunk mutation and snapshot packing so pending input/poll work gets a chance to run before the snapshot encode/persist step.
- Added targeted queue-drain coverage for capped prioritization.

Measured D5 traversal after this slice:

| Metric | Observed |
|---|---:|
| traversal duration | `30407 ms` |
| chunk-view changes | `11` |
| `set_chunk_view` ack p50 / p95 / max during traversal | `0.9 ms` / `1.2 ms` / `1.2 ms` |
| `set_player_input` latency during traversal | `1.0 ms` |
| `poll_world_updates` p50 / p95 / p99 / max during traversal | `0.9 ms` / `7.5 ms` / `238.7 ms` / `254.9 ms` |
| `poll_world_updates` with chunks p50 / p95 / max | `1.2 ms` / `19.9 ms` / `246.1 ms` |
| chunk snapshots per poll p50 / p95 / max | `1` / `1` / `2` |
| player tick response gap p50 / p95 / p99 / max during traversal | `56.3 ms` / `60.0 ms` / `292.6 ms` / `304.0 ms` |
| frame gap p95 / p99 / max | `10.3 ms` / `10.4 ms` / `12.4 ms` |
| long tasks | `0` |

This slice split chunk delivery and removed duplicate snapshot packing. It improved chunk-bearing poll p50/p95 and response size shape, but D6 is still not complete: p99 poll latency and p99 player tick delivery still track a single synchronous chunk generation/decor/snapshot quantum.

### Chunk status phase split slice

- Split cooperative chunk-view jobs into explicit terrain/load, decoration, and snapshot publication passes.
- Added a cooperative terrain path that yields between density fill, surface/bedrock, carvers, chunk copy, and publication work.
- Added cooperative biome decoration that preserves the existing feature order while yielding between decorated feature placement units.
- Suppressed automatic synchronous decoration while host cooperative decoration is active, so feature placement reads do not recursively generate/decorate neighboring chunks before the view's terrain pass has filled them.
- Tracked published chunk snapshots separately from in-memory terrain/decor state, allowing chunks produced under an older job batch to publish later if they are still visible in the current view.
- Marked hydrated stored chunks as decorated when loading from packed snapshots.
- Increased debug boot chunk-ring polling and remote test drain loops to account for deliberately yielded 225-chunk startup.

Measured D5 traversal after this slice:

| Metric | Observed |
|---|---:|
| traversal duration | `30136 ms` |
| chunk-view changes | `12` (`[60,199] -> [66,193]`) |
| `set_chunk_view` ack p50 / p95 / max during traversal | `1.3 ms` / `11.6 ms` / `11.6 ms` |
| `set_player_input` p50 / p95 / p99 / max during traversal | `1.0 ms` / `1.0 ms` / `1.0 ms` / `1.0 ms` |
| `poll_world_updates` p50 / p95 / p99 / max during traversal | `0.9 ms` / `10.6 ms` / `19.8 ms` / `26.4 ms` |
| `poll_world_updates` with chunks p50 / p95 / p99 / max | `1.3 ms` / `17.2 ms` / `23.8 ms` / `26.4 ms` |
| chunk snapshots per poll p50 / p95 / max | `1` / `1` / `2` |
| player tick response gap p50 / p95 / p99 / max during traversal | `50.1 ms` / `61.4 ms` / `100.0 ms` / `119.3 ms` |
| sampled tick gap p50 / p95 / p99 / max | `253.5 ms` / `382.9 ms` / `425.1 ms` / `441.7 ms` |
| frame gap p95 / p99 / max | `9.3 ms` / `9.4 ms` / `9.4 ms` |
| long tasks | `0` |
| final loaded chunks | `225` |
| render-world delta | ingest batches `180`, mesh builds `13119`, mesh completions `1273`, GPU uploads `1678` |

Artifact paths:

- `/tmp/mclone-d5-start.png`
- `/tmp/mclone-d5-end.png`
- `/tmp/mclone-d5-traversal-report.json`
- `/tmp/mclone-d5-traversal-trace.json`

Visual inspection: the start screenshot shows a filled cliff/cave face, and the end screenshot shows populated savanna/desert/mountain terrain with no blank chunks.

Interpretation: the host single-chunk quantum was the measured bottleneck. Splitting chunk work into status-like cooperative phases brought `poll_world_updates` p99 down from the previous `238.7 ms` to `19.8 ms`, while frame pacing stayed clean and chunk-bearing polls remained capped. The remaining D6 risk is the response max and player tick max around `26-119 ms`, plus subjective low-view walking; if that still feels bad, the next host-only work should move snapshot/persistence and/or feature placement into a dedicated job worker rather than changing transport, `SharedArrayBuffer`, render-world ownership, or client prediction.

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

Manually validate low-view debug walking on `debug.html?viewDistance=1` against the phase-split scheduler. If movement still visibly stalls, continue D6 by reducing the remaining host response max with host-owned job work for snapshot/persistence and/or feature placement. If manual walking is acceptable, close D6 and return to the D5 acceptance decision. Do not start push transport, `SharedArrayBuffer`, render-world subworkers, or client-side prediction as part of this decision.
