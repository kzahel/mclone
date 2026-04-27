# Authoritative Host Scheduling

Durable guidance for keeping authoritative player/session work responsive while chunk loading, generation, saving, and snapshot assembly are active.

This document fills a gap left by [`architecture.md`](./architecture.md), [`protocol.md`](./protocol.md), and [`loading-persistence.md`](./loading-persistence.md). Those docs say the host owns authority and chunk lifecycle; this doc says how host work must be scheduled so authority does not stall behind chunk jobs. The parity-critical generation, lighting, and publication order is centralized in [`worldgen-deterministic-order.md`](worldgen-deterministic-order.md).

## Core Rule

Authoritative movement, input handling, session state, and polling must not wait for chunk load/generation/snapshot work to finish.

Changing chunk interest should be a cheap host operation:

1. record the session's new interest
2. update aggregate residency requirements
3. enqueue newly required chunk jobs
4. return or emit a session acknowledgement promptly
5. publish chunk snapshots later as each job finishes and is still relevant

This is true for both browser singleplayer and dedicated Node hosting. Browser singleplayer is not a renderer shortcut; it is a browser client joined to a local authoritative host. That local host still needs server-style scheduling.

## Why This Matters Now

The current debug URL:

```text
http://localhost:5173/?mode=debug&viewDistance=1
```

uses the local browser worker transport unless `worldTransport=remote` or `worldAuthority=dedicated` is explicitly present. So visible walking stutter at that URL is not a dedicated WebSocket or Node-host problem. It is evidence that the local authoritative host worker and/or debug client loop can stall movement while chunk work happens.

Current repo mismatch:

- `WorkerWorldTransport` sends each command to one host worker session.
- `GeneratedWorldHost.setChunkView(...)` preloads, generates, persists, and snapshots chunks before returning.
- `GeneratedWorldRemoteService.runWorldOperation(...)` serializes `set_chunk_view`, `set_player_input`, and `poll_world_updates` through one per-world operation queue.
- The debug render tick still awaits `setChunkView(...)` before scheduling the next useful tick.

That shape is useful as a correctness baseline, but it is not the long-term server scheduling model.

## Vanilla 1.17.1 Server Baseline

Reference files:

- `reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerGamePacketListenerImpl.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java`
- `reference/minecraft-1.17.1/src/net/minecraft/util/thread/ProcessorMailbox.java`

### Server Tick Thread Owns Authority

`MinecraftServer` is a `ReentrantBlockableEventLoop<TickTask>`. Its `runServer()` loop advances every 50 ms, calls `tickServer(...)`, then drains scheduled tasks while waiting for the next tick.

The important shape:

- gameplay authority is centralized on the server tick thread
- packet work is marshalled onto that thread
- delayed tasks and chunk tasks are polled opportunistically while there is tick budget
- overload is treated as a server tick problem, not as a reason for clients to own truth

### Movement Is Processed As Player State, Not As Chunk Delivery

`ServerGamePacketListenerImpl.handleMovePlayer(...)` validates and applies movement packets on the server thread. When the player crosses chunk sections, it calls:

```java
this.player.getLevel().getChunkSource().move(this.player);
```

That move updates tracking and tickets. It does not synchronously generate every newly visible chunk before accepting the next movement packet.

Vanilla can still lag if the server is overloaded, but the conceptual boundary is important: player movement updates interest; chunk delivery follows the chunk scheduler.

### Chunk Residency Is Ticket-Based

`DistanceManager` tracks players per chunk and updates player tickets. `ChunkMap.move(...)` updates player section position, distance tickets, tracking, and per-player chunk visibility.

The key responsibilities are separate:

- session/player position determines ticket sources
- tickets determine which `ChunkHolder`s should advance
- chunk holders expose futures for statuses such as `FULL`, ticking, and entity ticking
- chunk availability is observed through those futures, not returned synchronously by movement

### Chunk Work Is Asynchronous And Prioritized

`ChunkMap` schedules chunk load/generation through `CompletableFuture`s and processor mailboxes:

- `scheduleChunkLoad(...)`
- `scheduleChunkGeneration(...)`
- `worldgenMailbox`
- `mainThreadMailbox`
- `ThreadedLevelLightEngine`
- `ChunkTaskPriorityQueueSorter`

`prepareTickingChunk(...)` publishes a chunk to players only after the relevant future completes, via `playerLoadedChunk(...)`.

This is the part we missed in the earlier architecture analysis: vanilla's server is not "one command handler that returns all chunk data." It is a tick-owned authority core plus a chunk task graph that reports completions back to the server.

### Worldgen Order Contract

The detailed vanilla generation order lives in [`worldgen-deterministic-order.md`](worldgen-deterministic-order.md). This host-scheduling doc does not decide that order.

For `mclone`, a `vanilla17` path may use engine-native workers and queues, but the host scheduler must treat the deterministic-order contract as an input: enqueue eligible work, prioritize it, cancel stale work, and publish completed chunks without weakening the documented status, lighting, and send gates. If a scheduling change needs to alter status order or finality, update [`worldgen-deterministic-order.md`](worldgen-deterministic-order.md) with source evidence first.

### Vanilla Still Has Blocking Escape Hatches

`ServerChunkCache.getChunk(...)` can block with `managedBlock(...)` when code explicitly requires a chunk. That is not the view-subscription model, and it should not become our normal movement/chunk-interest path.

For `mclone`, blocking chunk reads may still exist for generation dependencies or future simulation rules, but they must be measured and isolated. A client changing view distance or walking across a boundary should not call the blocking path for the whole view.

## Browser And Node Constraints

We should not transliterate vanilla's JVM thread model class-for-class.

Platform constraints:

- a browser worker has one JS event loop and CPU-heavy JS is not preempted
- browser workers cannot share ordinary mutable object graphs
- `SharedArrayBuffer` needs explicit cross-origin isolation and lifetime policy
- Node and browser should share the same logical host design
- renderer WebGPU work remains main-thread-owned

So the divergence is scheduling machinery, not authority:

| Vanilla concept | `mclone` target |
|---|---|
| server tick thread | host/session worker or Node event-loop authority lane |
| packet listener marshalled to server thread | high-priority command lane for input/session/poll |
| `DistanceManager` tickets | engine chunk-interest/residency tracker |
| `ChunkHolder` status futures | host-owned chunk job records with explicit states |
| `worldgenMailbox` / light mailbox | browser workers / Node worker_threads job pool |
| `ChunkTaskPriorityQueueSorter` | priority chunk job scheduler keyed by distance and current interest |
| `playerLoadedChunk(...)` | queued `chunk_snapshot` update after job completion |

## Target `mclone` Host Model

```text
client main thread
  input/render/presentation
  sends commands, consumes updates

host/session authority lane
  accepts input immediately
  advances player/session ticks
  updates chunk interest and tickets
  queues chunk jobs
  integrates completed chunk jobs
  emits player/session/chunk updates

chunk job workers
  load stored chunk records
  generate missing chunks
  run chunk-stage side effects
  encode packed snapshots
  return completed chunk payloads

renderer/render-world worker
  consumes authoritative snapshots
  owns client render-world cache and mesh input reads
```

The host/session lane may be a browser worker for singleplayer or a Node runtime for dedicated hosting. Either way, it must keep player input, player ticks, and polling responsive while chunk jobs are active.

## Scheduling Priorities

### Highest Priority

- accept latest player input
- run due authoritative player/session ticks
- answer `poll_world_updates`
- enqueue or replace pending session interest
- send small session/player updates

### Medium Priority

- compute aggregate chunk interest
- create/update chunk job records
- integrate completed chunk jobs that are still relevant
- queue chunk snapshots to sessions

### Lower Priority / Budgeted

- start additional generation jobs beyond the near-ring priority budget
- serialize large snapshot batches
- save/evict chunks
- retry failed or stale jobs
- background prefetch

Large update payloads should be budgeted. A session should be able to receive player state promptly even if chunk snapshots are still queued behind a response-size or per-frame budget.

## Protocol Implications

`set_chunk_view` should not mean "return all chunks in this response."

Durable semantics should be:

- `set_chunk_view` acknowledges interest/session revision quickly
- chunk snapshots are server-originated updates
- polling transports may drain snapshots through `poll_world_updates`
- push transports can deliver the same updates without changing authority
- the logical message model stays the same across local worker and remote transports

This does not require client-side movement prediction. Without prediction, the client still waits for authoritative `player_state`, but that state should continue arriving at a stable cadence while chunk jobs run.

## Data Model Implications

The existing packed chunk facts still fit.

Required host-side chunk job states:

```text
absent
  -> queued_load
  -> loading
  -> queued_generation
  -> generating
  -> loaded_clean
  -> loaded_dirty
  -> queued_snapshot_encode
  -> snapshot_ready
  -> saving
  -> evictable
```

These are host scheduling states. They do not change the logical chunk snapshot format.

Important rules:

- a stale completed job must be dropped or cached if no current interest needs it
- only the host/session authority lane mutates authoritative residency/session state
- chunk workers return data, not authority decisions
- persistence remains an adapter behind host-owned lifecycle policy

## Acceptance Bar

D5 and the follow-up tactical should measure these explicitly:

- `set_player_input` latency while chunks are loading/generating
- `poll_world_updates` latency while chunks are loading/generating
- authoritative `player_state.tick` gaps during chunk-view changes
- `set_chunk_view` acknowledgement latency, separate from chunk snapshot completion
- chunk job queue depth by priority
- chunk load/generation/snapshot encode duration
- response/update byte budgets and chunk snapshot backlog

Initial target:

- no multi-frame visual freeze from waiting on chunk-view completion
- no player-state tick gap caused by chunk generation or snapshot assembly
- input and poll latency remain bounded while crossing chunk boundaries
- chunk snapshots may arrive later, but movement authority remains live

## Tactical Consequence

If D5 confirms that player input, poll, or authoritative tick delivery is blocked by chunk work, the next tactical should be:

```text
D6-authoritative-host-scheduler
```

That D6 should come before push transport, `SharedArrayBuffer`, or render-world subworkers unless measurements show a different bottleneck.

Expected D6 scope:

- split chunk interest acknowledgement from chunk snapshot delivery
- introduce host-owned chunk job records and a priority queue
- move browser local chunk generation/loading work out of the session authority worker when needed
- add the equivalent Node worker-thread path or a shared scheduler abstraction
- preserve the existing logical protocol and packed chunk facts
- keep renderer and client render-world ownership unchanged

Non-goals for that D6:

- no client-side movement prediction
- no client authority
- no new gameplay semantics
- no renderer ownership regression
- no speculative `SharedArrayBuffer`

## Design Checklist

Before changing host scheduling, answer:

1. Which vanilla responsibility is this mirroring: tick authority, distance tickets, chunk futures, task priority, or chunk publication?
2. Which platform constraint forces a different implementation shape?
3. Does input/poll/player tick work have a path that avoids chunk job waits?
4. Are chunk snapshots published as completions rather than command return values?
5. Can the same logical scheduler run in browser singleplayer and dedicated Node hosting?
6. Does the renderer remain a consumer of authoritative outputs?
7. Does this scheduler defer status order and finality rules to [`worldgen-deterministic-order.md`](worldgen-deterministic-order.md) instead of embedding a second model?
8. Do worker or queue changes preserve the already-documented publication gates without reinterpreting them?
