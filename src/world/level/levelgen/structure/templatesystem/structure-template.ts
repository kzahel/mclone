import { BlockPos } from "../../../../../core/block-pos";
import type { WorldGenLevel } from "../../../world-gen-level";
import type { DiscreteVoxelShape } from "../../../../phys/shapes/discrete-voxel-shape";

export class StructureTemplate {
  public static updateShapeAtEdge(level: WorldGenLevel, flags: number, shape: DiscreteVoxelShape, x: number, y: number, z: number): void {
    shape.forAllFaces((direction, faceX, faceY, faceZ) => {
      const pos = new BlockPos(x + faceX, y + faceY, z + faceZ);
      const neighborPos = pos.relative(direction);
      const state = level.getBlockState(pos);
      const neighborState = level.getBlockState(neighborPos);
      const updatedState = state.updateShape(direction, neighborState, level, pos, neighborPos);
      if (state !== updatedState) {
        level.setBlock(pos, updatedState, flags & -2);
      }

      const updatedNeighborState = neighborState.updateShape(direction.getOpposite(), updatedState, level, neighborPos, pos);
      if (neighborState !== updatedNeighborState) {
        level.setBlock(neighborPos, updatedNeighborState, flags & -2);
      }
    });
  }
}
