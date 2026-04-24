import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import type { Block } from "../../../../src/world/level/block/block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { ConfiguredFeature } from "../../../../src/worldgen/levelgen/feature/configured-feature";
import { CountConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/count-configuration";
import { DecoratedFeatureConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorated-feature-configuration";
import type { DecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorator-configuration";
import { NoneDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/none-decorator-configuration";
import { ReplaceBlockConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/replace-block-configuration";
import { RangeDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/range-decorator-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { OreFeatures } from "../../../../src/worldgen/levelgen/feature/ore-features";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { WorldgenRandom } from "../../../../src/worldgen/prng/worldgen-random";
import { UniformInt } from "../../../../src/util/valueproviders/uniform-int";

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

function unwrapDecoratedFeature(feature: ConfiguredFeature<any, any>) {
  const decoratorConfigs: DecoratorConfiguration[] = [];
  let current = feature;

  while (current.config instanceof DecoratedFeatureConfiguration) {
    decoratorConfigs.push(current.config.decorator.config());
    current = current.config.feature();
  }

  return { current, decoratorConfigs };
}

describe("Replace block feature", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("replaces matching stone and deepslate origins with emerald ore variants", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = new StaticRenderLevel(blocks.airState, 15, 15, 0, 32);
    const generator = createGenerator();
    const feature = Features.REPLACE_SINGLE_BLOCK.configured(new ReplaceBlockConfiguration(OreFeatures.ORE_EMERALD_TARGET_LIST));
    const stonePos = new BlockPos(8, 8, 8);
    const deepslatePos = new BlockPos(9, 8, 8);
    const dirtPos = new BlockPos(10, 8, 8);

    level.setBlock(stonePos, getState("minecraft:stone"));
    level.setBlock(deepslatePos, getState("minecraft:deepslate"));
    level.setBlock(dirtPos, getState("minecraft:dirt"));

    expect(feature.place(level, generator, new WorldgenRandom(1n), stonePos)).toBe(true);
    expect(level.getBlockState(stonePos)).toBe(getState("minecraft:emerald_ore"));

    expect(feature.place(level, generator, new WorldgenRandom(2n), deepslatePos)).toBe(true);
    expect(level.getBlockState(deepslatePos)).toBe(getState("minecraft:deepslate_emerald_ore"));

    expect(feature.place(level, generator, new WorldgenRandom(3n), dirtPos)).toBe(true);
    expect(level.getBlockState(dirtPos)).toBe(getState("minecraft:dirt"));
  });

  test("emerald configured feature matches the vanilla replace-single-block defaults", () => {
    registerGeneratedRenderBlocks();

    const { current, decoratorConfigs } = unwrapDecoratedFeature(OreFeatures.ORE_EMERALD);
    expect(current.feature).toBe(Features.REPLACE_SINGLE_BLOCK);
    expect(current.config).toBeInstanceOf(ReplaceBlockConfiguration);

    const config = current.config as ReplaceBlockConfiguration;
    expect(config.targetStates.map((targetState) => targetState.state.getBlock().getLocation()!.toString())).toEqual([
      "minecraft:emerald_ore",
      "minecraft:deepslate_emerald_ore",
    ]);

    const countConfig = decoratorConfigs.find((decoratorConfig): decoratorConfig is CountConfiguration =>
      decoratorConfig instanceof CountConfiguration
    );

    expect(countConfig?.count()).toEqual(UniformInt.of(3, 8));
    expect(decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof RangeDecoratorConfiguration)).toBe(true);
    expect(decoratorConfigs.some((decoratorConfig) => decoratorConfig instanceof NoneDecoratorConfiguration)).toBe(true);
  });
});
