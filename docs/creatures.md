# Creatures

Research and implementation notes for Minecraft Java 1.17.1-style overworld creature spawning in `mclone`.

This document is a reference for future creature work. It is not a tactical slice by itself. The parity-critical parts should be direct TypeScript ports of the 1.17.1 spawn tables, spawn placement checks, mob caps, despawn rules, and entity tick semantics. Runtime ownership, worker boundaries, transport, and persistence adapters should fit the existing authoritative host architecture.

## Goals

- Match vanilla 1.17.1 basics for overworld-adjacent creatures: cows, sheep, pigs, chickens, horses, donkeys, wolves, foxes, rabbits, goats, llamas, common water creatures, ambient bats, and common hostile overworld mobs such as spiders, zombies, skeletons, creepers, slimes, endermen, and witches.
- Keep entity spawning and ticking authoritative-host owned, not renderer owned.
- Preserve the distinction between seed-time chunk-generation creatures and live natural spawns.
- Keep browser main-thread work limited to presentation, interpolation, and GPU upload.
- Preserve one logical entity lifecycle for browser singleplayer, remote clients, Node hosts, storage, and oracle tests.
- Keep the first slice smaller than full Minecraft entity behavior: no dungeon spawners, monster rooms, raids, patrols, phantoms, villages, villagers, cats, wandering traders, structure-specific mobs, breeding, taming, combat AI, loot, equipment, or full pathfinding unless explicitly pulled into scope.

## Current Status

`Creatures0` landed the official-server generated-entity fixture path and the committed passive sheep fixture for seed `12345`, chunk `(-7, -15)`.

`Entities0` landed the host-owned entity manager foundation that generated entities can enter through `EntityRuntime.addWorldGenChunkEntities(...)`.

`Creatures1` now ports the generation-time passive path needed by that fixture:

- `MobCategory`, minimal passive `EntityType` metadata, and sheep color data
- biome `MobSpawnSettings` / `SpawnerData` for the current overworld passive tables
- `SpawnPlacements` entries and passive animal spawn predicates
- `NaturalSpawner.spawnMobsForChunkGeneration(...)`
- `NoiseBasedChunkGenerator.spawnOriginalMobs(...)`
- fixture-backed comparison that inserts generated mobs through the host-owned entity runtime

`Creatures2` wires that generation path into `GeneratedWorldHost`: generated original mobs enter the host-owned `EntityRuntime`, publish `entity_snapshot` protocol records, flow through local and remote clients, and clear when their chunk unloads.

`Creatures4` lands the first cow baseline:

- committed vanilla cow fixture for seed `12345`, chunk `(2, -18)`
- cow fixture/runtime publication coverage alongside the sheep fixture
- `entity_update` and `entity_remove` protocol records beside `entity_snapshot`
- client hydration for entity add/update/remove lifecycle
- explicit remove/untrack messages for unloaded interest and dropped remote player slots
- vanilla-shaped cow renderer/model registration and extracted cow texture loading

Still not landed: live natural spawning, player-distance spawn eligibility, mob caps/counting, despawn, meaningful cow ticking behavior, AI/pathfinding/wandering, persistence adapters beyond the in-memory runtime path, and cow gameplay interactions.

## Reference Source Map

| Concern | Vanilla source |
|---|---|
| Natural spawn loop and generation-time creature spawn | `reference/minecraft-1.17.1/src/net/minecraft/world/level/NaturalSpawner.java` |
| Server chunk tick integration | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java` |
| Server entity tick loop | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java` |
| Chunk visibility and entity ticking status | `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java` |
| Player/chunk distance trackers | `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java` |
| Entity section lifecycle and persistence manager | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/PersistentEntitySectionManager.java` |
| Entity ticking set | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/EntityTickList.java` |
| Entity visibility states | `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/Visibility.java` |
| Mob categories and caps | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/MobCategory.java` |
| Biome spawn settings | `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/MobSpawnSettings.java` |
| Default overworld spawn lists | `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` |
| Vanilla biome-specific spawn additions | `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` |
| Spawn placement registry | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/SpawnPlacements.java` |
| Generic mob spawn predicate | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Mob.java` |
| Passive animal spawn rules | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Animal.java` |
| Hostile spawn rules | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/monster/Monster.java` |
| Entity type registration and tracking ranges | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/EntityType.java` |
| Dedicated server spawn flags | `reference/minecraft-1.17.1/src/net/minecraft/server/dedicated/DedicatedServerProperties.java` |
| `doMobSpawning` gamerule | `reference/minecraft-1.17.1/src/net/minecraft/world/level/GameRules.java` |

Read these files before writing the port. Spawn algorithms, category caps, placement predicates, despawn rules, and entity tick semantics are simulation parity logic. The host scheduler and worker layout may diverge, but they must preserve the same observable world state.

## Terms

- **Entity**: any dynamic world object. This document is mostly about mobs, not items, projectiles, block entities, or players.
- **Mob**: an entity with server-side lifecycle and AI hooks.
- **Creature**: vanilla's `MobCategory.CREATURE`, which includes common passive land animals and is treated as a persistent category for natural-spawn frequency.
- **Original mobs**: passive creatures added during chunk generation by `ChunkStatus.SPAWN` through `ChunkGenerator.spawnOriginalMobs(...)`.
- **Natural spawn**: live server-tick spawning through `NaturalSpawner.spawnForChunk(...)`.
- **Spawner data**: biome weighted entries: entity type, weight, minimum group size, maximum group size.
- **Tracked entity**: an entity visible to server queries and potentially to clients, but not necessarily ticking.
- **Ticking entity**: an entity currently in `ServerLevel.entityTickList`.
- **Ticking chunk**: a full chunk whose block/random/chunk tick work can run.
- **Entity-ticking chunk**: a stronger chunk status whose non-always-ticking entities are allowed to tick.
- **Spawnable chunk**: for natural spawning, a player-distance chunk counted by `DistanceManager`'s natural-spawn tracker, not every loaded chunk.

## Vanilla Model

Vanilla has two different paths that people often blur together.

### Generation-Time Original Mobs

During world generation, `ChunkStatus.SPAWN` calls `ChunkGenerator.spawnOriginalMobs(...)`. For the 1.17.1 overworld `NoiseBasedChunkGenerator`, that method:

1. skips generation if `NoiseGeneratorSettings.disableMobGeneration()` is true
2. looks up the center chunk's biome
3. seeds a `WorldgenRandom` with `setDecorationSeed(worldSeed, minBlockX, minBlockZ)`
4. calls `NaturalSpawner.spawnMobsForChunkGeneration(...)`

`spawnMobsForChunkGeneration(...)` only uses the biome's `MobCategory.CREATURE` list. It loops while `random.nextFloat() < biome.mobSettings.creatureGenerationProbability`, which defaults to `0.1`, chooses a weighted `SpawnerData`, picks the group count, then attempts nearby top-surface positions with `MobSpawnType.CHUNK_GENERATION`.

This is not a feature/decorator in the block-placement sense, but it is generation content. It produces real entities stored with the chunk, not decorative block states.

In `mclone`, this path is intentionally host-owned: `NaturalSpawner` emits `GeneratedMobEntity` records to a sink shaped like `addFreshEntityWithPassengers(...)`, and tests feed that sink into `EntityRuntime.addWorldGenChunkEntities(...)`. The renderer still receives no entity truth from this slice.

### Live Natural Spawning

Every server tick, `ServerChunkCache.tickChunks()`:

1. reads `doMobSpawning`
2. computes `spawnableChunkCount` from chunks within the natural spawn distance tracker
3. builds a `NaturalSpawner.SpawnState` by counting existing entities by `MobCategory`, excluding mobs marked `PersistenceRequired` or requiring custom persistence
4. shuffles ticking chunks
5. for each eligible entity-ticking chunk near a non-spectator player, calls `NaturalSpawner.spawnForChunk(...)`

Natural spawning is not seed-fixed. The biome, terrain, light, and chunk positions come from the seed, but the live outcome depends on server random state, time, loaded chunks, players, existing mobs, deaths, despawns, and tick order.

## Categories And Caps

`MobCategory` defines vanilla's category caps:

| Category | Cap | Friendly | Persistent category | Despawn distance |
|---|---:|---|---|---:|
| `MONSTER` | 70 | no | no | 128 |
| `CREATURE` | 10 | yes | yes | 128 |
| `AMBIENT` | 15 | yes | no | 128 |
| `UNDERGROUND_WATER_CREATURE` | 5 | yes | no | 128 |
| `WATER_CREATURE` | 5 | yes | no | 128 |
| `WATER_AMBIENT` | 20 | yes | no | 64 |

The effective cap is scaled by spawnable chunks:

```text
effectiveCap = categoryCap * spawnableChunkCount / 17^2
```

`17^2` is the vanilla `MAGIC_NUMBER`, matching the 17x17 chunk natural-spawn region around a player. Ordinary cows, sheep, pigs, chickens, horses, wolves, and similar animals count against the `CREATURE` cap. Mobs marked `PersistenceRequired` or requiring custom persistence do not count toward natural-spawn caps.

`CREATURE` is special in live natural spawning: because it is a persistent category, `spawnForChunk(...)` only attempts it every `400` game ticks, about every 20 seconds at 20 TPS. Non-persistent categories such as monsters can be attempted every tick.

## Overworld Spawn Lists

The core common overworld lists live in `BiomeDefaultFeatures`.

`farmAnimals(...)` adds:

| Entity | Weight | Group |
|---|---:|---|
| sheep | 12 | 4-4 |
| pig | 10 | 4-4 |
| chicken | 10 | 4-4 |
| cow | 8 | 4-4 |

`plainsSpawns(...)` adds those farm animals plus:

| Entity | Weight | Group |
|---|---:|---|
| horse | 5 | 2-6 |
| donkey | 1 | 1-3 |

Common monsters come from `monsters(...)`:

| Entity | Weight | Group |
|---|---:|---|
| spider | 100 | 4-4 |
| zombie | usually 95 | 4-4 |
| zombie villager | usually 5 | 1-1 |
| skeleton | usually 100 | 4-4 |
| creeper | 100 | 4-4 |
| slime | 100 | 4-4 |
| enderman | 10 | 1-4 |
| witch | 5 | 1-1 |

`VanillaBiomes` layers in biome-specific creatures, for example wolves, rabbits, foxes, llamas, goats, horses, and donkeys in the relevant biome builders. A first implementation should port the spawn settings needed by the biome set we already render, then broaden through oracle-backed slices.

## Spawn Placement Rules

NaturalSpawner has a two-stage validation:

1. category/type/biome selection and mob-cap checks
2. entity-specific placement and spawn predicates through `SpawnPlacements`

For common passive land animals, `SpawnPlacements` registers `Type.ON_GROUND`, `Heightmap.Types.MOTION_BLOCKING_NO_LEAVES`, and `Animal.checkAnimalSpawnRules(...)`.

`Animal.checkAnimalSpawnRules(...)` requires:

- the block below is `grass_block`
- raw brightness at the spawn position is greater than `8`

For common monsters, `SpawnPlacements` registers `Type.ON_GROUND` and `Monster.checkMonsterSpawnRules(...)`.

`Monster.checkMonsterSpawnRules(...)` requires:

- difficulty is not peaceful
- sky light is low enough against a random `0..31` threshold
- local raw brightness is low enough against a random `0..7` threshold
- the block below is a valid spawn block

So spiders are not decorations. They are selected from the biome monster list and then live-spawned by the dark-place rules, subject to mob caps, distance from players, difficulty, and chunk eligibility.

## Spawn Attempt Geometry

For each category/chunk attempt:

- choose a random x/z in the chunk
- choose y uniformly between min build height and `WORLD_SURFACE + 1`
- skip if the candidate block is a redstone conductor
- run up to three local group attempts
- within each group, wander x/z by `random.nextInt(6) - random.nextInt(6)`
- require a nearest player
- reject positions within 24 blocks of a player
- reject positions within 24 blocks of the shared spawn point
- require the position to be in the current chunk or another entity-ticking chunk
- reject non-far-spawning entity types beyond the category despawn distance
- check biome/structure spawn list membership
- check placement type, spawn predicate, no-collision, `Mob.checkSpawnRules(...)`, and `Mob.checkSpawnObstruction(...)`
- call `finalizeSpawn(...)`
- add the entity with passengers

Group size is controlled by the selected `SpawnerData` and the mob's `getMaxSpawnClusterSize()` / `isMaxGroupSizeReached(...)`.

## Active Chunks And Entity Activity

Vanilla does not have one single "active chunk" bit. The relevant levels are:

| Vanilla status | Entity visibility | Entity ticking | Main use |
|---|---|---|---|
| `INACCESSIBLE` | hidden | no | not usable |
| `BORDER` | tracked | no | loaded/visible boundary data |
| `TICKING` | tracked | no | chunk/block/random tick work |
| `ENTITY_TICKING` | tracked | yes | entity ticks and natural-spawn eligibility |

`PersistentEntitySectionManager.updateChunkStatus(...)` maps `ENTITY_TICKING` to `Visibility.TICKING`; `BORDER` and `TICKING` are accessible/tracked but not ticking for normal entities. When a chunk becomes entity-ticking, entities in that chunk enter `EntityTickList`. When it demotes, they leave the tick list but can remain stored/tracked.

`ServerLevel` ticks only entities in `entityTickList`. A ticking mob runs:

```text
checkDespawn()
tickNonPassenger()
  Entity.tick()
  passenger ticks
```

For mobs, server AI work happens inside `Mob.serverAiStep()`: sensing, target selector, goal selector, navigation, custom AI, movement/look/jump controls, and debug packet hooks. We should not start with this entire AI surface; the first creature slice can keep server-owned entity records and simple movement/state ticking before porting full goals.

## Player Distance And Spawn Eligibility

Natural spawning is player-proximity driven:

- `DistanceManager` maintains a natural-spawn tracker with max distance `8` chunks.
- `ChunkMap.noPlayersCloseForSpawning(...)` rejects chunks with no nearby non-spectator player and also checks player-to-chunk euclidean distance is less than `128` blocks.
- Individual spawn positions are rejected within `24` blocks of the nearest player.
- Positions are rejected within `24` blocks of the shared spawn point.
- The target chunk must be entity-ticking, or the candidate must remain in the chunk currently being processed.

This means "loaded" is not enough. A loaded chunk outside the entity-ticking/player-spawn region should not run natural spawning and should not advance ordinary mob AI.

## Despawn And Death Lifecycle

Natural death or player-caused death removes the entity. There is no per-animal respawn record and no "restore the original seed cow" step.

What can happen after death:

- the entity is gone from storage and live tracking
- category counts may drop below the natural cap
- later natural spawning can create new mobs if all category, biome, light, distance, collision, and chunk-status checks pass

For common passive animals, this usually feels persistent because `Animal.removeWhenFarAway(...)` returns `false`, so cows, sheep, pigs, chickens, horses, wolves, and similar animals do not naturally despawn just because the player leaves. If killed, new passive animals can spawn, but only through the slow `CREATURE` natural-spawn pass and cap rules. It is replacement by chance, not respawn.

For common hostiles, `Mob.checkDespawn()` removes mobs immediately beyond category despawn distance if `removeWhenFarAway(...)` is true, and can randomly despawn inactive mobs beyond the no-despawn distance after `noActionTime > 600`. Monsters also despawn in peaceful if `shouldDespawnInPeaceful()` returns true. Hostile populations are therefore much more fluid: dark eligible areas refill as old mobs die or despawn.

## Configurability In Vanilla 1.17.1

Vanilla exposes only coarse controls:

- `gamerule doMobSpawning`: gates live natural spawning and custom spawners in `ServerChunkCache.tickChunks()`.
- `server.properties spawn-animals`: controls friendly category live natural spawning on dedicated servers.
- `server.properties spawn-monsters`: controls hostile live natural spawning on dedicated servers.
- difficulty: peaceful blocks common monster spawning and despawns peaceful-sensitive monsters.
- `server.properties view-distance`: affects player chunk tickets and therefore which chunks can reach ticking/entity-ticking status.

Generation-time original mobs are not gated by `doMobSpawning`, `spawn-animals`, or `spawn-monsters` in this path. The vanilla overworld generator checks `NoiseGeneratorSettings.disableMobGeneration()` before `spawnOriginalMobs(...)`; that is worldgen settings data, not the normal dedicated-server spawning flags.

Important non-configurable vanilla constants in 1.17.1:

- natural-spawn tracker radius: 8 chunks
- spawnable chunk divisor: `17^2`
- minimum player and shared-spawn distance: 24 blocks
- category despawn distance: usually 128 blocks, 64 for `WATER_AMBIENT`
- no-despawn distance: 32 blocks
- `CREATURE` natural-spawn interval: 400 ticks
- category caps and spawn attempt geometry

Minecraft Java 1.17.1 does not have the later separate `simulation-distance` server property. For our 1.17.1 target, keep simulation/chunk activity tied to the 1.17.1 ticket model unless we deliberately document an engine-profile divergence.

## Seed Dependence

Seed-dependent:

- biome layout
- terrain, surfaces, caves, vegetation, and therefore valid spawn surfaces
- generation-time original creature group placement/type loops through `WorldgenRandom.setDecorationSeed(...)`
- slime chunk selection and some structure-specific hostile behavior, when those systems are in scope

Do not treat full entity NBT as seed-stable by default. Entity UUIDs and some finalized type-specific fields can come from entity-local or level random state. Oracle fixtures should normalize away UUIDs until the exact vanilla source of a field is ported and intentionally asserted.

Not purely seed-dependent:

- live natural spawn outcomes
- exact timing and location of replacement animals after deaths
- hostile population churn
- anything affected by player position, loaded chunks, save/load timing, current entity counts, difficulty, and server random state

For `mclone`, generated original mobs can be oracle-tested as chunk content once entity fixture support exists. Live natural spawning needs tick-scripted server fixtures, similar in spirit to liquid simulation fixtures, because the outcome depends on dynamic server state.

## Current Mclone Context

Today the repo has authoritative host/session plumbing, chunk snapshots, an early host-owned entity runtime, and generated passive entity publication:

| Area | Current role |
|---|---|
| `src/runtime/session/player-loop.ts` | debug authoritative player-state loop, not vanilla `Player`/`Entity` |
| `docs/protocol.md` | documents current `entity_snapshot` and future `entity_delta` semantics |
| `src/runtime/protocol/world-messages.ts` | concrete protocol has session, player, chunk, unload, error, and generated entity snapshot messages |
| `src/runtime/host/generated-world-host.ts` | owns chunk interest, player state, chunk snapshot delivery, and generated original mob publication |
| `src/runtime/node/generated-world-http-server.ts` | dedicated host path with the same logical ownership and per-session entity snapshot replay |
| `src/worldgen/biome/` | biome source exists, with the first passive spawn settings needed for generation-time creatures |
| `src/world/level/chunk-snapshot.ts` | carries blocks/biomes/ticks, not entities |
| `src/renderer/` | renders chunks and debug camera state, not mobs |

The first creature architecture should therefore add entities as authoritative world state, not as renderer-side decorations.

## Mclone Architecture

Keep the same division used by lighting and liquids:

| Layer | Creature responsibility |
|---|---|
| Simulation core | direct ports of entity type metadata, mob categories, biome spawn settings, spawn placements, natural spawn algorithm, despawn rules, and minimal entity state transitions |
| Authoritative host | owns game time, player/session positions, chunk ticket/activity state, entity storage, natural-spawn scheduling, entity ticks, mutation batching, persistence dirtying |
| Chunk workers | generate chunks and return blocks plus generation-time original entity records when `ChunkStatus.SPAWN` lands |
| Storage adapters | persist chunk/section entity records and world-level entity indexes behind engine-native records |
| Protocol | publish entity snapshots/deltas with authoritative ids, chunk residency, and tick/revision context |
| Renderer/client runtime | consume authoritative entity state for interpolation and drawing; never decide spawning or despawning |

### Architectural Divergence

Vanilla centralizes entity ticking and spawning on the server tick thread and uses `PersistentEntitySectionManager` for chunk-section storage and visibility transitions. `mclone` can map that to a browser worker or Node event-loop authority lane instead of a JVM thread.

The allowed divergence is:

- host/session authority lane owns entity mutation, spawn order, and tick order
- chunk jobs can run off-thread and return generation-created entity records
- renderer receives entity snapshots/deltas after authoritative mutation
- entity rendering can interpolate between authoritative ticks

The not-allowed divergence for a vanilla profile is:

- renderer-owned spawning or despawning
- treating generation-time animals as block decorations
- respawning killed original animals from the seed
- running natural spawning in merely loaded/non-entity-ticking chunks
- ignoring category caps, 24-block player/spawn exclusions, light checks, or biome spawn lists
- making animals per-frame visual particles instead of persisted entities

## Data Model And Persistence

Entity records should be separate from block-section records but chunk-addressable.

Minimum durable entity facts for the first slice:

- stable entity id and UUID-equivalent identity
- entity type id
- position, rotation, velocity if simulated
- owning chunk/section position
- alive/removed state
- persistence-required/custom-persistence flags needed by caps and despawn
- category/type metadata needed for spawn counts
- basic type-specific data for any rendered first-slice creature, such as sheep color or horse variant, only when that type lands

Chunk snapshots can continue to focus on block state. Entity snapshots/deltas should be separate protocol messages so block meshing does not depend on entity churn. Persistence should save entities with the chunk or an entity-section sidecar, but the simulation should consume a logical entity-section model independent of the physical storage adapter.

## Suggested First Scope

The first tactical slice is [`tactical/Creatures0-generation-entity-oracle-foundation.md`](tactical/Creatures0-generation-entity-oracle-foundation.md): build normalized official-server entity fixtures before porting the spawning algorithm.

`Creatures0` now has the first fixture-support pass: entity-region decoding, legacy `Level.Entities` fallback, stable normalization, comparison helpers, a server-wrapper scan mode, and a committed non-empty seed `12345` sheep fixture at chunk `(-7,-15)`.

A useful first creature slice is not "all mobs." Keep it narrow:

1. Content tables: `MobCategory`, minimal `EntityType` records, `MobSpawnSettings.SpawnerData`, and the common overworld spawn settings needed for current biomes.
2. Done in `Entities0`: host-owned entity records keyed by id/uuid and chunk section, with tracked vs ticking visibility.
3. Done in `Creatures1`: generation original mobs, `spawnOriginalMobs(...)`, `spawnMobsForChunkGeneration(...)` for `CREATURE` only, and passive spawn placement/collision rules enough for the committed sheep fixture.
4. Done in [`Creatures2`](tactical/Creatures2-host-entity-publication.md): integrate the entity runtime into the generated-world host lifecycle and publish simple generated-entity snapshots as data.
5. Next: rendering follow-through, draw simple authoritative entity placeholders before first real models or behavior.

A later slice can add live natural spawning for `CREATURE`. Another can add common `MONSTER` spawning once stored lighting and entity ticking are credible, because hostile spawn rules depend on sky/block light and despawn behavior.

## Verification

Use three tiers.

### Direct Unit Tests

These should use small synthetic worlds and compare exact local outcomes:

- category cap scaling by spawnable chunk count
- category gating from `spawn-animals`, `spawn-monsters`, and `doMobSpawning`
- `CREATURE` 400-tick interval gating
- weighted spawn-list selection with seeded RNG
- `Animal.checkAnimalSpawnRules(...)`
- `Monster.isDarkEnoughToSpawn(...)` once lighting exists
- distance rejection: 24 blocks from player, 24 blocks from shared spawn, 128-block chunk/player proximity
- tracked vs entity-ticking visibility transitions
- despawn checks for persistent passive animals vs despawning monsters

### Java Oracles

Generation fixtures should use the 1.17.1 server or Java harness to capture entity records for selected chunks after `ChunkStatus.SPAWN` / full chunk generation.

Dynamic fixtures should be scripted server scenarios:

- fixed seed and player position
- controlled time/difficulty/gamerules
- bounded area with known grass/light or darkness
- exact tick count
- dump entity list, categories, positions, and pending persistence state

Do not validate live spawning from screenshots alone. Screenshots are only for visible follow-through after data parity is covered.

### Browser Validation

When a creature slice produces pixels, run the smallest browser probe that reaches the new draw path, save screenshots to `/tmp`, and inspect them. The first drawable milestone can be a simple entity placeholder at authoritative positions; visual polish should not precede host-owned lifecycle correctness.

## Open Decisions

- Whether first entity persistence should be stored inside the chunk record or a sidecar entity-section record.
- How much of vanilla `EntityType` to port before the first visual creature, versus a minimal metadata table that is intentionally shaped for future expansion.
- How broadly to expand generation-original-mobs fixtures beyond the current exact sheep type/position/rotation/color comparison.
- Whether host chunk activity should keep exposing vanilla names (`BORDER`, `TICKING`, `ENTITY_TICKING`) at every API boundary or wrap them at higher runtime layers.
- Whether first browser rendering should use placeholder billboards, extracted vanilla models, or a deliberately small custom model path while entity behavior is still being ported.
