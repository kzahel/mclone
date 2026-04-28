import { BlockPos } from "../../../../core/block-pos";
import { floor } from "../../../../util/mth";
import type { PathfinderMob } from "../pathfinder-mob";
import { BlockPathTypes } from "../../../level/pathfinder/block-path-types";
import { Path } from "../../../level/pathfinder/path";
import { PathComputationType } from "../../../level/pathfinder/path-computation-type";
import { PathFinder } from "../../../level/pathfinder/path-finder";
import { WalkNodeEvaluator } from "../../../level/pathfinder/walk-node-evaluator";
import { Fluids } from "../../../level/material/fluids";
import { Vec3 } from "../../../phys/vec3";
import { PathNavigation } from "./path-navigation";

export class GroundPathNavigation extends PathNavigation {
  private avoidSun = false;

  public constructor(mob: PathfinderMob) {
    super(mob);
  }

  protected override createPathFinder(maxVisitedNodes: number): PathFinder {
    this.nodeEvaluator = new WalkNodeEvaluator();
    this.nodeEvaluator.setCanPassDoors(true);
    return new PathFinder(this.nodeEvaluator, maxVisitedNodes);
  }

  protected override canUpdatePath(): boolean {
    return this.mob.isOnGround() || this.isInLiquid();
  }

  protected override getTempMobPos(): Vec3 {
    return new Vec3(this.mob.getX(), this.getSurfaceY(), this.mob.getZ());
  }

  public override createPath(pos: BlockPos, accuracy: number): Path | undefined;
  public override createPath(x: number, y: number, z: number, accuracy: number): Path | undefined;
  public override createPath(first: BlockPos | number, second: number, third?: number, fourth?: number): Path | undefined {
    if (!(first instanceof BlockPos)) {
      return this.createPath(new BlockPos(first, second, third!), fourth!);
    }

    const level = this.mob.getAiLevel();
    if (level === undefined) {
      return undefined;
    }

    let pos = first;
    if (level.getBlockState(pos).isAir()) {
      let cursor = pos.below();

      while (cursor.getY() > level.getMinBuildHeight() && level.getBlockState(cursor).isAir()) {
        cursor = cursor.below();
      }

      if (cursor.getY() > level.getMinBuildHeight()) {
        return super.createPath(cursor.above(), second);
      }

      while (cursor.getY() < level.getMaxBuildHeight() && level.getBlockState(cursor).isAir()) {
        cursor = cursor.above();
      }

      pos = cursor;
    }

    if (!level.getBlockState(pos).getMaterial().isSolid()) {
      return super.createPath(pos, second);
    }

    let cursor = pos.above();
    while (cursor.getY() < level.getMaxBuildHeight() && level.getBlockState(cursor).getMaterial().isSolid()) {
      cursor = cursor.above();
    }

    return super.createPath(cursor, second);
  }

  private getSurfaceY(): number {
    if (this.mob.isInWaterOrBubble() && this.canFloat()) {
      const level = this.mob.getAiLevel();
      if (level === undefined) {
        return this.mob.getBlockY();
      }

      let y = this.mob.getBlockY();
      let state = level.getBlockState(new BlockPos(this.mob.getX(), y, this.mob.getZ()));
      let checks = 0;

      while (state.getFluidState().getType().isSame(Fluids.WATER)) {
        state = level.getBlockState(new BlockPos(this.mob.getX(), ++y, this.mob.getZ()));
        if (++checks > 16) {
          return this.mob.getBlockY();
        }
      }

      return y;
    }

    return floor(this.mob.getY() + 0.5);
  }

  protected override trimPath(): void {
    super.trimPath();
    if (this.avoidSun) {
      // Runtime: sky visibility is not part of MobAiLevel yet. Keep the hook for parity follow-through.
    }
  }

  protected override canMoveDirectly(from: Vec3, to: Vec3, sizeX: number, sizeY: number, sizeZ: number): boolean {
    const level = this.mob.getAiLevel();
    if (level === undefined) {
      return false;
    }

    let x = floor(from.x);
    let z = floor(from.z);
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    const distanceSqr = (dx * dx) + (dz * dz);
    if (distanceSqr < 1.0e-8) {
      return false;
    }

    const invDistance = 1.0 / Math.sqrt(distanceSqr);
    dx *= invDistance;
    dz *= invDistance;
    sizeX += 2;
    sizeZ += 2;
    if (!this.canWalkOn(x, floor(from.y), z, sizeX, sizeY, sizeZ, from, dx, dz)) {
      return false;
    }

    sizeX -= 2;
    sizeZ -= 2;
    const stepXDistance = 1.0 / Math.abs(dx);
    const stepZDistance = 1.0 / Math.abs(dz);
    let nextXDistance = x - from.x;
    let nextZDistance = z - from.z;
    if (dx >= 0.0) {
      nextXDistance++;
    }
    if (dz >= 0.0) {
      nextZDistance++;
    }
    nextXDistance /= dx;
    nextZDistance /= dz;
    const xStep = dx < 0.0 ? -1 : 1;
    const zStep = dz < 0.0 ? -1 : 1;
    const targetX = floor(to.x);
    const targetZ = floor(to.z);
    let remainingX = targetX - x;
    let remainingZ = targetZ - z;

    while (remainingX * xStep > 0 || remainingZ * zStep > 0) {
      if (nextXDistance < nextZDistance) {
        nextXDistance += stepXDistance;
        x += xStep;
        remainingX = targetX - x;
      } else {
        nextZDistance += stepZDistance;
        z += zStep;
        remainingZ = targetZ - z;
      }

      if (!this.canWalkOn(x, floor(from.y), z, sizeX, sizeY, sizeZ, from, dx, dz)) {
        return false;
      }
    }

    return true;
  }

  private canWalkOn(
    x: number,
    y: number,
    z: number,
    sizeX: number,
    sizeY: number,
    sizeZ: number,
    from: Vec3,
    stepX: number,
    stepZ: number,
  ): boolean {
    if (this.mob.getAiLevel() === undefined) {
      return false;
    }

    const minX = x - floor(sizeX / 2);
    const minZ = z - floor(sizeZ / 2);
    if (!this.canWalkAbove(minX, y, minZ, sizeX, sizeY, sizeZ, from, stepX, stepZ)) {
      return false;
    }

    for (let blockX = minX; blockX < minX + sizeX; blockX++) {
      for (let blockZ = minZ; blockZ < minZ + sizeZ; blockZ++) {
        const xOffset = blockX + 0.5 - from.x;
        const zOffset = blockZ + 0.5 - from.z;
        if (!(xOffset * stepX + zOffset * stepZ < 0.0)) {
          let type = this.nodeEvaluator.getBlockPathType(this.mob.getAiLevel()!, blockX, y - 1, blockZ, this.mob, sizeX, sizeY, sizeZ, true, true);
          if (!this.hasValidPathType(type)) {
            return false;
          }

          type = this.nodeEvaluator.getBlockPathType(this.mob.getAiLevel()!, blockX, y, blockZ, this.mob, sizeX, sizeY, sizeZ, true, true);
          const malus = this.mob.getPathfindingMalus(type);
          if (malus < 0.0 || malus >= 8.0) {
            return false;
          }

          if (type === BlockPathTypes.DAMAGE_FIRE || type === BlockPathTypes.DANGER_FIRE || type === BlockPathTypes.DAMAGE_OTHER) {
            return false;
          }
        }
      }
    }

    return true;
  }

  private canWalkAbove(
    minX: number,
    y: number,
    minZ: number,
    sizeX: number,
    sizeY: number,
    sizeZ: number,
    from: Vec3,
    stepX: number,
    stepZ: number,
  ): boolean {
    const level = this.mob.getAiLevel();
    if (level === undefined) {
      return false;
    }

    for (let x = minX; x < minX + sizeX; x++) {
      for (let blockY = y; blockY < y + sizeY; blockY++) {
        for (let z = minZ; z < minZ + sizeZ; z++) {
          const xOffset = x + 0.5 - from.x;
          const zOffset = z + 0.5 - from.z;
          const pos = new BlockPos(x, blockY, z);
          if (!(xOffset * stepX + zOffset * stepZ < 0.0)
            && !level.getBlockState(pos).isPathfindable(level, pos, PathComputationType.LAND)) {
            return false;
          }
        }
      }
    }

    return true;
  }

  public setCanOpenDoors(canOpenDoors: boolean): void {
    this.nodeEvaluator.setCanOpenDoors(canOpenDoors);
  }

  public canPassDoors(): boolean {
    return this.nodeEvaluator.canPassDoors();
  }

  public setCanPassDoors(canPassDoors: boolean): void {
    this.nodeEvaluator.setCanPassDoors(canPassDoors);
  }

  public canOpenDoors(): boolean {
    return this.nodeEvaluator.canOpenDoors();
  }

  public setAvoidSun(avoidSun: boolean): void {
    this.avoidSun = avoidSun;
  }
}
