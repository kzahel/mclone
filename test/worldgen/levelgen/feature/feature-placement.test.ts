import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { Heightmap } from "../../../../src/worldgen/levelgen/heightmap";
import { SimpleBlockPlacer } from "../../../../src/worldgen/levelgen/feature/blockplacers/simple-block-placer";
import { CountConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/count-configuration";
import type { DecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/decorator-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { HeightmapConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/heightmap-configuration";
import { NoneDecoratorConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/none-decorator-configuration";
import { RandomPatchConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/random-patch-configuration";
import { SimpleBlockConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/simple-block-configuration";
import { SimpleStateProvider } from "../../../../src/worldgen/levelgen/feature/stateproviders/simple-state-provider";
import { DecorationContext } from "../../../../src/worldgen/levelgen/placement/decoration-context";
import type { ConfiguredDecorator } from "../../../../src/worldgen/levelgen/placement/configured-decorator";
import { FeatureDecorators } from "../../../../src/worldgen/levelgen/placement/feature-decorators";
import { WorldgenRandom } from "../../../../src/worldgen/prng/worldgen-random";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import type { Block } from "../../../../src/world/level/block/block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";

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

function createFlatLevel(airState: BlockState, surfaceState: BlockState): StaticRenderLevel {
  const level = new StaticRenderLevel(airState, 15, 15, 0, 64);
  for (let z = 0; z < 48; z++) {
    for (let x = 0; x < 48; x++) {
      level.setBlock(new BlockPos(x, 10, z), surfaceState);
    }
  }

  return level;
}

function createCountSquareHeightmapDecorator(count: number) {
  let decorator: ConfiguredDecorator<DecoratorConfiguration> = FeatureDecorators.HEIGHTMAP.configured(
    new HeightmapConfiguration(Heightmap.Types.WORLD_SURFACE_WG),
  );
  decorator = decorator.decorated(FeatureDecorators.SQUARE.configured(NoneDecoratorConfiguration.INSTANCE));
  decorator = decorator.decorated(FeatureDecorators.COUNT.configured(new CountConfiguration(count)));
  return decorator;
}

describe("Feature placement", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("count + square + heightmap decorator projects repeated positions onto the surface", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();
    const positions = createCountSquareHeightmapDecorator(4).getPositions(
      new DecorationContext(level, generator),
      new WorldgenRandom(12345n),
      new BlockPos(0, 0, 0),
    );

    expect(positions).toHaveLength(4);
    for (const pos of positions) {
      expect(pos.getX()).toBeGreaterThanOrEqual(0);
      expect(pos.getX()).toBeLessThan(16);
      expect(pos.getZ()).toBeGreaterThanOrEqual(0);
      expect(pos.getZ()).toBeLessThan(16);
      expect(pos.getY()).toBe(11);
    }
  });

  test("random patch feature places vegetation onto the translated grass surface", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();
    const grassPlantState = getState("minecraft:grass");
    const feature = Features.RANDOM_PATCH.configured(
      new RandomPatchConfiguration(
        new SimpleStateProvider(grassPlantState),
        SimpleBlockPlacer.INSTANCE,
        new Set(),
        new Set(),
        24,
        4,
        2,
        4,
        false,
        false,
        false,
      ),
    ).decorated(createCountSquareHeightmapDecorator(2));

    const placed = feature.place(level, generator, new WorldgenRandom(6789n), new BlockPos(0, 0, 0));

    expect(placed).toBe(true);

    let plantCount = 0;
    for (let z = 0; z < 16; z++) {
      for (let x = 0; x < 16; x++) {
        const pos = new BlockPos(x, 11, z);
        if (level.getBlockState(pos).is(grassPlantState.getBlock())) {
          plantCount++;
          expect(level.getBlockState(pos.below()).getBlock()).toBe(getState("minecraft:grass_block").getBlock());
        }
      }
    }

    expect(plantCount).toBeGreaterThan(0);
  });

  test("simple block feature enforces translated sugar cane survival and allows stacking", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    const generator = createGenerator();
    const sugarCaneState = getState("minecraft:sugar_cane");
    const sandState = getState("minecraft:sand");
    const waterState = getState("minecraft:water");
    const feature = Features.SIMPLE_BLOCK.configured(new SimpleBlockConfiguration(new SimpleStateProvider(sugarCaneState)));

    level.setBlock(new BlockPos(8, 10, 8), sandState);
    level.setBlock(new BlockPos(9, 10, 8), waterState);

    expect(feature.place(level, generator, new WorldgenRandom(1n), new BlockPos(8, 11, 8))).toBe(true);
    expect(feature.place(level, generator, new WorldgenRandom(2n), new BlockPos(8, 12, 8))).toBe(true);

    const dryLevel = createFlatLevel(blocks.airState, getState("minecraft:grass_block"));
    dryLevel.setBlock(new BlockPos(8, 10, 8), sandState);

    expect(feature.place(dryLevel, generator, new WorldgenRandom(3n), new BlockPos(8, 11, 8))).toBe(false);
  });
});
