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

  test("overworld biome settings wire the new swamp, flower-forest, and birch tables", () => {
    registerGeneratedRenderBlocks();
    const swampFeatures = getOverworldBiomeGenerationSettings("minecraft:swamp").features().flat().map((supplier) => getBaseFeature(supplier()));
    const swampHillsFeatures = getOverworldBiomeGenerationSettings("minecraft:swamp_hills").features().flat().map((supplier) => getBaseFeature(supplier()));
    const flowerForestFeatures = getOverworldBiomeGenerationSettings("minecraft:flower_forest").features().flat().map((supplier) => getBaseFeature(supplier()));
    const birchForestFeatures = getOverworldBiomeGenerationSettings("minecraft:birch_forest").features().flat().map((supplier) => getBaseFeature(supplier()));

    expect(swampFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(true);
    expect(swampHillsFeatures.some((feature) => feature === Features.SEAGRASS)).toBe(false);
    expect(flowerForestFeatures.some((feature) => feature === Features.FLOWER)).toBe(true);
    expect(flowerForestFeatures.some((feature) => feature === Features.SIMPLE_RANDOM_SELECTOR)).toBe(true);
    expect(birchForestFeatures.some((feature) => feature === Features.RANDOM_SELECTOR || feature === Features.TREE)).toBe(true);
  });
});
