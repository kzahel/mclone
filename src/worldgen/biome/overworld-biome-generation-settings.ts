import { GenerationStep } from "../levelgen/generation-step";
import {
  DEFAULT_OVERWORLD_AIR_CARVERS,
  OCEAN_OVERWORLD_AIR_CARVERS,
  OCEAN_OVERWORLD_LIQUID_CARVERS,
} from "../carver/overworld-configured-carvers";
import { BiomeGenerationSettings } from "./biome-generation-settings";
import { VegetationFeatures } from "../levelgen/feature/vegetation-features";
import { WaterFeatures } from "../levelgen/feature/water-features";

const OCEAN_BIOME_KEYS = new Set([
  "minecraft:ocean",
  "minecraft:deep_ocean",
  "minecraft:warm_ocean",
  "minecraft:deep_warm_ocean",
  "minecraft:lukewarm_ocean",
  "minecraft:deep_lukewarm_ocean",
  "minecraft:cold_ocean",
  "minecraft:deep_cold_ocean",
  "minecraft:frozen_ocean",
  "minecraft:deep_frozen_ocean",
]);

function addDefaultCarvers(builder: BiomeGenerationSettings.Builder): void {
  for (const carver of DEFAULT_OVERWORLD_AIR_CARVERS) {
    builder.addCarver(GenerationStep.Carving.AIR, carver);
  }
}

function addOceanCarvers(builder: BiomeGenerationSettings.Builder): void {
  for (const carver of OCEAN_OVERWORLD_AIR_CARVERS) {
    builder.addCarver(GenerationStep.Carving.AIR, carver);
  }

  for (const carver of OCEAN_OVERWORLD_LIQUID_CARVERS) {
    builder.addCarver(GenerationStep.Carving.LIQUID, carver);
  }
}

function addDefaultLakes(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.LAKES, () => WaterFeatures.LAKE_WATER);
}

function addDefaultSprings(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => WaterFeatures.SPRING_WATER);
}

function addForestFlowers(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FOREST_FLOWER_VEGETATION);
}

function addForestGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_FOREST);
}

function addPlainGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_TALL_GRASS_2);
}

function addDefaultFlowers(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FLOWER_DEFAULT);
}

function addDefaultMushrooms(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_NORMAL);
}

function addDefaultExtraVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
}

function addBadlandGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_BADLANDS);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_DEAD_BUSH_BADLANDS);
}

function addBadlandExtraVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE_BADLANDS);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_CACTUS_DECORATED);
}

function addDesertVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_DEAD_BUSH_2);
}

function addDesertExtraVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE_DESERT);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_CACTUS_DESERT);
}

function addSwampVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TREES_SWAMP);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FLOWER_SWAMP);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_DEAD_BUSH);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_WATERLILLY);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_SWAMP);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_SWAMP);
}

function addSwampExtraVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE_SWAMP);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
}

function addPlainVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PLAIN_VEGETATION);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FLOWER_PLAIN_DECORATED);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_PLAIN);
}

function buildMountainSettings(edge: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  builder.addFeature(
    GenerationStep.Decoration.VEGETAL_DECORATION,
    () => (edge ? VegetationFeatures.TREES_MOUNTAIN_EDGE : VegetationFeatures.TREES_MOUNTAIN),
  );
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_BADLANDS);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  return builder.build();
}

function buildTaigaSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_LARGE_FERN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TAIGA_VEGETATION);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_TAIGA_2);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_TAIGA);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_TAIGA);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_BERRY_SPARSE);
  addDefaultSprings(builder);
  return builder.build();
}

function buildDesertSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultMushrooms(builder);
  addDesertVegetation(builder);
  addDesertExtraVegetation(builder);
  addDefaultSprings(builder);
  return builder.build();
}

function buildBadlandsSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addBadlandGrass(builder);
  addBadlandExtraVegetation(builder);
  addDefaultSprings(builder);
  return builder.build();
}

function buildForestSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addForestFlowers(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BIRCH_OTHER);
  addDefaultFlowers(builder);
  addForestGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  return builder.build();
}

function buildFlowerForestSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FOREST_FLOWER_VEGETATION_COMMON);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FOREST_FLOWER_TREES);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FLOWER_FOREST);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_NORMAL);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  return builder.build();
}

function buildDarkForestSettings(redMushroomBias: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  builder.addFeature(
    GenerationStep.Decoration.VEGETAL_DECORATION,
    () => (redMushroomBias ? VegetationFeatures.DARK_FOREST_VEGETATION_RED : VegetationFeatures.DARK_FOREST_VEGETATION_BROWN),
  );
  addForestFlowers(builder);
  addDefaultFlowers(builder);
  addForestGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  return builder.build();
}

function buildBirchForestSettings(tall: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addForestFlowers(builder);
  builder.addFeature(
    GenerationStep.Decoration.VEGETAL_DECORATION,
    () => (tall ? VegetationFeatures.BIRCH_TALL : VegetationFeatures.TREES_BIRCH),
  );
  addDefaultFlowers(builder);
  addForestGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  return builder.build();
}

function buildPlainsSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addPlainGrass(builder);
  addPlainVegetation(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  return builder.build();
}

function buildSwampSettings(swampHills: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addSwampVegetation(builder);
  addDefaultMushrooms(builder);
  addSwampExtraVegetation(builder);
  addDefaultSprings(builder);
  if (!swampHills) {
    builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.SEAGRASS_SWAMP);
  }
  return builder.build();
}

const SETTINGS_BY_KEY = new Map<string, BiomeGenerationSettings>([
  ["minecraft:badlands", buildBadlandsSettings()],
  ["minecraft:badlands_plateau", buildBadlandsSettings()],
  ["minecraft:birch_forest", buildBirchForestSettings(false)],
  ["minecraft:birch_forest_hills", buildBirchForestSettings(false)],
  ["minecraft:desert", buildDesertSettings()],
  ["minecraft:desert_hills", buildDesertSettings()],
  ["minecraft:desert_lakes", buildDesertSettings()],
  ["minecraft:dark_forest", buildDarkForestSettings(false)],
  ["minecraft:dark_forest_hills", buildDarkForestSettings(true)],
  ["minecraft:forest", buildForestSettings()],
  ["minecraft:flower_forest", buildFlowerForestSettings()],
  ["minecraft:mountains", buildMountainSettings(false)],
  ["minecraft:wooded_mountains", buildMountainSettings(false)],
  ["minecraft:mountain_edge", buildMountainSettings(true)],
  ["minecraft:plains", buildPlainsSettings()],
  ["minecraft:sunflower_plains", buildPlainsSettings()],
  ["minecraft:swamp", buildSwampSettings(false)],
  ["minecraft:swamp_hills", buildSwampSettings(true)],
  ["minecraft:tall_birch_forest", buildBirchForestSettings(true)],
  ["minecraft:tall_birch_hills", buildBirchForestSettings(true)],
  ["minecraft:taiga", buildTaigaSettings()],
  ["minecraft:taiga_hills", buildTaigaSettings()],
  ["minecraft:taiga_mountains", buildTaigaSettings()],
  ["minecraft:wooded_hills", buildForestSettings()],
]);

const DEFAULT_CARVER_ONLY_SETTINGS = (() => {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  return builder.build();
})();

const OCEAN_CARVER_ONLY_SETTINGS = (() => {
  const builder = new BiomeGenerationSettings.Builder();
  addOceanCarvers(builder);
  return builder.build();
})();

export function getOverworldBiomeGenerationSettings(key: string): BiomeGenerationSettings {
  return SETTINGS_BY_KEY.get(key) ?? (OCEAN_BIOME_KEYS.has(key) ? OCEAN_CARVER_ONLY_SETTINGS : DEFAULT_CARVER_ONLY_SETTINGS);
}
