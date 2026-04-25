# Creatures1 - Generation passive spawning

Status: landed.

This slice follows [`Creatures0-generation-entity-oracle-foundation.md`](Creatures0-generation-entity-oracle-foundation.md) and [`Entities0-runtime-entity-foundation.md`](Entities0-runtime-entity-foundation.md). It ports the vanilla 1.17.1 generation-time passive creature path far enough to reproduce the committed sheep fixture and insert generated entities into the host-owned entity runtime.

## Goal

Implement `NoiseBasedChunkGenerator.spawnOriginalMobs(...)` and `NaturalSpawner.spawnMobsForChunkGeneration(...)` for passive `MobCategory.CREATURE` generation against the committed fixture:

```text
test/fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json
```

The result should be generated entity records, not block data, renderer objects, or local test-only structures. The insertion surface is a host-owned entity sink shaped like vanilla `addFreshEntityWithPassengers(...)` / `addWorldGenChunkEntities(...)`.

## Reference Source

Read these before changing this slice:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java` | `spawnOriginalMobs(...)`, decoration seed, `disableMobGeneration()` gate |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/NaturalSpawner.java` | exact generation-time loop, top-position logic, collision/spawn-rule order |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/SpawnPlacements.java` | placement type, heightmap type, passive predicate registry |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Animal.java` | grass-block and brightness predicate |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Sheep.java` | generated sheep color data |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/MobSpawnSettings.java` | weighted spawner data shape |
| `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/BiomeDefaultFeatures.java` | farm-animal spawn table |
| `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/biome/VanillaBiomes.java` | biome-specific passive additions |
| `reference/minecraft-1.17.1/src/net/minecraft/world/entity/EntityType.java` | passive entity dimensions and summon flags |

## Landed Scope

- `src/world/entity/mob-category.ts`: vanilla category metadata and caps
- `src/world/entity/entity-type.ts`: minimal passive entity metadata and `GeneratedMobEntity`
- `src/world/entity/animal/animal.ts`: passive spawn predicates used by current tables
- `src/world/entity/spawn-placements.ts`: passive placement registry
- `src/worldgen/biome/mob-spawn-settings.ts`: `MobSpawnSettings`, `SpawnerData`, and weighted selection
- `src/worldgen/biome/overworld-biome-mob-spawn-settings.ts`: current overworld passive tables
- `Biome.getMobSettings()` and biome-data wiring
- `NoiseGeneratorSettings.disableMobGeneration()`
- `src/worldgen/levelgen/natural-spawner.ts`: generation-time passive spawning into a host sink
- `NoiseBasedChunkGenerator.spawnOriginalMobs(...)`
- `test/worldgen/levelgen/creature-generation.test.ts`: fixture comparison plus runtime insertion

## Intentional Deferrals

- live natural spawning through `NaturalSpawner.spawnForChunk(...)`
- category cap counting over visible entities
- player-distance and shared-spawn-distance checks
- difficulty, gamerules, dedicated-server spawn flags
- hostile, ambient, water, and underground-water spawn tables
- full `Mob`, `LivingEntity`, AI, navigation, goals, despawn, breeding, taming, and combat
- entity deltas and renderer consumption
- entity rendering, models, animations, sounds, particles, and selection UI
- persistent entity storage adapters beyond the in-memory runtime test path

## Validation

Focused validation:

```bash
pnpm test -- test/worldgen/levelgen/creature-generation.test.ts
pnpm test -- test/world/level/entity test/runtime/entity-runtime.test.ts test/worldgen/levelgen/creature-generation.test.ts test/oracle/committed-creature-fixture.test.ts
pnpm typecheck
```

No browser screenshot is required; this slice does not produce pixels.

## Done Criteria

- done: `NoiseBasedChunkGenerator.spawnOriginalMobs(...)` uses vanilla decoration seed and `disableMobGeneration()`
- done: biome passive spawn settings are represented on `Biome`
- done: `NaturalSpawner.spawnMobsForChunkGeneration(...)` follows the Java loop and emits to a host-owned entity sink
- done: generated sheep fixture matches normalized type, position, rotation, age, on-ground, and color facts
- done: spawned generated entities enter `EntityRuntime.addWorldGenChunkEntities(...)`
- done: no live natural spawning, AI, despawn, renderer, or protocol path was added

## Follow-Up

[`Creatures2-host-entity-publication.md`](Creatures2-host-entity-publication.md) landed the immediate follow-up:

- integrated a host-owned entity runtime into the generated-world host lifecycle
- defined the first entity snapshot protocol records
- published tracked generated entities to local and remote clients as data
- kept browser rendering as a follow-up slice after the protocol/cache boundary was clear

Live natural spawning should wait until entity ticking, mob caps, player-distance eligibility, and despawn have enough runtime support to avoid a one-off local spawner.
