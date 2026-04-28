# Structures

Durable architecture notes for Minecraft Java 1.17.1 overworld structures in `mclone`.

This document is about vanilla `StructureFeature` generation, not every worldgen feature that looks structure-like. It should be read with:

- [`worldgen-status.md`](worldgen-status.md) for current implementation state and priorities
- [`loading-persistence.md`](loading-persistence.md) for the vanilla `ChunkStatus` pipeline
- [`worldgen-deterministic-order.md`](worldgen-deterministic-order.md) for chunk scheduling, `FEATURES` finality, lighting, and publication gates
- [`authoritative-host-scheduling.md`](authoritative-host-scheduling.md) for runtime scheduler responsiveness around chunk jobs
- [`tactical/README.md`](tactical/README.md) for tactical sequencing

The target is vanilla Java `1.17.1` overworld parity. Nether and End structures are out of scope unless the target changes.

## Reference Source Map

Primary vanilla files:

| Concern | Source |
|---|---|
| Status hooks | [`ChunkStatus.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java) |
| Structure start/reference creation | [`ChunkGenerator.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java) |
| Structure feature start selection | [`StructureFeature.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java) |
| Structure lookup from references | [`StructureFeatureManager.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/StructureFeatureManager.java) |
| Start/piece placement into chunks | [`StructureStart.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/StructureStart.java), [`StructurePiece.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/StructurePiece.java) |
| Structure data stored on chunks | [`ChunkAccess.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkAccess.java), [`ProtoChunk.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java), [`LevelChunk.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunk.java) |
| Structure NBT persistence | [`ChunkSerializer.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java) |
| Biome decoration placement loop | [`Biome.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java) |
| Registered structure types | [`StructureFeature.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`StructureFeatures.java`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/StructureFeatures.java) |
| Jigsaw structures | [`JigsawFeature.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/JigsawFeature.java), [`JigsawPlacement.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/structures/JigsawPlacement.java), [`PoolElementStructurePiece.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/PoolElementStructurePiece.java) |
| Template-backed structures | [`TemplateStructurePiece.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/TemplateStructurePiece.java), [`StructureTemplate.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/templatesystem/StructureTemplate.java), [`StructurePlaceSettings.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/templatesystem/StructurePlaceSettings.java) |
| Noise-affecting terrain blend | [`Beardifier.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/Beardifier.java), [`NoiseBasedChunkGenerator.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java) |

Read these files before porting a structure family. Do not infer structure behavior from carvers or ordinary features.

## Vanilla Pipeline

Structures are metadata-first and placement-later.

### 1. `STRUCTURE_STARTS`

`ChunkStatus.STRUCTURE_STARTS` runs before biomes/noise and calls `ChunkGenerator.createStructures(...)` when feature generation is enabled ([`ChunkStatus.java:42`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)). `ChunkGenerator.createStructures(...)` checks strongholds first, then the primary biome's configured structure list ([`ChunkGenerator.java:241`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java)).

For each configured structure, `ChunkGenerator.createStructure(...)` asks the feature to generate a `StructureStart`, then stores that start on the chunk ([`ChunkGenerator.java:250`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java)). `StructureFeature.generate(...)` chooses the potential feature chunk, checks whether the current chunk is that candidate and whether the biome/config permits the start, creates the start, and calls `generatePieces(...)` to build piece metadata ([`StructureFeature.java:274`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java)).

At this point vanilla has not necessarily placed the final blocks. It has recorded a `StructureStart` with pieces, bounding boxes, generation depth, references, and structure-specific state.

### 2. `STRUCTURE_REFERENCES`

`ChunkStatus.STRUCTURE_REFERENCES` has dependency range `8` and runs before `BIOMES` ([`ChunkStatus.java:57`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)). For a target chunk, `ChunkGenerator.createReferences(...)` scans start chunks in a fixed `[-8,+8]` square. When a valid start's bounding box intersects the target chunk's 16x16 column, the target chunk records a reference to that start chunk ([`ChunkGenerator.java:264`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java)).

Those references are stored separately from starts on the chunk (`getReferencesForFeature(...)`, `addReferenceForFeature(...)`) ([`ProtoChunk.java:321`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java), [`ProtoChunk.java:344`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java)). `StructureFeatureManager.startsForFeature(...)` later reads the current chunk's references, loads each referenced start chunk at `STRUCTURE_STARTS`, and returns valid starts for that feature ([`StructureFeatureManager.java:32`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/StructureFeatureManager.java)).

The important model is:

```text
start chunk owns structure metadata
touched chunks store references to that start
each touched chunk later places its own clipped slice
```

### 3. Noise-Affecting Structures

Some structures influence terrain shape before blocks are placed. Vanilla lists `PILLAGER_OUTPOST`, `VILLAGE`, `NETHER_FOSSIL`, and `STRONGHOLD` in `StructureFeature.NOISE_AFFECTING_FEATURES` ([`StructureFeature.java:110`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java)). For the overworld target, the relevant members are pillager outposts, villages, and strongholds.

`NoiseBasedChunkGenerator.doFill(...)` constructs a `Beardifier` from the chunk's `StructureFeatureManager` and chunk ([`NoiseBasedChunkGenerator.java:351`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java)). `Beardifier` gathers close structure pieces and jigsaw junctions from referenced noise-affecting starts ([`Beardifier.java:38`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/Beardifier.java)). The density path then adds `beardifyOrBury(...)` to noise samples ([`NoiseBasedChunkGenerator.java:245`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java)).

Do not enable faithful villages/outposts/strongholds without this terrain-blending path, or the structure blocks may appear but the surrounding terrain will be wrong.

### 4. Placement During `FEATURES`

Actual structure block placement is part of biome decoration. `ChunkStatus.FEATURES` creates a `WorldGenRegion` and calls `ChunkGenerator.applyBiomeDecoration(...)` ([`ChunkStatus.java:111`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java)). That calls `Biome.generate(...)` for the center chunk ([`ChunkGenerator.java:198`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java)).

Inside `Biome.generate(...)`, for each `GenerationStep.Decoration`, vanilla places structures for that step before ordinary configured features. It calls `startsForFeature(...)`, then `StructureStart.placeInChunk(...)` with a bounding box clipped to the current chunk's 16x16 column ([`Biome.java:220`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java)). `StructureStart.placeInChunk(...)` iterates its pieces and calls `StructurePiece.postProcess(...)` only when the piece bounding box intersects that target bounding box ([`StructureStart.java:81`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/StructureStart.java)).

This means structures are not arbitrary late writes from faraway chunks. A start may be far enough away to need a reference, but each chunk applies the intersecting structure pieces during its own `FEATURES` pass. Ordinary configured features in the same generation step then run after those structure pieces and can observe the structure blocks.

## Overworld Structure Set

Vanilla 1.17.1 registers more structure types than the overworld target needs. The overworld-relevant `StructureFeature` families are:

| Family | Notes | Sources |
|---|---|---|
| Mineshaft | normal + mesa configurations; `UNDERGROUND_STRUCTURES` | [`StructureFeature.java:59`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`StructureFeatures.java:25`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/StructureFeatures.java) |
| Stronghold | special global/ring placement; noise-affecting | [`StructureFeature.java:83`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`ChunkGenerator.java:80`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java) |
| Village | jigsaw; plains/desert/savanna/snowy/taiga pools; noise-affecting | [`StructureFeature.java:101`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`StructureFeatures.java:80`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/StructureFeatures.java), [`VillageFeature.java:6`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/VillageFeature.java) |
| Pillager outpost | jigsaw; avoids nearby village candidates; noise-affecting | [`StructureFeature.java:56`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`PillagerOutpostFeature.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/PillagerOutpostFeature.java) |
| Desert pyramid | single custom piece; good early target | [`StructureFeature.java:68`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`DesertPyramidFeature.java:29`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/DesertPyramidFeature.java) |
| Jungle temple | custom piece | [`StructureFeature.java:65`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`JunglePyramidFeature.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/JunglePyramidFeature.java) |
| Swamp hut | custom piece plus spawning implications | [`StructureFeature.java:80`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`SwamplandHutFeature.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/SwamplandHutFeature.java) |
| Igloo | template-backed | [`StructureFeature.java:71`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`IglooFeature.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/IglooFeature.java) |
| Woodland mansion | large template/piece system | [`StructureFeature.java:62`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`WoodlandMansionFeature.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/WoodlandMansionFeature.java) |
| Ocean monument | large custom ocean structure | [`StructureFeature.java:86`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`OceanMonumentFeature.java`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/OceanMonumentFeature.java) |
| Ocean ruin | warm/cold template-backed variants | [`StructureFeature.java:89`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`StructureFeatures.java:58`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/StructureFeatures.java) |
| Shipwreck | normal + beached template-backed variants | [`StructureFeature.java:77`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`StructureFeatures.java:43`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/StructureFeatures.java) |
| Buried treasure | small underground structure with chest/loot semantics | [`StructureFeature.java:98`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`StructureFeatures.java:74`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/StructureFeatures.java) |
| Ruined portal | overworld standard/desert/jungle/swamp/mountain/ocean variants; exclude nether variant | [`StructureFeature.java:74`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/StructureFeature.java), [`StructureFeatures.java:95`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/StructureFeatures.java) |

Out of scope for the overworld target:

- `NETHER_BRIDGE`
- `NETHER_FOSSIL`
- `BASTION_REMNANT`
- `END_CITY`
- `RUINED_PORTAL_NETHER`

## Structure-Like Features That Are Not `StructureFeature`s

Several vanilla worldgen features look like structures but do not use the start/reference pipeline:

- Monster rooms / dungeons are ordinary configured features in `UNDERGROUND_STRUCTURES` ([`Features.java:387`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/Features.java), [`BiomeDefaultFeatures.java:47`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java)).
- Desert wells are ordinary configured features in `SURFACE_STRUCTURES` ([`Features.java:390`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/Features.java), [`BiomeDefaultFeatures.java:315`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java)).
- Overworld fossils are ordinary configured `Feature.FOSSIL`, distinct from `StructureFeature.NETHER_FOSSIL` ([`Features.java:413`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/Features.java), [`BiomeDefaultFeatures.java:319`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java)).

Implement these through the existing configured-feature/decorator path, not through `STRUCTURE_STARTS` / `STRUCTURE_REFERENCES`.

For sequencing: that ordinary-feature lane is now complete. Desert wells, monster rooms, and overworld fossils are already covered through the configured-feature/decorator path and should stay outside true `StructureFeature` work.

## Scheduling And Finality Implications

Structures explain why the status pipeline cannot be collapsed to "terrain then decoration":

- `STRUCTURE_STARTS` must exist before `STRUCTURE_REFERENCES`.
- `STRUCTURE_REFERENCES` must exist before `BIOMES` / `NOISE`, because noise-affecting structures can alter density through `Beardifier`.
- Structure block placement happens during `FEATURES`, before ordinary configured features in the same decoration step.
- A chunk's structure slice is placed by that chunk's own `FEATURES` task using references; a far start chunk does not later mutate the target through its own `FEATURES` task.
- The broader `FEATURES` stability rule from [`worldgen-deterministic-order.md`](worldgen-deterministic-order.md) still applies: initial lighting for a target chunk waits for the target plus its 8 neighbors to complete `FEATURES`, and client publication waits for the 3x3 `FULL` gate.

For a `vanilla17` profile, structure work must be status-aware even if the runtime scheduler uses engine-native workers and futures.

## Recommended Implementation Order

Do not start with villages. They combine jigsaw pools, templates, processors, terrain blending, and settlement-specific interactions.

1. **Keep the ordinary configured-feature oddities out of structure work**
   - Desert wells, monster rooms, and overworld fossils are already landed through the normal feature/decorator path.
   - Do not reimplement them as `STRUCTURE_STARTS` / `STRUCTURE_REFERENCES` work just because they look structure-like.
   - The next structure slice should be a real `StructureFeature` foundation, not more configured-feature backfill.

2. **Status and metadata foundation**
   - Add `STRUCTURE_STARTS` and `STRUCTURE_REFERENCES` concepts to the local generation model.
   - Represent `StructureStart`, `StructurePiece`, `BoundingBox`, starts-by-feature, and references-by-feature.
   - Persist or at least keep the data shape compatible with vanilla chunk status/resume semantics.
   - Add oracle fixtures that can inspect structure starts/references separately from block placement.

3. **Per-chunk clipped placement skeleton**
   - Port enough `StructureStart.placeInChunk(...)` / `StructurePiece.postProcess(...)` flow to place only the intersecting slice of a structure into the current chunk.
   - Prove the pipeline with one small custom structure before adding templates or jigsaw.

4. **Simple custom overworld structures**
   - Start with desert pyramid, then jungle temple, swamp hut, and buried treasure.
   - These validate start selection, references, per-chunk clipping, chest/block-entity data, and post-processing without requiring the full template or jigsaw stacks.

5. **Mineshafts**
   - Mineshafts are structure starts, not carvers. They randomly decide starts, create an initial room, recursively add corridor/crossing/stair pieces, then place those pieces in `UNDERGROUND_STRUCTURES` ([`MineshaftFeature.java:29`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/MineshaftFeature.java), [`MineshaftFeature.java:55`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/MineshaftFeature.java), [`MineShaftPieces.java:70`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/structure/MineShaftPieces.java)).
   - They are a good middle step because they are multi-piece and cross-chunk, but do not require the jigsaw pool system.

6. **Template-backed medium structures**
   - Port template loading from extracted structure NBTs, placement settings, rotation/mirror, processors, and block entities.
   - Then add shipwrecks, igloos, ocean ruins, and overworld ruined portals.

7. **Strongholds and terrain-blending foundation**
   - Strongholds have special global placement via `ChunkGenerator.generateStrongholds(...)` and are noise-affecting ([`ChunkGenerator.java:80`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java)).
   - Port the metadata and placement path, but treat exact terrain blending as blocked on `Beardifier`.

8. **Large custom/template structures**
   - Ocean monuments and woodland mansions have large custom/template piece systems and many block/entity consequences. Do these after the smaller placement/template paths are stable.

9. **Beardifier, jigsaw, villages, and pillager outposts**
   - Port `Beardifier` and jigsaw pools before claiming village/outpost parity.
   - Villages use `JigsawFeature`, configured village pools, `JigsawPlacement`, `PoolElementStructurePiece`, terrain matching, and jigsaw junctions ([`VillageFeature.java:6`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/VillageFeature.java), [`JigsawFeature.java:42`](../reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/feature/JigsawFeature.java), [`StructureFeatures.java:80`](../reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/StructureFeatures.java)).
   - Pillager outposts share the jigsaw shape and also avoid nearby village candidates.

## Validation Strategy

Each structure slice should prove the smallest stable layer before broadening:

- start/reference fixtures: structure type, start chunk, bounding boxes, piece count, references stored on touched chunks
- placement fixtures: exact block diff for a bounded chunk set around a known structure
- persistence fixtures: chunk status, structure starts, references, block entities, and loot/container state where relevant
- rendered probes: one visual structure frame only after server/oracle or unit evidence says the chunk data is correct

Do not use renderer appearance as the primary oracle for structure placement. Structures cross chunks and often depend on metadata that is invisible until another chunk or gameplay system queries it.
