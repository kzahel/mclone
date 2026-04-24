import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { BlockTags } from "../../../tags/block-tags";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import type { Block } from "../../../world/level/block/block";
import { PointedDripstoneBlock } from "../../../world/level/block/pointed-dripstone-block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { DripstoneThickness } from "../../../world/level/block/state/properties/dripstone-thickness";
import { Fluids } from "../../../world/level/material/fluids";
import type { LevelSimulatedReader } from "../../../world/level/level-simulated-reader";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";

const DRIPSTONE_BLOCK_LOCATION = new ResourceLocation("minecraft:dripstone_block");
const POINTED_DRIPSTONE_LOCATION = new ResourceLocation("minecraft:pointed_dripstone");
const WATER_LOCATION = new ResourceLocation("minecraft:water");
const LAVA_LOCATION = new ResourceLocation("minecraft:lava");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function buildBaseToTipColumn(
  direction: Direction,
  length: number,
  mergeTip: boolean,
  consumer: (state: BlockState) => void,
): void {
  if (length >= 3) {
    consumer(createPointedDripstone(direction, DripstoneThickness.BASE));

    for (let index = 0; index < length - 3; index++) {
      consumer(createPointedDripstone(direction, DripstoneThickness.MIDDLE));
    }
  }

  if (length >= 2) {
    consumer(createPointedDripstone(direction, DripstoneThickness.FRUSTUM));
  }

  if (length >= 1) {
    consumer(createPointedDripstone(direction, mergeTip ? DripstoneThickness.TIP_MERGE : DripstoneThickness.TIP));
  }
}

function createPointedDripstone(direction: Direction, thickness: DripstoneThickness): BlockState {
  return getRequiredState(POINTED_DRIPSTONE_LOCATION)
    .setValue(PointedDripstoneBlock.TIP_DIRECTION, direction)
    .setValue(PointedDripstoneBlock.THICKNESS, thickness);
}

export class DripstoneUtils {
  public static growPointedDripstone(
    level: WorldGenLevel,
    pos: BlockPos,
    direction: Direction,
    length: number,
    mergeTip: boolean,
  ): void {
    const mutable = pos.mutable();
    buildBaseToTipColumn(direction, length, mergeTip, (state) => {
      const placedState = state.setValue(PointedDripstoneBlock.WATERLOGGED, level.getFluidState(mutable).getType().isSame(Fluids.WATER));
      level.setBlock(mutable, placedState, 2);
      mutable.move(direction);
    });
  }

  public static placeDripstoneBlockIfPossible(level: WorldGenLevel, pos: BlockPos): boolean {
    const state = level.getBlockState(pos);
    if (!state.is(BlockTags.DRIPSTONE_REPLACEABLE)) {
      return false;
    }

    level.setBlock(pos, getRequiredState(DRIPSTONE_BLOCK_LOCATION), 2);
    return true;
  }

  public static isDripstoneBaseOrLava(state: BlockState): boolean {
    return DripstoneUtils.isDripstoneBase(state) || state.is(getRequiredState(LAVA_LOCATION).getBlock());
  }

  public static isDripstoneBase(state: BlockState): boolean {
    return state.is(getRequiredState(DRIPSTONE_BLOCK_LOCATION).getBlock()) || state.is(BlockTags.DRIPSTONE_REPLACEABLE);
  }

  public static isEmptyOrWater(level: LevelSimulatedReader, pos: BlockPos): boolean;
  public static isEmptyOrWater(state: BlockState): boolean;
  public static isEmptyOrWater(first: LevelSimulatedReader | BlockState, second?: BlockPos): boolean {
    if (second === undefined) {
      const state = first as BlockState;
      return state.isAir() || state.is(getRequiredState(WATER_LOCATION).getBlock());
    }

    const level = first as LevelSimulatedReader;
    return level.isStateAtPosition(second, DripstoneUtils.isEmptyOrWater);
  }

  public static isEmptyOrWaterOrLava(level: LevelSimulatedReader, pos: BlockPos): boolean;
  public static isEmptyOrWaterOrLava(state: BlockState): boolean;
  public static isEmptyOrWaterOrLava(first: LevelSimulatedReader | BlockState, second?: BlockPos): boolean {
    if (second === undefined) {
      const state = first as BlockState;
      return state.isAir() || state.is(getRequiredState(WATER_LOCATION).getBlock()) || state.is(getRequiredState(LAVA_LOCATION).getBlock());
    }

    const level = first as LevelSimulatedReader;
    return level.isStateAtPosition(second, DripstoneUtils.isEmptyOrWaterOrLava);
  }
}
