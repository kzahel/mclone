import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";

export class CactusBlock extends Block {
  public static readonly AGE = BlockStateProperties.AGE_15;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(CactusBlock.AGE, 0));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(CactusBlock.AGE);
  }
}
