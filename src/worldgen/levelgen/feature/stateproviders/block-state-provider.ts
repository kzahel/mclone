import { BlockPos } from "../../../../core/block-pos";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";

export abstract class BlockStateProvider {
  public abstract getState(random: SimpleRandomSource, pos: BlockPos): BlockState;
}
