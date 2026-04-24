import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { BlockTags } from "../../../tags/block-tags";
import { ClampedNormalFloat } from "../../../util/valueproviders/clamped-normal-float";
import type { Block } from "../../../world/level/block/block";
import { Fluids } from "../../../world/level/material/fluids";
import { Column } from "../column";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { DripstoneClusterConfiguration } from "./configurations/dripstone-cluster-configuration";
import { DripstoneUtils } from "./dripstone-utils";

const WATER_LOCATION = new ResourceLocation("minecraft:water");
const DRIPSTONE_BLOCK_LOCATION = new ResourceLocation("minecraft:dripstone_block");
const POINTED_DRIPSTONE_LOCATION = new ResourceLocation("minecraft:pointed_dripstone");
const LAVA_LOCATION = new ResourceLocation("minecraft:lava");

function getRequiredBlock(location: ResourceLocation): Block {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function clampedMap(value: number, fromMin: number, fromMax: number, toMin: number, toMax: number): number {
  if (fromMax === fromMin) {
    return toMin;
  }

  const delta = clamp((value - fromMin) / (fromMax - fromMin), 0, 1);
  return toMin + (delta * (toMax - toMin));
}

function randomBetweenInclusive(
  random: import("../../prng/simple-random-source").SimpleRandomSource,
  minInclusive: number,
  maxInclusive: number,
): number {
  return minInclusive + random.nextInt((maxInclusive - minInclusive) + 1);
}

function atY(pos: BlockPos, y: number): BlockPos {
  return new BlockPos(pos.getX(), y, pos.getZ());
}

export class DripstoneClusterFeature extends Feature<DripstoneClusterConfiguration> {
  public override place(context: FeaturePlaceContext<DripstoneClusterConfiguration>): boolean {
    const level = context.level();
    const origin = context.origin();
    const config = context.config();
    const random = context.random();
    if (!DripstoneUtils.isEmptyOrWater(level, origin)) {
      return false;
    }

    const height = config.height.sample(random);
    const wetness = config.wetness.sample(random);
    const density = config.density.sample(random);
    const radiusX = config.radius.sample(random);
    const radiusZ = config.radius.sample(random);

    for (let offsetX = -radiusX; offsetX <= radiusX; offsetX++) {
      for (let offsetZ = -radiusZ; offsetZ <= radiusZ; offsetZ++) {
        const chance = this.getChanceOfStalagmiteOrStalactite(radiusX, radiusZ, offsetX, offsetZ, config);
        const pos = origin.offset(offsetX, 0, offsetZ);
        this.placeColumn(level, random, pos, offsetX, offsetZ, wetness, chance, height, density, config);
      }
    }

    return true;
  }

  private placeColumn(
    level: import("../../../world/level/world-gen-level").WorldGenLevel,
    random: import("../../prng/simple-random-source").SimpleRandomSource,
    pos: BlockPos,
    offsetX: number,
    offsetZ: number,
    wetness: number,
    chance: number,
    height: number,
    density: number,
    config: DripstoneClusterConfiguration,
  ): void {
    const column = Column.scan(level, pos, config.floorToCeilingSearchRange, DripstoneUtils.isEmptyOrWater, DripstoneUtils.isDripstoneBaseOrLava);
    if (column === undefined) {
      return;
    }

    const ceiling = column.getCeiling();
    const floor = column.getFloor();
    if (ceiling === undefined && floor === undefined) {
      return;
    }

    const placePool = random.nextFloat() < wetness;
    let workingColumn = column;
    if (placePool && floor !== undefined && this.canPlacePool(level, atY(pos, floor))) {
      workingColumn = column.withFloor(floor - 1);
      level.setBlock(atY(pos, floor), getRequiredBlock(WATER_LOCATION).defaultBlockState(), 2);
    }

    const adjustedFloor = workingColumn.getFloor();
    let stalactiteHeight: number;
    if (ceiling !== undefined && random.nextDouble() < chance && !this.isLava(level, atY(pos, ceiling))) {
      const blockLayerThickness = config.dripstoneBlockLayerThickness.sample(random);
      this.replaceBlocksWithDripstoneBlocks(level, atY(pos, ceiling), blockLayerThickness, Direction.UP);
      const maxHeight = adjustedFloor !== undefined ? Math.min(height, ceiling - adjustedFloor) : height;
      stalactiteHeight = this.getDripstoneHeight(random, offsetX, offsetZ, density, maxHeight, config);
    } else {
      stalactiteHeight = 0;
    }

    let stalagmiteHeight: number;
    if (adjustedFloor !== undefined && random.nextDouble() < chance && !this.isLava(level, atY(pos, adjustedFloor))) {
      const blockLayerThickness = config.dripstoneBlockLayerThickness.sample(random);
      this.replaceBlocksWithDripstoneBlocks(level, atY(pos, adjustedFloor), blockLayerThickness, Direction.DOWN);
      stalagmiteHeight = Math.max(
        0,
        stalactiteHeight + randomBetweenInclusive(random, -config.maxStalagmiteStalactiteHeightDiff, config.maxStalagmiteStalactiteHeightDiff),
      );
    } else {
      stalagmiteHeight = 0;
    }

    let finalStalactiteHeight: number;
    let finalStalagmiteHeight: number;
    if (ceiling !== undefined && adjustedFloor !== undefined && ceiling - stalactiteHeight <= adjustedFloor + stalagmiteHeight) {
      const minHeight = Math.max(ceiling - stalactiteHeight, adjustedFloor + 1);
      const maxHeight = Math.min(adjustedFloor + stalagmiteHeight, ceiling - 1);
      const sharedTip = randomBetweenInclusive(random, minHeight, maxHeight + 1);
      finalStalactiteHeight = ceiling - sharedTip;
      finalStalagmiteHeight = (sharedTip - 1) - adjustedFloor;
    } else {
      finalStalactiteHeight = stalactiteHeight;
      finalStalagmiteHeight = stalagmiteHeight;
    }

    const columnHeight = workingColumn.getHeight();
    const mergeTip =
      random.nextBoolean()
      && finalStalactiteHeight > 0
      && finalStalagmiteHeight > 0
      && columnHeight !== undefined
      && finalStalactiteHeight + finalStalagmiteHeight === columnHeight;
    if (ceiling !== undefined) {
      DripstoneUtils.growPointedDripstone(level, atY(pos, ceiling - 1), Direction.DOWN, finalStalactiteHeight, mergeTip);
    }

    if (adjustedFloor !== undefined) {
      DripstoneUtils.growPointedDripstone(level, atY(pos, adjustedFloor + 1), Direction.UP, finalStalagmiteHeight, mergeTip);
    }
  }

  private isLava(level: import("../../../world/level/block-getter").BlockGetter, pos: BlockPos): boolean {
    return level.getBlockState(pos).is(getRequiredBlock(LAVA_LOCATION));
  }

  private getDripstoneHeight(
    random: import("../../prng/simple-random-source").SimpleRandomSource,
    offsetX: number,
    offsetZ: number,
    density: number,
    maxHeight: number,
    config: DripstoneClusterConfiguration,
  ): number {
    if (random.nextFloat() > density) {
      return 0;
    }

    const distance = Math.abs(offsetX) + Math.abs(offsetZ);
    const mean = clampedMap(distance, 0, config.maxDistanceFromCenterAffectingHeightBias, maxHeight / 2.0, 0.0);
    return Math.trunc(DripstoneClusterFeature.randomBetweenBiased(random, 0.0, maxHeight, mean, config.heightDeviation));
  }

  private canPlacePool(level: import("../../../world/level/world-gen-level").WorldGenLevel, pos: BlockPos): boolean {
    const state = level.getBlockState(pos);
    if (state.is(getRequiredBlock(WATER_LOCATION)) || state.is(getRequiredBlock(DRIPSTONE_BLOCK_LOCATION)) || state.is(getRequiredBlock(POINTED_DRIPSTONE_LOCATION))) {
      return false;
    }

    for (const direction of Direction.Plane.HORIZONTAL) {
      if (!this.canBeAdjacentToWater(level, pos.relative(direction))) {
        return false;
      }
    }

    return this.canBeAdjacentToWater(level, pos.below());
  }

  private canBeAdjacentToWater(level: import("../../../world/level/block-getter").BlockGetter, pos: BlockPos): boolean {
    const state = level.getBlockState(pos);
    return state.is(BlockTags.BASE_STONE_OVERWORLD) || level.getFluidState(pos).getType().isSame(Fluids.WATER);
  }

  private replaceBlocksWithDripstoneBlocks(
    level: import("../../../world/level/world-gen-level").WorldGenLevel,
    pos: BlockPos,
    length: number,
    direction: Direction,
  ): void {
    const mutable = pos.mutable();
    for (let index = 0; index < length; index++) {
      if (!DripstoneUtils.placeDripstoneBlockIfPossible(level, mutable)) {
        return;
      }

      mutable.move(direction);
    }
  }

  private getChanceOfStalagmiteOrStalactite(
    radiusX: number,
    radiusZ: number,
    offsetX: number,
    offsetZ: number,
    config: DripstoneClusterConfiguration,
  ): number {
    const remainingX = radiusX - Math.abs(offsetX);
    const remainingZ = radiusZ - Math.abs(offsetZ);
    const minRemaining = Math.min(remainingX, remainingZ);
    return clampedMap(
      minRemaining,
      0,
      config.maxDistanceFromEdgeAffectingChanceOfDripstoneColumn,
      config.chanceOfDripstoneColumnAtMaxDistanceFromCenter,
      1.0,
    );
  }

  private static randomBetweenBiased(
    random: import("../../prng/simple-random-source").SimpleRandomSource,
    min: number,
    max: number,
    mean: number,
    deviation: number,
  ): number {
    return ClampedNormalFloat.sample(random, mean, deviation, min, max);
  }
}
