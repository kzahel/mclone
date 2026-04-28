import { BlockPos } from "../../../../../core/block-pos";
import { Vec3i } from "../../../../../core/vec3i";
import { Mirror } from "../../../block/mirror";
import { Rotation } from "../../../block/rotation";
import type { BlockState } from "../../../block/state/block-state";
import { BoundingBox } from "../bounding-box";
import type { WorldGenLevel } from "../../../world-gen-level";
import type { DiscreteVoxelShape } from "../../../../phys/shapes/discrete-voxel-shape";
import type { SimpleRandomSource } from "../../../../../worldgen/prng/simple-random-source";
import type { StructurePlaceSettings } from "./structure-place-settings";

export interface StructureBlockInfo {
  readonly pos: BlockPos;
  readonly state: BlockState;
}

export class StructureTemplate {
  public constructor(
    private readonly size: Vec3i,
    private readonly blocks: readonly StructureBlockInfo[],
  ) {}

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

  public static calculateRelativePosition(settings: StructurePlaceSettings, pos: BlockPos): BlockPos {
    return StructureTemplate.transform(pos, settings.getMirror(), settings.getRotation(), settings.getRotationPivot());
  }

  public static processBlockInfos(
    level: WorldGenLevel,
    structureOrigin: BlockPos,
    placementOrigin: BlockPos,
    settings: StructurePlaceSettings,
    blocks: readonly StructureBlockInfo[],
  ): StructureBlockInfo[] {
    const placedBlocks: StructureBlockInfo[] = [];

    for (const originalBlockInfo of blocks) {
      const targetPos = StructureTemplate.calculateRelativePosition(settings, originalBlockInfo.pos)
        .offset(structureOrigin.getX(), structureOrigin.getY(), structureOrigin.getZ());
      let currentBlockInfo: StructureBlockInfo | undefined = {
        pos: targetPos,
        state: originalBlockInfo.state,
      };

      for (const processor of settings.getProcessors()) {
        if (currentBlockInfo === undefined) {
          break;
        }

        currentBlockInfo = processor.processBlock(
          level,
          structureOrigin,
          placementOrigin,
          originalBlockInfo,
          currentBlockInfo,
          settings,
        );
      }

      if (currentBlockInfo !== undefined) {
        placedBlocks.push(currentBlockInfo);
      }
    }

    return placedBlocks;
  }

  public getSize(rotation: Rotation): Vec3i {
    switch (rotation) {
      case Rotation.COUNTERCLOCKWISE_90:
      case Rotation.CLOCKWISE_90:
        return new Vec3i(this.size.getZ(), this.size.getY(), this.size.getX());
      default:
        return this.size;
    }
  }

  public static transform(pos: BlockPos, mirror: Mirror, rotation: Rotation, pivot: BlockPos): BlockPos {
    let x = pos.getX();
    const y = pos.getY();
    let z = pos.getZ();
    let mirrored = true;
    switch (mirror) {
      case Mirror.LEFT_RIGHT:
        z = -z;
        break;
      case Mirror.FRONT_BACK:
        x = -x;
        break;
      default:
        mirrored = false;
    }

    const pivotX = pivot.getX();
    const pivotZ = pivot.getZ();
    switch (rotation) {
      case Rotation.COUNTERCLOCKWISE_90:
        return new BlockPos(pivotX - pivotZ + z, y, pivotX + pivotZ - x);
      case Rotation.CLOCKWISE_90:
        return new BlockPos(pivotX + pivotZ - z, y, pivotZ - pivotX + x);
      case Rotation.CLOCKWISE_180:
        return new BlockPos((pivotX + pivotX) - x, y, (pivotZ + pivotZ) - z);
      default:
        return mirrored ? new BlockPos(x, y, z) : pos;
    }
  }

  public getZeroPositionWithTransform(targetPos: BlockPos, mirror: Mirror, rotation: Rotation): BlockPos {
    return StructureTemplate.getZeroPositionWithTransform(targetPos, mirror, rotation, this.size.getX(), this.size.getZ());
  }

  public static getZeroPositionWithTransform(
    pos: BlockPos,
    mirror: Mirror,
    rotation: Rotation,
    sizeX: number,
    sizeZ: number,
  ): BlockPos {
    sizeX--;
    sizeZ--;
    const mirrorX = mirror === Mirror.FRONT_BACK ? sizeX : 0;
    const mirrorZ = mirror === Mirror.LEFT_RIGHT ? sizeZ : 0;
    switch (rotation) {
      case Rotation.COUNTERCLOCKWISE_90:
        return pos.offset(mirrorZ, 0, sizeX - mirrorX);
      case Rotation.CLOCKWISE_90:
        return pos.offset(sizeZ - mirrorZ, 0, mirrorX);
      case Rotation.CLOCKWISE_180:
        return pos.offset(sizeX - mirrorX, 0, sizeZ - mirrorZ);
      case Rotation.NONE:
      default:
        return pos.offset(mirrorX, 0, mirrorZ);
    }
  }

  public getBoundingBox(settings: StructurePlaceSettings, startPos: BlockPos): BoundingBox {
    return this.getBoundingBoxWithTransform(startPos, settings.getRotation(), settings.getRotationPivot(), settings.getMirror());
  }

  public getBoundingBoxWithTransform(startPos: BlockPos, rotation: Rotation, pivotPos: BlockPos, mirror: Mirror): BoundingBox {
    return StructureTemplate.getBoundingBox(startPos, rotation, pivotPos, mirror, this.size);
  }

  protected static getBoundingBox(startPos: BlockPos, rotation: Rotation, pivotPos: BlockPos, mirror: Mirror, size: Vec3i): BoundingBox {
    const maxCorner = new BlockPos(size.getX() - 1, size.getY() - 1, size.getZ() - 1);
    const first = StructureTemplate.transform(BlockPos.ZERO, mirror, rotation, pivotPos);
    const second = StructureTemplate.transform(maxCorner, mirror, rotation, pivotPos);
    return BoundingBox.fromCorners(first, second).move(startPos);
  }

  public placeInWorld(
    level: WorldGenLevel,
    structureOrigin: BlockPos,
    placementOrigin: BlockPos,
    settings: StructurePlaceSettings,
    _random: SimpleRandomSource,
    flags: number,
  ): boolean {
    if (this.blocks.length === 0 || this.size.getX() < 1 || this.size.getY() < 1 || this.size.getZ() < 1) {
      return false;
    }

    const boundingBox = settings.getBoundingBox();
    let placed = false;
    for (const blockInfo of StructureTemplate.processBlockInfos(level, structureOrigin, placementOrigin, settings, this.blocks)) {
      if (boundingBox !== undefined && !boundingBox.isInside(blockInfo.pos)) {
        continue;
      }

      const state = blockInfo.state.mirror(settings.getMirror()).rotate(settings.getRotation());
      if (level.setBlock(blockInfo.pos, state, flags)) {
        placed = true;
      }
    }

    return placed;
  }
}
