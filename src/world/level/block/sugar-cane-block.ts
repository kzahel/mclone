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

export class SugarCaneBlock extends Block {
  public static readonly AGE = BlockStateProperties.AGE_15;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(SugarCaneBlock.AGE, 0));
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    const belowState = level.getBlockState(pos.below());
    if (belowState.is(this)) {
      return true;
    }

    const belowMaterial = belowState.getMaterial();
    if (belowMaterial !== Material.DIRT && belowMaterial !== Material.GRASS && belowMaterial !== Material.SAND) {
      return false;
    }

    const belowPos = pos.below();
    for (const direction of Direction.Plane.HORIZONTAL) {
      if (level.getFluidState(belowPos.relative(direction)).getType().isSame(Fluids.WATER)) {
        return true;
      }
    }

    return false;
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(SugarCaneBlock.AGE);
  }
}
