import type { BlockGetter } from "../block-getter";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import type { BlockState } from "./state/block-state";

export class BushBlock extends Block {
  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  public override propagatesSkylightDown(state: BlockState, _level: BlockGetter, _pos: import("../../../core/block-pos").BlockPos): boolean {
    return state.getFluidState().isEmpty();
  }
}
