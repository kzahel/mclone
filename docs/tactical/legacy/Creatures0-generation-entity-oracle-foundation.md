# Creatures0 - Generation entity oracle foundation

Standing after the durable creature reference in [`../creatures.md`](../creatures.md). This is the first creature-system tactical. It builds the oracle and data-shape substrate needed to validate generation-time passive creature spawning before porting `NaturalSpawner` into TypeScript.

## Goal

Extend the existing Minecraft 1.17.1 oracle infrastructure so it can capture normalized entity records for generated overworld chunks:

- read vanilla's 1.17.1 entity storage from `world/entities/*.mca`
- also tolerate legacy/proto chunk `Entities` lists when present
- normalize entity records into a stable fixture shape
- preserve type, position, rotation, chunk, and selected stable entity data
- intentionally omit UUIDs and other unstable full-NBT fields from equality assertions
- commit at least one small fixture proving the path with generation-time `MobCategory.CREATURE` entities
- add comparison helpers that later TS entity generation can use

At the end of `Creatures0`, we should be able to say: "for this generated vanilla chunk, these normalized passive entity records are Minecraft's result." The TypeScript `NaturalSpawner` port does not exist yet.

## Why this slice first

Creature spawning has three independent risks:

1. whether we can observe vanilla entity output correctly
2. whether we faithfully port generation-time `CREATURE` spawning
3. whether we later run live natural spawning, entity ticking, despawn, and rendering through the authoritative host

`Creatures0` solves only the first risk. It is deliberately analogous to `Liquid0`: build the official-server fixture path first, then implement against exact data instead of wiki summaries or screenshots.

The recommended validation mode for the next slice is:

```text
official MC server generated world
  -> dump normalized generated entities for selected chunks
  -> run TS generation-original-mobs path for same seed/chunks
  -> compare normalized type/position/rotation/entity-data facts
```

## Reference Source

Read these before implementing:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/NaturalSpawner.java` | generation-time `spawnMobsForChunkGeneration(...)` target behavior |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java` | `ChunkStatus.SPAWN` ordering |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java` | `spawnOriginalMobs(...)` hook |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java` | overworld `spawnOriginalMobs(...)` implementation and decoration seed |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java` | generation-time `addFreshEntity(...)` writes to target chunks |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/EntityStorage.java` | full-chunk entity storage in `world/entities/*.mca` |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java` | legacy/proto chunk `Entities` handling |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/ChunkEntities.java` | chunk-addressed entity storage model |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java` | persisted NBT fields and unstable UUID behavior |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/MobCategory.java` | category facts needed in fixtures |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/MobSpawnSettings.java` | spawn-list entry shape |
| `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` | farm animal and common spawn tables |
| `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` | biome-specific creature additions |

Use the official server oracle path for committed fixtures. Do not use visual expectation as the oracle.

## Current TS Context

| TS source | Current role |
|---|---|
| `src/oracle/anvil/region.ts` | generic `.mca` region reader, reusable for `entities/` region files |
| `src/oracle/anvil/nbt.ts` | NBT decoder used by chunk and liquid fixtures |
| `src/oracle/anvil/chunk.ts` | decodes block chunks, light, heightmaps, and biomes but not entities |
| `src/oracle/anvil/entity-chunk.ts` | decodes top-level entity chunks and legacy/proto `Level.Entities` lists |
| `oracle/integration/run-server.sh` | creates a pinned official-server generated world |
| `oracle/integration/dump-chunks.ts` | dumps generated block chunks from `world/region/*.mca` |
| `oracle/integration/dump-creature-fixture.ts` | dumps normalized generated entity records from `world/entities/*.mca` with legacy chunk fallback |
| `oracle/integration/gen-creature-fixture.sh` | official-server wrapper for creature fixture generation and scan mode |
| `src/oracle/integration/chunk-fixture.ts` | chunk fixture builder/comparison helpers |
| `src/oracle/integration/creature-fixture.ts` | fixture schema, entity normalizer, category mapper, stable sorting, and comparison helpers |
| `src/oracle/integration/liquid-fixture.ts` | model for bounded, property-preserving dynamic fixtures |
| `src/worldgen/biome/biome.ts` | biome data exists, but mob spawn settings are not represented yet |
| `src/runtime/protocol/world-messages.ts` | no concrete entity snapshot/delta messages yet |
| `src/world/level/chunk-snapshot.ts` | block chunk snapshots do not carry entities |

Do not put entity facts into block mesh payloads. For `Creatures0`, fixtures can live under `src/oracle/integration` and `test/fixtures/creatures/` without changing runtime protocol.

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Entity fixture schema | add a normalized `module: "creature-generation"` fixture type with seed, chunks, entity records, and normalization metadata |
| 2 | Entity storage reader | read `world/entities/r.x.z.mca` using the existing region/NBT decoder and decode top-level `Entities` + `Position` |
| 3 | Legacy/proto fallback | decode chunk `Level.Entities` only when entity storage is absent or when a fixture explicitly targets proto/legacy chunks |
| 4 | Entity normalizer | convert NBT to stable facts: type id, chunk, position, rotation, on-ground flag, age where stable, and allowlisted type data |
| 5 | Comparison helpers | compare expected/actual normalized entity fixtures with useful diffs and stable sorting |
| 6 | Fixture generator | add an oracle entry point that generates or reuses an official-server world, dumps selected chunks' entity records, and writes JSON |
| 7 | First fixture | commit one or two small generated-world entity fixtures, preferably chunks that contain passive `CREATURE` mobs |
| 8 | Tests | cover entity-region decode, normalization, unstable field omission, sorting, and comparison diffs |
| 9 | Docs | document fixture regeneration command and caveats |

## Explicit Non-Goals

- no TypeScript `NaturalSpawner` port
- no `EntityType` runtime registry beyond fixture string ids
- no host-owned entity manager
- no entity persistence adapter changes
- no runtime protocol changes
- no renderer or entity model work
- no live natural spawning
- no despawn/tick/AI/pathfinding
- no hostile monster fixture requirement
- no village, dungeon, spawner, raid, patrol, phantom, trader, or structure-specific entity work

## Fixture Shape

Recommended JSON shape:

```jsonc
{
  "module": "creature-generation",
  "minecraftVersion": "1.17.1",
  "dataVersion": 2730,
  "seed": "12345",
  "source": {
    "entityStorage": "entities",
    "normalization": [
      "uuid omitted",
      "motion omitted until simulated",
      "only allowlisted type data compared"
    ]
  },
  "chunks": [
    { "chunkX": 0, "chunkZ": 0 }
  ],
  "entities": [
    {
      "chunkX": 0,
      "chunkZ": 0,
      "type": "minecraft:sheep",
      "category": "creature",
      "pos": [6.5, 64, 11.5],
      "rotation": [132.25, 0],
      "onGround": true,
      "age": 0,
      "data": {
        "Color": 0
      }
    }
  ]
}
```

Rules:

- Sort entities by chunk, type, position, then rotation.
- Store double positions exactly as decoded from NBT; comparison helpers can choose exact or epsilon checks later.
- Omit UUIDs from committed equality fixtures.
- Omit `Motion` until runtime movement simulation exists.
- Omit health, attributes, brain/memory data, equipment, leash data, and full passengers unless a later slice intentionally owns them.
- Include type-specific data only through an allowlist. Sheep color is a reasonable first candidate; horse variant should wait until the relevant source and stability are understood.
- If a generated chunk has no entities, the fixture should still be valid, but `Creatures0` should commit at least one non-empty fixture to prove the path.

## Fixture Discovery

Generation-time passive mobs are probabilistic. A single pinned spawn chunk can be empty. Add a small scanner mode instead of hard-coding a guess:

1. Generate the official-server world for the pinned seed.
2. Read entity records for the generated spawn area.
3. Print chunk coordinates with non-empty `MobCategory.CREATURE` entities.
4. Commit fixtures for a small selected subset.

If the current startup runner does not reliably promote/save entity storage for enough chunks, add an explicit server command/scripted path later. Do not fake entity records from block fixtures.

## Validation

Focused validation:

```bash
pnpm test -- test/oracle/creature-fixture.test.ts test/oracle/committed-creature-fixture.test.ts
pnpm typecheck
```

Fixture generation should use the existing oracle bootstrap expectations:

```bash
./scripts/fetch-server-jar.sh 1.17.1
./oracle/integration/gen-creature-fixture.sh \
  --seed 12345 \
  --chunks -7,-15 \
  --out test/fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json
```

Use scan mode first when selecting a non-empty passive-creature fixture:

```bash
./oracle/integration/gen-creature-fixture.sh \
  --seed 12345 \
  --scan \
  --out /tmp/mclone-creature-scan-seed-12345.json
```

The exact command is documented in `oracle/README.md`.

## Implementation Status

The first implementation pass covers schema, entity-region decode, legacy/proto `Level.Entities` fallback, stable normalizing, sorting, comparison helpers, scanner output, the official-server wrapper, and the first committed non-empty passive fixture: `test/fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json`.

## Done When

- official-server entity storage can be decoded from `world/entities/*.mca`
- normalized creature-generation fixtures can be built and compared
- unstable UUID/full-NBT fields are intentionally omitted from equality
- at least one committed fixture contains a real passive generated entity
- tests fail with readable diffs for missing, extra, or moved entities
- the fixture generator command is documented

## Next

`Creatures1` is now [`Creatures1-generation-passive-spawning.md`](Creatures1-generation-passive-spawning.md). It ports the generation-time passive spawning path against the `Creatures0` fixture:

- `MobCategory`
- minimal `EntityType` metadata for first passive creatures
- `MobSpawnSettings` / `SpawnerData`
- biome spawn settings for the fixture biomes
- `SpawnPlacements.Type.ON_GROUND`
- `Animal.checkAnimalSpawnRules(...)`
- `NaturalSpawner.spawnMobsForChunkGeneration(...)`
- a minimal generation entity sink shaped like `WorldGenRegion.addFreshEntity(...)`

Still defer live natural spawning, host entity ticking, AI, despawn, runtime protocol, and rendering until the generated-entity data path is connected beyond tests.
