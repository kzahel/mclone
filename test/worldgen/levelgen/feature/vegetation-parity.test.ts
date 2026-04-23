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
import { DecoratedFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-feature-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { TreeFeatures } from "../../../../src/worldgen/levelgen/feature/tree-features";
import { VegetationFeatures } from "../../../../src/worldgen/levelgen/feature/vegetation-features";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { WorldgenRandom } from "../../../../src/worldgen/prng/worldgen-random";

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

const JUNGLE_TREE_OUTPUT_LOCATIONS = new Set([
  "minecraft:oak_log",
  "minecraft:oak_leaves",
  "minecraft:jungle_log",
  "minecraft:jungle_leaves",
  "minecraft:vine",
  "minecraft:cocoa",
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
  for (let z = 0; z < 32; z++) {
    for (let x = 0; x < 32; x++) {
      level.setBlock(new BlockPos(x, 11, z), waterState);
      level.setBlock(new BlockPos(x, 12, z), waterState);
    }
  }

  return level;
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

  test("overworld biome settings wire the new swamp, flower-forest, birch, dark-forest, savanna, and jungle tables", () => {
    registerGeneratedRenderBlocks();
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
    expect(jungleFeatures.some((feature) => feature === Features.VINES)).toBe(true);
    expect(jungleHillsFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(jungleHillsFeatures.some((feature) => feature === Features.VINES)).toBe(true);
    expect(jungleEdgeFeatures.some((feature) => feature === Features.RANDOM_SELECTOR)).toBe(true);
    expect(jungleEdgeFeatures.some((feature) => feature === Features.VINES)).toBe(true);
  });
});
