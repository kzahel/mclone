import { beforeEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../../../src/core/block-pos";
import { Direction } from "../../../../src/core/direction";
import { Registry } from "../../../../src/core/registry";
import { ResourceLocation } from "../../../../src/core/resource-location";
import { ConstantInt } from "../../../../src/util/valueproviders/constant-int";
import { UniformFloat } from "../../../../src/util/valueproviders/uniform-float";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { StaticRenderLevel } from "../../../../src/world/level/static-render-level";
import type { Block } from "../../../../src/world/level/block/block";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { BlockStateProperties } from "../../../../src/world/level/block/state/properties/block-state-properties";
import { GlowLichenConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/glow-lichen-configuration";
import { DripstoneClusterConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/dripstone-cluster-configuration";
import { SmallDripstoneConfiguration } from "../../../../src/worldgen/levelgen/feature/configurations/small-dripstone-configuration";
import { Features } from "../../../../src/worldgen/levelgen/feature/features";
import { NoiseBasedChunkGenerator } from "../../../../src/worldgen/levelgen/noise-based-chunk-generator";
import { OverworldBiomeSource } from "../../../../src/worldgen/biome/overworld-biome-source";
import { WorldgenRandom } from "../../../../src/worldgen/prng/worldgen-random";

function getState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing block ${location}`);
  }

  return block.defaultBlockState();
}

function createGenerator(): NoiseBasedChunkGenerator {
  return new NoiseBasedChunkGenerator(new OverworldBiomeSource(12345n), 12345n);
}

function countBlocks(level: StaticRenderLevel, location: string, min: BlockPos, max: BlockPos): number {
  const target = getState(location).getBlock();
  let count = 0;
  for (let y = min.getY(); y <= max.getY(); y++) {
    for (let z = min.getZ(); z <= max.getZ(); z++) {
      for (let x = min.getX(); x <= max.getX(); x++) {
        if (level.getBlockState(new BlockPos(x, y, z)).is(target)) {
          count++;
        }
      }
    }
  }

  return count;
}

describe("Underground decoration features", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
  });

  test("glow lichen attaches to a supported face", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = new StaticRenderLevel(blocks.airState, 15, 15, 0, 32);
    const stoneState = getState("minecraft:stone");
    const origin = new BlockPos(8, 8, 8);
    level.setBlock(origin.east(), stoneState);

    const feature = Features.GLOW_LICHEN.configured(new GlowLichenConfiguration(1, false, false, true, 0.0, [stoneState]));

    expect(feature.place(level, createGenerator(), new WorldgenRandom(1n), origin)).toBe(true);

    const lichenState = level.getBlockState(origin);
    expect(lichenState.getBlock().getLocation()!.toString()).toBe("minecraft:glow_lichen");
    expect(lichenState.getValue(BlockStateProperties.EAST)).toBe(true);
    expect(lichenState.getValue(BlockStateProperties.WATERLOGGED)).toBe(false);
  });

  test("small dripstone places a pointed dripstone and converts a nearby base block", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = new StaticRenderLevel(blocks.airState, 15, 15, 0, 32);
    const stoneState = getState("minecraft:stone");
    const dripstoneBlock = getState("minecraft:dripstone_block").getBlock();
    const pointedDripstone = getState("minecraft:pointed_dripstone").getBlock();
    const origin = new BlockPos(8, 8, 8);

    level.setBlock(origin.below(), stoneState);
    level.setBlock(origin.above(), stoneState);

    const feature = Features.SMALL_DRIPSTONE.configured(new SmallDripstoneConfiguration(1, 1, 0, 0.0));

    expect(feature.place(level, createGenerator(), new WorldgenRandom(2n), origin)).toBe(true);

    const state = level.getBlockState(origin);
    expect(state.is(pointedDripstone)).toBe(true);
    expect([Direction.UP, Direction.DOWN]).toContain(state.getValue(BlockStateProperties.VERTICAL_DIRECTION));
    expect(level.getBlockState(origin.above()).is(dripstoneBlock) || level.getBlockState(origin.below()).is(dripstoneBlock)).toBe(true);
  });

  test("dripstone cluster grows pointed dripstone inside a simple stone cavity", () => {
    const blocks = registerGeneratedRenderBlocks();
    const level = new StaticRenderLevel(blocks.airState, 15, 15, 0, 48);
    const stoneState = getState("minecraft:stone");
    const origin = new BlockPos(12, 18, 12);

    level.setBlock(new BlockPos(origin.getX(), 12, origin.getZ()), stoneState);
    level.setBlock(new BlockPos(origin.getX(), 24, origin.getZ()), stoneState);

    const feature = Features.DRIPSTONE_CLUSTER.configured(
      new DripstoneClusterConfiguration(
        12,
        ConstantInt.of(2),
        ConstantInt.of(0),
        0,
        0,
        ConstantInt.of(1),
        UniformFloat.of(0.99, 1.0),
        UniformFloat.of(0.0, 0.01),
        1.0,
        1,
        1,
      ),
    );

    expect(feature.place(level, createGenerator(), new WorldgenRandom(3n), origin)).toBe(true);
    expect(countBlocks(level, "minecraft:pointed_dripstone", origin.offset(-1, -8, -1), origin.offset(1, 8, 1))).toBeGreaterThan(0);
  });
});
