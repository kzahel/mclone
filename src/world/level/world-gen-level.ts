import { BlockPos } from "../../core/block-pos";
import { Heightmap } from "../../worldgen/levelgen/heightmap";
import type { BlockGetter } from "./block-getter";
import type { BlockState } from "./block/state/block-state";
import type { LevelSimulatedReader } from "./level-simulated-reader";

export interface WorldGenLevel extends BlockGetter, LevelSimulatedReader {
  setBlock(pos: BlockPos, state: BlockState, flags?: number): boolean;

  isEmptyBlock(pos: BlockPos): boolean;

  getHeight(type: Heightmap.Types, x: number, z: number): number;

  getMinBuildHeight(): number;

  getMaxBuildHeight(): number;
}
