import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import type { Block } from "../../../../src/world/level/block/block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { getOverworldBiomeGenerationSettings } from "../../../../src/worldgen/biome/overworld-biome-generation-settings";
import { GenerationStep } from "../../../../src/worldgen/levelgen/generation-step";
import { ConfiguredFeature } from "../../../../src/worldgen/levelgen/feature/configured-feature";
import { CountConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/count-configuration";
import { CarvingMaskDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/carving-mask-decorator-configuration";
import { ChanceDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/chance-decorator-configuration";
import { DecoratedDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-decorator-configuration";
import { DecoratedFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-feature-configuration";
import type { DecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorator-configuration";
import { FrequencyWithExtraChanceDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/frequency-with-extra-chance-decorator-configuration";
import { ProbabilityFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/probability-feature-configuration";
import { RandomPatchConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/random-patch-configuration";
import { RandomFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/random-feature-configuration";
import { SimpleBlockConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/simple-block-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { SimpleStateProvider } from "../../../../src/worldgen/levelgen/feature/stateproviders/simple-state-provider";
import { TreeFeatures } from "../../../../src/worldgen/levelgen/feature/tree-features";
import { BeehiveDecorator } from "../../../../src/worldgen/levelgen/feature/treedecorators/beehive-decorator";
import { LeaveVineDecorator } from "../../../../src/worldgen/levelgen/feature/treedecorators/leave-vine-decorator";
import { VegetationFeatures } from "../../../../src/worldgen/levelgen/feature/vegetation-features";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { WorldgenRandom } from "../../../../src/worldgen/prng/worldgen-random";
import { ConstantInt } from "../../../../src/util/valueproviders/constant-int";

const PLAIN_FLOWER_LOCATIONS = new Set([
  "minecraft:orange_tulip",
  "minecraft:red_tulip",
  "minecraft:pink_tulip",
  "minecraft:white_tulip",
  "minecraft:poppy",
  "minecraft:azure_bluet",
  "minecraft:oxeye_daisy",
  "minecraft:cornflower",
  "minecraft:dandelion",
]);

const FOREST_FLOWER_LOCATIONS = new Set([
  "minecraft:dandelion",
  "minecraft:poppy",
  "minecraft:allium",
  "minecraft:azure_bluet",
  "minecraft:red_tulip",
  "minecraft:orange_tulip",
  "minecraft:white_tulip",
  "minecraft:pink_tulip",
  "minecraft:oxeye_daisy",
  "minecraft:cornflower",
  "minecraft:lily_of_the_valley",
]);

const DARK_FOREST_OUTPUT_LOCATIONS = new Set([
  "minecraft:oak_log",
  "minecraft:oak_leaves",
  "minecraft:birch_log",
  "minecraft:birch_leaves",
  "minecraft:dark_oak_log",
  "minecraft:dark_oak_leaves",
  "minecraft:brown_mushroom_block",
  "minecraft:red_mushroom_block",
  "minecraft:mushroom_stem",
]);

const SAVANNA_OUTPUT_LOCATIONS = new Set([
  "minecraft:oak_log",
  "minecraft:oak_leaves",
  "minecraft:acacia_log",
  "minecraft:acacia_leaves",
]);

const BADLANDS_TREE_OUTPUT_LOCATIONS = new Set([
  "minecraft:dirt",
  "minecraft:oak_log",
  "minecraft:oak_leaves",
]);

const JUNGLE_TREE_OUTPUT_LOCATIONS = new Set([
  "minecraft:oak_log",
  "minecraft:oak_leaves",
  "minecraft:jungle_log",
  "minecraft:jungle_leaves",
  "minecraft:vine",
  "minecraft:cocoa",
]);

const BAMBOO_VEGETATION_OUTPUT_LOCATIONS = new Set([
  "minecraft:oak_log",
  "minecraft:oak_leaves",
  "minecraft:jungle_log",
  "minecraft:jungle_leaves",
  "minecraft:vine",
  "minecraft:cocoa",
  "minecraft:grass",
  "minecraft:fern",
]);

const SNOWY_TREE_OUTPUT_LOCATIONS = new Set([
  "minecraft:spruce_log",
  "minecraft:spruce_leaves",
]);

const GIANT_TAIGA_OUTPUT_LOCATIONS = new Set([
  "minecraft:dirt",
  "minecraft:spruce_log",
  "minecraft:spruce_leaves",
  "minecraft:podzol",
]);

const MUSHROOM_FIELD_OUTPUT_LOCATIONS = new Set([
  "minecraft:brown_mushroom_block",
  "minecraft:red_mushroom_block",
  "minecraft:mushroom_stem",
]);

const AQUATIC_OUTPUT_LOCATIONS = new Set([
  "minecraft:seagrass",
  "minecraft:tall_seagrass",
  "minecraft:kelp",
  "minecraft:kelp_plant",
]);

function getState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function createGenerator(): NoiseBasedChunkGenerator {
  const biomeSource = new OverworldBiomeSource(12345n);
  return new NoiseBasedChunkGenerator(biomeSource, 12345n);
}

function createFlatLevel(airState: BlockState, floorState: BlockState): StaticRenderLevel {
  const level = new StaticRenderLevel(airState, 15, 15, 0, 64);
  for (let z = 0; z < 48; z++) {
    for (let x = 0; x < 48; x++) {
      level.setBlock(new BlockPos(x, 10, z), floorState);
    }
  }

  return level;
}

function createWaterLevel(airState: BlockState, floorState: BlockState, waterState: BlockState): StaticRenderLevel {
  const level = createFlatLevel(airState, floorState);
  return fillWater(level, waterState, 11, 12);
}

function fillWater(level: StaticRenderLevel, waterState: BlockState, minY: number, maxY: number): StaticRenderLevel {
  for (let z = 0; z < 32; z++) {
    for (let x = 0; x < 32; x++) {
      for (let y = minY; y <= maxY; y++) {
        level.setBlock(new BlockPos(x, y, z), waterState);
      }
    }
  }

  return level;
}

function createDeepWaterLevel(airState: BlockState, floorState: BlockState, waterState: BlockState): StaticRenderLevel {
  return fillWater(createFlatLevel(airState, floorState), waterState, 11, 24);
}

function collectPlacedLocations(level: StaticRenderLevel, yMin: number, yMax: number): string[] {
  const locations: string[] = [];
  for (let z = 0; z < 32; z++) {
    for (let y = yMin; y <= yMax; y++) {
      for (let x = 0; x < 32; x++) {
        const location = level.getBlockState(new BlockPos(x, y, z)).getBlock().getLocation()?.toString();
        if (location !== undefined && location !== "minecraft:air" && location !== "minecraft:grass_block" && location !== "minecraft:sand" && location !== "minecraft:water") {
          locations.push(location);
        }
      }
    }
  }

  return locations;
}

function getBaseFeature(configuredFeature: { feature: unknown; config: unknown }): unknown {
  let current = configuredFeature;
  while (current.config instanceof DecoratedFeatureConfiguration) {
    current = current.config.feature() as { feature: unknown; config: unknown };
  }

  return current.feature;
}

function collectDecoratorConfigs(config: DecoratorConfiguration, target: DecoratorConfiguration[]): void {
  target.push(config);
  if (config instanceof DecoratedDecoratorConfiguration) {
    collectDecoratorConfigs(config.outer().config(), target);
    collectDecoratorConfigs(config.inner().config(), target);
  }
}

function unwrapConfiguredFeature(feature: ConfiguredFeature<any, any>): {
  readonly current: ConfiguredFeature<any, any>;
  readonly decoratorConfigs: readonly DecoratorConfiguration[];
} {
  const decoratorConfigs: DecoratorConfiguration[] = [];
  let current = feature;

  while (current.config instanceof DecoratedFeatureConfiguration) {
    collectDecoratorConfigs(current.config.decorator.config(), decoratorConfigs);
    current = current.config.feature();
  }

  return { current, decoratorConfigs };
}

function expectBeeDecoratedTreeFeature(feature: ConfiguredFeature<any, any>, probability: number): void {
  const treeConfig = feature.config as { readonly decorators: readonly unknown[] };
  expect(treeConfig.decorators).toHaveLength(1);
  expect(treeConfig.decorators[0]).toBeInstanceOf(BeehiveDecorator);
  expect((treeConfig.decorators[0] as { readonly probability: number }).probability).toBeCloseTo(probability);
}

function expectLeafVineDecoratedTreeFeature(feature: ConfiguredFeature<any, any>): void {
  const treeConfig = feature.config as { readonly decorators: readonly unknown[] };
  expect(treeConfig.decorators).toHaveLength(1);
  expect(treeConfig.decorators[0]).toBeInstanceOf(LeaveVineDecorator);
}

function getVegetalFeatures(key: string): ConfiguredFeature<any, any>[] {
  return (getOverworldBiomeGenerationSettings(key).features()[GenerationStep.Decoration.VEGETAL_DECORATION] ?? []).map((supplier) => supplier());
}

function getTreeExtraCount(key: string): number {
  const treeFeature = getVegetalFeatures(key).find((feature) => {
    const rootFeature = unwrapConfiguredFeature(feature).current.feature;
    return rootFeature === Features.RANDOM_SELECTOR || rootFeature === Features.TREE;
  });
  expect(treeFeature).toBeDefined();

  const countConfig = unwrapConfiguredFeature(treeFeature!).decoratorConfigs.find(
    (decoratorConfig): decoratorConfig is FrequencyWithExtraChanceDecoratorConfiguration =>
      decoratorConfig instanceof FrequencyWithExtraChanceDecoratorConfiguration,
  );
  expect(countConfig).toBeDefined();
  return countConfig!.count;
}

function isRandomPatchFeatureForState(feature: ConfiguredFeature<any, any>, location: string): boolean {
  const { current } = unwrapConfiguredFeature(feature);
  if (current.feature !== Features.RANDOM_PATCH || !(current.config instanceof RandomPatchConfiguration)) {
    return false;
  }

  if (!(current.config.stateProvider instanceof SimpleStateProvider)) {
    return false;
  }

  return current.config.stateProvider.getState(new WorldgenRandom(0n), BlockPos.ZERO).getBlock().getLocation()?.toString() === location;
}

function isSimpleBlockFeatureForState(feature: ConfiguredFeature<any, any>, location: string): boolean {
  const { current } = unwrapConfiguredFeature(feature);
  if (current.feature !== Features.SIMPLE_BLOCK || !(current.config instanceof SimpleBlockConfiguration)) {
    return false;
  }

  if (!(current.config.toPlace instanceof SimpleStateProvider)) {
    return false;
  }

  return current.config.toPlace.getState(new WorldgenRandom(0n), BlockPos.ZERO).getBlock().getLocation()?.toString() === location;
}

describe("Vegetation parity", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("FLOWER_PLAIN_DECORATED places only translated plain-flower states", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();

    expect(VegetationFeatures.FLOWER_PLAIN_DECORATED.place(level, generator, new WorldgenRandom(1234n), new BlockPos(0, 0, 0))).toBe(true);

    const placedLocations = collectPlacedLocations(level, 11, 11);
    expect(placedLocations.length).toBeGreaterThan(0);
    for (const location of placedLocations) {
      expect(PLAIN_FLOWER_LOCATIONS.has(location)).toBe(true);
    }
  });

  test("FLOWER_FOREST places only translated forest-flower-provider states", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();

    expect(VegetationFeatures.FLOWER_FOREST.place(level, generator, new WorldgenRandom(4321n), new BlockPos(0, 0, 0))).toBe(true);

    const placedLocations = collectPlacedLocations(level, 11, 11);
    expect(placedLocations.length).toBeGreaterThan(0);
    for (const location of placedLocations) {
      expect(FOREST_FLOWER_LOCATIONS.has(location)).toBe(true);
    }
  });

  test("PATCH_SUNFLOWER places only translated sunflower double-plant states", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();

    expect(VegetationFeatures.PATCH_SUNFLOWER.place(level, generator, new WorldgenRandom(2468n), new BlockPos(0, 0, 0))).toBe(true);

    const placedLocations = collectPlacedLocations(level, 11, 12);
    expect(placedLocations.length).toBeGreaterThan(0);
    expect(placedLocations.every((location) => location === "minecraft:sunflower")).toBe(true);
  });

  test("dead-bush and seagrass vegetation place onto translated desert and swamp surfaces", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();

    const desertLevel = createFlatLevel(blocks.airState, getState("minecraft:sand"));
    expect(VegetationFeatures.PATCH_DEAD_BUSH_BADLANDS.place(desertLevel, generator, new WorldgenRandom(99n), new BlockPos(8, 11, 8))).toBe(true);
    const desertPlacements = collectPlacedLocations(desertLevel, 11, 11);
    expect(desertPlacements).toContain("minecraft:dead_bush");

    const swampLevel = createWaterLevel(blocks.airState, getState("minecraft:stone"), getState("minecraft:water"));
    expect(VegetationFeatures.SEAGRASS_SWAMP.place(swampLevel, generator, new WorldgenRandom(7n), new BlockPos(8, 0, 8))).toBe(true);
    const swampPlacements = collectPlacedLocations(swampLevel, 11, 12);
    expect(swampPlacements.some((location) => location === "minecraft:seagrass" || location === "minecraft:tall_seagrass")).toBe(true);
  });

  test("river and ocean aquatic vegetation paths place translated seagrass and kelp blocks", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();

    const riverLevel = createWaterLevel(blocks.airState, getState("minecraft:stone"), getState("minecraft:water"));
    expect(VegetationFeatures.SEAGRASS_RIVER.place(riverLevel, generator, new WorldgenRandom(17n), new BlockPos(8, 0, 8))).toBe(true);
    const riverPlacements = collectPlacedLocations(riverLevel, 11, 16);
    expect(riverPlacements.length).toBeGreaterThan(0);
    expect(riverPlacements.every((location) => AQUATIC_OUTPUT_LOCATIONS.has(location))).toBe(true);

    let kelpColdLevel: StaticRenderLevel | undefined;
    for (let seed = 0n; seed < 64n; seed++) {
      const candidate = createWaterLevel(blocks.airState, getState("minecraft:stone"), getState("minecraft:water"));
      if (VegetationFeatures.KELP_COLD.place(candidate, generator, new WorldgenRandom(seed), new BlockPos(8, 0, 8))) {
        kelpColdLevel = candidate;
        break;
      }
    }
    expect(kelpColdLevel).toBeDefined();
    const kelpColdPlacements = collectPlacedLocations(kelpColdLevel!, 11, 24);
    expect(kelpColdPlacements.length).toBeGreaterThan(0);
    expect(kelpColdPlacements.every((location) => AQUATIC_OUTPUT_LOCATIONS.has(location))).toBe(true);
    expect(kelpColdPlacements.some((location) => location === "minecraft:kelp" || location === "minecraft:kelp_plant")).toBe(true);

    let kelpWarmLevel: StaticRenderLevel | undefined;
    for (let seed = 0n; seed < 64n; seed++) {
      const candidate = createWaterLevel(blocks.airState, getState("minecraft:stone"), getState("minecraft:water"));
      if (VegetationFeatures.KELP_WARM.place(candidate, generator, new WorldgenRandom(seed), new BlockPos(8, 0, 8))) {
        kelpWarmLevel = candidate;
        break;
      }
    }
    expect(kelpWarmLevel).toBeDefined();
    const kelpWarmPlacements = collectPlacedLocations(kelpWarmLevel!, 11, 24);
    expect(kelpWarmPlacements.length).toBeGreaterThan(0);
    expect(kelpWarmPlacements.every((location) => AQUATIC_OUTPUT_LOCATIONS.has(location))).toBe(true);
    expect(kelpWarmPlacements.some((location) => location === "minecraft:kelp" || location === "minecraft:kelp_plant")).toBe(true);
  });

  test("warm-ocean vegetation and sea-pickle paths place translated coral and sea-pickle blocks", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();

    let warmOceanLevel: StaticRenderLevel | undefined;
    for (let seed = 0n; seed < 256n; seed++) {
      const candidate = createDeepWaterLevel(blocks.airState, getState("minecraft:stone"), getState("minecraft:water"));
      if (VegetationFeatures.WARM_OCEAN_VEGETATION.place(candidate, generator, new WorldgenRandom(seed), new BlockPos(8, 0, 8))) {
        warmOceanLevel = candidate;
        break;
      }
    }
    expect(warmOceanLevel).toBeDefined();
    const warmOceanPlacements = collectPlacedLocations(warmOceanLevel!, 11, 24);
    expect(warmOceanPlacements.length).toBeGreaterThan(0);
    expect(warmOceanPlacements.every((location) => location.includes("coral") || location === "minecraft:sea_pickle")).toBe(true);

    const seaPickleLevel = createDeepWaterLevel(blocks.airState, getState("minecraft:stone"), getState("minecraft:water"));
    expect(
      Features.SEA_PICKLE
        .configured(new CountConfiguration(ConstantInt.of(20)))
        .place(seaPickleLevel, generator, new WorldgenRandom(12345n), new BlockPos(8, 0, 8)),
    ).toBe(true);
    expect(collectPlacedLocations(seaPickleLevel, 11, 24)).toContain("minecraft:sea_pickle");
  });

  test("huge mushroom and dark-forest vegetation paths place translated dark-oak and mushroom blocks", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();

    const brownMushroomLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    expect(VegetationFeatures.HUGE_BROWN_MUSHROOM.place(brownMushroomLevel, generator, new WorldgenRandom(123n), new BlockPos(16, 11, 16))).toBe(true);
    const brownPlacements = collectPlacedLocations(brownMushroomLevel, 11, 24);
    expect(brownPlacements).toContain("minecraft:brown_mushroom_block");
    expect(brownPlacements).toContain("minecraft:mushroom_stem");

    const redMushroomLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    expect(VegetationFeatures.HUGE_RED_MUSHROOM.place(redMushroomLevel, generator, new WorldgenRandom(456n), new BlockPos(16, 11, 16))).toBe(true);
    const redPlacements = collectPlacedLocations(redMushroomLevel, 11, 24);
    expect(redPlacements).toContain("minecraft:red_mushroom_block");
    expect(redPlacements).toContain("minecraft:mushroom_stem");

    const darkForestLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    expect(VegetationFeatures.DARK_FOREST_VEGETATION_BROWN.place(darkForestLevel, generator, new WorldgenRandom(789n), new BlockPos(0, 0, 0))).toBe(
      true,
    );
    const darkForestPlacements = collectPlacedLocations(darkForestLevel, 11, 32);
    expect(darkForestPlacements.length).toBeGreaterThan(0);
    expect(darkForestPlacements.some((location) => DARK_FOREST_OUTPUT_LOCATIONS.has(location))).toBe(true);
    for (const location of darkForestPlacements) {
      expect(DARK_FOREST_OUTPUT_LOCATIONS.has(location)).toBe(true);
    }
  });

  test("savanna vegetation paths place translated acacia blocks and savanna settings are wired", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();

    const acaciaLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    expect(TreeFeatures.ACACIA.place(acaciaLevel, generator, new WorldgenRandom(2468n), new BlockPos(16, 11, 16))).toBe(true);
    const acaciaPlacements = collectPlacedLocations(acaciaLevel, 11, 24);
    expect(acaciaPlacements).toContain("minecraft:acacia_log");
    expect(acaciaPlacements).toContain("minecraft:acacia_leaves");

    const savannaLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    expect(VegetationFeatures.TREES_SAVANNA.place(savannaLevel, generator, new WorldgenRandom(1357n), new BlockPos(0, 0, 0))).toBe(true);
    const savannaPlacements = collectPlacedLocations(savannaLevel, 11, 32);
    expect(savannaPlacements.length).toBeGreaterThan(0);
    expect(savannaPlacements.some((location) => SAVANNA_OUTPUT_LOCATIONS.has(location))).toBe(true);
    for (const location of savannaPlacements) {
      expect(SAVANNA_OUTPUT_LOCATIONS.has(location)).toBe(true);
    }
  });

  test("jungle vegetation paths place translated jungle, melon, and vine blocks", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();

    const jungleLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    expect(VegetationFeatures.TREES_JUNGLE.place(jungleLevel, generator, new WorldgenRandom(16n), new BlockPos(0, 0, 0))).toBe(true);
    const junglePlacements = collectPlacedLocations(jungleLevel, 11, 40);
    expect(junglePlacements.length).toBeGreaterThan(0);
    expect(junglePlacements.some((location) => JUNGLE_TREE_OUTPUT_LOCATIONS.has(location))).toBe(true);
    for (const location of junglePlacements) {
      expect(JUNGLE_TREE_OUTPUT_LOCATIONS.has(location)).toBe(true);
    }

    const melonLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    let melonPlaced = false;
    for (let seed = 0n; seed < 128n; seed++) {
      if (VegetationFeatures.PATCH_MELON.place(melonLevel, generator, new WorldgenRandom(seed), new BlockPos(8, 11, 8))) {
        melonPlaced = true;
        break;
      }
    }
    expect(melonPlaced).toBe(true);
    expect(collectPlacedLocations(melonLevel, 11, 11)).toContain("minecraft:melon");

    const vinesLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    for (let z = 0; z < 32; z++) {
      for (let x = 0; x < 32; x++) {
        vinesLevel.setBlock(new BlockPos(x, 13, z), getState("minecraft:jungle_log"));
      }
    }
    expect(VegetationFeatures.VINES.place(vinesLevel, generator, new WorldgenRandom(5n), new BlockPos(0, 12, 0))).toBe(true);
    expect(collectPlacedLocations(vinesLevel, 12, 12)).toContain("minecraft:vine");
  });

  test("bamboo feature and bamboo-jungle vegetation paths place translated bamboo and jungle-family blocks", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();

    const bambooLightLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    expect(
      Features.BAMBOO.configured(new ProbabilityFeatureConfiguration(0.0)).place(bambooLightLevel, generator, new WorldgenRandom(21n), new BlockPos(8, 11, 8)),
    ).toBe(true);
    const bambooLightPlacements = collectPlacedLocations(bambooLightLevel, 11, 32);
    expect(bambooLightPlacements).toContain("minecraft:bamboo");
    expect(bambooLightPlacements).not.toContain("minecraft:podzol");

    let bambooLevel: StaticRenderLevel | undefined;
    for (let seed = 0n; seed < 128n; seed++) {
      const candidate = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
      if (Features.BAMBOO.configured(new ProbabilityFeatureConfiguration(0.2)).place(candidate, generator, new WorldgenRandom(seed), new BlockPos(8, 11, 8))) {
        bambooLevel = candidate;
        if (collectPlacedLocations(candidate, 10, 32).includes("minecraft:podzol")) {
          break;
        }
      }
    }
    expect(bambooLevel).toBeDefined();
    const bambooPlacements = collectPlacedLocations(bambooLevel!, 10, 32);
    expect(bambooPlacements).toContain("minecraft:bamboo");
    expect(bambooPlacements).toContain("minecraft:podzol");

    let bambooVegetationLevel: StaticRenderLevel | undefined;
    for (let seed = 0n; seed < 64n; seed++) {
      const candidate = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
      if (VegetationFeatures.BAMBOO_VEGETATION.place(candidate, generator, new WorldgenRandom(seed), new BlockPos(0, 0, 0))) {
        bambooVegetationLevel = candidate;
        break;
      }
    }
    expect(bambooVegetationLevel).toBeDefined();
    const bambooVegetationPlacements = collectPlacedLocations(bambooVegetationLevel!, 11, 40);
    expect(bambooVegetationPlacements.length).toBeGreaterThan(0);
    expect(bambooVegetationPlacements.some((location) => BAMBOO_VEGETATION_OUTPUT_LOCATIONS.has(location))).toBe(true);
    for (const location of bambooVegetationPlacements) {
      expect(BAMBOO_VEGETATION_OUTPUT_LOCATIONS.has(location)).toBe(true);
    }
  });

  test("snowy, giant-taiga, and mushroom-field feature paths place translated spruce, podzol, and huge-mushroom blocks", () => {
    const blocks = registerGeneratedRenderBlocks();
    const generator = createGenerator();

    const snowyLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    expect(TreeFeatures.SPRUCE.place(snowyLevel, generator, new WorldgenRandom(321n), new BlockPos(16, 11, 16))).toBe(true);
    const snowyPlacements = collectPlacedLocations(snowyLevel, 11, 40);
    expect(snowyPlacements.length).toBeGreaterThan(0);
    for (const location of snowyPlacements) {
      expect(SNOWY_TREE_OUTPUT_LOCATIONS.has(location)).toBe(true);
    }

    const giantTaigaLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    expect(VegetationFeatures.TREES_GIANT.place(giantTaigaLevel, generator, new WorldgenRandom(123n), new BlockPos(0, 0, 0))).toBe(true);
    const giantTaigaPlacements = collectPlacedLocations(giantTaigaLevel, 10, 40);
    expect(giantTaigaPlacements.length).toBeGreaterThan(0);
    expect(giantTaigaPlacements.some((location) => GIANT_TAIGA_OUTPUT_LOCATIONS.has(location))).toBe(true);
    for (const location of giantTaigaPlacements) {
      expect(GIANT_TAIGA_OUTPUT_LOCATIONS.has(location)).toBe(true);
    }

    let badlandsTreeLevel: StaticRenderLevel | undefined;
    for (let seed = 0n; seed < 32n; seed++) {
      const candidate = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
      if (VegetationFeatures.TREES_BADLANDS.place(candidate, generator, new WorldgenRandom(seed), new BlockPos(0, 0, 0))) {
        badlandsTreeLevel = candidate;
        break;
      }
    }
    expect(badlandsTreeLevel).toBeDefined();
    const badlandsTreePlacements = collectPlacedLocations(badlandsTreeLevel!, 10, 24);
    expect(badlandsTreePlacements.length).toBeGreaterThan(0);
    for (const location of badlandsTreePlacements) {
      expect(BADLANDS_TREE_OUTPUT_LOCATIONS.has(location)).toBe(true);
    }

    let mushroomFieldLevel: StaticRenderLevel | undefined;
    for (let seed = 0n; seed < 32n; seed++) {
      const candidate = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
      if (VegetationFeatures.MUSHROOM_FIELD_VEGETATION.place(candidate, generator, new WorldgenRandom(seed), new BlockPos(0, 0, 0))) {
        mushroomFieldLevel = candidate;
        break;
      }
    }
    expect(mushroomFieldLevel).toBeDefined();
    const mushroomFieldPlacements = collectPlacedLocations(mushroomFieldLevel!, 11, 24);
    expect(mushroomFieldPlacements.length).toBeGreaterThan(0);
    for (const location of mushroomFieldPlacements) {
      expect(MUSHROOM_FIELD_OUTPUT_LOCATIONS.has(location)).toBe(true);
    }
  });

  test("flower-forest, birch, and plains selectors use the translated bee-decorated tree variants", () => {
    registerGeneratedRenderBlocks();

    const flowerForestConfig = unwrapConfiguredFeature(VegetationFeatures.FOREST_FLOWER_TREES).current.config as RandomFeatureConfiguration;
    expectBeeDecoratedTreeFeature(flowerForestConfig.features[0]!.feature, 0.02);
    expectBeeDecoratedTreeFeature(flowerForestConfig.features[1]!.feature, 0.02);
    expectBeeDecoratedTreeFeature(flowerForestConfig.defaultFeature, 0.02);

    const birchTallConfig = unwrapConfiguredFeature(VegetationFeatures.BIRCH_TALL).current.config as RandomFeatureConfiguration;
    expectBeeDecoratedTreeFeature(birchTallConfig.features[0]!.feature, 0.002);
    expectBeeDecoratedTreeFeature(birchTallConfig.defaultFeature, 0.002);

    const treesBirchFeature = unwrapConfiguredFeature(VegetationFeatures.TREES_BIRCH).current;
    expectBeeDecoratedTreeFeature(treesBirchFeature, 0.002);

    const birchOtherConfig = unwrapConfiguredFeature(VegetationFeatures.BIRCH_OTHER).current.config as RandomFeatureConfiguration;
    expectBeeDecoratedTreeFeature(birchOtherConfig.features[0]!.feature, 0.002);
    expectBeeDecoratedTreeFeature(birchOtherConfig.features[1]!.feature, 0.002);
    expectBeeDecoratedTreeFeature(birchOtherConfig.defaultFeature, 0.002);

    const plainVegetationConfig = unwrapConfiguredFeature(VegetationFeatures.PLAIN_VEGETATION).current.config as RandomFeatureConfiguration;
    expectBeeDecoratedTreeFeature(plainVegetationConfig.features[0]!.feature, 0.05);
    expectBeeDecoratedTreeFeature(plainVegetationConfig.defaultFeature, 0.05);
  });

  test("sunflower plains insert PATCH_SUNFLOWER and keep the vanilla vegetal ordering split", () => {
    registerGeneratedRenderBlocks();

    const plainsVegetal = getVegetalFeatures("minecraft:plains");
    const sunflowerVegetal = getVegetalFeatures("minecraft:sunflower_plains");

    expect(sunflowerVegetal.length).toBe(plainsVegetal.length + 1);

    const tallGrassIndex = sunflowerVegetal.findIndex((feature) => isRandomPatchFeatureForState(feature, "minecraft:tall_grass"));
    const sunflowerIndex = sunflowerVegetal.findIndex((feature) => isRandomPatchFeatureForState(feature, "minecraft:sunflower"));
    const plainVegetationIndex = sunflowerVegetal.findIndex((feature) => unwrapConfiguredFeature(feature).current.feature === Features.RANDOM_SELECTOR);
    const sugarCaneIndex = sunflowerVegetal.findIndex((feature) => isRandomPatchFeatureForState(feature, "minecraft:sugar_cane"));
    const brownMushroomIndex = sunflowerVegetal.findIndex((feature) => isRandomPatchFeatureForState(feature, "minecraft:brown_mushroom"));

    expect(tallGrassIndex).toBeGreaterThan(-1);
    expect(sunflowerIndex).toBeGreaterThan(-1);
    expect(plainVegetationIndex).toBeGreaterThan(-1);
    expect(tallGrassIndex).toBeLessThan(sunflowerIndex);
    expect(sunflowerIndex).toBeLessThan(plainVegetationIndex);
    expect(sugarCaneIndex).toBeGreaterThan(-1);
    expect(brownMushroomIndex).toBeGreaterThan(-1);
    expect(sugarCaneIndex).toBeLessThan(brownMushroomIndex);
  });

  test("desert biome settings carry the translated SURFACE_STRUCTURES desert well path", () => {
    registerGeneratedRenderBlocks();

    const desertSurfaceStructures = (getOverworldBiomeGenerationSettings("minecraft:desert").features()[GenerationStep.Decoration.SURFACE_STRUCTURES] ?? [])
      .map((supplier) => supplier());
    expect(desertSurfaceStructures).toHaveLength(1);

    const desertWell = desertSurfaceStructures[0]!;
    expect(unwrapConfiguredFeature(desertWell).current.feature).toBe(Features.DESERT_WELL);

    const chanceConfig = unwrapConfiguredFeature(desertWell).decoratorConfigs.find(
      (decoratorConfig): decoratorConfig is ChanceDecoratorConfiguration => decoratorConfig instanceof ChanceDecoratorConfiguration,
    );
    expect(chanceConfig).toBeDefined();
    expect(chanceConfig!.chance).toBe(1000);
  });

  test("swamp vegetation uses the translated swamp-oak leaf-vine tree path", () => {
    registerGeneratedRenderBlocks();

    const swampTreeFeature = getVegetalFeatures("minecraft:swamp").find((feature) => unwrapConfiguredFeature(feature).current.feature === Features.TREE);
    expect(swampTreeFeature).toBeDefined();
    expectLeafVineDecoratedTreeFeature(unwrapConfiguredFeature(swampTreeFeature!).current);
    expectLeafVineDecoratedTreeFeature(unwrapConfiguredFeature(VegetationFeatures.TREES_SWAMP).current);
  });

  test("deep warm ocean adds vanilla seagrass_simple through the liquid carving mask decorator", () => {
    registerGeneratedRenderBlocks();

    const warmVegetal = getVegetalFeatures("minecraft:warm_ocean");
    const deepWarmVegetal = getVegetalFeatures("minecraft:deep_warm_ocean");
    expect(deepWarmVegetal.length).toBeLessThan(warmVegetal.length);

    const seagrassSimple = deepWarmVegetal.find((feature) => isSimpleBlockFeatureForState(feature, "minecraft:seagrass"));
    expect(seagrassSimple).toBeDefined();

    const { decoratorConfigs } = unwrapConfiguredFeature(seagrassSimple!);
    const carvingMaskConfig = decoratorConfigs.find(
      (decoratorConfig): decoratorConfig is CarvingMaskDecoratorConfiguration =>
        decoratorConfig instanceof CarvingMaskDecoratorConfiguration,
    );
    expect(carvingMaskConfig).toBeDefined();
    expect(carvingMaskConfig!.step).toBe(GenerationStep.Carving.LIQUID);
    expect(warmVegetal.some((feature) => isSimpleBlockFeatureForState(feature, "minecraft:seagrass"))).toBe(false);
  });

  test("overworld biome settings wire the shoreline, ocean, swamp, forest, savanna, jungle, bamboo-jungle, snowy, giant-taiga, mushroom, mountain, and badlands tables", () => {
    registerGeneratedRenderBlocks();
    const beachFeatures = getOverworldBiomeGenerationSettings("minecraft:beach").features().flat().map((supplier) => getBaseFeature(supplier()));
    const stoneShoreFeatures = getOverworldBiomeGenerationSettings("minecraft:stone_shore").features().flat().map((supplier) => getBaseFeature(supplier()));
    const snowyBeachFeatures = getOverworldBiomeGenerationSettings("minecraft:snowy_beach").features().flat().map((supplier) => getBaseFeature(supplier()));
    const riverFeatures = getOverworldBiomeGenerationSettings("minecraft:river").features().flat().map((supplier) => getBaseFeature(supplier()));
    const frozenRiverFeatures = getOverworldBiomeGenerationSettings("minecraft:frozen_river").features().flat().map((supplier) => getBaseFeature(supplier()));
    const oceanFeatures = getOverworldBiomeGenerationSettings("minecraft:ocean").features().flat().map((supplier) => getBaseFeature(supplier()));
    const deepOceanFeatures = getOverworldBiomeGenerationSettings("minecraft:deep_ocean").features().flat().map((supplier) => getBaseFeature(supplier()));
    const coldOceanFeatures = getOverworldBiomeGenerationSettings("minecraft:cold_ocean").features().flat().map((supplier) => getBaseFeature(supplier()));
    const deepColdOceanFeatures = getOverworldBiomeGenerationSettings("minecraft:deep_cold_ocean").features().flat().map((supplier) => getBaseFeature(supplier()));
    const lukewarmOceanFeatures = getOverworldBiomeGenerationSettings("minecraft:lukewarm_ocean")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const deepLukewarmOceanFeatures = getOverworldBiomeGenerationSettings("minecraft:deep_lukewarm_ocean")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const warmOceanFeatures = getOverworldBiomeGenerationSettings("minecraft:warm_ocean")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const deepWarmOceanFeatures = getOverworldBiomeGenerationSettings("minecraft:deep_warm_ocean")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const frozenOceanFeatures = getOverworldBiomeGenerationSettings("minecraft:frozen_ocean")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const deepFrozenOceanFeatures = getOverworldBiomeGenerationSettings("minecraft:deep_frozen_ocean")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const swampFeatures = getOverworldBiomeGenerationSettings("minecraft:swamp").features().flat().map((supplier) => getBaseFeature(supplier()));
    const swampHillsFeatures = getOverworldBiomeGenerationSettings("minecraft:swamp_hills").features().flat().map((supplier) => getBaseFeature(supplier()));
    const flowerForestFeatures = getOverworldBiomeGenerationSettings("minecraft:flower_forest").features().flat().map((supplier) => getBaseFeature(supplier()));
    const birchForestFeatures = getOverworldBiomeGenerationSettings("minecraft:birch_forest").features().flat().map((supplier) => getBaseFeature(supplier()));
    const darkForestFeatures = getOverworldBiomeGenerationSettings("minecraft:dark_forest").features().flat().map((supplier) => getBaseFeature(supplier()));
    const darkForestHillsFeatures = getOverworldBiomeGenerationSettings("minecraft:dark_forest_hills")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const savannaFeatures = getOverworldBiomeGenerationSettings("minecraft:savanna").features().flat().map((supplier) => getBaseFeature(supplier()));
    const savannaPlateauFeatures = getOverworldBiomeGenerationSettings("minecraft:savanna_plateau").features().flat().map((supplier) => getBaseFeature(supplier()));
    const shatteredSavannaFeatures = getOverworldBiomeGenerationSettings("minecraft:shattered_savanna").features().flat().map((supplier) => getBaseFeature(supplier()));
    const shatteredSavannaPlateauFeatures = getOverworldBiomeGenerationSettings("minecraft:shattered_savanna_plateau")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const jungleFeatures = getOverworldBiomeGenerationSettings("minecraft:jungle").features().flat().map((supplier) => getBaseFeature(supplier()));
    const jungleHillsFeatures = getOverworldBiomeGenerationSettings("minecraft:jungle_hills").features().flat().map((supplier) => getBaseFeature(supplier()));
    const jungleEdgeFeatures = getOverworldBiomeGenerationSettings("minecraft:jungle_edge").features().flat().map((supplier) => getBaseFeature(supplier()));
    const modifiedJungleFeatures = getOverworldBiomeGenerationSettings("minecraft:modified_jungle")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const modifiedJungleEdgeFeatures = getOverworldBiomeGenerationSettings("minecraft:modified_jungle_edge")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const bambooJungleFeatures = getOverworldBiomeGenerationSettings("minecraft:bamboo_jungle")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const bambooJungleHillsFeatures = getOverworldBiomeGenerationSettings("minecraft:bamboo_jungle_hills")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const snowyTundraFeatures = getOverworldBiomeGenerationSettings("minecraft:snowy_tundra").features().flat().map((supplier) => getBaseFeature(supplier()));
    const snowyMountainsFeatures = getOverworldBiomeGenerationSettings("minecraft:snowy_mountains").features().flat().map((supplier) => getBaseFeature(supplier()));
    const iceSpikesFeatures = getOverworldBiomeGenerationSettings("minecraft:ice_spikes").features().flat().map((supplier) => getBaseFeature(supplier()));
    const snowyTaigaFeatures = getOverworldBiomeGenerationSettings("minecraft:snowy_taiga").features().flat().map((supplier) => getBaseFeature(supplier()));
    const snowyTaigaHillsFeatures = getOverworldBiomeGenerationSettings("minecraft:snowy_taiga_hills").features().flat().map((supplier) => getBaseFeature(supplier()));
    const snowyTaigaMountainsFeatures = getOverworldBiomeGenerationSettings("minecraft:snowy_taiga_mountains")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const giantTaigaFeatures = getOverworldBiomeGenerationSettings("minecraft:giant_tree_taiga").features().flat().map((supplier) => getBaseFeature(supplier()));
    const giantTaigaHillsFeatures = getOverworldBiomeGenerationSettings("minecraft:giant_tree_taiga_hills")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const giantSpruceTaigaFeatures = getOverworldBiomeGenerationSettings("minecraft:giant_spruce_taiga")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const giantSpruceTaigaHillsFeatures = getOverworldBiomeGenerationSettings("minecraft:giant_spruce_taiga_hills")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const plainsFeatures = getOverworldBiomeGenerationSettings("minecraft:plains").features().flat().map((supplier) => getBaseFeature(supplier()));
    const sunflowerPlainsFeatures = getOverworldBiomeGenerationSettings("minecraft:sunflower_plains")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const mountainsFeatures = getOverworldBiomeGenerationSettings("minecraft:mountains").features().flat().map((supplier) => getBaseFeature(supplier()));
    const woodedMountainsFeatures = getOverworldBiomeGenerationSettings("minecraft:wooded_mountains")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const mountainEdgeFeatures = getOverworldBiomeGenerationSettings("minecraft:mountain_edge")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const gravellyMountainFeatures = getOverworldBiomeGenerationSettings("minecraft:gravelly_mountains")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const modifiedGravellyMountainFeatures = getOverworldBiomeGenerationSettings("minecraft:modified_gravelly_mountains")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const badlandsVegetal = getVegetalFeatures("minecraft:badlands");
    const woodedBadlandsVegetal = getVegetalFeatures("minecraft:wooded_badlands_plateau");
    const erodedBadlandsVegetal = getVegetalFeatures("minecraft:eroded_badlands");
    const modifiedWoodedBadlandsVegetal = getVegetalFeatures("minecraft:modified_wooded_badlands_plateau");
    const modifiedBadlandsPlateauVegetal = getVegetalFeatures("minecraft:modified_badlands_plateau");
    const mushroomFieldFeatures = getOverworldBiomeGenerationSettings("minecraft:mushroom_fields")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));
    const mushroomFieldShoreFeatures = getOverworldBiomeGenerationSettings("minecraft:mushroom_field_shore")
      .features()
      .flat()
      .map((supplier) => getBaseFeature(supplier()));

    expect(beachFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(stoneShoreFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(snowyBeachFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(snowyBeachFeatures.some((feature) => feature === Features.FREEZE_TOP_LAYER)).toBe(true);
    expect(riverFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(riverFeatures.some((feature) => feature === Features.RANDOM_SELECTOR || feature === Features.TREE)).toBe(true);
    expect(frozenRiverFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(false);
    expect(frozenRiverFeatures.some((feature) => feature === Features.FREEZE_TOP_LAYER)).toBe(true);
    expect(oceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(oceanFeatures.some((feature) => feature === Features.KELP)).toBe(true);
    expect(oceanFeatures.some((feature) => feature === Features.FREEZE_TOP_LAYER)).toBe(true);
    expect(deepOceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(deepOceanFeatures.some((feature) => feature === Features.KELP)).toBe(true);
    expect(coldOceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(coldOceanFeatures.some((feature) => feature === Features.KELP)).toBe(true);
    expect(deepColdOceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(deepColdOceanFeatures.some((feature) => feature === Features.KELP)).toBe(true);
    expect(lukewarmOceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(lukewarmOceanFeatures.some((feature) => feature === Features.KELP)).toBe(true);
    expect(deepLukewarmOceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(deepLukewarmOceanFeatures.some((feature) => feature === Features.KELP)).toBe(true);
    expect(warmOceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(warmOceanFeatures.some((feature) => feature === Features.SIMPLE_BLOCK)).toBe(false);
    expect(warmOceanFeatures.some((feature) => feature === Features.SIMPLE_RANDOM_SELECTOR)).toBe(true);
    expect(warmOceanFeatures.some((feature) => feature === Features.SEA_PICKLE)).toBe(true);
    expect(warmOceanFeatures.some((feature) => feature === Features.KELP)).toBe(false);
    expect(deepWarmOceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(deepWarmOceanFeatures.some((feature) => feature === Features.SIMPLE_BLOCK)).toBe(true);
    expect(deepWarmOceanFeatures.some((feature) => feature === Features.SEA_PICKLE)).toBe(false);
    expect(deepWarmOceanFeatures.some((feature) => feature === Features.SIMPLE_RANDOM_SELECTOR)).toBe(false);
    expect(deepWarmOceanFeatures.some((feature) => feature === Features.KELP)).toBe(false);
    expect(frozenOceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(false);
    expect(frozenOceanFeatures.some((feature) => feature === Features.KELP)).toBe(false);
    expect(frozenOceanFeatures.some((feature) => feature === Features.RANDOM_SELECTOR || feature === Features.TREE)).toBe(true);
    expect(frozenOceanFeatures.some((feature) => feature === Features.FREEZE_TOP_LAYER)).toBe(true);
    expect(deepFrozenOceanFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(false);
    expect(deepFrozenOceanFeatures.some((feature) => feature === Features.KELP)).toBe(false);
    expect(deepFrozenOceanFeatures.some((feature) => feature === Features.RANDOM_SELECTOR || feature === Features.TREE)).toBe(true);
    expect(deepFrozenOceanFeatures.some((feature) => feature === Features.FREEZE_TOP_LAYER)).toBe(true);
    expect(swampFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(swampHillsFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(false);
    expect(flowerForestFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(flowerForestFeatures.some((feature) => feature === Features.SIMPLE_RANDOM_SELECTOR)).toBe(true);
    expect(birchForestFeatures.some((feature) => feature === Features.RANDOM_SELECTOR || feature === Features.TREE)).toBe(true);
    expect(darkForestFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(darkForestHillsFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(savannaFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(savannaFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(savannaPlateauFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(shatteredSavannaFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(shatteredSavannaFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(shatteredSavannaPlateauFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(jungleFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(jungleFeatures.some((feature) => feature === Features.BAMBOO)).toBe(true);
    expect(jungleFeatures.some((feature) => feature === Features.VINES)).toBe(true);
    expect(jungleHillsFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(jungleHillsFeatures.some((feature) => feature === Features.BAMBOO)).toBe(true);
    expect(jungleHillsFeatures.some((feature) => feature === Features.VINES)).toBe(true);
    expect(jungleEdgeFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(jungleEdgeFeatures.some((feature) => feature === Features.BAMBOO)).toBe(false);
    expect(jungleEdgeFeatures.some((feature) => feature === Features.VINES)).toBe(true);
    expect(modifiedJungleFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(modifiedJungleFeatures.some((feature) => feature === Features.BAMBOO)).toBe(false);
    expect(modifiedJungleFeatures.some((feature) => feature === Features.VINES)).toBe(true);
    expect(modifiedJungleFeatures.length).toBe(jungleFeatures.length - 1);
    expect(modifiedJungleEdgeFeatures).toEqual(jungleEdgeFeatures);
    expect(bambooJungleFeatures.some((feature) => feature === Features.BAMBOO)).toBe(true);
    expect(bambooJungleFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(bambooJungleFeatures.some((feature) => feature === Features.VINES)).toBe(true);
    expect(bambooJungleHillsFeatures.some((feature) => feature === Features.BAMBOO)).toBe(true);
    expect(bambooJungleHillsFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(bambooJungleHillsFeatures.some((feature) => feature === Features.VINES)).toBe(true);
    expect(snowyTundraFeatures.some((feature) => feature === Features.TREE)).toBe(true);
    expect(snowyTundraFeatures.some((feature) => feature === Features.FREEZE_TOP_LAYER)).toBe(true);
    expect(snowyMountainsFeatures.some((feature) => feature === Features.TREE)).toBe(true);
    expect(snowyMountainsFeatures.some((feature) => feature === Features.FREEZE_TOP_LAYER)).toBe(true);
    expect(iceSpikesFeatures.some((feature) => feature === Features.ICE_SPIKE)).toBe(true);
    expect(iceSpikesFeatures.some((feature) => feature === Features.ICE_PATCH)).toBe(true);
    expect(iceSpikesFeatures.some((feature) => feature === Features.FREEZE_TOP_LAYER)).toBe(true);
    expect(snowyTaigaFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(snowyTaigaFeatures.some((feature) => feature === Features.FREEZE_TOP_LAYER)).toBe(true);
    expect(snowyTaigaHillsFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(snowyTaigaMountainsFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(giantTaigaFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(giantTaigaHillsFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(giantSpruceTaigaFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(giantSpruceTaigaHillsFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(plainsFeatures.filter((feature) => feature === Features.RANDOM_PATCH).length).toBeGreaterThan(0);
    expect(sunflowerPlainsFeatures.filter((feature) => feature === Features.RANDOM_PATCH).length).toBe(
      plainsFeatures.filter((feature) => feature === Features.RANDOM_PATCH).length + 1,
    );
    expect(mountainsFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(woodedMountainsFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(mountainEdgeFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(gravellyMountainFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(modifiedGravellyMountainFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(getTreeExtraCount("minecraft:mountains")).toBe(0);
    expect(getTreeExtraCount("minecraft:gravelly_mountains")).toBe(0);
    expect(getTreeExtraCount("minecraft:modified_gravelly_mountains")).toBe(0);
    expect(getTreeExtraCount("minecraft:wooded_mountains")).toBe(3);
    expect(getTreeExtraCount("minecraft:mountain_edge")).toBe(3);
    expect(badlandsVegetal).toHaveLength(10);
    expect(woodedBadlandsVegetal).toHaveLength(11);
    expect(erodedBadlandsVegetal).toHaveLength(10);
    expect(modifiedWoodedBadlandsVegetal).toHaveLength(11);
    expect(modifiedBadlandsPlateauVegetal).toHaveLength(10);
    expect(badlandsVegetal.some((feature) => unwrapConfiguredFeature(feature).current.feature === Features.TREE)).toBe(false);
    expect(erodedBadlandsVegetal.some((feature) => unwrapConfiguredFeature(feature).current.feature === Features.TREE)).toBe(false);
    expect(modifiedBadlandsPlateauVegetal.some((feature) => unwrapConfiguredFeature(feature).current.feature === Features.TREE)).toBe(false);
    expect(woodedBadlandsVegetal.some((feature) => unwrapConfiguredFeature(feature).current.feature === Features.TREE)).toBe(true);
    expect(modifiedWoodedBadlandsVegetal.some((feature) => unwrapConfiguredFeature(feature).current.feature === Features.TREE)).toBe(true);
    expect(getTreeExtraCount("minecraft:wooded_badlands_plateau")).toBe(5);
    expect(getTreeExtraCount("minecraft:modified_wooded_badlands_plateau")).toBe(5);
    expect(mushroomFieldFeatures.some((feature) => feature === Features.RANDOM_BOOLEAN_SELECTOR)).toBe(true);
    expect(mushroomFieldShoreFeatures.some((feature) => feature === Features.RANDOM_BOOLEAN_SELECTOR)).toBe(true);
  });
});
