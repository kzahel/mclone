import type { BlockPos } from "../../core/block-pos";
import { Direction } from "../../core/direction";
import type { BlockGetter } from "./block-getter";
import type { ColorResolver } from "./color-resolver";
import { LightLayer } from "./light-layer";

export interface BlockAndTintGetter extends BlockGetter {
  getShade(direction: Direction, shade: boolean): number;

  getBrightness(layer: LightLayer, pos: BlockPos): number;

  getBlockTint(pos: BlockPos, resolver?: ColorResolver): number;
}
