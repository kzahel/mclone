import { BlockPos } from "../../core/block-pos";
import { Heightmap } from "../../worldgen/levelgen/heightmap";
import type { BlockGetter } from "./block-getter";
import type { BlockState } from "./block/state/block-state";

export interface WorldGenLevel extends BlockGetter {
  setBlock(pos: BlockPos, state: BlockState, flags?: number): boolean;

  isEmptyBlock(pos: BlockPos): boolean;

  getHeightmapPos(type: Heightmap.Types, pos: BlockPos): BlockPos;

  getHeight(type: Heightmap.Types, x: number, z: number): number;

  getMinBuildHeight(): number;

  getMaxBuildHeight(): number;
}
