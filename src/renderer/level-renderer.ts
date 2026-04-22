import type { BlockPos } from "../core/block-pos";
import type { BlockAndTintGetter } from "../world/level/block-and-tint-getter";
import { LightLayer } from "../world/level/light-layer";
import type { BlockState } from "../world/level/block/state/block-state";

export class LevelRenderer {
  public static getLightColor(level: BlockAndTintGetter, pos: BlockPos): number {
    return LevelRenderer.getLightColorFromState(level, level.getBlockState(pos), pos);
  }

  public static getLightColorFromState(level: BlockAndTintGetter, state: BlockState, pos: BlockPos): number {
    if (state.emissiveRendering(level, pos)) {
      return 15_728_880;
    }

    const sky = level.getBrightness(LightLayer.SKY, pos);
    let block = level.getBrightness(LightLayer.BLOCK, pos);
    const lightEmission = state.getLightEmission();
    if (block < lightEmission) {
      block = lightEmission;
    }

    return (sky << 20) | (block << 4);
  }
}
