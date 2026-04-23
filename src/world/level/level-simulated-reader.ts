import { BlockPos } from "../../core/block-pos";
import { Heightmap } from "../../worldgen/levelgen/heightmap";
import type { BlockState } from "./block/state/block-state";

export interface LevelSimulatedReader {
  isStateAtPosition(pos: BlockPos, predicate: (state: BlockState) => boolean): boolean;

  getHeightmapPos(type: Heightmap.Types, pos: BlockPos): BlockPos;
}
