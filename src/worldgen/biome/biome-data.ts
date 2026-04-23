import { Biome, GrassColorModifier, TemperatureModifier, type BiomeDefinition } from "./biome";
import { getOverworldBiomeGenerationSettings } from "./overworld-biome-generation-settings";

function visuals(
  id: number,
  key: string,
  depth: number,
  scale: number,
  temperature: number,
  downfall: number,
  waterColor: number,
  foliageColorOverride?: number,
  grassColorOverride?: number,
  grassColorModifier: GrassColorModifier = GrassColorModifier.NONE,
): BiomeDefinition {
  return { id, key, depth, scale, temperature, downfall, waterColor, foliageColorOverride, grassColorOverride, grassColorModifier };
}

function plains(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.8, 0.4, 4_159_204);
}

function desert(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 2.0, 0.0, 4_159_204);
}

function mountain(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.2, 0.3, 4_159_204);
}

function forest(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.7, 0.8, 4_159_204);
}

function birchForest(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.6, 0.6, 4_159_204);
}

function taiga(id: number, key: string, depth: number, scale: number, snowy: boolean): BiomeDefinition {
  return visuals(id, key, depth, scale, snowy ? -0.5 : 0.25, snowy ? 0.4 : 0.8, snowy ? 4_020_182 : 4_159_204);
}

function giantTaiga(id: number, key: string, depth: number, scale: number, temperature: number): BiomeDefinition {
  return visuals(id, key, depth, scale, temperature, 0.8, 4_159_204);
}

function jungle(id: number, key: string, depth: number, scale: number, downfall: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.95, downfall, 4_159_204);
}

function ocean(id: number, key: string, depth: number, scale: number, waterColor: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.5, 0.5, waterColor);
}

function frozenOcean(id: number, key: string, depth: number, scale: number, deep: boolean): BiomeDefinition {
  return { ...visuals(id, key, depth, scale, deep ? 0.5 : 0.0, 0.5, 3_750_089), temperatureModifier: TemperatureModifier.FROZEN };
}

function tundra(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.0, 0.5, 4_159_204);
}

function mushroom(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.9, 1.0, 4_159_204);
}

function beach(id: number, key: string, depth: number, scale: number, temperature: number, downfall: number, waterColor: number): BiomeDefinition {
  return visuals(id, key, depth, scale, temperature, downfall, waterColor);
}

function darkForest(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.7, 0.8, 4_159_204, undefined, undefined, GrassColorModifier.DARK_FOREST);
}

function swamp(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 0.8, 0.9, 6_388_580, 6_975_545, undefined, GrassColorModifier.SWAMP);
}

function river(id: number, key: string, depth: number, scale: number, temperature: number, waterColor: number): BiomeDefinition {
  return visuals(id, key, depth, scale, temperature, 0.5, waterColor);
}

function savanna(id: number, key: string, depth: number, scale: number, temperature: number): BiomeDefinition {
  return visuals(id, key, depth, scale, temperature, 0.0, 4_159_204);
}

function badlands(id: number, key: string, depth: number, scale: number): BiomeDefinition {
  return visuals(id, key, depth, scale, 2.0, 0.0, 4_159_204, 10_387_789, 9_470_285);
}

const ALL_OVERWORLD_LAYERED_BIOME_DEFINITIONS = [
  ocean(0, "minecraft:ocean", -1, 0.10000000149011612, 4_159_204),
  plains(1, "minecraft:plains", 0.125, 0.05000000074505806),
  desert(2, "minecraft:desert", 0.125, 0.05000000074505806),
  mountain(3, "minecraft:mountains", 1, 0.5),
  forest(4, "minecraft:forest", 0.10000000149011612, 0.20000000298023224),
  taiga(5, "minecraft:taiga", 0.20000000298023224, 0.20000000298023224, false),
  swamp(6, "minecraft:swamp", -0.20000000298023224, 0.10000000149011612),
  river(7, "minecraft:river", -0.5, 0, 0.5, 4_159_204),
  frozenOcean(10, "minecraft:frozen_ocean", -1, 0.10000000149011612, false),
  river(11, "minecraft:frozen_river", -0.5, 0, 0.0, 3_750_089),
  tundra(12, "minecraft:snowy_tundra", 0.125, 0.05000000074505806),
  tundra(13, "minecraft:snowy_mountains", 0.44999998807907104, 0.30000001192092896),
  mushroom(14, "minecraft:mushroom_fields", 0.20000000298023224, 0.30000001192092896),
  mushroom(15, "minecraft:mushroom_field_shore", 0, 0.02500000037252903),
  beach(16, "minecraft:beach", 0, 0.02500000037252903, 0.8, 0.4, 4_159_204),
  desert(17, "minecraft:desert_hills", 0.44999998807907104, 0.30000001192092896),
  forest(18, "minecraft:wooded_hills", 0.44999998807907104, 0.30000001192092896),
  taiga(19, "minecraft:taiga_hills", 0.44999998807907104, 0.30000001192092896, false),
  mountain(20, "minecraft:mountain_edge", 0.800000011920929, 0.30000001192092896),
  jungle(21, "minecraft:jungle", 0.10000000149011612, 0.20000000298023224, 0.9),
  jungle(22, "minecraft:jungle_hills", 0.44999998807907104, 0.30000001192092896, 0.9),
  jungle(23, "minecraft:jungle_edge", 0.10000000149011612, 0.20000000298023224, 0.8),
  ocean(24, "minecraft:deep_ocean", -1.7999999523162842, 0.10000000149011612, 4_159_204),
  beach(25, "minecraft:stone_shore", 0.10000000149011612, 0.800000011920929, 0.2, 0.3, 4_159_204),
  beach(26, "minecraft:snowy_beach", 0, 0.02500000037252903, 0.05, 0.3, 4_020_182),
  birchForest(27, "minecraft:birch_forest", 0.10000000149011612, 0.20000000298023224),
  birchForest(28, "minecraft:birch_forest_hills", 0.44999998807907104, 0.30000001192092896),
  darkForest(29, "minecraft:dark_forest", 0.10000000149011612, 0.20000000298023224),
  taiga(30, "minecraft:snowy_taiga", 0.20000000298023224, 0.20000000298023224, true),
  taiga(31, "minecraft:snowy_taiga_hills", 0.44999998807907104, 0.30000001192092896, true),
  giantTaiga(32, "minecraft:giant_tree_taiga", 0.20000000298023224, 0.20000000298023224, 0.3),
  giantTaiga(33, "minecraft:giant_tree_taiga_hills", 0.44999998807907104, 0.30000001192092896, 0.3),
  mountain(34, "minecraft:wooded_mountains", 1, 0.5),
  savanna(35, "minecraft:savanna", 0.125, 0.05000000074505806, 1.2),
  savanna(36, "minecraft:savanna_plateau", 1.5, 0.02500000037252903, 1.0),
  badlands(37, "minecraft:badlands", 0.10000000149011612, 0.20000000298023224),
  badlands(38, "minecraft:wooded_badlands_plateau", 1.5, 0.02500000037252903),
  badlands(39, "minecraft:badlands_plateau", 1.5, 0.02500000037252903),
  ocean(44, "minecraft:warm_ocean", -1, 0.10000000149011612, 4_445_678),
  ocean(45, "minecraft:lukewarm_ocean", -1, 0.10000000149011612, 4_566_514),
  ocean(46, "minecraft:cold_ocean", -1, 0.10000000149011612, 4_020_182),
  ocean(47, "minecraft:deep_warm_ocean", -1.7999999523162842, 0.10000000149011612, 4_445_678),
  ocean(48, "minecraft:deep_lukewarm_ocean", -1.7999999523162842, 0.10000000149011612, 4_566_514),
  ocean(49, "minecraft:deep_cold_ocean", -1.7999999523162842, 0.10000000149011612, 4_020_182),
  frozenOcean(50, "minecraft:deep_frozen_ocean", -1.7999999523162842, 0.10000000149011612, true),
  plains(129, "minecraft:sunflower_plains", 0.125, 0.05000000074505806),
  desert(130, "minecraft:desert_lakes", 0.22499999403953552, 0.25),
  mountain(131, "minecraft:gravelly_mountains", 1, 0.5),
  forest(132, "minecraft:flower_forest", 0.10000000149011612, 0.4000000059604645),
  taiga(133, "minecraft:taiga_mountains", 0.30000001192092896, 0.4000000059604645, false),
  swamp(134, "minecraft:swamp_hills", -0.10000000149011612, 0.30000001192092896),
  tundra(140, "minecraft:ice_spikes", 0.42500001192092896, 0.45000001788139343),
  jungle(149, "minecraft:modified_jungle", 0.20000000298023224, 0.4000000059604645, 0.9),
  jungle(151, "minecraft:modified_jungle_edge", 0.20000000298023224, 0.4000000059604645, 0.8),
  birchForest(155, "minecraft:tall_birch_forest", 0.20000000298023224, 0.4000000059604645),
  birchForest(156, "minecraft:tall_birch_hills", 0.550000011920929, 0.5),
  darkForest(157, "minecraft:dark_forest_hills", 0.20000000298023224, 0.4000000059604645),
  taiga(158, "minecraft:snowy_taiga_mountains", 0.30000001192092896, 0.4000000059604645, true),
  giantTaiga(160, "minecraft:giant_spruce_taiga", 0.20000000298023224, 0.20000000298023224, 0.25),
  giantTaiga(161, "minecraft:giant_spruce_taiga_hills", 0.20000000298023224, 0.20000000298023224, 0.25),
  mountain(162, "minecraft:modified_gravelly_mountains", 1, 0.5),
  savanna(163, "minecraft:shattered_savanna", 0.36250001192092896, 1.225000023841858, 1.1),
  savanna(164, "minecraft:shattered_savanna_plateau", 1.0499999523162842, 1.2125000953674316, 1.0),
  badlands(165, "minecraft:eroded_badlands", 0.10000000149011612, 0.20000000298023224),
  badlands(166, "minecraft:modified_wooded_badlands_plateau", 0.44999998807907104, 0.30000001192092896),
  badlands(167, "minecraft:modified_badlands_plateau", 0.44999998807907104, 0.30000001192092896),
  jungle(168, "minecraft:bamboo_jungle", 0.10000000149011612, 0.20000000298023224, 0.9),
  jungle(169, "minecraft:bamboo_jungle_hills", 0.44999998807907104, 0.30000001192092896, 0.9),
] as const satisfies readonly BiomeDefinition[];

const ALL_OVERWORLD_LAYERED_BIOMES = ALL_OVERWORLD_LAYERED_BIOME_DEFINITIONS.map(
  (definition) =>
    new Biome(
      definition.id,
      definition.key,
      definition.depth,
      definition.scale,
      definition.temperature,
      definition.downfall,
      definition.waterColor,
      definition.foliageColorOverride,
      definition.grassColorOverride,
      definition.grassColorModifier,
      definition.temperatureModifier,
      getOverworldBiomeGenerationSettings(definition.key),
    ),
);

export const OVERWORLD_LAYERED_BIOMES = ALL_OVERWORLD_LAYERED_BIOMES.filter(
  (biome) => biome.getId() !== 168 && biome.getId() !== 169,
);

const OVERWORLD_LAYERED_BIOMES_BY_ID = new Map(ALL_OVERWORLD_LAYERED_BIOMES.map((biome) => [biome.getId(), biome] as const));
const OVERWORLD_LAYERED_BIOMES_BY_KEY = new Map(ALL_OVERWORLD_LAYERED_BIOMES.map((biome) => [biome.getKey(), biome] as const));

export function getLayeredBiomeById(id: number): Biome {
  const biome = OVERWORLD_LAYERED_BIOMES_BY_ID.get(id);
  if (biome === undefined) {
    throw new Error(`unknown layered biome id ${id}`);
  }

  return biome;
}

export function getLayeredBiomeByKey(key: string): Biome {
  const biome = OVERWORLD_LAYERED_BIOMES_BY_KEY.get(key);
  if (biome === undefined) {
    throw new Error(`unknown layered biome key '${key}'`);
  }

  return biome;
}
