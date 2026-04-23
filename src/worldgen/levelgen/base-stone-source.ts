import type { BlockPos } from "../../core/block-pos.ts";
import type { BlockState } from "../../world/level/block/state/block-state.ts";

export interface BaseStoneSource {
  getBaseBlock(pos: BlockPos): BlockState;
}

export class SingleBaseStoneSource implements BaseStoneSource {
  public constructor(private readonly state: BlockState) {}

  public getBaseBlock(_pos: BlockPos): BlockState {
    return this.state;
  }
}
