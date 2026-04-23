import { GenerationStep } from "../levelgen/generation-step";
import { BiomeGenerationSettings } from "./biome-generation-settings";
import { VegetationFeatures } from "../levelgen/feature/vegetation-features";

function buildMountainSettings(edge: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  builder.addFeature(
    GenerationStep.Decoration.VEGETAL_DECORATION,
    () => (edge ? VegetationFeatures.TREES_MOUNTAIN_EDGE : VegetationFeatures.TREES_MOUNTAIN),
  );
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_BADLANDS);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  return builder.build();
}

function buildTaigaSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
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
  return builder.build();
}

function buildDesertSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE_DESERT);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_CACTUS_DESERT);
  return builder.build();
}

function buildBadlandsSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_BADLANDS);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE_BADLANDS);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_CACTUS_DECORATED);
  return builder.build();
}

const SETTINGS_BY_KEY = new Map<string, BiomeGenerationSettings>([
  ["minecraft:badlands", buildBadlandsSettings()],
  ["minecraft:badlands_plateau", buildBadlandsSettings()],
  ["minecraft:desert", buildDesertSettings()],
  ["minecraft:desert_hills", buildDesertSettings()],
  ["minecraft:desert_lakes", buildDesertSettings()],
  ["minecraft:mountains", buildMountainSettings(false)],
  ["minecraft:wooded_mountains", buildMountainSettings(false)],
  ["minecraft:mountain_edge", buildMountainSettings(true)],
  ["minecraft:taiga", buildTaigaSettings()],
  ["minecraft:taiga_hills", buildTaigaSettings()],
  ["minecraft:taiga_mountains", buildTaigaSettings()],
]);

export function getOverworldBiomeGenerationSettings(key: string): BiomeGenerationSettings {
  return SETTINGS_BY_KEY.get(key) ?? BiomeGenerationSettings.EMPTY;
}
