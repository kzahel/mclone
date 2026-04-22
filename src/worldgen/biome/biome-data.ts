import { Biome, type BiomeDefinition } from "./biome";

const ALL_OVERWORLD_LAYERED_BIOME_DEFINITIONS = [
  { id: 0, key: "minecraft:ocean", depth: -1, scale: 0.10000000149011612 },
  { id: 1, key: "minecraft:plains", depth: 0.125, scale: 0.05000000074505806 },
  { id: 2, key: "minecraft:desert", depth: 0.125, scale: 0.05000000074505806 },
  { id: 3, key: "minecraft:mountains", depth: 1, scale: 0.5 },
  { id: 4, key: "minecraft:forest", depth: 0.10000000149011612, scale: 0.20000000298023224 },
  { id: 5, key: "minecraft:taiga", depth: 0.20000000298023224, scale: 0.20000000298023224 },
  { id: 6, key: "minecraft:swamp", depth: -0.20000000298023224, scale: 0.10000000149011612 },
  { id: 7, key: "minecraft:river", depth: -0.5, scale: 0 },
  { id: 10, key: "minecraft:frozen_ocean", depth: -1, scale: 0.10000000149011612 },
  { id: 11, key: "minecraft:frozen_river", depth: -0.5, scale: 0 },
  { id: 12, key: "minecraft:snowy_tundra", depth: 0.125, scale: 0.05000000074505806 },
  { id: 13, key: "minecraft:snowy_mountains", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 14, key: "minecraft:mushroom_fields", depth: 0.20000000298023224, scale: 0.30000001192092896 },
  { id: 15, key: "minecraft:mushroom_field_shore", depth: 0, scale: 0.02500000037252903 },
  { id: 16, key: "minecraft:beach", depth: 0, scale: 0.02500000037252903 },
  { id: 17, key: "minecraft:desert_hills", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 18, key: "minecraft:wooded_hills", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 19, key: "minecraft:taiga_hills", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 20, key: "minecraft:mountain_edge", depth: 0.800000011920929, scale: 0.30000001192092896 },
  { id: 21, key: "minecraft:jungle", depth: 0.10000000149011612, scale: 0.20000000298023224 },
  { id: 22, key: "minecraft:jungle_hills", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 23, key: "minecraft:jungle_edge", depth: 0.10000000149011612, scale: 0.20000000298023224 },
  { id: 24, key: "minecraft:deep_ocean", depth: -1.7999999523162842, scale: 0.10000000149011612 },
  { id: 25, key: "minecraft:stone_shore", depth: 0.10000000149011612, scale: 0.800000011920929 },
  { id: 26, key: "minecraft:snowy_beach", depth: 0, scale: 0.02500000037252903 },
  { id: 27, key: "minecraft:birch_forest", depth: 0.10000000149011612, scale: 0.20000000298023224 },
  { id: 28, key: "minecraft:birch_forest_hills", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 29, key: "minecraft:dark_forest", depth: 0.10000000149011612, scale: 0.20000000298023224 },
  { id: 30, key: "minecraft:snowy_taiga", depth: 0.20000000298023224, scale: 0.20000000298023224 },
  { id: 31, key: "minecraft:snowy_taiga_hills", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 32, key: "minecraft:giant_tree_taiga", depth: 0.20000000298023224, scale: 0.20000000298023224 },
  { id: 33, key: "minecraft:giant_tree_taiga_hills", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 34, key: "minecraft:wooded_mountains", depth: 1, scale: 0.5 },
  { id: 35, key: "minecraft:savanna", depth: 0.125, scale: 0.05000000074505806 },
  { id: 36, key: "minecraft:savanna_plateau", depth: 1.5, scale: 0.02500000037252903 },
  { id: 37, key: "minecraft:badlands", depth: 0.10000000149011612, scale: 0.20000000298023224 },
  { id: 38, key: "minecraft:wooded_badlands_plateau", depth: 1.5, scale: 0.02500000037252903 },
  { id: 39, key: "minecraft:badlands_plateau", depth: 1.5, scale: 0.02500000037252903 },
  { id: 44, key: "minecraft:warm_ocean", depth: -1, scale: 0.10000000149011612 },
  { id: 45, key: "minecraft:lukewarm_ocean", depth: -1, scale: 0.10000000149011612 },
  { id: 46, key: "minecraft:cold_ocean", depth: -1, scale: 0.10000000149011612 },
  { id: 47, key: "minecraft:deep_warm_ocean", depth: -1.7999999523162842, scale: 0.10000000149011612 },
  { id: 48, key: "minecraft:deep_lukewarm_ocean", depth: -1.7999999523162842, scale: 0.10000000149011612 },
  { id: 49, key: "minecraft:deep_cold_ocean", depth: -1.7999999523162842, scale: 0.10000000149011612 },
  { id: 50, key: "minecraft:deep_frozen_ocean", depth: -1.7999999523162842, scale: 0.10000000149011612 },
  { id: 129, key: "minecraft:sunflower_plains", depth: 0.125, scale: 0.05000000074505806 },
  { id: 130, key: "minecraft:desert_lakes", depth: 0.22499999403953552, scale: 0.25 },
  { id: 131, key: "minecraft:gravelly_mountains", depth: 1, scale: 0.5 },
  { id: 132, key: "minecraft:flower_forest", depth: 0.10000000149011612, scale: 0.4000000059604645 },
  { id: 133, key: "minecraft:taiga_mountains", depth: 0.30000001192092896, scale: 0.4000000059604645 },
  { id: 134, key: "minecraft:swamp_hills", depth: -0.10000000149011612, scale: 0.30000001192092896 },
  { id: 140, key: "minecraft:ice_spikes", depth: 0.42500001192092896, scale: 0.45000001788139343 },
  { id: 149, key: "minecraft:modified_jungle", depth: 0.20000000298023224, scale: 0.4000000059604645 },
  { id: 151, key: "minecraft:modified_jungle_edge", depth: 0.20000000298023224, scale: 0.4000000059604645 },
  { id: 155, key: "minecraft:tall_birch_forest", depth: 0.20000000298023224, scale: 0.4000000059604645 },
  { id: 156, key: "minecraft:tall_birch_hills", depth: 0.550000011920929, scale: 0.5 },
  { id: 157, key: "minecraft:dark_forest_hills", depth: 0.20000000298023224, scale: 0.4000000059604645 },
  { id: 158, key: "minecraft:snowy_taiga_mountains", depth: 0.30000001192092896, scale: 0.4000000059604645 },
  { id: 160, key: "minecraft:giant_spruce_taiga", depth: 0.20000000298023224, scale: 0.20000000298023224 },
  { id: 161, key: "minecraft:giant_spruce_taiga_hills", depth: 0.20000000298023224, scale: 0.20000000298023224 },
  { id: 162, key: "minecraft:modified_gravelly_mountains", depth: 1, scale: 0.5 },
  { id: 163, key: "minecraft:shattered_savanna", depth: 0.36250001192092896, scale: 1.225000023841858 },
  { id: 164, key: "minecraft:shattered_savanna_plateau", depth: 1.0499999523162842, scale: 1.2125000953674316 },
  { id: 165, key: "minecraft:eroded_badlands", depth: 0.10000000149011612, scale: 0.20000000298023224 },
  { id: 166, key: "minecraft:modified_wooded_badlands_plateau", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 167, key: "minecraft:modified_badlands_plateau", depth: 0.44999998807907104, scale: 0.30000001192092896 },
  { id: 168, key: "minecraft:bamboo_jungle", depth: 0.10000000149011612, scale: 0.20000000298023224 },
  { id: 169, key: "minecraft:bamboo_jungle_hills", depth: 0.44999998807907104, scale: 0.30000001192092896 },
] as const satisfies readonly BiomeDefinition[];

const ALL_OVERWORLD_LAYERED_BIOMES = ALL_OVERWORLD_LAYERED_BIOME_DEFINITIONS.map(
  (definition) => new Biome(definition.id, definition.key, definition.depth, definition.scale),
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
