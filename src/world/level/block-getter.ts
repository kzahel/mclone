import type { BlockPos } from "../../core/block-pos";
import type { BlockState } from "./block/state/block-state";
import type { FluidState } from "./material/fluid-state";

export interface BlockGetter {
  getBlockState(pos: BlockPos): BlockState;

  getFluidState(pos: BlockPos): FluidState;

  getMaxLightLevel(): number;
}
