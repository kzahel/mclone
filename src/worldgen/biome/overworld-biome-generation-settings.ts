import { GenerationStep } from "../levelgen/generation-step";
import {
  DEFAULT_OVERWORLD_AIR_CARVERS,
  OCEAN_OVERWORLD_AIR_CARVERS,
  OCEAN_OVERWORLD_LIQUID_CARVERS,
} from "../carver/overworld-configured-carvers";
import { BiomeGenerationSettings } from "./biome-generation-settings";
import { VegetationFeatures } from "../levelgen/feature/vegetation-features";
import { OreFeatures } from "../levelgen/feature/ore-features";
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

function addDefaultUndergroundVariety(builder: BiomeGenerationSettings.Builder, skipGlowLichen = false): void {
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_DIRT);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_GRAVEL);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_GRANITE);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_DIORITE);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_ANDESITE);
  if (!skipGlowLichen) {
    builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => OreFeatures.GLOW_LICHEN);
  }
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_TUFF);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_DEEPSLATE);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_DECORATION, () => OreFeatures.RARE_DRIPSTONE_CLUSTER_FEATURE);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_DECORATION, () => OreFeatures.RARE_SMALL_DRIPSTONE_FEATURE);
}

function addDefaultOres(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_COAL);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_IRON);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_GOLD);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_REDSTONE);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_DIAMOND);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_LAPIS);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_COPPER);
}

function addExtraGold(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_GOLD_EXTRA);
}

function addExtraEmeralds(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.ORE_EMERALD);
}

function addInfestedStone(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_DECORATION, () => OreFeatures.ORE_INFESTED);
}

function addDefaultSoftDisks(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.DISK_SAND);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.DISK_CLAY);
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.DISK_GRAVEL);
}

function addSwampClayDisk(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.UNDERGROUND_ORES, () => OreFeatures.DISK_CLAY);
}

function addDefaultSprings(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => WaterFeatures.SPRING_WATER);
}

function addSurfaceFreezing(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.TOP_LAYER_MODIFICATION, () => VegetationFeatures.FREEZE_TOP_LAYER);
}

function addWaterTrees(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TREES_WATER);
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

function addWarmFlowers(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FLOWER_WARM);
}

function addDefaultGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_BADLANDS);
}

function addDefaultMushrooms(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_NORMAL);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_NORMAL);
}

function addDefaultExtraVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_SUGAR_CANE);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_PUMPKIN);
}

function addColdOceanExtraVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.KELP_COLD);
}

function addLukeWarmKelp(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.KELP_WARM);
}

function addWarmOceanVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.WARM_OCEAN_VEGETATION);
}

function addSeaPickles(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.SEA_PICKLE);
}

function addBadlandGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_BADLANDS);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_DEAD_BUSH_BADLANDS);
}

function addBadlandsTrees(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TREES_BADLANDS);
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

function addSavannaTrees(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TREES_SAVANNA);
}

function addShatteredSavannaTrees(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TREES_SHATTERED_SAVANNA);
}

function addSavannaGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_TALL_GRASS);
}

function addShatteredSavannaGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_NORMAL);
}

function addSavannaExtraGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_SAVANNA);
}

function addSnowyTrees(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TREES_SNOWY);
}

function addTaigaTrees(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.TAIGA_VEGETATION);
}

function addTaigaGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_TAIGA_2);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_TAIGA);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_TAIGA);
}

function addBerryBushes(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_BERRY_DECORATED);
}

function addSparseBerryBushes(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_BERRY_SPARSE);
}

function addGiantTaigaTrees(builder: BiomeGenerationSettings.Builder, giantSpruce: boolean): void {
  builder.addFeature(
    GenerationStep.Decoration.VEGETAL_DECORATION,
    () => (giantSpruce ? VegetationFeatures.TREES_GIANT_SPRUCE : VegetationFeatures.TREES_GIANT),
  );
}

function addGiantTaigaVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_TAIGA);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_DEAD_BUSH);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_GIANT);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_GIANT);
}

function addJungleTrees(builder: BiomeGenerationSettings.Builder, edge: boolean): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => (edge ? VegetationFeatures.TREES_JUNGLE_EDGE : VegetationFeatures.TREES_JUNGLE));
}

function addLightBambooVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BAMBOO_LIGHT);
}

function addBambooVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BAMBOO);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BAMBOO_VEGETATION);
}

function addJungleGrass(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_JUNGLE);
}

function addJungleExtraVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_MELON);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.VINES);
}

function addMushroomFieldVegetation(builder: BiomeGenerationSettings.Builder): void {
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.MUSHROOM_FIELD_VEGETATION);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BROWN_MUSHROOM_TAIGA);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.RED_MUSHROOM_TAIGA);
}

function buildMountainSettings(edge: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  builder.addFeature(
    GenerationStep.Decoration.VEGETAL_DECORATION,
    () => (edge ? VegetationFeatures.TREES_MOUNTAIN_EDGE : VegetationFeatures.TREES_MOUNTAIN),
  );
  addDefaultFlowers(builder);
  addDefaultGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addExtraEmeralds(builder);
  addInfestedStone(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildTaigaSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_LARGE_FERN);
  addTaigaTrees(builder);
  addTaigaGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addSparseBerryBushes(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildSnowyTundraSettings(iceSpikes = false): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  if (iceSpikes) {
    builder.addFeature(GenerationStep.Decoration.SURFACE_STRUCTURES, () => VegetationFeatures.ICE_SPIKE);
    builder.addFeature(GenerationStep.Decoration.SURFACE_STRUCTURES, () => VegetationFeatures.ICE_PATCH);
  }
  addSnowyTrees(builder);
  addDefaultFlowers(builder);
  addDefaultGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildSnowyTaigaSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_LARGE_FERN);
  addTaigaTrees(builder);
  addDefaultFlowers(builder);
  addTaigaGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  addBerryBushes(builder);
  return builder.build();
}

function buildGiantTaigaSettings(giantSpruce: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_LARGE_FERN);
  addGiantTaigaTrees(builder, giantSpruce);
  addDefaultFlowers(builder);
  addGiantTaigaVegetation(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addSparseBerryBushes(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildMushroomFieldSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  addMushroomFieldVegetation(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildDesertSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  addDefaultMushrooms(builder);
  addDesertVegetation(builder);
  addDesertExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildBadlandsSettings(wooded = false): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addExtraGold(builder);
  addDefaultSoftDisks(builder);
  if (wooded) {
    addBadlandsTrees(builder);
  }
  addBadlandGrass(builder);
  addDefaultMushrooms(builder);
  addBadlandExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildForestSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  addForestFlowers(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.BIRCH_OTHER);
  addDefaultFlowers(builder);
  addForestGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildFlowerForestSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FOREST_FLOWER_VEGETATION_COMMON);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FOREST_FLOWER_TREES);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.FLOWER_FOREST);
  builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.PATCH_GRASS_NORMAL);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildDarkForestSettings(redMushroomBias: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
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
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildBirchForestSettings(tall: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
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
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildPlainsSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  addPlainGrass(builder);
  addPlainVegetation(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildSwampSettings(swampHills: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addSwampClayDisk(builder);
  addSwampVegetation(builder);
  addDefaultMushrooms(builder);
  addSwampExtraVegetation(builder);
  addDefaultSprings(builder);
  if (!swampHills) {
    builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.SEAGRASS_SWAMP);
  }
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildSavannaSettings(shattered: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  if (!shattered) {
    addSavannaGrass(builder);
  }
  if (shattered) {
    addShatteredSavannaTrees(builder);
    addDefaultFlowers(builder);
    addShatteredSavannaGrass(builder);
  } else {
    addSavannaTrees(builder);
    addWarmFlowers(builder);
    addSavannaExtraGrass(builder);
  }
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildJungleSettings(edge: boolean, bamboo = false, modified = false): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  if (bamboo) {
    addBambooVegetation(builder);
  } else {
    if (!edge && !modified) {
      addLightBambooVegetation(builder);
    }

    addJungleTrees(builder, edge);
  }
  addWarmFlowers(builder);
  addJungleGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addJungleExtraVegetation(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildBeachSettings(): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  addDefaultFlowers(builder);
  addDefaultGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  addSurfaceFreezing(builder);
  return builder.build();
}

function buildRiverSettings(frozen: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addDefaultCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  addWaterTrees(builder);
  addDefaultFlowers(builder);
  addDefaultGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  if (!frozen) {
    builder.addFeature(GenerationStep.Decoration.VEGETAL_DECORATION, () => VegetationFeatures.SEAGRASS_RIVER);
  }
  addSurfaceFreezing(builder);
  return builder.build();
}

type OceanSettingsKind = "cold" | "normal" | "lukewarm" | "warm" | "frozen";

function buildOceanSettings(kind: OceanSettingsKind, deep: boolean): BiomeGenerationSettings {
  const builder = new BiomeGenerationSettings.Builder();
  addOceanCarvers(builder);
  addDefaultLakes(builder);
  addDefaultUndergroundVariety(builder, true);
  addDefaultOres(builder);
  addDefaultSoftDisks(builder);
  addWaterTrees(builder);
  addDefaultFlowers(builder);
  addDefaultGrass(builder);
  addDefaultMushrooms(builder);
  addDefaultExtraVegetation(builder);
  addDefaultSprings(builder);
  switch (kind) {
    case "cold":
      builder.addFeature(
        GenerationStep.Decoration.VEGETAL_DECORATION,
        () => (deep ? VegetationFeatures.SEAGRASS_DEEP_COLD : VegetationFeatures.SEAGRASS_COLD),
      );
      addColdOceanExtraVegetation(builder);
      break;
    case "normal":
      builder.addFeature(
        GenerationStep.Decoration.VEGETAL_DECORATION,
        () => (deep ? VegetationFeatures.SEAGRASS_DEEP : VegetationFeatures.SEAGRASS_NORMAL),
      );
      addColdOceanExtraVegetation(builder);
      break;
    case "lukewarm":
      builder.addFeature(
        GenerationStep.Decoration.VEGETAL_DECORATION,
        () => (deep ? VegetationFeatures.SEAGRASS_DEEP_WARM : VegetationFeatures.SEAGRASS_WARM),
      );
      addLukeWarmKelp(builder);
      break;
    case "warm":
      builder.addFeature(
        GenerationStep.Decoration.VEGETAL_DECORATION,
        () => (deep ? VegetationFeatures.SEAGRASS_DEEP_WARM : VegetationFeatures.SEAGRASS_WARM),
      );
      if (!deep) {
        addWarmOceanVegetation(builder);
        addSeaPickles(builder);
      }
      break;
    case "frozen":
      break;
  }
  addSurfaceFreezing(builder);
  return builder.build();
}

const SETTINGS_BY_KEY = new Map<string, BiomeGenerationSettings>([
  ["minecraft:badlands", buildBadlandsSettings()],
  ["minecraft:badlands_plateau", buildBadlandsSettings()],
  ["minecraft:beach", buildBeachSettings()],
  ["minecraft:birch_forest", buildBirchForestSettings(false)],
  ["minecraft:birch_forest_hills", buildBirchForestSettings(false)],
  ["minecraft:cold_ocean", buildOceanSettings("cold", false)],
  ["minecraft:desert", buildDesertSettings()],
  ["minecraft:desert_hills", buildDesertSettings()],
  ["minecraft:desert_lakes", buildDesertSettings()],
  ["minecraft:dark_forest", buildDarkForestSettings(false)],
  ["minecraft:dark_forest_hills", buildDarkForestSettings(true)],
  ["minecraft:deep_cold_ocean", buildOceanSettings("cold", true)],
  ["minecraft:deep_frozen_ocean", buildOceanSettings("frozen", true)],
  ["minecraft:deep_warm_ocean", buildOceanSettings("warm", true)],
  ["minecraft:deep_lukewarm_ocean", buildOceanSettings("lukewarm", true)],
  ["minecraft:deep_ocean", buildOceanSettings("normal", true)],
  ["minecraft:forest", buildForestSettings()],
  ["minecraft:flower_forest", buildFlowerForestSettings()],
  ["minecraft:frozen_ocean", buildOceanSettings("frozen", false)],
  ["minecraft:frozen_river", buildRiverSettings(true)],
  ["minecraft:bamboo_jungle", buildJungleSettings(false, true)],
  ["minecraft:bamboo_jungle_hills", buildJungleSettings(false, true)],
  ["minecraft:eroded_badlands", buildBadlandsSettings()],
  ["minecraft:giant_spruce_taiga", buildGiantTaigaSettings(true)],
  ["minecraft:giant_spruce_taiga_hills", buildGiantTaigaSettings(true)],
  ["minecraft:giant_tree_taiga", buildGiantTaigaSettings(false)],
  ["minecraft:giant_tree_taiga_hills", buildGiantTaigaSettings(false)],
  ["minecraft:gravelly_mountains", buildMountainSettings(false)],
  ["minecraft:ice_spikes", buildSnowyTundraSettings(true)],
  ["minecraft:lukewarm_ocean", buildOceanSettings("lukewarm", false)],
  ["minecraft:mountains", buildMountainSettings(false)],
  ["minecraft:jungle", buildJungleSettings(false)],
  ["minecraft:jungle_edge", buildJungleSettings(true)],
  ["minecraft:jungle_hills", buildJungleSettings(false)],
  ["minecraft:modified_badlands_plateau", buildBadlandsSettings()],
  ["minecraft:modified_gravelly_mountains", buildMountainSettings(false)],
  ["minecraft:modified_jungle", buildJungleSettings(false, false, true)],
  ["minecraft:modified_jungle_edge", buildJungleSettings(true, false, true)],
  ["minecraft:modified_wooded_badlands_plateau", buildBadlandsSettings(true)],
  ["minecraft:mushroom_fields", buildMushroomFieldSettings()],
  ["minecraft:mushroom_field_shore", buildMushroomFieldSettings()],
  ["minecraft:ocean", buildOceanSettings("normal", false)],
  ["minecraft:warm_ocean", buildOceanSettings("warm", false)],
  ["minecraft:wooded_badlands_plateau", buildBadlandsSettings(true)],
  ["minecraft:wooded_mountains", buildMountainSettings(true)],
  ["minecraft:mountain_edge", buildMountainSettings(true)],
  ["minecraft:plains", buildPlainsSettings()],
  ["minecraft:river", buildRiverSettings(false)],
  ["minecraft:savanna", buildSavannaSettings(false)],
  ["minecraft:savanna_plateau", buildSavannaSettings(false)],
  ["minecraft:shattered_savanna", buildSavannaSettings(true)],
  ["minecraft:shattered_savanna_plateau", buildSavannaSettings(true)],
  ["minecraft:snowy_mountains", buildSnowyTundraSettings()],
  ["minecraft:snowy_beach", buildBeachSettings()],
  ["minecraft:snowy_taiga", buildSnowyTaigaSettings()],
  ["minecraft:snowy_taiga_hills", buildSnowyTaigaSettings()],
  ["minecraft:snowy_taiga_mountains", buildSnowyTaigaSettings()],
  ["minecraft:snowy_tundra", buildSnowyTundraSettings()],
  ["minecraft:stone_shore", buildBeachSettings()],
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
