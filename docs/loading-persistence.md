# Loading And Persistence

Durable guidance for world creation, chunk loading, generation, saving, eviction, and reload semantics.

This document has two jobs:

1. Describe Minecraft Java 1.17.1's loading and persistence model closely enough to guide parity work.
2. Describe where `mclone` intentionally or accidentally diverges today, with immediate fixes separated from acceptable deferrals.

[`architecture.md`](./architecture.md) owns runtime boundaries. [`runtime-data-model.md`](./runtime-data-model.md) owns logical chunk and block-state facts. [`protocol.md`](./protocol.md) owns host/client messages. [`persistence-architecture.md`](./persistence-architecture.md) owns the broader shared persistence target for chunks, entity chunks, player data, saved data, and platform backends. [`authoritative-host-scheduling.md`](./authoritative-host-scheduling.md) owns scheduler rules that keep player/session authority responsive while chunk jobs run. [`worldgen-deterministic-order.md`](./worldgen-deterministic-order.md) owns vanilla status order, finality, lighting gates, and chunk publication gates. [`structures.md`](./structures.md) owns the structure-specific start/reference/placement model inside the status pipeline.

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

Structure-specific status semantics are not optional details: `STRUCTURE_STARTS` records starts, `STRUCTURE_REFERENCES` records touched-start references, noise-affecting structures can alter `NOISE`, and final structure slices are placed during `FEATURES`. The canonical status-order and finality model is [`worldgen-deterministic-order.md`](./worldgen-deterministic-order.md); structure implementation details live in [`structures.md`](./structures.md).

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

- `native/crates/mclone-server/src/persistence.rs` defines the shared
  `WorldStore` contract, `ChunkRecord`, `PersistenceActor`,
  `PersistenceMailbox`, `NullWorldStore`, `MemoryWorldStore`,
  native `SqliteWorldStore`, `ChunkSnapshotWorldStore`, and the current
  `ChunkSnapshotStore` compatibility layer.
- `native/crates/mclone-server/src/scheduler.rs` owns dirty holder tracking, save-on-unload, and `save_dirty_chunks()`.
- `native/crates/mclone-server/src/integrated.rs` exposes integrated-server save/reload behavior to native clients.

The shared store contract is completion-based and the scheduler now polls the
actor mailbox directly:

```text
WorldStore requests/completions
  load/save chunk records
  load/save entity chunk records for stores that advertise support
  pending same-key write visibility
  cache/durable write lanes
  flush/close

ChunkScheduler actor integration
  schedule load before generation
  drop stale load completions when interest disappears
  keep dirty pending-unload holders resident until durable save ack
  queue generated-clean cache writes separately from durable dirty writes
  flush durable dirty saves through the actor lane
  pack/hydrate scheduled block/fluid ticks through chunk records
  route entity chunk load completions and save entity records before holder
  unload
```

The synchronous facade remains only as compatibility/testing glue around the
same mailbox. On native targets, `PersistenceMailbox` can also run a `Send`
world store on its own worker thread behind the same request/completion
contract; the inline backend remains the default for tests, compatibility
stores, and WASM paths. Entity chunks have a shared record/mailbox foundation
and are wired through scheduler/integrated-host load, dirty-save, and unload
handling for `WorldStore` backends that opt into entity chunks. Snapshot-only
compatibility stores remain chunk-only.

Native `SqliteWorldStore` persists block and entity chunk records in one
SQLite database and can run through the threaded mailbox. It is not yet wired
into native app or dedicated-server world-dir startup paths. Browser
singleplayer uses IndexedDB inside the authoritative worker. Dedicated/remote
host uses file-backed JSON records under a save root. Unit tests generally use
memory storage.

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
  -> generated-clean cache write queued
  -> evicted from in-memory view when outside interest
```

`GeneratedRenderLevel` has no vanilla-style `ChunkStatus`. It tracks loaded chunks and a `decoratedChunks` set. In the cooperative browser path, `updateChunkView(...)` returns missing chunks when a chunk is absent or present but not decorated.

### Current Load/Generate Flow

Browser worker and remote Node host both use cooperative scheduling.

For each chunk job:

1. The host tries `preloadStoredChunk(...)`.
2. If storage returns a snapshot, the host hydrates block states and scheduled ticks into a `LevelChunk` and marks it as a full generated snapshot.
3. If storage misses, the host advances explicit generated chunk statuses through the terrain/carver boundary.
4. Newly generated chunks advance through `FEATURES` in the deterministic status order.
5. The host computes lighting only after the 3x3 `FEATURES` input is ready.
6. The host records the latest generated proto/full access in the resident holder's `chunkToSave` slot.
7. The host marks chunks `FULL`, publishes only chunks that satisfy the publication gate, queues persistence as a storage side effect, and may later publish light deltas.

The important point: current code checks storage before generation and now reports that as a distinct `Checking saved chunks` phase with stored/existing/missing counts. Missing chunks then move through `Generating status chunks`, `Advancing FEATURES`, `Computing light`, and `Publishing chunks`.

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

Hydration currently restores blocks and scheduled ticks. It ignores persisted biomes; biomes are regenerated from the current biome source when snapshots are rebuilt. The storage write path omits packed light, so persisted chunk records no longer carry light bytes.

### Current Proto/ChunkAccess Facts

`mclone` now has an in-memory generated chunk access shape for holder-owned generation progress:

- `GeneratedProtoChunk` records chunk position, completed status, section presence, unsaved state, and generated-content version.
- `GeneratedChunkHolder.chunkToSave` tracks the latest proto/full access while the holder is resident.
- Metadata-only status records can exist without block sections.
- `FULL` promotion converts the holder access to a full/level access.

Storage now has generated records beside packed full snapshots:

- `loadGeneratedChunk(...)` / `saveGeneratedChunk(...)` read and write status-shaped generated access records.
- Metadata-only records persist without block sections.
- Sectioned partials persist their status plus a packed section snapshot so `LIQUID_CARVERS` and `FEATURES` work can resume without rerunning completed phases.
- Generated records carry a per-chunk `writeVersion`; storage adapters reject lower-version generated-record writes so older queued partial records cannot overwrite newer full/dirty generated state.
- Host preload checks generated records before falling back to legacy packed snapshots.

This is still not a full vanilla `ProtoChunk`. The record does not yet store real structures, heightmaps, carving masks beyond the existing section snapshot path, postprocessing offsets, entities/block entities, inhabited time, or trusted light state.

### Current Light Behavior

The host writes light into packed snapshots sent to clients, but stored light is not hydrated back into the host light engine. The storage write path strips light from chunk records. On reload, stored chunks are hydrated as blocks, then `buildPackedChunkSnapshot(...)` calls lighting setup and recomputes light from current block state.

That means persisted light is currently not authoritative for host reload. A lighting algorithm fix should take effect after reload even if an older IndexedDB/file record used to contain light bytes. The remaining future risk is narrower:

- If we later hydrate stored light without a version/trust protocol, old IndexedDB/file records can become stale immediately.

### Current Save Policy

`mclone` currently has lazy generated-cache persistence plus explicit dirty saves:

- Published generated-clean chunks queue discardable cache writes as storage side effects instead of blocking publication.
- Queued storage side effects are keyed by chunk coordinate and run with bounded concurrency, so a slow save for one chunk does not block unrelated chunk cache writes. Same-chunk queued work remains ordered.
- Dirty chunks still use the durable save path before dirty chunk data is discarded.
- Generated status progress also marks the resident holder's proto/full `chunkToSave` access unsaved; host flush and holder pruning queue those generated records for storage.
- Holder residency now enters through named `player_view` and `generation_dependency` ticket sources. The generation ticket still covers the same derived square as before, but visible interest is tracked separately and covered-chunk counters use the union of ticketed chunks.
- Holder unloads now pass through a budgeted drop queue: normal passes process 200 holders and leave the rest resident but queued, while explicit flush drains the queue before waiting for storage side effects.
- Holders pruned while a generated-record save is pending stay in a pending-unload map; if interest returns before the save completes, the holder is resurrected instead of reading stale storage or regenerating.
- Partial generated records, generated-clean full cache records, and dirty full records use the same host-owned generated-record write-version stream; stale lower-version generated records are skipped by memory, file, and IndexedDB storage.
- Host preloads read through same-chunk pending writes for generated records and packed snapshots before consulting the storage adapter, matching vanilla `IOWorker.pendingWrites` semantics for host-queued saves.
- Async generated-record and legacy snapshot preloads re-check chunk-view revision and authority before hydrating; stale results are skipped when a walk moves the authority window while storage is still reading.
- Storage sessions carry adapter-local epochs. Reopening a save, resetting incompatible metadata, or closing the current session supersedes older sessions; stale-session loads return missing and stale-session writes/evicts are no-ops.
- Host block mutations mark the owning chunk dirty and mark published chunks for replacement snapshot publication.
- Dirty published chunks are saved when flushed.
- Dirty chunks are saved before eviction; clean eviction still only records adapter-local `lastEvictedAtMs`.

There is still no full vanilla `isUnsaved` equivalent across all future gameplay state. The current dirty state covers host block mutations and liquid-driven scheduled updates. Generated-clean chunks still use the same adapter-level `saveChunk(...)` call, but the host code now distinguishes cache writes from dirty durable saves before calling the adapter.

This is a deliberate early runtime simplification, not vanilla behavior.

### Current Remote Host

The remote HTTP service creates one `GeneratedWorldHost` per save id. It tracks multiple sessions separately, computes an aggregate chunk view, sends that aggregate interest to the host, and filters host snapshots/deltas back to each session.

This preserves the main ownership rule: chunks are generated/loaded once per authoritative world. It diverges from vanilla's ticket/distance system because aggregation is currently a rectangular view union rather than a general ticket graph.

### Current Test Isolation

Current test behavior is mixed:

- Unit tests use `MemoryWorldStorage`.
- Browser smoke/integration tests mostly use the remote Node host with a per-test temporary file save root.
- Worker-backed browser probes use IndexedDB through the browser worker and pass `clearWorldStorage=1` when they need deterministic empty storage.

Fresh Playwright contexts reduce leakage, but the explicit query is the deterministic clean-start path. Dev browser refreshes reuse the real browser profile and will reuse `mclone-world-storage` unless `clearWorldStorage=1` is present.

## Delta Matrix

| Area | Vanilla 1.17.1 | Current `mclone` | Risk | Decision |
|---|---|---|---|---|
| Authority | Integrated/dedicated server owns chunks, simulation, lighting, save policy | Host worker/Node service owns chunks; renderer is a client cache | Good architectural match | Keep |
| Physical storage | Region/NBT plus `DataVersion` and DataFixer | Engine-native snapshots in IndexedDB or JSON files | Intentional platform divergence | Keep adapter boundary |
| Chunk status | Explicit persisted `ChunkStatus`; resume partial generation | Explicit runtime and storage status labels exist for generated access records | Remaining risk is missing full proto payload fields, not the status fact itself | Tactical 58 partial save/load landed |
| Status future coalescing | `ChunkHolder` owns one future per `ChunkStatus` and reuses pending/completed work | Generated host now has holder slots for preload, status jobs, and `chunkToSave`; current chunk views request recursive statuses instead of building separate terrain/features batches | Remaining gap is ticket-shaped orchestration, not in-flight same-status coalescing | Tactical 57 landed |
| Partial proto save/resume | `chunkToSave` tracks latest `ProtoChunk`/`LevelChunk`; unsaved partials can save on unload | Holder `chunkToSave` records save/load metadata-only and sectioned partial records; section payload still reuses packed snapshots | Future vanilla fields are not represented yet | Tactical 58 continue proto payload expansion |
| Pending unload race safety | `pendingUnloads` holds a holder until `chunkToSave` is saved; returning tickets resurrect that holder; `IOWorker.pendingWrites` makes queued stores visible to loads | Generated host queues holder drops with a vanilla-sized 200-holder pass budget, holds pruned holders with queued generated-record saves, resurrects them before storage has the record, and reads host pending writes before adapter loads | Residency has player and generation ticket sources but still needs lighting/entity/forced tickets and real ticket levels | Tactical 60 |
| Load before generate | Disk load at `EMPTY`, then generate missing statuses | Storage lookup before terrain generation | Match in principle | Keep |
| Generated clean persistence | Dirty/save policy; not every publish writes | Published generated-clean chunks now queue lazy discardable cache writes through the shared actor; stale queued cache writes skip if a dirty save supersedes them | Still lacks a native durable backend | Tactical 134 |
| Dirty tracking | `isUnsaved` gates save | Host block mutations mark durable dirty chunks; future gameplay domains still need to join that policy | Entity/block-entity/player state could bypass dirty saving until implemented | Extend with each gameplay domain |
| Light persistence | Saved and hydrated only through `isLightOn`/light-correct trust path | Sent to clients but omitted from storage; recomputed on reload | Cannot benefit from trusted saved light yet | Keep until trusted-light hydration exists |
| Tick persistence | Proto/full tick lists preserved; unpacked into server tick lists when accessible | Block/liquid tick snapshots restored into chunks; liquid host hydrates published chunk ticks | Reasonable partial match for liquid work | Continue parity work |
| Heightmaps | Stored and primed if missing | Not stored in snapshots | Current systems recompute or avoid persisted heightmaps | Defer until needed |
| Structures | Stored starts/references | Not persisted | Structures are post-MVP | Defer |
| Entities/block entities | Persisted and loaded | Entity chunk records persist Cow, Chicken, and Item state through `WorldStore` backends that opt in; block entities are not modeled yet | Generated-original entity placement and tombstone suppression still need follow-up slices | Tactical 134 |
| Postprocessing/carving masks | Persisted for proto chunks | Not modeled in storage | Relevant to full vanilla status pipeline | Defer |
| Save on unload/close | Dirty chunks saved before unload/flush/close | Dirty block and entity chunk records save before holder unload through actor acknowledgements; mailbox flush/close exists, but app-level clean shutdown wiring is still pending | Unsafe once dedicated/native app worlds use real durable storage without close wiring | Add host close/flush lifecycle with durable backend |
| Client chunk visibility | Server filters packets per player interest | Remote service filters snapshots per session | Close enough for current multiplayer | Keep until ticket model grows |
| Progress UI | Status listener reports status changes | UI reports saved-chunk lookup, missing generation, decoration, lighting, and publish phases | Coarse lighting progress only | Keep improving with future status work |
| Test storage isolation | N/A | Worker probes can request `clearWorldStorage=1`; dev profile still persists unless requested | Manual dev refreshes can still intentionally reuse local saves | Keep explicit reset path |

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
- Generated-clean chunks may be cached lazily, but the cache must be versioned and discardable.
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

These were small enough and high enough leverage to do before deeper persistence work. D7 landed items 1, 2, 3, and 5. D8 landed item 4 for current host block mutations, generated-cache writes, dirty publication, and dirty-save-before-evict.

1. Rename loading progress stages and include real counts.

   Use stages like `Checking saved chunks`, `Generating status chunks`, `Advancing FEATURES`, `Computing light`, and `Publishing chunks`. Report storage hits, storage misses, generated count, and published count separately.

2. Add explicit browser storage hygiene.

   Add a dev/debug command or query option to clear `mclone-world-storage`, and add a Playwright fixture for worker-backed probes that deletes the IndexedDB database or uses unique save ids.

3. Resolve current light persistence ambiguity.

   Prefer omitting light from persisted storage until we implement trusted light hydration. If keeping it, add a documented light-data version and make stale light invalidation explicit.

4. Add dirty/cache terminology to host code.

   Done for the current generated-world host. `GeneratedWorldHost` now distinguishes "cache generated chunk snapshot" from "save dirty authoritative chunk". Future gameplay domains must mark dirty chunks through the same policy rather than treating every saved generated chunk as user state.

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
