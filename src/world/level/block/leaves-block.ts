import type { BlockGetter } from "../block-getter";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";

export class LeavesBlock extends Block {
  public static readonly DISTANCE = BlockStateProperties.DISTANCE;
  public static readonly PERSISTENT = BlockStateProperties.PERSISTENT;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(LeavesBlock.DISTANCE, 7).setValue(LeavesBlock.PERSISTENT, false));
  }

  public override getLightBlock(_state: BlockState, _level: BlockGetter, _pos: import("../../../core/block-pos").BlockPos): number {
    return 1;
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(LeavesBlock.DISTANCE, LeavesBlock.PERSISTENT);
  }
}
