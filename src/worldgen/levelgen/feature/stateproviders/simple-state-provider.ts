import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { BlockStateProvider } from "./block-state-provider";

export class SimpleStateProvider extends BlockStateProvider {
  public constructor(private readonly state: BlockState) {
    super();
  }

  public override getState(_random: SimpleRandomSource, _pos: BlockPos): BlockState {
    return this.state;
  }
}
