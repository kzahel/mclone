import { ResourceLocation } from "../../../core/resource-location";
import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { BlockGetter } from "../block-getter";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluids } from "../material/fluids";
import type { FluidState } from "../material/fluid-state";
import { DoubleBlockHalf } from "./state/properties/double-block-half";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";
import { DoublePlantBlock } from "./double-plant-block";

const MAGMA_BLOCK_LOCATION = new ResourceLocation("minecraft:magma_block");

export class TallSeagrassBlock extends DoublePlantBlock {
  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  protected override mayPlaceOn(state: BlockState, level: BlockGetter, pos: BlockPos): boolean {
    return state.isFaceSturdy(level, pos, Direction.UP) && state.getBlock().getLocation()?.toString() !== MAGMA_BLOCK_LOCATION.toString();
  }

  public override canSurvive(state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    if (state.getValue(DoublePlantBlock.HALF) === DoubleBlockHalf.UPPER) {
      const belowState = level.getBlockState(pos.below());
      return belowState.is(this) && belowState.getValue(DoublePlantBlock.HALF) === DoubleBlockHalf.LOWER;
    }

    const fluidState = level.getFluidState(pos);
    return super.canSurvive(state, level, pos) && fluidState.getType().isSame(Fluids.WATER) && fluidState.getAmount() === 8;
  }

  public override getFluidState(_state: BlockState): FluidState {
    return Fluids.WATER.defaultFluidState();
  }
}
