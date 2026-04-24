import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import { ConstantInt } from "../../../../src/util/valueproviders/constant-int";
import { getOverworldBiomeGenerationSettings } from "../../../../src/worldgen/biome/overworld-biome-generation-settings";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import type { Block } from "../../../../src/world/level/block/block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { GenerationStep } from "../../../../src/worldgen/levelgen/generation-step";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { ConfiguredFeature } from "../../../../src/worldgen/levelgen/feature/configured-feature";
import { ChanceDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/chance-decorator-configuration";
import { CountConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/count-configuration";
import { DecoratedDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-decorator-configuration";
import { DecoratedFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-feature-configuration";
import type { DecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorator-configuration";
import { DiskConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/disk-configuration";
import { DripstoneClusterConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/dripstone-cluster-configuration";
import { GlowLichenConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/glow-lichen-configuration";
import { HeightmapConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/heightmap-configuration";
import { NoneDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/none-decorator-configuration";
import { OreConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/ore-configuration";
import { ReplaceBlockConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/replace-block-configuration";
import { RangeDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/range-decorator-configuration";
import { SmallDripstoneConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/small-dripstone-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { OreFeature } from "../../../../src/worldgen/levelgen/feature/ore-feature";
import { OreFeatures } from "../../../../src/worldgen/levelgen/feature/ore-features";
import { WorldgenRandom } from "../../../../src/worldgen/prng/worldgen-random";
import { UniformInt } from "../../../../src/util/valueproviders/uniform-int";

interface OreFeatureDescription {
  readonly states: readonly string[];
  readonly size: number;
  readonly discardChanceOnAirExposure: number;
  readonly count: number;
  readonly hasRange: boolean;
  readonly hasSquare: boolean;
}

interface ReplaceBlockFeatureDescription {
  readonly states: readonly string[];
  readonly hasRange: boolean;
  readonly hasSquare: boolean;
}

interface IntProviderRange {
  readonly min: number;
  readonly max: number;
}

interface DiskFeatureDescription {
  readonly state: string;
  readonly radius: IntProviderRange;
  readonly halfHeight: number;
  readonly targets: readonly string[];
  readonly count: IntProviderRange;
  readonly hasHeightmap: boolean;
  readonly hasSquare: boolean;
}

interface GlowLichenFeatureDescription {
  readonly searchRange: number;
  readonly canPlaceOnFloor: boolean;
  readonly canPlaceOnCeiling: boolean;
  readonly canPlaceOnWall: boolean;
  readonly chanceOfSpreading: number;
  readonly canBePlacedOnStates: readonly string[];
  readonly count: IntProviderRange;
  readonly hasRange: boolean;
  readonly hasSquare: boolean;
}

interface SmallDripstoneFeatureDescription {
  readonly maxPlacements: number;
  readonly emptySpaceSearchRadius: number;
  readonly maxOffsetFromOrigin: number;
  readonly chanceOfTallerDripstone: number;
  readonly count: IntProviderRange;
  readonly rarity: number | undefined;
  readonly hasRange: boolean;
  readonly hasSquare: boolean;
}

interface DripstoneClusterFeatureDescription {
  readonly floorToCeilingSearchRange: number;
  readonly height: IntProviderRange;
  readonly radius: IntProviderRange;
  readonly maxStalagmiteStalactiteHeightDiff: number;
  readonly heightDeviation: number;
  readonly dripstoneBlockLayerThickness: IntProviderRange;
  readonly chanceOfDripstoneColumnAtMaxDistanceFromCenter: number;
  readonly maxDistanceFromEdgeAffectingChanceOfDripstoneColumn: number;
  readonly maxDistanceFromCenterAffectingHeightBias: number;
  readonly count: IntProviderRange;
  readonly rarity: number | undefined;
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

const EXPECTED_EXTRA_GOLD: OreFeatureDescription = {
  states: ["minecraft:gold_ore", "minecraft:deepslate_gold_ore"],
  size: 9,
  discardChanceOnAirExposure: 0,
  count: 20,
  hasRange: true,
  hasSquare: true,
};

const EXPECTED_INFESTED: OreFeatureDescription = {
  states: ["minecraft:infested_stone", "minecraft:infested_deepslate"],
  size: 9,
  discardChanceOnAirExposure: 0,
  count: 7,
  hasRange: true,
  hasSquare: true,
};

const EXPECTED_EMERALD: ReplaceBlockFeatureDescription = {
  states: ["minecraft:emerald_ore", "minecraft:deepslate_emerald_ore"],
  hasRange: true,
  hasSquare: true,
};

const EXPECTED_SOFT_DISKS: readonly DiskFeatureDescription[] = [
  {
    state: "minecraft:sand",
    radius: { min: 2, max: 6 },
    halfHeight: 2,
    targets: ["minecraft:dirt", "minecraft:grass_block"],
    count: { min: 3, max: 3 },
    hasHeightmap: true,
    hasSquare: true,
  },
  {
    state: "minecraft:clay",
    radius: { min: 2, max: 3 },
    halfHeight: 1,
    targets: ["minecraft:dirt", "minecraft:clay"],
    count: { min: 1, max: 1 },
    hasHeightmap: true,
    hasSquare: true,
  },
  {
    state: "minecraft:gravel",
    radius: { min: 2, max: 5 },
    halfHeight: 2,
    targets: ["minecraft:dirt", "minecraft:grass_block"],
    count: { min: 1, max: 1 },
    hasHeightmap: true,
    hasSquare: true,
  },
] as const;

const EXPECTED_GLOW_LICHEN: GlowLichenFeatureDescription = {
  searchRange: 20,
  canPlaceOnFloor: false,
  canPlaceOnCeiling: true,
  canPlaceOnWall: true,
  chanceOfSpreading: 0.5,
  canBePlacedOnStates: [
    "minecraft:stone",
    "minecraft:andesite",
    "minecraft:diorite",
    "minecraft:granite",
    "minecraft:dripstone_block",
    "minecraft:calcite",
    "minecraft:tuff",
    "minecraft:deepslate",
  ],
  count: { min: 20, max: 30 },
  hasRange: true,
  hasSquare: true,
};

const EXPECTED_RARE_SMALL_DRIPSTONE: SmallDripstoneFeatureDescription = {
  maxPlacements: 5,
  emptySpaceSearchRadius: 10,
  maxOffsetFromOrigin: 2,
  chanceOfTallerDripstone: 0.2,
  count: { min: 40, max: 80 },
  rarity: 30,
  hasRange: true,
  hasSquare: true,
};

const EXPECTED_RARE_DRIPSTONE_CLUSTER: DripstoneClusterFeatureDescription = {
  floorToCeilingSearchRange: 12,
  height: { min: 3, max: 3 },
  radius: { min: 2, max: 6 },
  maxStalagmiteStalactiteHeightDiff: 1,
  heightDeviation: 3,
  dripstoneBlockLayerThickness: { min: 2, max: 2 },
  chanceOfDripstoneColumnAtMaxDistanceFromCenter: 0.1,
  maxDistanceFromEdgeAffectingChanceOfDripstoneColumn: 3,
  maxDistanceFromCenterAffectingHeightBias: 8,
  count: { min: 10, max: 10 },
  rarity: 25,
  hasRange: true,
  hasSquare: true,
};

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

function describeIntProvider(provider: ReturnType<CountConfiguration["count"]>): IntProviderRange {
  if (provider instanceof ConstantInt) {
    const value = (provider as unknown as { readonly value: number }).value;
    return { min: value, max: value };
  }

  if (provider instanceof UniformInt) {
    const uniform = provider as unknown as { readonly minInclusive: number; readonly maxInclusive: number };
    return { min: uniform.minInclusive, max: uniform.maxInclusive };
  }

  throw new Error(`Unsupported IntProvider ${provider.constructor.name}`);
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

function hasRootFeature(feature: ConfiguredFeature<any, any>, expected: unknown): boolean {
  return unwrapConfiguredFeature(feature).current.feature === expected;
}

function describeConfiguredOreFeature(feature: ConfiguredFeature<any, any>): OreFeatureDescription {
  const { current, decoratorConfigs } = unwrapConfiguredFeature(feature);

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

function describeConfiguredReplaceBlockFeature(feature: ConfiguredFeature<any, any>): ReplaceBlockFeatureDescription {
  const { current, decoratorConfigs } = unwrapConfiguredFeature(feature);

  expect(current.feature).toBe(Features.REPLACE_SINGLE_BLOCK);
  expect(current.config).toBeInstanceOf(ReplaceBlockConfiguration);
  const config = current.config as ReplaceBlockConfiguration;

  return {
    states: config.targetStates.map((targetState) => targetState.state.getBlock().getLocation()!.toString()),
    hasRange: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof RangeDecoratorConfiguration),
    hasSquare: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof NoneDecoratorConfiguration),
  };
}

function describeConfiguredDiskFeature(feature: ConfiguredFeature<any, any>): DiskFeatureDescription {
  const { current, decoratorConfigs } = unwrapConfiguredFeature(feature);

  expect(current.feature).toBe(Features.DISK);
  expect(current.config).toBeInstanceOf(DiskConfiguration);
  const config = current.config as DiskConfiguration;
  const countConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is CountConfiguration =>
    decoratorConfig instanceof CountConfiguration
  );

  return {
    state: config.state.getBlock().getLocation()!.toString(),
    radius: describeIntProvider(config.radius),
    halfHeight: config.halfHeight,
    targets: config.targets.map((state) => state.getBlock().getLocation()!.toString()),
    count: describeIntProvider(countConfig?.count() ?? ConstantInt.of(1)),
    hasHeightmap: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof HeightmapConfiguration),
    hasSquare: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof NoneDecoratorConfiguration),
  };
}

function describeConfiguredGlowLichenFeature(feature: ConfiguredFeature<any, any>): GlowLichenFeatureDescription {
  const { current, decoratorConfigs } = unwrapConfiguredFeature(feature);

  expect(current.feature).toBe(Features.GLOW_LICHEN);
  expect(current.config).toBeInstanceOf(GlowLichenConfiguration);
  const config = current.config as GlowLichenConfiguration;
  const countConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is CountConfiguration =>
    decoratorConfig instanceof CountConfiguration
  );

  return {
    searchRange: config.searchRange,
    canPlaceOnFloor: config.canPlaceOnFloor,
    canPlaceOnCeiling: config.canPlaceOnCeiling,
    canPlaceOnWall: config.canPlaceOnWall,
    chanceOfSpreading: config.chanceOfSpreading,
    canBePlacedOnStates: config.canBePlacedOnStates.map((state) => state.getBlock().getLocation()!.toString()),
    count: describeIntProvider(countConfig?.count() ?? ConstantInt.of(1)),
    hasRange: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof RangeDecoratorConfiguration),
    hasSquare: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof NoneDecoratorConfiguration),
  };
}

function describeConfiguredSmallDripstoneFeature(feature: ConfiguredFeature<any, any>): SmallDripstoneFeatureDescription {
  const { current, decoratorConfigs } = unwrapConfiguredFeature(feature);

  expect(current.feature).toBe(Features.SMALL_DRIPSTONE);
  expect(current.config).toBeInstanceOf(SmallDripstoneConfiguration);
  const config = current.config as SmallDripstoneConfiguration;
  const countConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is CountConfiguration =>
    decoratorConfig instanceof CountConfiguration
  );
  const rarityConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is ChanceDecoratorConfiguration =>
    decoratorConfig instanceof ChanceDecoratorConfiguration
  );

  return {
    maxPlacements: config.maxPlacements,
    emptySpaceSearchRadius: config.emptySpaceSearchRadius,
    maxOffsetFromOrigin: config.maxOffsetFromOrigin,
    chanceOfTallerDripstone: config.chanceOfTallerDripstone,
    count: describeIntProvider(countConfig?.count() ?? ConstantInt.of(1)),
    rarity: rarityConfig?.chance,
    hasRange: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof RangeDecoratorConfiguration),
    hasSquare: decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof NoneDecoratorConfiguration),
  };
}

function describeConfiguredDripstoneClusterFeature(feature: ConfiguredFeature<any, any>): DripstoneClusterFeatureDescription {
  const { current, decoratorConfigs } = unwrapConfiguredFeature(feature);

  expect(current.feature).toBe(Features.DRIPSTONE_CLUSTER);
  expect(current.config).toBeInstanceOf(DripstoneClusterConfiguration);
  const config = current.config as DripstoneClusterConfiguration;
  const countConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is CountConfiguration =>
    decoratorConfig instanceof CountConfiguration
  );
  const rarityConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is ChanceDecoratorConfiguration =>
    decoratorConfig instanceof ChanceDecoratorConfiguration
  );

  return {
    floorToCeilingSearchRange: config.floorToCeilingSearchRange,
    height: describeIntProvider(config.height),
    radius: describeIntProvider(config.radius),
    maxStalagmiteStalactiteHeightDiff: config.maxStalagmiteStalactiteHeightDiff,
    heightDeviation: config.heightDeviation,
    dripstoneBlockLayerThickness: describeIntProvider(config.dripstoneBlockLayerThickness),
    chanceOfDripstoneColumnAtMaxDistanceFromCenter: config.chanceOfDripstoneColumnAtMaxDistanceFromCenter,
    maxDistanceFromEdgeAffectingChanceOfDripstoneColumn: config.maxDistanceFromEdgeAffectingChanceOfDripstoneColumn,
    maxDistanceFromCenterAffectingHeightBias: config.maxDistanceFromCenterAffectingHeightBias,
    count: describeIntProvider(countConfig?.count() ?? ConstantInt.of(1)),
    rarity: rarityConfig?.chance,
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

    expect(plains.slice(0, EXPECTED_DEFAULT_UNDERGROUND_ORES.length).map((feature) => describeConfiguredOreFeature(feature()))).toEqual(
      EXPECTED_DEFAULT_UNDERGROUND_ORES,
    );
    expect(plains.slice(-EXPECTED_SOFT_DISKS.length).map((feature) => describeConfiguredDiskFeature(feature()))).toEqual(EXPECTED_SOFT_DISKS);
    expect(ocean.slice(0, EXPECTED_DEFAULT_UNDERGROUND_ORES.length).map((feature) => describeConfiguredOreFeature(feature()))).toEqual(
      EXPECTED_DEFAULT_UNDERGROUND_ORES,
    );
    expect(ocean.slice(-EXPECTED_SOFT_DISKS.length).map((feature) => describeConfiguredDiskFeature(feature()))).toEqual(EXPECTED_SOFT_DISKS);
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

  test("biome-specific underground extras match the vanilla badlands and mountain wiring", () => {
    registerGeneratedRenderBlocks();

    expect(describeConfiguredOreFeature(OreFeatures.ORE_GOLD_EXTRA)).toEqual(EXPECTED_EXTRA_GOLD);
    expect(describeConfiguredOreFeature(OreFeatures.ORE_INFESTED)).toEqual(EXPECTED_INFESTED);
    expect(describeConfiguredReplaceBlockFeature(OreFeatures.ORE_EMERALD)).toEqual(EXPECTED_EMERALD);

    const badlandsOres = getOverworldBiomeGenerationSettings("minecraft:badlands").features()[GenerationStep.Decoration.UNDERGROUND_ORES]!;
    expect(badlandsOres.slice(0, EXPECTED_DEFAULT_UNDERGROUND_ORES.length).map((feature) => describeConfiguredOreFeature(feature()))).toEqual(
      EXPECTED_DEFAULT_UNDERGROUND_ORES,
    );
    expect(describeConfiguredOreFeature(badlandsOres[EXPECTED_DEFAULT_UNDERGROUND_ORES.length]!())).toEqual(EXPECTED_EXTRA_GOLD);
    expect(badlandsOres.slice(-EXPECTED_SOFT_DISKS.length).map((feature) => describeConfiguredDiskFeature(feature()))).toEqual(EXPECTED_SOFT_DISKS);

    const mountainOres = getOverworldBiomeGenerationSettings("minecraft:mountains").features()[GenerationStep.Decoration.UNDERGROUND_ORES]!;
    expect(mountainOres.slice(0, EXPECTED_DEFAULT_UNDERGROUND_ORES.length).map((feature) => describeConfiguredOreFeature(feature()))).toEqual(
      EXPECTED_DEFAULT_UNDERGROUND_ORES,
    );
    expect(mountainOres.slice(EXPECTED_DEFAULT_UNDERGROUND_ORES.length, -1).map((feature) => describeConfiguredDiskFeature(feature()))).toEqual(
      EXPECTED_SOFT_DISKS,
    );
    expect(describeConfiguredReplaceBlockFeature(mountainOres.at(-1)!())).toEqual(EXPECTED_EMERALD);

    const mountainDecoration = getOverworldBiomeGenerationSettings("minecraft:mountains").features()[GenerationStep.Decoration.UNDERGROUND_DECORATION]!;
    const mountainUndergroundTail = mountainDecoration.slice(0, 2).map((feature) => feature());
    expect(hasRootFeature(mountainUndergroundTail[0]!, Features.DRIPSTONE_CLUSTER)).toBe(true);
    expect(hasRootFeature(mountainUndergroundTail[1]!, Features.SMALL_DRIPSTONE)).toBe(true);
    expect(describeConfiguredOreFeature(mountainDecoration.at(-1)!())).toEqual(EXPECTED_INFESTED);
  });

  test("soft disks and underground tail decoration match the vanilla overworld wiring", () => {
    registerGeneratedRenderBlocks();

    expect(
      [OreFeatures.DISK_SAND, OreFeatures.DISK_CLAY, OreFeatures.DISK_GRAVEL].map((feature) => describeConfiguredDiskFeature(feature)),
    ).toEqual(EXPECTED_SOFT_DISKS);
    expect(describeConfiguredGlowLichenFeature(OreFeatures.GLOW_LICHEN)).toEqual(EXPECTED_GLOW_LICHEN);
    expect(describeConfiguredSmallDripstoneFeature(OreFeatures.RARE_SMALL_DRIPSTONE_FEATURE)).toEqual(EXPECTED_RARE_SMALL_DRIPSTONE);
    expect(describeConfiguredDripstoneClusterFeature(OreFeatures.RARE_DRIPSTONE_CLUSTER_FEATURE)).toEqual(EXPECTED_RARE_DRIPSTONE_CLUSTER);

    const plainsVegetal = getOverworldBiomeGenerationSettings("minecraft:plains").features()[GenerationStep.Decoration.VEGETAL_DECORATION]!;
    const oceanVegetal = getOverworldBiomeGenerationSettings("minecraft:ocean").features()[GenerationStep.Decoration.VEGETAL_DECORATION]!;
    const plainsGlowLichen = plainsVegetal.map((feature) => feature()).filter((feature) => hasRootFeature(feature, Features.GLOW_LICHEN));
    const oceanGlowLichen = oceanVegetal.map((feature) => feature()).filter((feature) => hasRootFeature(feature, Features.GLOW_LICHEN));
    expect(plainsGlowLichen.map((feature) => describeConfiguredGlowLichenFeature(feature))).toEqual([EXPECTED_GLOW_LICHEN]);
    expect(oceanGlowLichen).toEqual([]);

    const plainsDecoration = getOverworldBiomeGenerationSettings("minecraft:plains").features()[GenerationStep.Decoration.UNDERGROUND_DECORATION]!;
    expect(plainsDecoration.map((feature) => feature()).filter((feature) => hasRootFeature(feature, Features.DRIPSTONE_CLUSTER)).map((feature) =>
      describeConfiguredDripstoneClusterFeature(feature)
    )).toEqual([EXPECTED_RARE_DRIPSTONE_CLUSTER]);
    expect(plainsDecoration.map((feature) => feature()).filter((feature) => hasRootFeature(feature, Features.SMALL_DRIPSTONE)).map((feature) =>
      describeConfiguredSmallDripstoneFeature(feature)
    )).toEqual([EXPECTED_RARE_SMALL_DRIPSTONE]);

    const swampOres = getOverworldBiomeGenerationSettings("minecraft:swamp").features()[GenerationStep.Decoration.UNDERGROUND_ORES]!;
    expect(describeConfiguredDiskFeature(swampOres.at(-1)!())).toEqual(EXPECTED_SOFT_DISKS[1]);
  });

  test("disk replace features only place from a water origin", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = new StaticRenderLevel(blocks.airState, 15, 15, 0, 32);
    const generator = createGenerator();
    const dirt = getState("minecraft:dirt");
    const grass = getState("minecraft:grass_block");
    const sand = getState("minecraft:sand");
    const water = getState("minecraft:water");
    const feature = Features.DISK.configured(new DiskConfiguration(sand, ConstantInt.of(2), 1, [dirt, grass]));
    const origin = new BlockPos(8, 11, 8);

    for (let z = 0; z < 16; z++) {
      for (let x = 0; x < 16; x++) {
        for (let y = 0; y < 10; y++) {
          level.setBlock(new BlockPos(x, y, z), dirt);
        }
        level.setBlock(new BlockPos(x, 10, z), grass);
      }
    }

    expect(feature.place(level, generator, new WorldgenRandom(0), origin)).toBe(false);
    expect(collectPositions(level, sand, 16)).toEqual([]);

    level.setBlock(origin, water);

    expect(feature.place(level, generator, new WorldgenRandom(0), origin)).toBe(true);
    expect(collectPositions(level, sand, 16).length).toBeGreaterThan(0);
  });
});
