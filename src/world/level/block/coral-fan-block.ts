import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluids } from "../material/fluids";
import { BaseCoralFanBlock } from "./base-coral-fan-block";
import { BaseCoralPlantTypeBlock } from "./base-coral-plant-type-block";
import { Block } from "./block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";

export class CoralFanBlock extends BaseCoralFanBlock {
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
    if (direction === Direction.DOWN && !this.canSurvive(state, level, pos)) {
      return this.airState();
    }

    this.tryScheduleDieTick(state, level, pos);
    if (state.getValue(BaseCoralPlantTypeBlock.WATERLOGGED)) {
      level.getLiquidTicks().scheduleTick(pos, Fluids.WATER, 1);
    }

    return super.updateShape(state, direction, neighborState, level, pos, neighborPos);
  }

  public getDeadBlock(): Block {
    return this.deadBlock;
  }
}
