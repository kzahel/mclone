import { BlockStateProperties } from "./state/properties/block-state-properties";
import { BooleanProperty } from "./state/properties/boolean-property";
import { StateDefinition } from "./state/state-definition";
import { Block } from "./block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";

export class SnowyDirtBlock extends Block {
  public static readonly SNOWY: BooleanProperty = BlockStateProperties.SNOWY;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.defaultBlockState().setValue(SnowyDirtBlock.SNOWY, false));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(SnowyDirtBlock.SNOWY);
  }
}
