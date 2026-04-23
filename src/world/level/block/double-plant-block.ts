import { BlockPos } from "../../../core/block-pos";
import { DoubleBlockHalf } from "./state/properties/double-block-half";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { EnumProperty } from "./state/properties/enum-property";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";
import { BushBlock } from "./bush-block";
import { BlockBehaviour } from "./state/block-behaviour";
import type { WorldGenLevel } from "../world-gen-level";

export class DoublePlantBlock extends BushBlock {
  public static readonly HALF: EnumProperty<DoubleBlockHalf> = BlockStateProperties.DOUBLE_BLOCK_HALF;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(DoublePlantBlock.HALF, DoubleBlockHalf.LOWER));
  }

  public override canSurvive(state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    if (state.getValue(DoublePlantBlock.HALF) !== DoubleBlockHalf.UPPER) {
      return super.canSurvive(state, level, pos);
    }

    const belowState = level.getBlockState(pos.below());
    return belowState.is(this) && belowState.getValue(DoublePlantBlock.HALF) === DoubleBlockHalf.LOWER;
  }

  public static placeAt(level: WorldGenLevel, state: BlockState, pos: BlockPos, flags: number): void {
    level.setBlock(pos, state.setValue(DoublePlantBlock.HALF, DoubleBlockHalf.LOWER), flags);
    level.setBlock(pos.above(), state.setValue(DoublePlantBlock.HALF, DoubleBlockHalf.UPPER), flags);
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<import("./block").Block, BlockState>): void {
    builder.add(DoublePlantBlock.HALF);
  }
}
