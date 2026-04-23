import { Direction } from "../../../core/direction";
import { BlockPos } from "../../../core/block-pos";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluids } from "../material/fluids";
import { Material } from "../material/material";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";

export class CactusBlock extends Block {
  public static readonly AGE = BlockStateProperties.AGE_15;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(CactusBlock.AGE, 0));
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    for (const direction of Direction.Plane.HORIZONTAL) {
      const neighborPos = pos.relative(direction);
      const neighborState = level.getBlockState(neighborPos);
      if (neighborState.getMaterial().isSolid() || level.getFluidState(neighborPos).getType().isSame(Fluids.LAVA)) {
        return false;
      }
    }

    const belowState = level.getBlockState(pos.below());
    return (belowState.is(this) || belowState.getMaterial() === Material.SAND) && !level.getBlockState(pos.above()).getMaterial().isLiquid();
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(CactusBlock.AGE);
  }
}
