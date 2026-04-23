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
  return builder.build();
}

function buildTaigaSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TAIGA_VEGETATION);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_TAIGA_2);
  return builder.build();
}

const SETTINGS_BY_KEY = new Map<string, BiomeGenerationSettings>([
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
