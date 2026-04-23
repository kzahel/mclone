import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluids } from "../material/fluids";
import { BaseCoralPlantTypeBlock } from "./base-coral-plant-type-block";
import { BaseCoralWallFanBlock } from "./base-coral-wall-fan-block";
import { Block } from "./block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";

export class CoralWallFanBlock extends BaseCoralWallFanBlock {
  public constructor(
    private readonly deadBlock: Block,
    properties: BlockBehaviour.Properties,
  ) {
    super(properties);
  }

  public override updateShape(
    state: BlockState,
    direction: Direction,
    neighborState: BlockState,
    level: WorldGenLevel,
    pos: BlockPos,
    neighborPos: BlockPos,
  ): BlockState {
    if (direction.getOpposite() === state.getValue(BaseCoralWallFanBlock.FACING) && !this.canSurvive(state, level, pos)) {
      return this.airState();
    }

    if (state.getValue(BaseCoralPlantTypeBlock.WATERLOGGED)) {
      level.getLiquidTicks().scheduleTick(pos, Fluids.WATER, 1);
    }

    this.tryScheduleDieTick(state, level, pos);
    return super.updateShape(state, direction, neighborState, level, pos, neighborPos);
  }

  public getDeadBlock(): Block {
    return this.deadBlock;
  }
}
