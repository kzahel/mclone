import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { BlockGetter } from "../block-getter";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluids } from "../material/fluids";
import { Block } from "./block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";

function scanForWater(level: BlockGetter, pos: BlockPos): boolean {
  for (const direction of Direction.values()) {
    if (level.getFluidState(pos.relative(direction)).getType().isSame(Fluids.WATER)) {
      return true;
    }
  }

  return false;
}

export class CoralBlock extends Block {
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
    if (!scanForWater(level, pos)) {
      level.getBlockTicks().scheduleTick(pos, this, 60);
    }

    return super.updateShape(state, direction, neighborState, level, pos, neighborPos);
  }

  public getDeadBlock(): Block {
    return this.deadBlock;
  }
}
