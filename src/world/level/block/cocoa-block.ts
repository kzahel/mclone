import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { BlockTags } from "../../../tags/block-tags";
import type { WorldGenLevel } from "../world-gen-level";
import { Block } from "./block";
import { HorizontalDirectionalBlock } from "./horizontal-directional-block";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";
import { IntegerProperty } from "./state/properties/integer-property";

export class CocoaBlock extends HorizontalDirectionalBlock {
  public static readonly AGE: IntegerProperty = BlockStateProperties.AGE_2;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(HorizontalDirectionalBlock.FACING, Direction.NORTH).setValue(CocoaBlock.AGE, 0));
  }

  public override canSurvive(state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    return level.getBlockState(pos.relative(state.getValue(HorizontalDirectionalBlock.FACING))).is(BlockTags.JUNGLE_LOGS);
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(HorizontalDirectionalBlock.FACING, CocoaBlock.AGE);
  }
}
