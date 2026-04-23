import { BlockPos } from "../../../core/block-pos";
import { BlockTags } from "../../../tags/block-tags";
import type { WorldGenLevel } from "../world-gen-level";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import type { BlockState } from "./state/block-state";
import { BambooLeaves } from "./state/properties/bamboo-leaves";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { StateDefinition } from "./state/state-definition";

export class BambooBlock extends Block {
  public static readonly AGE = BlockStateProperties.AGE_1;
  public static readonly LEAVES = BlockStateProperties.BAMBOO_LEAVES;
  public static readonly STAGE = BlockStateProperties.STAGE;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(
      this.stateDefinition
        .any()
        .setValue(BambooBlock.AGE, 0)
        .setValue(BambooBlock.LEAVES, BambooLeaves.NONE)
        .setValue(BambooBlock.STAGE, 0),
    );
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    return level.getBlockState(pos.below()).is(BlockTags.BAMBOO_PLANTABLE_ON);
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(BambooBlock.AGE, BambooBlock.LEAVES, BambooBlock.STAGE);
  }
}
