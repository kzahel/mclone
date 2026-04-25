import { Direction } from "../../../core/direction";
import { Block } from "./block";
import { HorizontalDirectionalBlock } from "./horizontal-directional-block";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { IntegerProperty } from "./state/properties/integer-property";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";

export class BeeNestBlock extends HorizontalDirectionalBlock {
  public static override readonly FACING = HorizontalDirectionalBlock.FACING;
  public static readonly HONEY_LEVEL: IntegerProperty = BlockStateProperties.LEVEL_HONEY;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(HorizontalDirectionalBlock.FACING, Direction.NORTH).setValue(BeeNestBlock.HONEY_LEVEL, 0));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(HorizontalDirectionalBlock.FACING, BeeNestBlock.HONEY_LEVEL);
  }
}
