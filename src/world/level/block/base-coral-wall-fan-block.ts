import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { WorldGenLevel } from "../world-gen-level";
import { HorizontalDirectionalBlock } from "./horizontal-directional-block";
import { BaseCoralFanBlock } from "./base-coral-fan-block";
import { StateDefinition } from "./state/state-definition";
import type { Block } from "./block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";
import { BaseCoralPlantTypeBlock } from "./base-coral-plant-type-block";
import { Fluids } from "../material/fluids";

export class BaseCoralWallFanBlock extends BaseCoralFanBlock {
  public static readonly FACING = HorizontalDirectionalBlock.FACING;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(
      this.stateDefinition
        .any()
        .setValue(BaseCoralWallFanBlock.FACING, Direction.NORTH)
        .setValue(BaseCoralPlantTypeBlock.WATERLOGGED, true),
    );
  }

  public override updateShape(
    state: BlockState,
    direction: Direction,
    neighborState: BlockState,
    level: WorldGenLevel,
    pos: BlockPos,
    neighborPos: BlockPos,
  ): BlockState {
    if (state.getValue(BaseCoralPlantTypeBlock.WATERLOGGED)) {
      level.getLiquidTicks().scheduleTick(pos, Fluids.WATER, 1);
    }

    return direction.getOpposite() === state.getValue(BaseCoralWallFanBlock.FACING) && !this.canSurvive(state, level, pos)
      ? this.airState()
      : super.updateShape(state, direction, neighborState, level, pos, neighborPos);
  }

  public override canSurvive(state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    const facing = state.getValue(BaseCoralWallFanBlock.FACING);
    const supportPos = pos.relative(facing.getOpposite());
    return level.getBlockState(supportPos).isFaceSturdy(level, supportPos, facing);
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(BaseCoralWallFanBlock.FACING, BaseCoralPlantTypeBlock.WATERLOGGED);
  }
}
