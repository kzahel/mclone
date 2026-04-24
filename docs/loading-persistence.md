# Loading And Persistence

Durable guidance for world creation, chunk loading, generation, saving, eviction, and reload semantics.

This document has two jobs:

1. Describe Minecraft Java 1.17.1's loading and persistence model closely enough to guide parity work.
2. Describe where `mclone` intentionally or accidentally diverges today, with immediate fixes separated from acceptable deferrals.

[`architecture.md`](./architecture.md) owns runtime boundaries. [`runtime-data-model.md`](./runtime-data-model.md) owns logical chunk and block-state facts. [`protocol.md`](./protocol.md) owns host/client messages. [`authoritative-host-scheduling.md`](./authoritative-host-scheduling.md) owns scheduler rules that keep player/session authority responsive while chunk jobs run.

## Core Rule

The authoritative host owns world lifecycle and persistence policy.

Clients express interest and send commands. Renderers consume authoritative snapshots and deltas. Storage adapters persist logical world/chunk records. Storage is not the simulation model, and IndexedDB/file layout should never become the source of gameplay truth.

## Create, Open, Join

Singleplayer and multiplayer should share the same conceptual flow:

1. Host opens or creates the save.
2. Client joins or resumes a session.
3. Client declares chunk interest.
4. Host loads or generates chunks needed for aggregate interest.
5. Host sends authoritative snapshots and later deltas.

Browser singleplayer differs only in host placement:

- Local singleplayer host: browser worker.
- Dedicated host: Node process.

The renderer should not call worldgen directly in either mode.

## Vanilla Reference Map

Primary 1.17.1 source files:

- Chunk residency, scheduling, load/generate choice: [`ChunkMap.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)
- Main-thread chunk access and server save/close entrypoints: [`ServerChunkCache.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java)
- Chunk status pipeline: [`ChunkStatus.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)
- NBT chunk read/write: [`ChunkSerializer.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java)
- Full chunk dirty flag, tick unpack/pack, post-load work: [`LevelChunk.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunk.java)
- Proto chunk status, generated lights, postprocessing, proto ticks: [`ProtoChunk.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java)
- Threaded lighting load/generation integration: [`ThreadedLevelLightEngine.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java)

## Vanilla Model

### Authority Boundary

Minecraft Java runs an authoritative server even in singleplayer. The server owns chunk residency, generation, lighting, ticks, entity/block-entity state, dirty flags, and disk IO. The client receives chunk and light packets and renders them.

The browser/WebGPU equivalent in `mclone` is not class-for-class parity, but the ownership rule should stay the same: the host owns loaded world state; the renderer is a client cache.

### Chunk Interest And Residency

Vanilla does not load chunks because the renderer asks for blocks directly. Player view, simulation needs, commands, and other systems create tickets through the distance manager. `ServerChunkCache.getChunk(...)` can also add a ticket when a caller requires a chunk.

`ChunkMap` owns `ChunkHolder`s. A holder tracks futures for chunk statuses and its current ticket level. When distance/tickets change, holders are created, promoted, demoted, or queued for unload.

Important consequences:

- Interest is aggregate, not renderer-owned.
- Loading work is asynchronous and status based.
- Unload is delayed until save/light/entity obligations are handled.
- Client visibility is a filtered view of authoritative residency.

### Status Pipeline

Vanilla chunks are not just "generated" or "loaded". They move through `ChunkStatus`:

```text
EMPTY
  -> STRUCTURE_STARTS
  -> STRUCTURE_REFERENCES
  -> BIOMES
  -> NOISE
  -> SURFACE
  -> CARVERS
  -> LIQUID_CARVERS
  -> FEATURES
  -> LIGHT
  -> SPAWN
  -> HEIGHTMAPS
  -> FULL
```

Each status declares a parent, dependency range, generation task, loading task, chunk type, and heightmaps available after that status. If a stored chunk already satisfies a requested status, the loading task advances or passes it through. If it does not, the generation task resumes from the stored status rather than starting from nothing.

This matters for future parity because persisted partial chunks are real vanilla state. For MVP we mostly deal in fully publishable chunks, but the architecture should not make status-based loading impossible later.

### Load Or Generate Flow

Vanilla's high-level flow is:

1. A system requests a status for a chunk through `ServerChunkCache` / `ChunkMap`.
2. If needed, tickets create or retain a `ChunkHolder`.
3. Requesting `ChunkStatus.EMPTY` schedules disk load.
4. `scheduleChunkLoad(...)` reads chunk NBT. If a valid chunk tag exists, `ChunkSerializer.read(...)` returns a `ProtoChunk` or an `ImposterProtoChunk` wrapping a `LevelChunk`. If no usable tag exists, vanilla creates an empty `ProtoChunk`.
5. For later requested statuses, `ChunkMap.schedule(...)` checks whether the loaded chunk already has that status. If yes, it runs the status loading task. If no, it schedules generation from the current status through the missing statuses.
6. `ChunkStatus.FULL` converts the proto path to `LevelChunk`, runs post-load work, and makes the chunk available to ticking/access/client send paths.

The key rule is storage lookup before generation, with status-aware resume.

### Stored Chunk Facts

`ChunkSerializer` writes vanilla's chunk NBT. Important fields include:

- `DataVersion`
- position and `Status`
- block sections (`Palette`, `BlockStates`)
- biomes
- heightmaps
- structures and structure references
- block entities
- entities for proto/legacy chunk paths
- postprocessing offsets
- carving masks for proto chunks
- pending block/fluid ticks
- light section bytes (`BlockLight`, `SkyLight`)
- `isLightOn`
- inhabited time and last update time

This is not just a render cache. It is the world state at a specific chunk status.

### Light Load And Save

Vanilla persists light as authoritative chunk data when a chunk is light-correct:

- On read, `isLightOn` controls whether stored light section bytes are queued into the light engine as trusted data.
- If a proto chunk is at or after `LIGHT` but does not have trusted light data, `ChunkSerializer.read(...)` rescans emitting blocks and records them for relighting.
- `ChunkStatus.LIGHT` calls `ThreadedLevelLightEngine.lightChunk(...)`.
- `lightChunk(...)` can reuse trusted light when the chunk status and `isLightCorrect()` say that is safe; otherwise it queues section status, sky sources, and block emitters, then marks the chunk light-correct after light work completes.
- Save writes non-empty light data layers and writes `isLightOn` only when the chunk is light-correct.

So in vanilla, stale persisted light is avoided by `isLightOn`, chunk status, light-correct state, and the light engine's load path. The stored light bytes are trusted only through that protocol.

### Tick Load And Save

Vanilla preserves scheduled ticks as chunk state:

- Proto chunks store `ToBeTicked` and `LiquidsToBeTicked`.
- Full chunks store `TileTicks` and `LiquidTicks`.
- On accessible/full chunk promotion, `LevelChunk.unpackTicks()` copies chunk-local saved ticks into the server tick lists.
- Before save, `LevelChunk.packTicks(...)` can pull pending ticks for that chunk back into chunk-local records.

This prevents "settling" fluids or block updates during load. Loading restores pending work; normal ticks execute it later if the chunk is eligible to tick.

### Dirty Save Policy

Vanilla does not save every publish. `ChunkAccess` exposes `isUnsaved()` and `setUnsaved(...)`. Full chunk block mutations mark the chunk unsaved. `LevelChunk.setLightCorrect(...)` also marks unsaved.

`ChunkMap.save(...)` returns early if the chunk is not unsaved. Dirty chunks are saved during explicit flushes, periodic/accessibility saves, and unload processing. On unload, vanilla waits for the chunk-to-save future, saves if needed, unloads entities/block entities, releases light data, and publishes status changes.

This is the main policy distinction:

- Loaded/generated chunks are authoritative in memory.
- Disk is durable storage for dirty or otherwise save-worthy chunk state.
- Clean generated chunks may remain unsaved until vanilla's save policy decides otherwise.

## Current `mclone` Model

### Storage Adapters

`mclone` has an engine-native storage boundary:

- [`WorldStorage`](../src/runtime/storage/world-storage.ts)
- [`IndexedDbWorldStorage`](../src/runtime/storage/indexeddb-world-storage.ts)
- [`FileWorldStorage`](../src/runtime/storage/file-world-storage.ts)
- [`MemoryWorldStorage`](../src/runtime/storage/memory-world-storage.ts)

The adapter contract is deliberately smaller than vanilla NBT:

```text
open world
load chunk snapshot
save chunk snapshot
record eviction
close
```

Browser singleplayer uses IndexedDB inside the authoritative worker. Dedicated/remote host uses file-backed JSON records under a save root. Unit tests generally use memory storage.

### Save Identity And Compatibility

Generated-world saves use:

```text
generated-world-v<GENERATED_WORLD_STORAGE_VERSION>-<preset>-<seed>
```

Metadata compatibility currently checks:

- storage version
- seed
- preset
- min build height
- height

If metadata is incompatible, the storage adapter resets that save's chunk records. This is a coarse replacement for vanilla `DataVersion`/DataFixer and for content-generation cache invalidation.

### Current Chunk Lifecycle

The practical host lifecycle today is:

```text
unloaded
  -> loaded from storage as a decorated full snapshot
  -> or generated terrain
  -> decorated
  -> light computed
  -> snapshot published
  -> eagerly saved
  -> evicted from in-memory view when outside interest
```

`GeneratedRenderLevel` has no vanilla-style `ChunkStatus`. It tracks loaded chunks and a `decoratedChunks` set. In the cooperative browser path, `updateChunkView(...)` returns missing chunks when a chunk is absent or present but not decorated.

### Current Load/Generate Flow

Browser worker and remote Node host both use cooperative scheduling.

For each chunk job:

1. The host tries `preloadStoredChunk(...)`.
2. If storage returns a snapshot, the host hydrates block states and scheduled ticks into a `LevelChunk` and marks it decorated.
3. If storage misses, the host generates terrain.
4. Newly generated chunks are decorated.
5. The host computes lighting for currently loaded chunks.
6. The host builds a packed snapshot, saves it, publishes it, and may later publish light deltas.

The important point: current code does check storage before generation, but the progress stage calls that phase "Generating terrain chunks" even when most work is actually storage lookup and snapshot hydration.

### Current Snapshot Facts

`PackedChunkSnapshot` contains:

- chunk coordinates
- packed block sections
- biome ids
- optional packed light sections
- pending block ticks
- pending liquid ticks

It does not contain:

- vanilla chunk status
- heightmaps
- structures or structure references
- entities
- block entities
- carving masks
- postprocessing offsets
- inhabited time
- per-chunk dirty/unsaved state
- explicit content/light algorithm version

Hydration currently restores blocks and scheduled ticks. It ignores persisted biomes and persisted light; biomes are regenerated from the current biome source when snapshots are rebuilt.

### Current Light Behavior

The host writes light into packed snapshots, but stored light is not hydrated back into the host light engine. On reload, stored chunks are hydrated as blocks, then `buildPackedChunkSnapshot(...)` calls lighting setup and recomputes light from current block state.

That means persisted light bytes are currently not authoritative for host reload. A lighting algorithm fix should take effect after reload even if IndexedDB contains old light bytes. The risk is still real in two ways:

- The data model suggests persisted light is trusted when it is not.
- If we later hydrate stored light without a version/trust protocol, old IndexedDB/file records can become stale immediately.

### Current Save Policy

`mclone` currently has eager snapshot persistence:

- The synchronous path saves all loaded chunks after a changed view.
- The cooperative path saves each chunk snapshot before publishing it.
- Dirty liquid chunks are saved when flushed.
- Eviction only records adapter-local `lastEvictedAtMs`; it does not perform a save because the current policy assumes chunks were already saved.

There is no `isUnsaved`/dirty flag equivalent for general chunk state. Generated clean chunks and mutated chunks use the same `saveChunk(...)` path.

This is a deliberate early runtime simplification, not vanilla behavior.

### Current Remote Host

The remote HTTP service creates one `GeneratedWorldHost` per save id. It tracks multiple sessions separately, computes an aggregate chunk view, sends that aggregate interest to the host, and filters host snapshots/deltas back to each session.

This preserves the main ownership rule: chunks are generated/loaded once per authoritative world. It diverges from vanilla's ticket/distance system because aggregation is currently a rectangular view union rather than a general ticket graph.

### Current Test Isolation

Current test behavior is mixed:

- Unit tests use `MemoryWorldStorage`.
- Browser smoke/integration tests mostly use the remote Node host with a per-test temporary file save root.
- Worker-backed browser probes use IndexedDB through the browser worker, but there is no explicit `indexedDB.deleteDatabase("mclone-world-storage")` test fixture.

Fresh Playwright contexts reduce leakage, but tests do not currently assert or force IndexedDB cleanup. Dev browser refreshes reuse the real browser profile and will reuse `mclone-world-storage`.

## Delta Matrix

| Area | Vanilla 1.17.1 | Current `mclone` | Risk | Decision |
|---|---|---|---|---|
| Authority | Integrated/dedicated server owns chunks, simulation, lighting, save policy | Host worker/Node service owns chunks; renderer is a client cache | Good architectural match | Keep |
| Physical storage | Region/NBT plus `DataVersion` and DataFixer | Engine-native snapshots in IndexedDB or JSON files | Intentional platform divergence | Keep adapter boundary |
| Chunk status | Explicit persisted `ChunkStatus`; resume partial generation | No persisted status; chunks are absent, generated terrain, decorated, or published | Future parity work can become harder if status does not fit later | Defer, but keep docs and APIs status-friendly |
| Load before generate | Disk load at `EMPTY`, then generate missing statuses | Storage lookup before terrain generation | Match in principle | Keep |
| Generated clean persistence | Dirty/save policy; not every publish writes | Eager save of every published chunk | Excess IO, confusing "save" semantics, stale generated cache after content changes | Revisit soon |
| Dirty tracking | `isUnsaved` gates save | No general dirty flag | Mutations and generated cache are conflated | Immediate design target |
| Light persistence | Saved and hydrated only through `isLightOn`/light-correct trust path | Saved but ignored on host reload | Confusing and future stale-data trap | Fix soon |
| Tick persistence | Proto/full tick lists preserved; unpacked into server tick lists when accessible | Block/liquid tick snapshots restored into chunks; liquid host hydrates published chunk ticks | Reasonable partial match for liquid work | Continue parity work |
| Heightmaps | Stored and primed if missing | Not stored in snapshots | Current systems recompute or avoid persisted heightmaps | Defer until needed |
| Structures | Stored starts/references | Not persisted | Structures are post-MVP | Defer |
| Entities/block entities | Persisted and loaded | Not persisted | Gameplay persistence missing | Defer until entity/block-entity slices |
| Postprocessing/carving masks | Persisted for proto chunks | Not modeled in storage | Relevant to full vanilla status pipeline | Defer |
| Save on unload/close | Dirty chunks saved before unload/flush/close | Chunks are expected to have been saved before publish; close is a no-op | Unsafe if we move to lazy dirty policy | Fix with dirty tracking |
| Client chunk visibility | Server filters packets per player interest | Remote service filters snapshots per session | Close enough for current multiplayer | Keep until ticket model grows |
| Progress UI | Status listener reports status changes | UI labels storage lookup as generation and skips hit/miss counts | User confusion | Immediate fix |
| Test storage isolation | N/A | No explicit IndexedDB cleanup for worker probes/dev profile | Stale local data can mask or confuse changes | Immediate test/dev hygiene |

## Recommended Target Shape

### Separate Durable State From Derived Cache

Use two concepts explicitly:

- **Durable save state**: user/world mutations and vanilla state that must survive reload.
- **Derived generated cache**: deterministic generated chunks or light data that may be discarded when algorithms or versions change.

Generated chunks can be cached for faster refresh, but the code and metadata should make that policy obvious. A generated-clean cache miss should be acceptable. A mutated chunk save miss is data loss.

### Host Chunk State

Use explicit host-side state names even before implementing full vanilla statuses:

```text
unloaded
  -> loading_from_storage
  -> loaded_from_storage_clean
  -> generating_terrain
  -> decorating
  -> lighting
  -> published_clean
  -> dirty
  -> saving
  -> evictable
  -> unloaded
```

Later, the generated/decorated/light phases can be replaced or refined by real `ChunkStatus` values.

### Save Policy

Target policy:

- Mutations mark chunks dirty.
- Dirty chunks are persisted before eviction and on world close/flush.
- Generated-clean chunks may be cached eagerly or lazily, but the cache must be versioned and discardable.
- If a chunk is both generated and mutated, durable mutation semantics win.
- Adapter-level `lastLoadedAtMs` and `lastEvictedAtMs` are diagnostics, not simulation state.

### Versioning

Keep `GENERATED_WORLD_STORAGE_VERSION`, but split responsibilities over time:

- physical snapshot schema version
- world definition/content version
- lighting data version
- optional feature/profile version

Until that split exists, bump `GENERATED_WORLD_STORAGE_VERSION` whenever stored generated chunks could be misleading after a code change.

### Light Policy

Pick one of these before treating saved light as authoritative:

1. **Do not persist light yet**. Recompute light from blocks on reload and omit light from storage records. Snapshots sent to clients can still include light.
2. **Persist and hydrate trusted light**. Store a light algorithm version, hydrate section data into the host light engine, honor `lightCorrect`, and invalidate/recompute when the version or trust marker is incompatible.

The current halfway state should not remain long-term.

## Immediate Work

These are small enough and high enough leverage to do before deeper persistence work.

1. Rename loading progress stages and include real counts.

   Use stages like `Checking saved chunks`, `Generating missing chunks`, `Decorating new chunks`, `Computing light`, and `Publishing chunks`. Report storage hits, storage misses, generated count, and published count separately.

2. Add explicit browser storage hygiene.

   Add a dev/debug command or query option to clear `mclone-world-storage`, and add a Playwright fixture for worker-backed probes that deletes the IndexedDB database or uses unique save ids.

3. Resolve current light persistence ambiguity.

   Prefer omitting light from persisted storage until we implement trusted light hydration. If keeping it, add a documented light-data version and make stale light invalidation explicit.

4. Add dirty/cache terminology to host code.

   Even before a full lazy save implementation, distinguish "cache generated chunk snapshot" from "save dirty authoritative chunk". This prevents future work from treating every saved generated chunk as user state.

5. Document the version bump rule in code near `GENERATED_WORLD_STORAGE_VERSION`.

   The comment should say exactly which changes require a bump: block-state ids/layout, generation output, snapshot hydration semantics, saved tick format, and trusted light policy.

## Deferred Work

These are real vanilla deltas, but they do not need to block current renderer/worldgen progress:

- Full `ChunkStatus` persistence and partial-status resume.
- Vanilla Anvil/region physical layout.
- DataFixer-style migrations instead of reset-on-incompatibility.
- Entity, player, inventory, and block-entity persistence.
- Structure starts/references in chunk saves.
- Heightmap persistence.
- Postprocessing and carving mask persistence.
- General ticket graph parity beyond rectangular chunk-view interest.
- Trusted persisted-light hydration, if we choose not to do it immediately.

## Working Rule For Future Slices

Before changing loading, persistence, or chunk publication, explicitly answer:

1. Is this state authoritative durable data or a discardable generated cache?
2. Does vanilla store it, derive it, or recompute it?
3. If stored, what version/trust marker invalidates stale data?
4. What marks the chunk dirty?
5. When is the dirty state flushed?
6. What does the renderer see while the host is loading, generating, saving, or evicting?

If those answers are unclear, update this document before changing code.
