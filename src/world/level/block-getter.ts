import type { BlockPos } from "../../core/block-pos";
import type { BlockState } from "./block/state/block-state";

export interface BlockGetter {
  getBlockState(pos: BlockPos): BlockState;

  getMaxLightLevel(): number;
}
