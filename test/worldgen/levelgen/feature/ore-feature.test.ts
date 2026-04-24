import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import { getOverworldBiomeGenerationSettings } from "../../../../src/worldgen/biome/overworld-biome-generation-settings";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import type { Block } from "../../../../src/world/level/block/block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { GenerationStep } from "../../../../src/worldgen/levelgen/generation-step";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { ConfiguredFeature } from "../../../../src/worldgen/levelgen/feature/configured-feature";
import { CountConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/count-configuration";
import { DecoratedFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-feature-configuration";
import type { DecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorator-configuration";
import { NoneDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/none-decorator-configuration";
import { OreConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/ore-configuration";
import { RangeDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/range-decorator-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { OreFeature } from "../../../../src/worldgen/levelgen/feature/ore-feature";
import { OreFeatures } from "../../../../src/worldgen/levelgen/feature/ore-features";
import { WorldgenRandom } from "../../../../src/worldgen/prng/worldgen-random";

interface OreFeatureDescription {
  readonly states: readonly string[];
  readonly size: number;
  readonly discardChanceOnAirExposure: number;
  readonly count: number;
  readonly hasRange: boolean;
  readonly hasSquare: boolean;
}

const EXPECTED_COMMON_ORES: readonly OreFeatureDescription[] = [
  {
    states: ["minecraft:coal_ore", "minecraft:deepslate_coal_ore"],
    size: 17,
    discardChanceOnAirExposure: 0,
    count: 20,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:iron_ore", "minecraft:deepslate_iron_ore"],
    size: 9,
    discardChanceOnAirExposure: 0,
    count: 20,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:gold_ore", "minecraft:deepslate_gold_ore"],
    size: 9,
    discardChanceOnAirExposure: 0,
    count: 2,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:redstone_ore", "minecraft:deepslate_redstone_ore"],
    size: 8,
    discardChanceOnAirExposure: 0,
    count: 8,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:diamond_ore", "minecraft:deepslate_diamond_ore"],
    size: 8,
    discardChanceOnAirExposure: 0,
    count: 1,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:lapis_ore", "minecraft:deepslate_lapis_ore"],
    size: 7,
    discardChanceOnAirExposure: 0,
    count: 1,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:copper_ore", "minecraft:deepslate_copper_ore"],
    size: 10,
    discardChanceOnAirExposure: 0,
    count: 6,
    hasRange: true,
    hasSquare: true,
  },
] as const;

const EXPECTED_UNDERGROUND_VARIETY: readonly OreFeatureDescription[] = [
  {
    states: ["minecraft:dirt"],
    size: 33,
    discardChanceOnAirExposure: 0,
    count: 10,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:gravel"],
    size: 33,
    discardChanceOnAirExposure: 0,
    count: 8,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:granite"],
    size: 33,
    discardChanceOnAirExposure: 0,
    count: 10,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:diorite"],
    size: 33,
    discardChanceOnAirExposure: 0,
    count: 10,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:andesite"],
    size: 33,
    discardChanceOnAirExposure: 0,
    count: 10,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:tuff"],
    size: 33,
    discardChanceOnAirExposure: 0,
    count: 1,
    hasRange: true,
    hasSquare: true,
  },
  {
    states: ["minecraft:deepslate"],
    size: 64,
    discardChanceOnAirExposure: 0,
    count: 2,
    hasRange: true,
    hasSquare: true,
  },
] as const;

const EXPECTED_DEFAULT_UNDERGROUND_ORES = [...EXPECTED_UNDERGROUND_VARIETY, ...EXPECTED_COMMON_ORES] as const;

function getState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function createGenerator(): NoiseBasedChunkGenerator {
  return new NoiseBasedChunkGenerator(new OverworldBiomeSource(12345n), 12345n);
}

function fillSolidLevel(level: StaticRenderLevel, state: BlockState, maxY: number): void {
  for (let y = 0; y < maxY; y++) {
    for (let z = 0; z < 48; z++) {
      for (let x = 0; x < 48; x++) {
        level.setBlock(new BlockPos(x, y, z), state);
      }
    }
  }
}

function collectPositions(level: StaticRenderLevel, state: BlockState, maxY: number): readonly string[] {
  const positions: string[] = [];
  for (let y = 0; y < maxY; y++) {
    for (let z = 0; z < 48; z++) {
      for (let x = 0; x < 48; x++) {
        if (level.getBlockState(new BlockPos(x, y, z)).is(state.getBlock())) {
          positions.push(`${x},${y},${z}`);
        }
      }
    }
  }

  return positions;
}

function setNeighborCross(level: StaticRenderLevel, center: BlockPos, state: BlockState): void {
  level.setBlock(center.above(), state);
  level.setBlock(center.below(), state);
  level.setBlock(center.north(), state);
  level.setBlock(center.south(), state);
  level.setBlock(center.east(), state);
  level.setBlock(center.west(), state);
}

function describeConfiguredOreFeature(feature: ConfiguredFeature<any, any>): OreFeatureDescription {
  const decoratorConfigs: DecoratorConfiguration[] = [];
  let current = feature;

  while (current.config instanceof DecoratedFeatureConfiguration) {
    decoratorConfigs.push(current.config.decorator.config());
    current = current.config.feature();
  }

  expect(current.feature).toBe(Features.ORE);
  expect(current.config).toBeInstanceOf(OreConfiguration);
  const config = current.config as OreConfiguration;
  const countConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is CountConfiguration =>
    decoratorConfig instanceof CountConfiguration
  );

  return {
    states: config.targetStates.map((targetState) => targetState.state.getBlock().getLocation()!.toString()),
    size: config.size,
    discardChanceOnAirExposure: config.discardChanceOnAirExposure,
    count: countConfig?.count().sample(new WorldgenRandom(0n)) ?? 1,
    hasRange: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof RangeDecoratorConfiguration),
    hasSquare: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof NoneDecoratorConfiguration),
  };
}

describe("Ore feature", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("canPlaceOre respects target predicates and air-exposure discard", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = new StaticRenderLevel(blocks.airState, 15, 15, 0, 32);
    const stoneState = getState("minecraft:stone");
    const dirtState = getState("minecraft:dirt");
    const center = new BlockPos(8, 8, 8);
    const targetState = OreFeatures.ORE_COAL_TARGET_LIST[0]!;

    level.setBlock(center, stoneState);
    setNeighborCross(level, center, stoneState);
    expect(
      OreFeature.canPlaceOre(
        level.getBlockState(center),
        (pos) => level.getBlockState(pos),
        new WorldgenRandom(1n),
        new OreConfiguration(OreFeatures.ORE_COAL_TARGET_LIST, 17),
        targetState,
        new BlockPos.MutableBlockPos(center.getX(), center.getY(), center.getZ()),
      ),
    ).toBe(true);

    level.setBlock(center.east(), blocks.airState);
    expect(
      OreFeature.canPlaceOre(
        level.getBlockState(center),
        (pos) => level.getBlockState(pos),
        new WorldgenRandom(2n),
        new OreConfiguration(OreFeatures.ORE_COAL_TARGET_LIST, 17, 1.0),
        targetState,
        new BlockPos.MutableBlockPos(center.getX(), center.getY(), center.getZ()),
      ),
    ).toBe(false);

    level.setBlock(center, dirtState);
    level.setBlock(center.east(), stoneState);
    expect(
      OreFeature.canPlaceOre(
        level.getBlockState(center),
        (pos) => level.getBlockState(pos),
        new WorldgenRandom(3n),
        new OreConfiguration(OreFeatures.ORE_COAL_TARGET_LIST, 17),
        targetState,
        new BlockPos.MutableBlockPos(center.getX(), center.getY(), center.getZ()),
      ),
    ).toBe(false);
  });

  test("natural stone predicate matches stone, tuff, and deepslate", () => {
    registerGeneratedRenderBlocks();
    const random = new WorldgenRandom(0n);

    expect(OreConfiguration.Predicates.NATURAL_STONE.test(getState("minecraft:stone"), random)).toBe(true);
    expect(OreConfiguration.Predicates.NATURAL_STONE.test(getState("minecraft:tuff"), random)).toBe(true);
    expect(OreConfiguration.Predicates.NATURAL_STONE.test(getState("minecraft:deepslate"), random)).toBe(true);
    expect(OreConfiguration.Predicates.NATURAL_STONE.test(getState("minecraft:dirt"), random)).toBe(false);
  });

  test("place is deterministic for the same seed and stone volume", () => {
    const blocks = registerGeneratedRenderBlocks();
    const stoneState = getState("minecraft:stone");
    const coalOreState = getState("minecraft:coal_ore");
    const levelA = new StaticRenderLevel(blocks.airState, 15, 15, 0, 64);
    const levelB = new StaticRenderLevel(blocks.airState, 15, 15, 0, 64);
    fillSolidLevel(levelA, stoneState, 48);
    fillSolidLevel(levelB, stoneState, 48);

    const feature = Features.ORE.configured(new OreConfiguration(OreFeatures.ORE_COAL_TARGET_LIST, 17));
    const generator = createGenerator();
    const origin = new BlockPos(24, 24, 24);

    expect(feature.place(levelA, generator, new WorldgenRandom(12345n), origin)).toBe(true);
    expect(feature.place(levelB, generator, new WorldgenRandom(12345n), origin)).toBe(true);

    const placedA = collectPositions(levelA, coalOreState, 48);
    const placedB = collectPositions(levelB, coalOreState, 48);
    expect(placedA).toEqual(placedB);
    expect(placedA.length).toBeGreaterThan(0);
  });

  test("common ore features match the vanilla-shaped defaults and biome wiring", () => {
    registerGeneratedRenderBlocks();

    const configuredDefaults = [
      OreFeatures.ORE_COAL,
      OreFeatures.ORE_IRON,
      OreFeatures.ORE_GOLD,
      OreFeatures.ORE_REDSTONE,
      OreFeatures.ORE_DIAMOND,
      OreFeatures.ORE_LAPIS,
      OreFeatures.ORE_COPPER,
    ].map(describeConfiguredOreFeature);

    expect(configuredDefaults).toEqual(EXPECTED_COMMON_ORES);

    const plains = getOverworldBiomeGenerationSettings("minecraft:plains").features()[GenerationStep.Decoration.UNDERGROUND_ORES]!;
    const ocean = getOverworldBiomeGenerationSettings("minecraft:ocean").features()[GenerationStep.Decoration.UNDERGROUND_ORES]!;

    expect(plains.map((feature) => describeConfiguredOreFeature(feature()))).toEqual(EXPECTED_DEFAULT_UNDERGROUND_ORES);
    expect(ocean.map((feature) => describeConfiguredOreFeature(feature()))).toEqual(EXPECTED_DEFAULT_UNDERGROUND_ORES);
  });

  test("underground variety features match the vanilla-shaped defaults", () => {
    registerGeneratedRenderBlocks();

    const configuredDefaults = [
      OreFeatures.ORE_DIRT,
      OreFeatures.ORE_GRAVEL,
      OreFeatures.ORE_GRANITE,
      OreFeatures.ORE_DIORITE,
      OreFeatures.ORE_ANDESITE,
      OreFeatures.ORE_TUFF,
      OreFeatures.ORE_DEEPSLATE,
    ].map(describeConfiguredOreFeature);

    expect(configuredDefaults).toEqual(EXPECTED_UNDERGROUND_VARIETY);
  });
});
