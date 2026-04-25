# Worldgen Deterministic Order

Canonical ordering contract for vanilla Java 1.17.1 world generation, initial lighting, chunk publication, and client render readiness in `mclone`.

This document owns the parity-critical order model. Other docs may summarize it, but should link here instead of restating the algorithm.

Related docs:

- [`worldgen-status.md`](worldgen-status.md): what is implemented and what is missing
- [`loading-persistence.md`](loading-persistence.md): status-aware loading, persistence, dirty/save policy
- [`authoritative-host-scheduling.md`](authoritative-host-scheduling.md): runtime scheduler and responsiveness rules
- [`worker-ownership.md`](worker-ownership.md): thread/worker ownership of mutable state
- [`structures.md`](structures.md): structure-specific starts, references, pieces, templates, and implementation order
- [`lighting.md`](lighting.md): light engine data structures and propagation algorithm
- [`tactical/47-generated-chunk-status-orchestration.md`](tactical/47-generated-chunk-status-orchestration.md): implementation slice that made this order model explicit in the generated runtime

## Scope

The target is vanilla Java `1.17.1` overworld behavior. Runtime machinery may use browser/Node workers instead of JVM executors, but a `vanilla17` profile must preserve the observable order of chunk-stage side effects.

This is not a status dashboard and not a worker topology doc. It answers:

- which chunk statuses depend on which neighboring statuses
- when generation is allowed to mutate a chunk
- when a chunk is stable enough for initial light
- when a chunk is publishable to a client
- how structures and carvers differ from ordinary decoration
- what minimum ordering model `mclone` should implement

## Primary Source Map

| Concern | Source |
|---|---|
| Status chain and dependency ranges | [`ChunkStatus.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java) |
| Status scheduling and range futures | [`ChunkMap.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkTaskPriorityQueueSorter.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java), [`ChunkTaskPriorityQueue.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueue.java), [`ProcessorMailbox.java`](../reference/minecraft-1.17.1/src/net/minecraft/util/thread/ProcessorMailbox.java) |
| Status futures and chunk data shape | [`ChunkHolder.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java), [`ChunkAccess.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkAccess.java), [`ProtoChunk.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java), [`LevelChunk.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunk.java), [`ImposterProtoChunk.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ImposterProtoChunk.java) |
| Player tickets and view-distance tracking | [`DistanceManager.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java), [`TicketType.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/TicketType.java), [`ChunkMap.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) |
| Generation tasks | [`ChunkGenerator.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java) |
| Worldgen write cutoff | [`WorldGenRegion.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java) |
| Structure placement during decoration | [`Biome.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java), [`StructureStart.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/StructureStart.java) |
| Startup spawn preparation | [`MinecraftServer.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java) |
| Client chunk cache | [`ClientChunkCache.java`](../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java) |
| Client render chunk grid and rebuilds | [`ViewArea.java`](../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java), [`LevelRenderer.java`](../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java), [`ChunkRenderDispatcher.java`](../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java) |

## Vanilla Status Chain

Vanilla chunks advance through this ordered `ChunkStatus` chain:

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

The chain and generation tasks are registered in `ChunkStatus.java` ([`ChunkStatus.java:39`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:42`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:57`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:71`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:95`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:111`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:135`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:155`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)).

Each status has:

- a parent status
- a dependency range
- a generation task
- a loading task
- a chunk type
- a set of heightmaps available after that status

`ChunkStatus.getRange()` exposes the declared dependency range ([`ChunkStatus.java:318`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)). `ChunkMap.scheduleChunkGeneration(...)` requests that dependency square before running the target status task ([`ChunkMap.java:512`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)).

## Dependency Ranges Are Status Inputs

Vanilla dependency ranges are Chebyshev squares. `ChunkMap.getChunkRangeFuture(...)` loops `-range..+range` in both X and Z, computes `max(abs(dx), abs(dz))`, and requests the status returned by the dependency function for that radius ([`ChunkMap.java:250`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:252`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:265`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)).

For generation, radius `0` maps to the requested status's parent. Neighbor radii map through `ChunkStatus.getStatusAroundFullChunk(...)` based on `ChunkStatus.getDistance(requestedStatus) + radius` ([`ChunkMap.java:555`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:557`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:560`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)). The range-to-status table starts:

```text
0 -> FULL
1 -> FEATURES
2 -> LIQUID_CARVERS
3+ -> STRUCTURE_STARTS
```

That mapping is encoded in `STATUS_BY_RANGE` ([`ChunkStatus.java:164`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)).

Important consequence: formulas such as "visible R, decorate R+1, terrain R+9" are at best shorthand. The faithful model is not a single flattened radius; it is recursive status scheduling through `getChunkRangeFuture(...)`, `getDependencyStatus(...)`, each status's parent, and `STATUS_BY_RANGE`.

## Status Futures And Partial Chunks

Vanilla does not compute a second "authority terrain radius" and eagerly materialize every chunk inside it. A `ChunkHolder` owns one future slot per `ChunkStatus`; `getOrScheduleFuture(status, chunkMap)` either returns an existing future or asks `ChunkMap.schedule(...)` to load/generate exactly that requested status if the current ticket level allows it ([`ChunkHolder.java`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java)). `ChunkMap.schedule(...)` loads `EMPTY`, recursively asks for the requested status's parent, and then either runs the requested status's loading task when the loaded chunk already satisfies it or calls `scheduleChunkGeneration(...)` for the missing status ([`ChunkMap.java:450`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:460`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:467`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)).

`scheduleChunkGeneration(...)` resolves the target status's dependency square as a list of `ChunkAccess` objects whose statuses may differ by offset. The dependency function is the `getDependencyStatus(...)` rule above, not "all chunks to the same terrain status" ([`ChunkMap.java:512`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:555`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)). The status task receives that mixed-status list and creates a `WorldGenRegion` over it when needed ([`ChunkStatus.java:57`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:127`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`WorldGenRegion.java:69`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java)).

The chunk object itself is status-shaped. `ProtoChunk` starts at `EMPTY` with an empty section array, but it can already carry chunk metadata: biome container, heightmaps, structure starts, structure references, carving masks, post-processing lists, proto ticks, entities, block entities, and generated light positions ([`ProtoChunk.java:42`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java), [`ProtoChunk.java:87`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java), [`ProtoChunk.java:321`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java), [`ProtoChunk.java:455`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java)). A `LevelChunk` is the `FULL` representation and copies the proto chunk's sections plus metadata at conversion time ([`LevelChunk.java:154`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunk.java), [`LevelChunk.java:797`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunk.java)). `ImposterProtoChunk` wraps an already-full chunk when the status pipeline needs a `ProtoChunk`-shaped view without mutating that full chunk ([`ImposterProtoChunk.java:24`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ImposterProtoChunk.java)).

Concrete examples:

- `STRUCTURE_REFERENCES` has range `8`, but the center needs only `STRUCTURE_STARTS` through the parent edge and the full `[-8,+8]` input square supplies structure-start metadata, not terrain blocks.
- `NOISE` has range `8`, but only the center advances from `BIOMES` to `NOISE`; neighbors in the range are there for already-recorded structure metadata that can affect density through `Beardifier`.
- `FEATURES` has range `8`, but `getDependencyStatus(FEATURES, r)` means the center and Chebyshev radius `1` are `LIQUID_CARVERS`, while radii `2..8` are only `STRUCTURE_STARTS`. Those outer chunks are metadata inputs, not carved terrain inputs.

For `mclone`, this is a parity requirement. Do not replace vanilla's mixed-status dependency futures with a flattened "hidden authority terrain window." The runtime needs host-owned chunk records that can represent partial `ProtoChunk`-like states and metadata-only statuses separately from materialized block sections. Browser or Node scheduling can differ in mechanics, but status requests, dependency statuses, and the data each status is allowed to require must follow the vanilla mechanism.

## Carvers

Classic carvers are target-local status tasks. `CARVERS` and `LIQUID_CARVERS` have dependency range `0`, so the status scheduler does not wait for a neighbor status ring before running them ([`ChunkStatus.java:95`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:103`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)).

The wide cave/ravine footprint is internal to the target chunk's own carver pass. `ChunkGenerator.applyCarvers(...)` receives the target `ChunkAccess`, creates the target chunk's carving mask, scans possible carver start chunks in a fixed `[-8,+8]` square, walks the biome carver list in order, seeds each candidate, and only then carves into the target chunk ([`ChunkGenerator.java:135`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [`ChunkGenerator.java:142`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [`ChunkGenerator.java:144`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [`ChunkGenerator.java:153`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [`ChunkGenerator.java:156`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java)).

Carver overlap is mostly subtractive air-over-air, but not formally commutative. The target `ProtoChunk` stores a carving mask for the step, and `WorldCarver` uses that mask as first-claim state before `carveBlock(...)` runs ([`ProtoChunk.java:455`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java), [`WorldCarver.java:164`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/carver/WorldCarver.java)). Preserve vanilla start scan order, list order, seed derivation, and mask behavior for parity.

## Structures

Structures are metadata-first and placement-later.

`STRUCTURE_STARTS` records starts on their start chunks before biomes/noise ([`ChunkStatus.java:42`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkGenerator.java:241`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [`ChunkGenerator.java:250`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java)).

`STRUCTURE_REFERENCES` has dependency range `8` and scans the `[-8,+8]` start-chunk square. If a valid start's bounding box intersects the target chunk, the target records a reference to the start chunk ([`ChunkStatus.java:57`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkGenerator.java:264`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [`ChunkGenerator.java:273`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [`ChunkGenerator.java:279`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java)).

Some structures affect terrain density before block placement. `NoiseBasedChunkGenerator` constructs a `Beardifier` from the chunk's structure references during noise fill, and the density path adds `beardifyOrBury(...)` ([`NoiseBasedChunkGenerator.java:351`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java), [`NoiseBasedChunkGenerator.java:245`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java), [`Beardifier.java:38`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/Beardifier.java)).

Actual structure block placement happens during `FEATURES`. For each decoration step, `Biome.generate(...)` places referenced structures for that step before ordinary configured features, clipped to the current chunk's 16x16 column ([`Biome.java:220`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java), [`Biome.java:223`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java), [`Biome.java:235`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java), [`Biome.java:237`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java), [`Biome.java:251`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java)).

Detailed structure architecture and implementation order live in [`structures.md`](structures.md). Do not implement large structures as late far-chunk writes or as ordinary decoration shortcuts.

## Ordinary Decoration And Finality

`FEATURES` has parent `LIQUID_CARVERS` and dependency range `8` ([`ChunkStatus.java:111`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:113`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:114`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)). The task creates a `WorldGenRegion` with `writeRadiusCutoff = 1`, then calls `applyBiomeDecoration(...)` ([`ChunkStatus.java:127`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:128`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)).

The range `8` is not a terrain-read guarantee. As described above, the `FEATURES` input list contains mixed statuses: center plus radius `1` at `LIQUID_CARVERS`, outer radii `2..8` at `STRUCTURE_STARTS`. `WorldGenRegion.getChunk(x, z, requestedStatus, required)` enforces the requested status if callers ask for one, and ordinary `getBlockState(...)` reads through the chunk object actually present in the cache ([`WorldGenRegion.java:105`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java), [`WorldGenRegion.java:114`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java), [`WorldGenRegion.java:144`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java)).

`WorldGenRegion.ensureCanWrite(...)` allows writes only when both the X and Z section distances from the region center are `<= writeRadiusCutoff` ([`WorldGenRegion.java:238`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java), [`WorldGenRegion.java:241`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java), [`WorldGenRegion.java:243`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java)). For `FEATURES`, that means the center chunk plus its immediate 8 neighbors.

A target chunk C can therefore receive ordinary decoration writes from the nine `FEATURES` tasks centered on C and its 8 neighbors. C is stable against ordinary decoration spillover only after that 3x3 `FEATURES` neighborhood is complete.

Decoration is deterministic because the chunk scheduler gives `FEATURES` tasks a single queue order, not because overlapping feature writes are order-independent. `ChunkMap.scheduleChunkGeneration(...)` submits the status task through `worldgenMailbox` and `ChunkTaskPriorityQueueSorter` after dependencies complete ([`ChunkMap.java:516`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:517`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)). `ChunkTaskPriorityQueue` pops the first queued chunk at the first non-empty priority level ([`ChunkTaskPriorityQueue.java:49`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueue.java), [`ChunkTaskPriorityQueue.java:82`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueue.java), [`ChunkTaskPriorityQueue.java:88`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueue.java), [`ChunkTaskPriorityQueue.java:90`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueue.java)). The sorter then asks the target mailbox to run that task and polls the next task after the returned futures complete ([`ChunkTaskPriorityQueueSorter.java:116`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java), [`ChunkTaskPriorityQueueSorter.java:122`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java), [`ChunkTaskPriorityQueueSorter.java:125`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java)). `ProcessorMailbox.tell(...)` queues tasks for execution by the mailbox dispatcher ([`ProcessorMailbox.java:107`](../reference/minecraft-1.17.1/src/net/minecraft/util/thread/ProcessorMailbox.java)).

This is not just an `mclone` porting concern. Vanilla Minecraft itself is order-dependent on the same seed: two worlds generated with the same seed and same version can differ slightly depending on the player's travel route, because chunk generation order determines which of two conflicting feature writes wins. This is publicly documented on the [Minecraft Wiki "World generation" page](https://minecraft.wiki/w/World_generation), tracked on the Mojang bug tracker as [MC-55596](https://bugs.mojang.com/browse/MC-55596), and catalogued for specific anomalous seeds on the [Minecraft Wiki "Anomalous world seeds" page](https://minecraft.wiki/w/Anomalous_world_seeds). The parity requirement for `mclone` is therefore not "match the seed" but "match the order in which vanilla schedules and commits cross-chunk `FEATURES` writes."

For `mclone`, adjacent `FEATURES` tasks must not commit directly into shared chunks in arbitrary parallel order. If generation workers are used, they should return write plans or isolated results; the authoritative host must apply side effects in one deterministic vanilla-shaped order.

Trace coverage note: the rule above is source-backed, and tactical [`48`](tactical/48-vanilla-scheduler-trace-oracle.md) now provides the first executable vanilla scheduler trace for a bounded spawn-bootstrap scenario. Tactical [`46`](tactical/46-full-decorated-spawn-chunk-parity.md) consumed that trace before claiming exact decorated block parity for the spawn fixture.

Tactical [`48`](tactical/48-vanilla-scheduler-trace-oracle.md) now includes the first bounded spawn-bootstrap trace fixture:
[`vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json`](../test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json).
For seed `12345`, chunk `(0,0)`, the committed trace observes this 3x3 `FEATURES` completion order:

```text
(-1,-1) -> (0,-1) -> (1,-1)
(-1, 0) -> (0, 0) -> (1, 0)
(-1, 1) -> (0, 1) -> (1, 1)
```

That order matches the current generated-host `sortChunkCoordinates(...)` order for the target 3x3. The oracle pins the child JVM's worker count and identity-hash mode to make the vanilla scheduler's otherwise JVM-sensitive collection iteration reproducible. Treat this as the initial measured spawn-bootstrap fixture, not a replacement for future player-ticket or same-run full decorated diagnostics. Vanilla does not source-sort decoration jobs by coordinate; the coordinate order above is measured scheduler behavior for this fixture and run shape.

## Lighting Gate

`LIGHT` has parent `FEATURES` and dependency range `1` ([`ChunkStatus.java:135`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:137`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:138`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)). Through `getDependencyStatus(...)` and `STATUS_BY_RANGE[1]`, lighting a target chunk waits for the target chunk plus its 8 neighbors to have completed `FEATURES` ([`ChunkMap.java:555`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:557`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkStatus.java:164`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)).

This matches the ordinary decoration stability rule. Initial light must not be treated as final if it was computed from only terrain, only local decoration, or a stale neighbor snapshot.

Vanilla also schedules `LIGHT` through the same chunk-holder future machinery as the other statuses. When `ChunkMap.schedule(...)` sees `ChunkStatus.LIGHT`, it adds a `TicketType.LIGHT` ticket for the target chunk before requesting the status future, and the threaded light engine releases that ticket only after the post-update completion path marks the chunk light-correct ([`ChunkMap.java:455`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:456`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ThreadedLevelLightEngine.java:156`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java), [`ThreadedLevelLightEngine.java:159`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java)). `ThreadedLevelLightEngine` queues chunk-priority `PRE_UPDATE` tasks, runs propagation, then runs `POST_UPDATE` tasks; browser/Node workers can budget this differently, but should preserve the order ([`ThreadedLevelLightEngine.java:173`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java), [`ThreadedLevelLightEngine.java:180`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java), [`ThreadedLevelLightEngine.java:186`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java), [`ThreadedLevelLightEngine.java:190`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java)).

For `mclone`, this means `LIGHT` should become status-scheduled work that starts as soon as the target's `3x3 FEATURES` dependency is ready. A whole-view "decorate everything, then compute light for everything" phase is a runtime shortcut, not the vanilla scheduling model. The light solver's physical propagation footprint and worker design are owned by [`lighting.md`](lighting.md) and [`lighting-worker-architecture.md`](lighting-worker-architecture.md).

## Full / Send Gate

`FULL` converts the proto path to a `LevelChunk` after `HEIGHTMAPS`, whose parent chain includes `LIGHT` ([`ChunkStatus.java:144`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:152`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:155`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)).

Before publishing a ticking chunk to players, `ChunkMap.prepareTickingChunk(...)` waits for a range-1 future where all chunks in the 3x3 are `FULL`, then calls `playerLoadedChunk(...)` ([`ChunkMap.java:599`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:601`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:607`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:610`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)).

`playerLoadedChunk(...)` sends chunk data and a light update packet to the player ([`ChunkMap.java:1018`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:1020`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:1021`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:1024`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)).

Faithful publication rule:

```text
initial LIGHT(C) waits for 3x3 FEATURES around C
client publication/ticking for C waits for 3x3 FULL around C
```

## Player View Distance And Tickets

Vanilla player-driven loading is ticket-driven. `TicketType.PLAYER` is a distinct ticket type ([`TicketType.java:15`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/TicketType.java)). `DistanceManager.addPlayer(...)` updates player and natural-spawn distance trackers for the player's chunk, while `removePlayer(...)` removes those updates when the player leaves ([`DistanceManager.java:182`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java), [`DistanceManager.java:185`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java), [`DistanceManager.java:189`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java)).

`ChunkMap.setViewDistance(...)` clamps the requested value plus one and updates player tickets ([`ChunkMap.java:682`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:683`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:687`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)). Player join/status and movement update chunk tracking over the current internal view-distance square, with a checkerboard/Chebyshev distance test ([`ChunkMap.java:823`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:826`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:838`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:893`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:923`](../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)).

Do not implement player movement as "generate the whole view synchronously, then accept movement." Movement updates interest and tracking; chunk futures complete later.

For an engine-visible radius `V`, every chunk intended for normal client publication must reach the `3x3 FULL` send gate above. Additional generated chunks outside the sent set are consequences of the status dependency graph, tickets, light tickets, spawn tickets, forced tickets, and storage/resume needs. Model those as status requests rather than a second ad-hoc visible/decorate/terrain radius formula.

## Spawn Bootstrap

Initial world startup is not the same as ongoing player movement. `MinecraftServer.prepareLevels(...)` adds a `START` region ticket of radius `11` around the shared spawn and waits until `getTickingGenerated()` reaches `441` chunks, i.e. a 21x21 ticking start region ([`MinecraftServer.java:176`](../reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java), [`MinecraftServer.java:177`](../reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java), [`MinecraftServer.java:499`](../reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java), [`MinecraftServer.java:507`](../reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java), [`MinecraftServer.java:509`](../reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java)).

That bootstrap forces an initial prepared area even before ordinary player-driven chunk streaming. `mclone` can expose a different UX for startup, but the distinction must stay explicit: spawn bootstrap is a start-region ticket/wait policy; ongoing movement is player-ticket-driven.

## Client Cache And Rendering

The client has no separate "decoration render" stage. It stores received `LevelChunk` packet data in `ClientChunkCache`, enables light sources, updates section light status, and notifies the level that the chunk loaded ([`ClientChunkCache.java:84`](../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java), [`ClientChunkCache.java:93`](../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java), [`ClientChunkCache.java:102`](../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java), [`ClientChunkCache.java:107`](../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java), [`ClientChunkCache.java:110`](../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java)).

Client chunk storage radius is not the same number as the renderer's chunk grid. `ClientChunkCache.calculateStorageRange(...)` uses `max(2, viewRadius) + 3` ([`ClientChunkCache.java:124`](../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java), [`ClientChunkCache.java:146`](../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java)). `ViewArea.setViewDistance(...)` sizes the render chunk grid as `viewDistance * 2 + 1` in X/Z ([`ViewArea.java:49`](../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java), [`ViewArea.java:50`](../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java)).

Rendering is mesh rebuild over the received client cache. Light updates mark sections dirty through `ClientChunkCache.onLightUpdate(...)`, and `ViewArea.setDirty(...)` marks the corresponding render chunk dirty ([`ClientChunkCache.java:161`](../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java), [`ViewArea.java:79`](../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java)). `LevelRenderer` only traverses render chunks with required neighbors available (`hasAllNeighbors()`) during culling/traversal ([`LevelRenderer.java:938`](../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java)).

For `mclone`, decorated blocks are just authoritative chunk data. The renderer should consume snapshots and rebuild meshes; it should not run decoration or define a separate decoration-finality concept.

## Minimum `mclone` Contract

For the `vanilla17` profile, implement chunk orchestration around these states and gates:

```text
status state per chunk:
  EMPTY
  STRUCTURE_STARTS
  STRUCTURE_REFERENCES
  BIOMES
  NOISE
  SURFACE
  CARVERS
  LIQUID_CARVERS
  FEATURES
  LIGHT
  SPAWN
  HEIGHTMAPS
  FULL

data state per chunk:
  ProtoChunk-like partial record with status, metadata, optional sections, masks, ticks, and light positions
  LevelChunk/full snapshot only after the FULL conversion path

status input examples:
  FEATURES(C): C and radius 1 at LIQUID_CARVERS; radii 2..8 at STRUCTURE_STARTS
  LIGHT(C): C and radius 1 at FEATURES

derived readiness:
  decoration_stable(C) = FEATURES complete for C and its 8 neighbors
  initial_light_ready(C) = LIGHT complete from decoration_stable input
  publishable(C) = FULL complete for C and its 8 neighbors
```

Implementation rules:

1. Schedule statuses through parent/range dependencies, not through a flattened radius shortcut.
2. Keep partial `ProtoChunk`-like state first-class; metadata-only chunks must not be forced to `LIQUID_CARVERS` or `FULL` just because they are in a dependency square.
3. Build `WorldGenRegion`-style inputs from mixed-status chunks exactly as `getDependencyStatus(...)` selects them.
4. Treat `FEATURES` side effects as ordered, non-commutative writes.
5. Keep ordinary `FEATURES` writes within the center 3x3 region.
6. Run carvers as target-local deterministic passes with the vanilla `[-8,+8]` start scan.
7. Represent structures as starts, references, optional noise influence, and clipped per-chunk placement during `FEATURES`.
8. Compute final initial light only after the 3x3 `FEATURES` gate.
9. Publish normal chunk snapshots only after the 3x3 `FULL` gate.
10. Let host scheduling and worker topology diverge from vanilla only when these ordering facts remain observable.
