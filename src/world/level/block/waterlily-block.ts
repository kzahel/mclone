import { BlockPos } from "../../../core/block-pos";
import type { BlockGetter } from "../block-getter";
import { Fluids } from "../material/fluids";
import { Material } from "../material/material";
import { BushBlock } from "./bush-block";
import { BlockBehaviour } from "./state/block-behaviour";
import type { BlockState } from "./state/block-state";

export class WaterlilyBlock extends BushBlock {
  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  protected override mayPlaceOn(state: BlockState, level: BlockGetter, pos: BlockPos): boolean {
    const fluidState = level.getFluidState(pos);
    const aboveFluidState = level.getFluidState(pos.above());
    return (fluidState.getType().isSame(Fluids.WATER) || state.getMaterial() === Material.ICE) && aboveFluidState.getType().isSame(Fluids.EMPTY);
  }
}
