import { BlockStateProperties } from "./state/properties/block-state-properties";
import { IntegerProperty } from "./state/properties/integer-property";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";
import { BushBlock } from "./bush-block";
import { BlockBehaviour } from "./state/block-behaviour";

export class SweetBerryBushBlock extends BushBlock {
  public static readonly MAX_AGE = 3;
  public static readonly AGE: IntegerProperty = BlockStateProperties.AGE_3;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(SweetBerryBushBlock.AGE, 0));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<import("./block").Block, BlockState>): void {
    builder.add(SweetBerryBushBlock.AGE);
  }
}
