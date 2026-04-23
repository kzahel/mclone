import { BlockPos } from "../../../core/block-pos";
import type { BlockGetter } from "../block-getter";
import type { WorldGenLevel } from "../world-gen-level";
import { Material } from "../material/material";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import type { BlockState } from "./state/block-state";

export class BushBlock extends Block {
  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  protected mayPlaceOn(state: BlockState, _level: BlockGetter, _pos: BlockPos): boolean {
    const material = state.getMaterial();
    return material === Material.DIRT || material === Material.GRASS;
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    const belowPos = pos.below();
    return this.mayPlaceOn(level.getBlockState(belowPos), level, belowPos);
  }

  public override propagatesSkylightDown(state: BlockState, _level: BlockGetter, _pos: import("../../../core/block-pos").BlockPos): boolean {
    return state.getFluidState().isEmpty();
  }
}
