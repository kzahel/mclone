# Lighting Worker Architecture

Target design for moving vanilla-style lighting out of the generated-world host worker.

## Decision

Lighting should run as a dedicated service owned by the authoritative world host. In the browser that service should be a separate module worker. In Node it can be a worker thread or an in-process implementation behind the same interface.

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

The host should use an interface first, then back it with a worker transport:

```ts
interface LightingService {
  configureWorld(request: ConfigureLightingWorld): Promise<void>;
  setView(request: SetLightingView): Promise<void>;
  upsertChunk(input: LightChunkInput): Promise<void>;
  removeChunk(chunkX: number, chunkZ: number, chunkRevision: number): Promise<void>;
  requestInitialLight(request: InitialLightRequest): Promise<void>;
  enqueueBlockChanges(request: LightBlockChangeBatch): Promise<void>;
  pollResults(request: PollLightingResults): Promise<LightingResultBatch>;
}
```

The host can keep using the same interface for oracle tests, browser workers, Node hosts, and future remote server work.

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
5. Once a publish candidate has a current 3x3 light-input neighborhood, host sends `request_initial_light`.
6. Lighting worker processes the request in mailbox order and returns `chunk_light_ready`.
7. Host accepts the result only if chunk-view revision and all relevant chunk revisions are still current.
8. Host builds one `chunk_snapshot` that includes the accepted light data.
9. Render-world worker ingests the lit snapshot and meshes once.

If a chunk is superseded before step 7, the host drops the result and sends no snapshot.

## Live Updates

For liquid flow, block edits, and future entity/block interactions:

1. Host mutates authoritative chunks and increments affected chunk revisions.
2. Host sends a batched block/light input update to the lighting worker.
3. Lighting worker queues `checkBlock`/emission changes as post-update work.
4. Lighting worker propagates within budget.
5. Host publishes the block-state snapshot and matching light deltas in revision order.

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
- A light delta is accepted only for a currently published chunk and current chunk revision.
- Unload always wins over pending light work.
- Missing neighbor chunks are "not ready", never "transparent air".
- Persisted chunk data should not trust stale light unless the stored light format is explicitly versioned and all neighbor dependencies are satisfied.

## Migration Plan

1. Add the `LightingService` interface and keep an in-process implementation using the existing `LevelLightEngine`.
2. Move current host light initialization behind that interface without changing externally visible behavior.
3. Add revision tracking for authoritative chunk mutations and neighbor dependency sets.
4. Add browser worker transport for `LightingService`.
5. Move `LevelLightEngine` ownership into the lighting worker.
6. Change cooperative chunk jobs to request initial light and publish only after `chunk_light_ready`.
7. Route liquid/block updates through batched light update requests and publish accepted deltas.
8. Split load radius from publish radius if needed for halo-only lighting work.
9. Add D5 traversal performance gates for host-worker responsiveness, lighting worker budget, render-world ingestion, and main-thread GPU uploads.

## Tests

Minimum coverage before enabling the worker path by default:

- initial chunk snapshots include correct light and are never unlit in normal mode.
- a chunk at a view boundary waits for its 3x3 light-input neighborhood.
- stale `chunk_light_ready` results are dropped after chunk-view movement.
- stale live deltas are dropped after chunk mutation or unload.
- liquid boundary ticks do not read missing chunks as air.
- player input/session polling continues while lighting worker has pending propagation.
- render-world ingestion batches stay bounded during chunk traversal.
- browser D5 traversal does not regress frame time while loading new chunks.
