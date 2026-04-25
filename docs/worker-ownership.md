# Worker Ownership Model

Baseline worker architecture for keeping the browser UI/GPU thread and authoritative host tick responsive while chunk generation, decoration, lighting, persistence, render-world ingest, and meshing are active.

This is the target ownership model, not a tactical implementation slice. Worker pooling for terrain generation and decoration is deliberately deferred. The immediate goal is a clean baseline with one owner per mutable cache and bounded cross-worker queues.

## Core Rule

The main thread must not run world simulation, generation, lighting, chunk snapshot decode, render-world neighborhood reads, or mesh construction.

The authoritative host tick must not wait for generation, decoration, lighting, persistence, snapshot transfer, render-world ingest, or GPU upload. It may integrate completed work, advance authoritative state, update chunk interest, and publish already-ready updates.

## Baseline Topology

```text
main thread
  input collection
  UI / debug overlay
  WebGPU resource creation, uploads, passes, presentation
  light texture update

generated-world host worker
  authoritative sessions, player/control state, chunk interest
  chunk lifecycle and revisions
  terrain/decor generation for the baseline
  liquid/entity/game ticks
  storage policy and eviction
  integration point for completed derived work

lighting worker
  LevelLightEngine ownership
  light-only chunk input cache
  bounded lighting mailbox
  initial chunk light and live light deltas

render-world worker
  client/render chunk cache
  packed snapshot and light-delta ingest
  dirty render-section tracking
  section mesh construction for the baseline

storage backend
  IndexedDB in browser singleplayer
  file storage in Node hosts
  adapter behind host-owned save policy
```

The baseline intentionally has one generated-world host worker, one lighting worker, and one render-world worker. Add pools only after measurement shows a specific single worker is saturated and the boundary can stay data-oriented.

## Ownership Table

| Resource | Owner | Other workers may |
|---|---|---|
| authoritative chunks | generated-world host worker | receive immutable packed snapshots or light-input extracts |
| chunk lifecycle/revisions | generated-world host worker | include revision ids in requests/results |
| player/session state | generated-world host worker | receive published state only |
| liquid/entity/game ticks | generated-world host worker | no direct access |
| `LevelLightEngine` and light section maps | lighting worker | receive accepted light snapshots/deltas only |
| render-world chunk cache | render-world worker | receive host snapshots/deltas only |
| mesh CPU buffers | render-world worker until result transfer | main thread receives final typed arrays |
| GPU buffers/textures/pipelines | main thread | no worker access in baseline |
| save policy and dirty state | generated-world host worker | storage adapter persists requested records |

No worker should mutate another worker's owned object graph. Cross-worker messages are immutable facts, compact inputs, completed outputs, or control/revision messages.

## Thread Budget Invariants

### Main Thread

Allowed:

- input sampling and event handling
- camera/controller presentation state
- render pass encoding/submission
- WebGPU buffer/texture/pipeline ownership
- bounded mesh upload work
- lightweight debug UI

Not allowed:

- chunk generation or decoration
- lighting propagation
- chunk snapshot decode for render-world ownership
- block-neighborhood queries for meshing
- long persistence operations
- waiting on `set_chunk_view` or host chunk completion during a frame

### Generated-World Host Worker

Allowed:

- authoritative command processing
- host tick and session/player state
- chunk interest and residency decisions
- baseline terrain/decor generation until a measured pool slice exists
- authoritative chunk mutation and revision assignment
- persistence orchestration
- integration of lighting results
- publication of snapshots/deltas

Not allowed:

- waiting synchronously for lighting propagation
- draining whole chunk-view generation/light/persistence work inside input/poll handlers
- running render-world ingest or mesh construction
- letting stale worker results mutate current authoritative chunks

### Lighting Worker

Allowed:

- owning `LevelLightEngine`
- owning light-section storage and light-only input cache
- processing bounded pre-update / propagation / post-update slices
- returning revisioned initial light snapshots and light deltas

Not allowed:

- calling back into live host chunks
- owning gameplay truth
- publishing directly to renderer clients
- treating missing neighbors as transparent final data

### Render-World Worker

Allowed:

- owning render/client chunk cache
- applying packed chunk snapshots and light deltas
- marking dirty sections
- building section meshes from current render-world cache

Not allowed:

- authoritative simulation or persistence decisions
- chunk generation
- lighting propagation
- GPU resource ownership in the baseline

## Normal Chunk Flow

```text
main thread
  sends input/chunk-interest commands without blocking the frame

generated-world host worker
  acknowledges interest
  generates or loads authoritative chunks
  decorates chunks
  assigns chunk revisions
  sends light-input chunks to lighting worker

lighting worker
  waits until required neighbor inputs are current
  computes initial light in bounded slices
  returns chunk_light_ready with revisions

generated-world host worker
  validates revisions
  builds one lit chunk_snapshot
  queues update to client transport

main thread / client runtime
  forwards chunk update to render-world worker

render-world worker
  applies snapshot
  marks dirty sections
  builds meshes from render-world cache

main thread
  receives mesh buffers
  uploads GPU buffers
  draws
```

Normal chunk publication should be lit-once. Do not publish unlit terrain as a default loading state, because that creates visible light pops and causes extra render-world ingest, mesh rebuilds, and GPU uploads.

## Live Update Flow

```text
generated-world host worker
  ticks liquid/entity/block systems
  mutates authoritative chunks
  increments affected chunk revisions
  sends revisioned block-light update batches to lighting worker

lighting worker
  applies block/light changes to its light-only chunk cache
  propagates within budget
  returns revisioned light deltas and a batch-complete marker

generated-world host worker
  validates revisions
  updates accepted light cache
  publishes block snapshots with matching light and light-only neighbor deltas

render-world worker
  applies updates
  rebuilds affected sections and neighbors

main thread
  uploads final mesh changes
```

For visual stability, block changes and their derived light changes should be batched whenever practical. If they are separated under load, that separation must be an explicit scheduling decision with measured visual and throughput impact.

## Neighbor Readiness

Neighbor-dependent systems must distinguish three states:

- authoritative chunk is loaded and current
- dependency input is present as a read-only halo
- dependency is missing or stale

Missing dependencies are not silently treated as air, transparent blocks, absent water, or final light. They make the dependent job pending, partial, or stale depending on the system.

Lighting needs current light-input data for the center chunk and immediate neighboring chunks before publishing a normal lit chunk. Light can propagate across chunk borders, and section visibility changes on one side of a border can invalidate the other side. The lighting worker may hold halo inputs without making those chunks render-published.

Liquid and block ticks at chunk borders may only mutate authoritative chunks owned by the host. If a neighboring chunk needed for a boundary decision is missing, the host should defer or reschedule that boundary work rather than interpreting the missing side as air or a source block.

Meshing is render-world derived data. The render-world worker may only build final boundary geometry when its sample neighborhood is current enough for face culling, ambient occlusion, and packed light lookup. If a neighbor arrives later, the affected boundary sections become dirty and rebuild inside the render-world budget.

## Data Boundary Rules

Prefer transferable typed arrays and compact records. Avoid structured-cloning rich object graphs.

Good worker payloads:

- packed chunk sections
- block-state id arrays
- compact light-input sections
- `DataLayer` bytes
- section mesh vertex/index buffers
- chunk and section revision metadata
- small command/control records

Bad worker payloads:

- `LevelChunk` instances
- `BlockState` object graphs
- `BlockPos` arrays for dense section data
- mutable `Map`/`Set` caches
- callbacks or service objects

`SharedArrayBuffer` is not part of the baseline. It remains a measured optimization path only if transfer/copy cost dominates after ownership boundaries are clean.

## Backpressure Rules

Every boundary that can carry bulk work needs an explicit budget:

- max host messages drained per poll/frame
- max lit chunks accepted per host turn
- max light propagation work per lighting worker turn
- max render-world ingest batch size
- max mesh completions uploaded per frame
- max dirty-section rebuilds queued per frame

When a budget is hit:

- player/session/control updates win over bulk chunk work
- current-view work wins over stale or halo work
- unload/cancel messages win over new work
- duplicate queued work collapses to the newest revision
- stale results are dropped, not integrated late

## Generation And Decoration Pooling

Worldgen and decoration are good future pool candidates, but not part of this baseline.

Reasons to defer:

- decoration has cross-chunk write/read behavior
- structures span many chunks through starts/references
- feature application must preserve vanilla ordering and deterministic seeds
- chunk workers must return data or write plans, not mutate host chunks directly

When pooling is introduced, keep this shape:

```text
worldgen pool
  computes terrain chunks, structure starts/references, or decoration write plans

generated-world host worker
  owns chunk mutation
  applies results in chunk-status order
  validates dependencies and revisions
```

Do not allow multiple worldgen workers to directly mutate neighboring authoritative chunks.

## Vanilla Dependency Notes

Minecraft 1.17.1 uses chunk-status dependency ranges:

```text
STRUCTURE_STARTS       range 0
STRUCTURE_REFERENCES   range 8
NOISE                  range 8
FEATURES               range 8
LIGHT                  range 1
```

Those ranges are scheduling dependencies, not permission for arbitrary concurrent mutation. During `FEATURES`, vanilla builds a `WorldGenRegion` with `writeRadiusCutoff = 1`; normal feature writes outside the center chunk plus immediate neighbors are rejected/logged. Large structures are split into starts/references and placed clipped to the currently generated chunk.

The baseline worker model should preserve that distinction:

- dependency windows can be large
- mutation ownership stays centralized
- outputs are applied in deterministic chunk-stage order

## Acceptance Bar

Before considering worker pooling, the baseline should satisfy:

- main thread has no long task caused by generation, decoration, lighting, snapshot decode, or meshing
- `set_player_input` and `poll_world_updates` stay bounded while chunks are loading
- host player/session ticks do not pause while lighting is active
- chunk snapshots are published once with correct initial light in normal mode
- render-world ingest and mesh completions are bounded enough to avoid GPU upload spikes
- stale generation/light/render results are rejected by revision
- D5 traversal schema `3` gates for host responsiveness, lighting worker budget, render-world ingest, and GPU upload behavior pass or identify the next bottleneck before a pool or shared-buffer slice is started

The lighting ownership baseline is landed: worker-backed browser/Node service shells, protocol types, a bounded mailbox, worker-owned `LevelLightEngine`, incremental initial `chunk_light_ready` publication, live `block_light_update_batch` / `chunk_light_delta` routing, and D5 lighting-worker telemetry exist under `src/runtime/lighting/`. The D5 gate is now executable; the baseline is not accepted until a post-lighting traversal run records whether host responsiveness and render/upload budgets still hold under chunk loading. The current measured blocker is throughput during initial lit-ring warmup, so the next ownership improvement is pipelining decoration and initial lighting rather than adding a pool or changing transport.

## Related Docs

- [`architecture.md`](architecture.md): broad runtime/host split
- [`authoritative-host-scheduling.md`](authoritative-host-scheduling.md): host responsiveness rules
- [`lighting-worker-architecture.md`](lighting-worker-architecture.md): dedicated lighting service details
- [`runtime-data-model.md`](runtime-data-model.md): chunk/block-state facts crossing boundaries
- [`protocol.md`](protocol.md): host/client message model
- [`loading-persistence.md`](loading-persistence.md): chunk lifecycle and persistence policy
