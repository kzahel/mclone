import { BlockPos } from "../../../core/block-pos";
import { BlockTags } from "../../../tags/block-tags";
import type { BlockGetter } from "../block-getter";
import { LightLayer } from "../light-layer";
import type { WorldGenLevel } from "../world-gen-level";
import type { BlockState } from "./state/block-state";
import { BushBlock } from "./bush-block";
import { BlockBehaviour } from "./state/block-behaviour";

interface RawBrightnessReader {
  getRawBrightness(pos: BlockPos, amount: number): number;
}

function hasRawBrightnessReader(level: BlockGetter): level is BlockGetter & RawBrightnessReader {
  return "getRawBrightness" in level && typeof level.getRawBrightness === "function";
}

export class MushroomBlock extends BushBlock {
  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  protected override mayPlaceOn(state: BlockState, level: BlockGetter, pos: BlockPos): boolean {
    return state.isSolidRender(level, pos);
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    const belowPos = pos.below();
    const belowState = level.getBlockState(belowPos);
    if (belowState.is(BlockTags.MUSHROOM_GROW_BLOCK)) {
      return true;
    }

    const rawBrightness = hasRawBrightnessReader(level)
      ? level.getRawBrightness(pos, 0)
      : ("getBrightness" in level && typeof level.getBrightness === "function" ? level.getBrightness(LightLayer.SKY, pos) : level.getMaxLightLevel());
    return rawBrightness < 13 && this.mayPlaceOn(belowState, level, belowPos);
  }
}
