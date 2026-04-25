import { EntityTypes } from "../../world/entity/entity-type";
import { MobCategory } from "../../world/entity/mob-category";
import { MobSpawnSettings, MobSpawnSettingsBuilder, SpawnerData } from "./mob-spawn-settings";

function spawn(type: typeof EntityTypes[keyof typeof EntityTypes], weight: number, minCount: number, maxCount: number): SpawnerData {
  return new SpawnerData(type, weight, minCount, maxCount);
}

function farmAnimals(builder: MobSpawnSettingsBuilder): void {
  builder
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.SHEEP, 12, 4, 4))
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.PIG, 10, 4, 4))
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.CHICKEN, 10, 4, 4))
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.COW, 8, 4, 4));
}

function farmAnimalSettings(): MobSpawnSettingsBuilder {
  const builder = new MobSpawnSettingsBuilder();
  farmAnimals(builder);
  return builder;
}

function defaultSpawns(): MobSpawnSettingsBuilder {
  return farmAnimalSettings();
}

function plainsSpawns(): MobSpawnSettings {
  return farmAnimalSettings()
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.HORSE, 5, 2, 6))
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.DONKEY, 1, 1, 3))
    .build();
}

function taigaSpawns(): MobSpawnSettings {
  const builder = new MobSpawnSettingsBuilder();
  farmAnimals(builder);
  return builder
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.WOLF, 8, 4, 4))
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.RABBIT, 4, 2, 3))
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.FOX, 8, 2, 4))
    .build();
}

function snowySpawns(): MobSpawnSettings {
  return new MobSpawnSettingsBuilder()
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.RABBIT, 10, 2, 3))
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.POLAR_BEAR, 1, 1, 2))
    .build();
}

function desertSpawns(): MobSpawnSettings {
  return new MobSpawnSettingsBuilder()
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.RABBIT, 4, 2, 3))
    .build();
}

function mooshroomSpawns(): MobSpawnSettings {
  return new MobSpawnSettingsBuilder()
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.MOOSHROOM, 8, 4, 8))
    .build();
}

function mountainSpawns(): MobSpawnSettings {
  const builder = new MobSpawnSettingsBuilder();
  farmAnimals(builder);
  return builder
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.LLAMA, 5, 4, 6))
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.GOAT, 10, 4, 6))
    .build();
}

function savannaSpawns(): MobSpawnSettings {
  return farmAnimalSettings()
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.HORSE, 1, 2, 6))
    .addSpawn(MobCategory.CREATURE, spawn(EntityTypes.DONKEY, 1, 1, 1))
    .build();
}

const DEFAULT_FARM_BIOMES = new Set([
  "minecraft:forest",
  "minecraft:wooded_hills",
  "minecraft:birch_forest",
  "minecraft:birch_forest_hills",
  "minecraft:tall_birch_forest",
  "minecraft:tall_birch_hills",
  "minecraft:dark_forest",
  "minecraft:dark_forest_hills",
  "minecraft:swamp",
  "minecraft:swamp_hills",
  "minecraft:jungle",
  "minecraft:jungle_hills",
  "minecraft:jungle_edge",
  "minecraft:modified_jungle",
  "minecraft:modified_jungle_edge",
  "minecraft:bamboo_jungle",
  "minecraft:bamboo_jungle_hills",
]);

export function getOverworldBiomeMobSpawnSettings(key: string): MobSpawnSettings {
  switch (key) {
    case "minecraft:plains":
    case "minecraft:sunflower_plains":
      return plainsSpawns();
    case "minecraft:taiga":
    case "minecraft:taiga_hills":
    case "minecraft:taiga_mountains":
    case "minecraft:giant_tree_taiga":
    case "minecraft:giant_tree_taiga_hills":
    case "minecraft:giant_spruce_taiga":
    case "minecraft:giant_spruce_taiga_hills":
      return taigaSpawns();
    case "minecraft:snowy_taiga":
    case "minecraft:snowy_taiga_hills":
    case "minecraft:snowy_taiga_mountains":
    case "minecraft:snowy_tundra":
    case "minecraft:snowy_mountains":
    case "minecraft:ice_spikes":
      return snowySpawns();
    case "minecraft:desert":
    case "minecraft:desert_hills":
    case "minecraft:desert_lakes":
      return desertSpawns();
    case "minecraft:mushroom_fields":
    case "minecraft:mushroom_field_shore":
      return mooshroomSpawns();
    case "minecraft:mountains":
    case "minecraft:wooded_mountains":
    case "minecraft:gravelly_mountains":
    case "minecraft:modified_gravelly_mountains":
      return mountainSpawns();
    case "minecraft:savanna":
    case "minecraft:savanna_plateau":
    case "minecraft:shattered_savanna":
    case "minecraft:shattered_savanna_plateau":
      return savannaSpawns();
    default:
      return DEFAULT_FARM_BIOMES.has(key) ? defaultSpawns().build() : MobSpawnSettings.EMPTY;
  }
}
