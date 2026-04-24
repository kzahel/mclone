# D4 — Browser render-world ownership

Standing after [`D3-packed-chunk-storage-protocol.md`](D3-packed-chunk-storage-protocol.md). `D3` moved authoritative chunk facts across protocol and storage as `PackedChunkSnapshot`, but the browser main thread still unpacks those facts into `ClientChunkCache`, gathers 3x3 chunk-neighbor `ChunkSnapshot` sets, and sends those object-heavy mesh inputs to the mesh worker.

## Goal

Move browser client chunk-cache ownership and mesh-input ownership off the main thread:

- a render-world worker owns the client chunk cache
- the render-world worker ingests packed `chunk_snapshot` and `chunk_unload` updates from the D3 protocol
- the render-world worker owns chunk-neighbor gathering and section mesh compilation input
- the main thread owns input, session/player presentation state, GPU buffers, uploads, draw submission, and small render metadata
- local singleplayer and remote multiplayer use the same browser render-world path

At the end of `D4`, the browser main thread may forward packed chunk buffers, loaded counts, dirty section coordinates, and mesh payloads. It should not own raw packed sections, unpack `ChunkSnapshot` objects, or gather chunk-neighborhood snapshots for meshing.

## Why this slice exists

`D3` removed name/property object graphs from host/storage/protocol boundaries, but the browser renderer still does expensive ownership work:

- `TransportWorldClient.applyHostMessages(...)` applies packed chunk snapshots directly to a main-thread `ClientChunkCache`
- `ClientChunkCache.applyPackedChunkSnapshot(...)` unpacks packed sections into current `ChunkSnapshot`/`SnapshotLevelChunk` compatibility objects
- `buildSectionMeshInput(...)` runs on the main thread and gathers 3x3 chunk snapshots for each section build
- the mesh worker receives object-heavy `ChunkSnapshot[]` payloads instead of reading from a worker-owned render-world cache

That is the wrong ownership shape for the accepted architecture. `D4` moves ownership, not authority. The authoritative host remains the only source of world truth.

## Reference source

Read these before implementing:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java` | receives level chunk packets, installs them into the client chunk cache, and dirties render sections |
| `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java` | client-owned cache of authoritative chunk data |
| `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLevelChunkPacket.java` | packed-ish chunk payload sent from server to client |
| `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java` | owns view area, dirty render sections, and render scheduling |
| `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java` | asynchronously compiles chunk sections and uploads render buffers |
| `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/RenderChunkRegion.java` | builds section compilation context from nearby chunks |

## Architecture divergence review

Minecraft Java keeps the client chunk cache on the client thread and uses executor-backed `ChunkRenderDispatcher` tasks for chunk compilation. That fits a JVM client where the render thread and client game thread share heap objects and where GL upload constraints are already embedded in the renderer.

The browser has different constraints:

- main-thread JavaScript competes with input, animation, and WebGPU submission
- workers cannot share normal JS heap objects with the main thread
- WebGPU resources and canvas presentation stay on the main thread today
- packed chunk buffers can be transferred, but decoded object graphs should not be owned on the main thread

The divergence scope is therefore client runtime ownership:

- keep vanilla-like client cache semantics: authoritative chunk packets populate a client cache, chunk unloads remove chunks, changed sections are marked dirty
- keep renderer-native WebGPU upload and draw submission on the main thread
- move cache ownership and mesh-input neighborhood reads into a worker
- do not change simulation parity, host authority, chunk protocol semantics, or storage semantics

This makes future parity work easier or neutral. The render-world worker can still model `ClientChunkCache`/`RenderChunkRegion` closely, while the main thread no longer depends on raw chunk objects.

## Current TS context

| TS source | Current role |
|---|---|
| `src/runtime/protocol/world-messages.ts` | D3 `chunk_snapshot` protocol payload is `PackedChunkSnapshot` |
| `src/runtime/transport/local-world-transport.ts` | `TransportWorldClient` owns session/player state and can route chunk messages through a `RenderWorldUpdateSink`; compatibility cache mirroring remains available during migration |
| `src/runtime/transport/worker-world-transport.ts` | local authoritative host worker transport with transferable packed chunk buffers |
| `src/runtime/transport/remote-world-transport.ts` | remote HTTP transport using the D3 packed wire codec |
| `src/world/level/client-chunk-cache.ts` | compatibility cache class; live browser scene now keeps only a metadata shell on the main thread while the render-world worker owns loaded chunks |
| `src/renderer/chunk/chunk-mesh-protocol.ts` | legacy mesh-worker/fallback protocol that sends `ChunkSnapshot[]`; not used by the live browser render-world path |
| `src/renderer/chunk/mesh-worker-context.ts` | shared browser worker setup for generated blocks, state ids, biome color tables, model baking, atlas metadata, and `BlockRenderDispatcher` |
| `src/renderer/chunk/mesh-worker.ts` | legacy mesh worker that builds a temporary `ClientChunkCache` per mesh job; kept for compatibility tests/fallbacks |
| `src/renderer/chunk/mesh-worker-client.ts` | legacy mesh worker client; live browser scene uses `RenderWorldWorkerClient` instead |
| `src/renderer/chunk/render-world-protocol.ts` | D4 internal render-world protocol for initialization, packed update ingest, stats, mesh build, not-ready, and error responses |
| `src/renderer/chunk/render-world-worker-client.ts` | D4 request-envelope client/session wrapper with request ids, worker errors, transferable buffer handling, and `RenderWorldWorkerUpdateSink` adapter |
| `src/renderer/chunk/render-world-worker.ts` | D4 worker handler with worker-owned `ClientChunkCache`, packed update ingest, vanilla-shaped dirty section metadata, mesh-neighbor readiness checks, and CPU mesh build responses |
| `src/renderer/chunk/chunk-render-dispatcher.ts` | main-thread render chunk scheduling, render-world mesh requests by section origin/camera, fallback compilation, GPU buffer upload |
| `src/renderer/scene-setup.ts` | wires one render-world worker as packed-chunk update sink and section mesh builder; the compatibility main-thread level is quarantined inside renderer/light setup and `RendererScene` exposes only small world bounds, stats, dirty-section drains, and GPU/render state |
| `src/renderer/debug/debug-free-cam.ts` | drives world polling/chunk view and reads loaded chunk counts from render-world worker stats |

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Render-world worker protocol | messages exist for initialization, packed chunk ingest, chunk unload, dirty section notification, section mesh build, stats, and errors |
| 2 | Render-world worker | owns `ClientChunkCache` or packed-cache equivalent and never exposes raw chunk sections to the main thread |
| 3 | Browser world client sink | protocol chunk updates are forwarded to the render-world worker instead of applied to main-thread cache |
| 4 | Main renderer integration | `RendererScene` no longer exposes a main-thread `ClientChunkCache` for live browser rendering |
| 5 | Mesh input redesign | section mesh build requests identify a section/camera, and the worker gathers neighbor chunks internally |
| 6 | Dirty-section flow | chunk ingest/unload marks affected render sections dirty without main-thread chunk decode |
| 7 | GPU upload path | main thread receives mesh layer payloads keyed by section origin and uploads them to existing `VertexBuffer`s |
| 8 | Local/remote parity | worker singleplayer and remote multiplayer both feed the same render-world worker |
| 9 | Tests and browser validation | unit/integration/browser checks prove behavior and no main-thread chunk ownership regression |

## Explicit non-goals

- no host protocol change beyond browser-client routing of existing D3 messages
- no storage format change
- no `SharedArrayBuffer`
- no push transport, WebSocket, or WebTransport migration
- no render-world subworker pool
- no moving WebGPU device/canvas ownership off the main thread
- no full lighting, heightmap, block entity, entity, or chunk delta expansion
- no renderer culling overhaul beyond what is needed to remove main-thread raw chunk reads
- no byte-for-byte vanilla packet or NBT serialization work
- no gameplay prediction or client-authoritative world mutation

## Target ownership

### Main thread may own

- input, UI, debug overlay, camera, and session/player presentation state
- `LevelRenderer`, `ViewArea`, render chunk positions, dirty flags, visibility state, and GPU `VertexBuffer`s
- small metadata from the render-world worker, such as loaded chunk count, loaded chunk keys, dirty section coordinates, worker queue stats, and request ids
- mesh payloads after the worker has compiled them

### Main thread must not own

- `ClientChunkCache` as the live browser cache
- `PackedChunkSnapshot` section arrays after forwarding them to the render-world worker
- unpacked `ChunkSnapshot` objects for live browser rendering
- `SnapshotLevelChunk`/block-state section objects for live browser rendering
- 3x3 chunk-neighborhood snapshot arrays for mesh jobs

### Render-world worker owns

- packed chunk snapshot ingest and unload application
- the client cache or packed-cache equivalent
- biome containers needed by mesh/color reads
- section-neighbor availability checks
- `RenderChunkRegion`/mesh input construction
- CPU mesh compilation, initially in this same worker

`D4` can keep compatibility helpers internally in the worker. The important boundary is that decoded chunk content does not return to the main thread.

## Proposed module shape

Exact names can move, but keep the render-world worker in renderer/client runtime code, not in authoritative host code:

```text
src/
  renderer/
    chunk/
      render-world-protocol.ts
      render-world-worker-client.ts
      render-world-worker.ts
```

Likely protocol surface:

```ts
interface InitializeRenderWorldRequest {
  readonly type: "initialize_render_world";
  readonly seed: bigint;
  readonly minBuildHeight: number;
  readonly height: number;
}

interface IngestRenderWorldUpdatesRequest {
  readonly type: "ingest_render_world_updates";
  readonly messages: readonly (
    | { readonly type: "chunk_snapshot"; readonly snapshot: PackedChunkSnapshot }
    | { readonly type: "chunk_unload"; readonly chunkX: number; readonly chunkZ: number }
  )[];
}

interface BuildRenderSectionMeshRequest {
  readonly type: "build_render_section_mesh";
  readonly origin: { readonly x: number; readonly y: number; readonly z: number };
  readonly camera: { readonly x: number; readonly y: number; readonly z: number };
}

interface RenderSectionMeshBuiltResponse {
  readonly type: "render_section_mesh_built";
  readonly origin: { readonly x: number; readonly y: number; readonly z: number };
  readonly result: SectionMeshResult;
}
```

Responses should also include:

- `render_world_ready`
- `render_world_stats`
- `render_world_dirty_sections`
- `render_world_mesh_not_ready` for missing neighbors or unloaded sections
- `render_world_error`

This is an internal browser render protocol. It must not replace the D3 host/client protocol.

## Current implementation status

Landed so far:

- `render-world-protocol.ts` defines the internal browser render-world messages for initialization, packed update ingest, stats, mesh build, mesh-not-ready, and worker errors
- `render-world-worker-client.ts` mirrors the existing mesh-worker request-envelope pattern and transfers packed chunk buffers to the worker plus mesh layer buffers back to the main thread
- `test/renderer/chunk/render-world-worker-client.test.ts` covers fake-endpoint initialization, request ids, packed snapshot transfer, mesh payload transfer, mesh-not-ready responses, and worker/endpoint errors
- `mesh-worker-context.ts` factors the browser worker block/model/atlas/color initialization out of the old mesh worker
- `render-world-worker.ts` initializes a worker-owned `ClientChunkCache`, ingests packed chunk updates, reports vanilla-shaped dirty section origins, checks mesh-neighbor availability, and returns mesh build or not-ready responses
- `test/renderer/chunk/render-world-worker.test.ts` pins the dirty-section coordinate expansion, mesh-neighbor chunk set, and initialization guard
- `TransportWorldClient` separates chunk message routing through `RenderWorldUpdateSink` from session/player presentation state, with an explicit temporary compatibility-cache mirror option
- `RenderWorldWorkerUpdateSink` adapts D3 `chunk_snapshot` / `chunk_unload` messages into bounded render-world worker ingest batches, so the browser scene can turn on forwarding when the dispatcher consumes worker-owned meshes
- `test/runtime/render-world-update-sink.test.ts` proves chunk messages can go to a sink without mutating the client cache, and can be mirrored only when requested
- `ChunkRenderDispatcher` can use a render-world mesh client: rebuild tasks send section origin/camera only, consume built/not-ready responses, and skip main-thread cache/neighbor snapshot reads on that path
- `src/renderer/scene-setup.ts` initializes `RenderWorldWorkerClient`, attaches `RenderWorldWorkerUpdateSink` to the world client with cache mirroring disabled, and passes the same worker to the dispatcher for mesh builds
- `src/renderer/main.ts` and `src/renderer/debug/debug-free-cam.ts` now wait on render-world loaded-count stats and drain worker dirty-section coordinates into `ViewArea.setDirty(...)`
- `test/renderer/chunk/chunk-render-infrastructure.test.ts` proves dispatcher render-world builds work while the main-thread compatibility cache has zero loaded chunks
- `RenderWorldWorkerClient`, `RenderWorldWorkerUpdateSink`, `ChunkRenderDispatcher`, `src/renderer/main.ts`, and `src/renderer/debug/debug-free-cam.ts` expose D4 performance-smoke counters for ingest batches, mesh build requests, not-ready responses, mesh completions, and main-thread GPU uploads
- `test/browser/debug-free-cam.test.ts` now uses scripted remote input across chunk interest and asserts render-world ingest counters increase after movement
- `test/browser/smoke.test.ts` asserts render-world ingest/build/upload counters are nonzero during browser boot
- `RendererScene` no longer exposes the compatibility `ClientChunkCache`; dirty-section clipping uses small `worldBounds` metadata
- browser remote-host validation is isolated per test by `test/browser/remote-world-host-fixture.ts`; `playwright.config.ts` starts only Vite, and remote specs create a fresh `GeneratedWorldHttpServer` on a random localhost port
- `GeneratedWorldHttpServer.stop()` closes active HTTP connections during shutdown, and the browser remote-host fixture retries temp save-root cleanup to avoid filesystem races with aborted browser requests
- browser validation for this slice passed with terrain visible in `/tmp/mclone-browser-smoke.png`, `/tmp/mclone-debug-free-cam.png`, `/tmp/mclone-debug-free-cam-tall.png`, and `/tmp/mclone-debug-cave-mouth.png`
- full `pnpm test:browser` passes 23/23 specs with the isolated remote-host fixture

## Implementation sequence

1. Add a render-world worker protocol and client wrapper.

   Done. The protocol/client wrapper exists with fake-endpoint tests before any real browser `Worker` hookup.

2. Move worker-side cache initialization.

   Done. `mesh-worker-context.ts` now shares generated/render block registration, `BlockStateIdMap`, biome color tables, model/atlas setup, and `BlockRenderDispatcher` construction between the old mesh worker and the render-world worker.

3. Add packed chunk ingest.

   Done. Worker-side ingest, bounded `RenderWorldWorkerUpdateSink` forwarding, and live browser scene hookup are active. The live scene disables compatibility-cache mirroring, so packed chunk section arrays transfer to the render-world worker instead of being decoded on the main thread.

4. Split protocol application from cache mutation.

   Done at the transport boundary. `TransportWorldClient` keeps `world_opened`, `session_state`, `player_state`, and errors on the main thread, while chunk messages can go to a `RenderWorldUpdateSink` without touching `ClientChunkCache`. The explicit mirror path exists for transitional call sites but is not enabled in the live scene yet.

5. Replace main-thread mesh input gathering.

   Done for the live browser path. `ChunkRenderDispatcher` still keeps the legacy `buildSectionMeshInput(...)` fallback, but render-world rebuild tasks send section origin/camera to `RenderWorldWorkerClient`; the worker gathers neighbors from its own cache and returns mesh data or a not-ready result.

6. Keep GPU upload main-thread only.

   Done. `VertexBuffer` ownership and upload remain in `ChunkRenderDispatcher.RenderChunk`. Worker output remains CPU-side mesh layer payloads and serialized draw state, transferred back to the main thread.

7. Update scene/debug state.

   Done. Debug overlays and browser smoke waits use render-world loaded-count/stats/counters, dirty-section coordinates are drained into `ViewArea`, and `RendererScene` exposes `worldBounds` instead of a compatibility cache.

8. Preserve non-browser/test adapters where useful.

   Unit tests may still instantiate `ClientChunkCache` directly. The D4 boundary requirement applies to the live browser renderer path.

9. Add D4 performance-smoke counters.

   Done. `RenderWorldWorkerClient` counts ingest batches, mesh build requests, not-ready responses, and mesh completions. `ChunkRenderDispatcher` counts main-thread GPU uploads. Browser smoke exposes and asserts nonzero boot counters, and debug free-cam asserts ingest batches increase after scripted chunk-interest movement.

10. Stabilize browser validation.

   Done. Remote browser specs use a per-test host fixture instead of a shared `4173` host, server shutdown closes active HTTP connections, temp save roots retry cleanup, and the full browser suite passes.

## Dirty-section policy

Follow vanilla's broad invalidation shape from `ClientPacketListener.handleLevelChunk(...)`: when a chunk snapshot arrives, all sections in that chunk should be considered dirty with neighbor awareness.

For D4:

- render-world worker ingests the chunk
- render-world worker reports dirty section coordinates for the changed chunk and neighbor section shell
- main thread marks matching `ViewArea` render chunks dirty using coordinates only
- rebuild jobs ask the render-world worker whether the section has enough neighbor data

Small coordinate metadata is allowed on the main thread. Raw chunk facts are not.

## Validation

### Unit tests

Render-world protocol:

- initializes once and rejects build/ingest before initialization
- transfers mesh layer buffers in responses
- preserves request ids and reports worker errors

Worker-owned cache:

- ingest packed chunk snapshot increases loaded chunk count without exposing a `ChunkSnapshot`
- unload removes the chunk and reports dirty sections
- section mesh build succeeds when required neighbor chunks are present
- section mesh build returns a not-ready response when neighbor chunks are missing
- packed snapshot buffers are not mutated by worker ingest unless they were explicitly transferred

Main-thread integration:

- browser world-client wrapper forwards chunk messages to a render-world sink and keeps session/player state locally
- live renderer setup no longer applies chunk snapshots to a main-thread `ClientChunkCache`
- `ChunkRenderDispatcher` requests meshes by section origin/camera, not by `ChunkSnapshot[]`
- no live browser path calls `buildSectionMeshInput(...)` on the main thread

### Existing tests to update

- `test/runtime/worker-world-transport.test.ts`
- `test/runtime/remote-world-transport.test.ts`
- `test/runtime/generated-world-boundary.test.ts`
- `test/world/client-chunk-cache.test.ts`
- `test/renderer/chunk/chunk-render-infrastructure.test.ts`
- browser smoke tests around `src/renderer/main.ts` and `src/renderer/debug/debug-free-cam.ts`

Keep direct `ClientChunkCache` tests for the compatibility class. Add separate render-world worker tests for the live browser ownership path.

### Browser visual validation

D4 changes pixels indirectly, so it requires browser validation:

- run `pnpm test:browser`
- capture a screenshot at the first successful rendered frame and save it under `/tmp`, for example `/tmp/mclone-d4-render-world.png`
- inspect the screenshot and confirm chunks render, terrain is not blank, and visible section boundaries do not obviously flicker or disappear
- if any canvas is blank, colors are wrong, chunks never appear, or WebGPU validation errors show up, stop and fix before continuing

### Performance smoke

This is not the final D5 measurement gate, but D4 includes a small sanity probe:

- run a scripted camera move across at least one chunk boundary
- confirm the main thread no longer performs raw chunk snapshot unpack, raw section decode, or neighbor snapshot gathering
- log or expose counts for render-world ingest, mesh build requests, mesh not-ready responses, mesh completions, and main-thread GPU uploads

Current coverage:

- `test/browser/debug-free-cam.test.ts` drives remote input until chunk interest crosses a chunk boundary and asserts render-world ingest batches increase
- `test/browser/smoke.test.ts` asserts ingest batches, mesh build requests, mesh completions, and main-thread GPU uploads are nonzero
- the exposed counter surface is `RenderWorldPerformanceCounters` from `src/renderer/scene-setup.ts`

Current browser validation:

- targeted remote browser specs pass: smoke, debug free-cam movement, debug free-cam tall resize, and cave-mouth
- full `pnpm test:browser` passes 23/23 specs
- inspected screenshots under `/tmp`: `/tmp/mclone-browser-smoke.png`, `/tmp/mclone-debug-free-cam.png`, `/tmp/mclone-debug-free-cam-tall.png`, and `/tmp/mclone-debug-cave-mouth.png`

### Static checks

- `pnpm typecheck`
- `pnpm test`

## Done when

- browser runtime no longer owns loaded chunks/raw sections in a main-thread `ClientChunkCache` for live world rendering
- host protocol/storage remain the D3 packed chunk contracts
- local worker singleplayer and remote multiplayer both forward packed chunk updates into the same render-world worker path
- render-world worker owns chunk cache state and section-neighbor gathering
- main thread mesh requests identify section origin/camera only, not `ChunkSnapshot[]`
- main thread receives mesh payloads and uploads GPU buffers without reading raw chunk sections
- dirty-section updates cross from worker to main as coordinates/small metadata only
- browser debug/smoke code uses render-world stats instead of `scene.level.getLoadedChunkCount()`
- no live browser path imports or calls `buildSectionMeshInput(...)` on the main thread
- no `SharedArrayBuffer` or push transport work is introduced
- browser screenshot validation passes with the screenshot saved to `/tmp`
- `pnpm typecheck`, `pnpm test`, and `pnpm test:browser` pass

## Next

D4 is complete. The next tactical is `D5`: transport measurement and push/SAB decision. Start by writing the detailed `D5` plan with a repeatable browser traversal, trace capture, frame-pacing metrics, and timing marks for host load/generation, snapshot encode, transport receive/decode, render-world ingest, mesh build, GPU upload, and request-to-visible latency.

After that plan is accepted, measure the actual browser traversal path with render-world ownership in place and decide from trace data whether HTTP polling, worker transfer/copy, mesh fan-out, or GPU upload is the next bottleneck.
