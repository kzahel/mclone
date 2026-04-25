# Lighting Worker Architecture

Target design for moving vanilla-style lighting out of the generated-world host worker.

This document is the lighting-specific service design. The broader worker/cache ownership baseline lives in [`worker-ownership.md`](worker-ownership.md).

## Decision

Lighting should run as a dedicated service owned by the authoritative world host, but outside the host worker. In browser runtimes it should be a separate module worker. In Node runtimes it should be a worker thread. Do not add a host-facing in-process `LightingService`; that makes the no-hangs contract optional and lets lighting regress back onto the authoritative tick path.

The generated-world host stays authoritative for chunks, ticks, storage, sessions, and player state. The lighting service is authoritative only for derived sky/block light data.

Do not publish normal terrain chunks unlit and then republish them lit. Initial chunk snapshots should include their initial light, because unlit-first publication causes visible light pops and forces extra render-world ingestion and mesh rebuilds.

## Thread Ownership

```text
main browser thread
  input, WebGPU submission, GPU uploads, light texture ticks

generated-world worker
  chunk view, terrain/decor generation, liquid/entity/game ticks, storage
  builds lighting inputs from authoritative chunks
  accepts/rejects lighting results by chunk revision
  publishes lit chunk snapshots and light deltas

lighting worker
  owns LevelLightEngine and light-section storage
  owns the light mailbox and propagation budget
  returns revisioned chunk light snapshots and dirty section deltas

render-world worker
  owns client/render chunk cache for meshing
  ingests lit snapshots and deltas
  marks affected render sections dirty
```

The lighting worker must not call back into live host chunks. It receives immutable, revisioned light inputs and returns immutable, revisioned light outputs.

## Host Service Interface

The host should use an interface whose host implementation is worker-backed:

```ts
interface LightingService {
  configureWorld(request: ConfigureLightingWorld): Promise<void>;
  setView(request: SetLightingView): Promise<void>;
  upsertChunk(input: LightChunkInput): Promise<void>;
  removeChunk(chunkX: number, chunkZ: number, chunkRevision: number): Promise<void>;
  requestInitialLight(request: InitialLightRequest): Promise<void>;
  enqueueBlockChanges(request: LightBlockChangeBatch): Promise<void>;
  pollResults(request: PollLightingResults): Promise<LightingResultBatch>;
  getPerformanceCounters?(): LightingServicePerformanceCounters;
}
```

Browser and Node hosts should use the same worker-backed interface. Command promises should mean "accepted/enqueued", not "lighting completed"; completed light data returns through bounded result polling.

Solver unit tests and Java-oracle comparisons may instantiate the ported lighting classes directly below this service boundary. They should not provide a synchronous host-facing `LightingService`, because that hides accidental host-tick lighting work.

## Implementation Status

Landed:

- `src/runtime/lighting/lighting-protocol.ts` defines the host/worker message shapes, revisioned requests/results, transferable light-input sections, and bounded result polling contract.
- `src/runtime/lighting/lighting-worker-client.ts` provides the browser module-worker client and enforces command promises as accepted/enqueued acknowledgements.
- `src/runtime/lighting/lighting-worker.ts` owns the worker-side light-only chunk cache, `LevelLightEngine`, serialized mailbox, bounded result polling, initial-light neighbor validation, and `chunk_light_ready` result production.
- `src/runtime/lighting/node-lighting-worker-client.ts` and `src/runtime/lighting/node-lighting-worker-thread.ts` provide the Node worker-thread shell for dedicated/headless hosts.
- `GeneratedWorldHost` no longer constructs `LevelLightEngine`; vanilla lighting mode requires a worker-backed `LightingService`.
- Initial chunk publication now packs light inputs, requests `chunk_light_ready`, validates chunk revisions, and publishes each snapshot as its own accepted worker light becomes ready.
- Live block/liquid mutations now coalesce into `block_light_update_batch` requests. The worker applies the block-state changes, drains propagation, returns revisioned `chunk_light_delta` section replacements, and finishes with `block_light_update_complete` so the host can update its accepted light cache before publishing dirty snapshots.
- Worker-side command timing and propagation-slice timing now flow through `light_performance` results, `LightingServicePerformanceCounters`, host `world_perf` snapshots, the debug runtime state, and the D5 traversal report.
- D5 traversal schema `3` now has regression gates for host responsiveness, lighting worker command/slice budgets, render-world ingest, and main-thread GPU uploads.

Still pending:

- Add sharper dependency tracking for accepted light when neighbor revisions change; the current host invalidates a conservative 3x3 chunk window.
- Pipeline decoration and initial lighting so the host starts lighting dependency-ready chunks before the entire publish ring has finished decoration.
- Run and record the first post-lighting D5 baseline with the schema `3` gates after any follow-up responsiveness fixes.

## Protocol Shape

Every message that can outlive a chunk-view turn must carry enough revision data to be dropped safely.

```ts
interface LightingRevision {
  readonly chunkViewRevision: number;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly chunkRevision: number;
}
```

Useful first messages:

- `configure_light_world`: world height, min section, seed/profile, block registry/profile version.
- `set_light_view`: center, radius, host chunk-view revision, priority origin.
- `upsert_light_chunk`: chunk position, chunk revision, decorated flag, and light input sections.
- `remove_light_chunk`: unload chunk light data and cancel pending work for that chunk revision.
- `request_initial_light`: compute initial light for a chunk once required neighbors are present.
- `block_light_update_batch`: live block opacity/emission changes from liquid/entity/player edits.
- `poll_light_results`: bounded result drain.

Worker results:

- `chunk_light_ready`: full initial `ChunkLightSnapshot` for a chunk.
- `chunk_light_delta`: section-level replacements for already published chunks.
- `block_light_update_complete`: revisioned completion marker for a live block-change batch.
- `light_performance`: worker-side command duration, propagation-slice duration, and mailbox/result backlog counters.
- `light_progress`: optional progress/backpressure diagnostics.
- `light_error`: fatal worker error with the triggering revision.

## Light Input Data

The first implementation should prefer correctness and a simple transport over premature compression.

Reasonable phase-1 input:

```ts
interface LightChunkInput {
  readonly type: "upsert_light_chunk";
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly chunkRevision: number;
  readonly sections: readonly PackedLightInputSection[];
}

interface PackedLightInputSection {
  readonly y: number;
  readonly blockStateIds: Uint16Array | Uint32Array;
}
```

The lighting worker can hydrate a light-only chunk view from block state ids and the same block registry tables used by the host. This avoids inventing a second opacity/emission model too early.

A later optimization can replace block state ids with compact light facts:

```text
opacity: 4-bit or 8-bit light blocking
emission: 4-bit block light source
shape flags: only if shape-occlusion parity needs them
```

Do not use `SharedArrayBuffer` for the first version. Transferable typed arrays are enough and avoid COOP/COEP deployment constraints.

## Neighbor Readiness

Vanilla `ChunkStatus.LIGHT` has range 1. `mclone` should mirror that scheduling fact even if the worker implementation is browser-specific.

For a chunk to produce an initial lit snapshot:

- the center chunk input must be present and current.
- the eight horizontal neighbor chunk inputs must be present and current.
- section status must be initialized for the center and neighbor columns.
- sky-source enablement must be applied before propagation.
- block emitters in those columns must be queued before propagation.

Block light falls off by one per block from level 15, so a one-chunk halo is enough for cross-boundary block-light contribution into the center chunk. Sky light is column-heavy but still crosses chunk boundaries around roofs, cave mouths, and side openings; missing neighbors must not be treated as final transparent data.

The host should distinguish:

```text
load radius     = render radius + lighting halo
publish radius  = chunks the renderer should receive now
```

Publishing halo chunks is optional cache warming. If halo chunks are published, they still need correct initial light. Do not publish a halo chunk solely because its terrain is ready.

## Chunk Lifecycle

For each chunk-view revision:

1. Host updates chunk interest and unloads stale chunks.
2. Host generates/decorates missing chunks in priority order.
3. Host assigns or increments `chunkRevision` for each authoritative chunk mutation.
4. Host sends `upsert_light_chunk` for center and halo chunks.
5. Once a publish candidate appears to have a current 3x3 light-input neighborhood, host sends `request_initial_light`.
6. Lighting worker validates the required neighbor revisions, keeps the request pending if inputs are missing or stale, processes ready work in mailbox order, and returns `chunk_light_ready`.
7. Host accepts the result only if chunk-view revision and all relevant chunk revisions are still current.
8. Host builds one `chunk_snapshot` that includes the accepted light data.
9. Render-world worker ingests the lit snapshot and meshes once.

If a chunk is superseded before step 7, the host drops the result and sends no snapshot.

## Live Updates

For liquid flow, block edits, and future entity/block interactions:

1. Host mutates authoritative chunks and increments affected chunk revisions.
2. Host coalesces pending per-position edits and sends a revisioned `block_light_update_batch` with old/new block-state ids.
3. Lighting worker applies the edits to its light-only chunk cache, queues `checkBlock`/emission changes, and propagates within budget.
4. Lighting worker returns `chunk_light_delta` section replacements for affected current chunks, then `block_light_update_complete`.
5. Host accepts only current-revision deltas, applies them to its accepted light cache, and marks batch chunks as current.
6. Host publishes dirty block-state snapshots with matching accepted light. Light-only neighbor changes are published as `chunk_light_delta`.

For visual stability, prefer batching visible block snapshots with their corresponding light deltas. If simulation throughput requires block snapshots to arrive first later, that should be a deliberate mode with known visual popping, not the default.

## Mailbox And Scheduling

The lighting worker should have one serialized mailbox. It can receive messages concurrently from the browser runtime, but it should process them in-order inside the worker.

Queue phases:

```text
control:
  configure, set view, cancel revision, unload/remove

preUpdate:
  upsert chunk input
  section status changes
  enable/disable sky sources
  initialize emitters

propagation:
  LevelLightEngine.runUpdates(updateBudget)

postUpdate:
  collect dirty light sections
  complete chunk_light_ready futures
  enqueue chunk_light_delta results
```

Budgets should be explicit:

```ts
interface LightingBudget {
  readonly maxPropagationUpdates: number;
  readonly maxWallTimeMs: number;
  readonly maxResults: number;
}
```

The worker should run until it hits a budget, post results, then yield. It must prioritize:

1. unload/cancel/current-view control messages.
2. initial light for chunks nearest the current priority origin.
3. visible live-update deltas.
4. halo/cache-warming work.

This is closer to vanilla's `ThreadedLevelLightEngine` shape than the current host-side cooperative loop: vanilla uses a task mailbox around `PRE_UPDATE`, propagation, and `POST_UPDATE`, with chunk priority supplied by the server chunk system.

## Backpressure

The host should not wait synchronously inside `set_chunk_view` for lighting. `set_chunk_view` should acknowledge the view/session change and start background generation and lighting jobs. Polling then streams snapshots as chunks become fully lit.

Backpressure rules:

- cap pending light inputs and requests per chunk-view revision.
- collapse duplicate block changes to the latest chunk/block revision before sending.
- drop queued work for chunks outside the current load radius.
- bound `poll_light_results` so the host does not create render-world ingestion spikes.
- keep player input/session messages responsive even while chunks are waiting on light.

## Correctness Rules

- A light result is accepted only when its chunk revision matches the host's current chunk revision.
- Initial light for a chunk is accepted only when the required neighbor revisions match the request.
- A light delta is accepted only for the current chunk-view and current chunk revision; it is published to clients only when the chunk is currently published and is not already being republished as a full dirty snapshot.
- Unload always wins over pending light work.
- Missing neighbor chunks are "not ready", never "transparent air".
- Persisted chunk data should not trust stale light unless the stored light format is explicitly versioned and all neighbor dependencies are satisfied.

## Migration Plan

1. Done: add `LightingService` protocol types, browser module-worker transport shell, Node worker-thread shell, and service-level mailbox tests.
2. Done: move `LevelLightEngine` ownership into the lighting worker shell.
3. Done: change chunk publication to request initial light and publish only after accepted `chunk_light_ready` results.
4. Done: track authoritative chunk revisions and reject stale initial light and live light results.
5. Done: route liquid/block updates through batched light update requests and publish accepted deltas.
6. Split load radius from publish radius if needed for halo-only lighting work.
7. Done: add D5 traversal performance gates for host-worker responsiveness, lighting worker budget, render-world ingestion, and main-thread GPU uploads.
8. Done: publish initial lit chunk snapshots incrementally as each `chunk_light_ready` result is accepted.
9. Pipeline decoration and initial lighting across the publish ring, then rerun and record the post-lighting D5 baseline. Use failures to choose the next bottleneck slice instead of loosening the gate after the fact.

## Tests

Minimum coverage before enabling the worker path by default:

- initial chunk snapshots include correct light and are never unlit in normal mode.
- a chunk at a view boundary waits for its 3x3 light-input neighborhood.
- stale `chunk_light_ready` results are dropped after chunk-view movement.
- stale live deltas are dropped after chunk mutation or unload.
- liquid boundary ticks do not read missing chunks as air.
- player input/session polling continues while lighting worker has pending propagation.
- render-world ingestion batches stay bounded during chunk traversal.
- browser D5 traversal schema `3` gates stay green while loading new chunks.
