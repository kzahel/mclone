import { GenerationStep } from "../levelgen/generation-step";
import { BiomeGenerationSettings } from "./biome-generation-settings";
import { VegetationFeatures } from "../levelgen/feature/vegetation-features";
import { WaterFeatures } from "../levelgen/feature/water-features";

function addDefaultLakes(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.LAKES, () => WaterFeatures.LAKE_WATER);
}

function addDefaultSprings(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => WaterFeatures.SPRING_WATER);
}

function buildMountainSettings(edge: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultLakes(builder);
  builder.addFeature(
    GenerationStep.Decoration.VEGETAL_DECORATION,
    () => (edge ? VegetationFeatures.TREES_MOUNTAIN_EDGE : VegetationFeatures.TREES_MOUNTAIN),
  );
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_BADLANDS);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  addDefaultSprings(builder);
  return builder.build();
}

function buildTaigaSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultLakes(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_LARGE_FERN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TAIGA_VEGETATION);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_TAIGA_2);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_TAIGA);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_TAIGA);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_BERRY_SPARSE);
  addDefaultSprings(builder);
  return builder.build();
}

function buildDesertSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultLakes(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE_DESERT);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_CACTUS_DESERT);
  addDefaultSprings(builder);
  return builder.build();
}

function buildBadlandsSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultLakes(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_BADLANDS);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE_BADLANDS);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_CACTUS_DECORATED);
  addDefaultSprings(builder);
  return builder.build();
}

function buildForestSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultLakes(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FOREST_FLOWER_VEGETATION);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_FOREST);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  addDefaultSprings(builder);
  return builder.build();
}

function buildPlainsSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultLakes(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PLAIN_VEGETATION);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_PLAIN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_TALL_GRASS_2);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  addDefaultSprings(builder);
  return builder.build();
}

function buildSwampSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultLakes(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TREES_SWAMP);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FLOWER_SWAMP);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_WATERLILLY);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_SWAMP);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_SWAMP);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE_SWAMP);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  addDefaultSprings(builder);
  return builder.build();
}

const SETTINGS_BY_KEY = new Map<string, BiomeGenerationSettings>([
  ["minecraft:badlands", buildBadlandsSettings()],
  ["minecraft:badlands_plateau", buildBadlandsSettings()],
  ["minecraft:desert", buildDesertSettings()],
  ["minecraft:desert_hills", buildDesertSettings()],
  ["minecraft:desert_lakes", buildDesertSettings()],
  ["minecraft:forest", buildForestSettings()],
  ["minecraft:flower_forest", buildForestSettings()],
  ["minecraft:mountains", buildMountainSettings(false)],
  ["minecraft:wooded_mountains", buildMountainSettings(false)],
  ["minecraft:mountain_edge", buildMountainSettings(true)],
  ["minecraft:plains", buildPlainsSettings()],
  ["minecraft:sunflower_plains", buildPlainsSettings()],
  ["minecraft:swamp", buildSwampSettings()],
  ["minecraft:swamp_hills", buildSwampSettings()],
  ["minecraft:taiga", buildTaigaSettings()],
  ["minecraft:taiga_hills", buildTaigaSettings()],
  ["minecraft:taiga_mountains", buildTaigaSettings()],
  ["minecraft:wooded_hills", buildForestSettings()],
]);

export function getOverworldBiomeGenerationSettings(key: string): BiomeGenerationSettings {
  return SETTINGS_BY_KEY.get(key) ?? BiomeGenerationSettings.EMPTY;
}
