import { BlockPos } from "../../../core/block-pos";
import type { BlockGetter } from "../block-getter";
import type { PathfinderMob } from "../../entity/ai/pathfinder-mob";
import { AABB } from "../../phys/aabb";

export interface PathNavigationRegion extends BlockGetter {
  getMinBuildHeight(): number;
  getHeight(): number;
  getMaxBuildHeight(): number;
  noCollision(entity: PathfinderMob | undefined, collisionBox: AABB): boolean;
}

export function blockGetterPathNavigationRegion(
  level: BlockGetter,
  minBuildHeight: number,
  height: number,
): PathNavigationRegion {
  return {
    getBlockState(pos) {
      return level.getBlockState(pos);
    },
    getFluidState(pos) {
      return level.getFluidState(pos);
    },
    getMaxLightLevel() {
      return level.getMaxLightLevel();
    },
    getMinBuildHeight() {
      return minBuildHeight;
    },
    getHeight() {
      return height;
    },
    getMaxBuildHeight() {
      return minBuildHeight + height;
    },
    noCollision(_entity, collisionBox) {
      return !hasBlockCollision(level, collisionBox);
    },
  };
}

function hasBlockCollision(level: BlockGetter, bounds: AABB): boolean {
  const epsilon = AABB.epsilon();
  const minX = Math.floor(bounds.minX - epsilon);
  const minY = Math.floor(bounds.minY - epsilon);
  const minZ = Math.floor(bounds.minZ - epsilon);
  const maxX = Math.floor(bounds.maxX + epsilon);
  const maxY = Math.floor(bounds.maxY + epsilon);
  const maxZ = Math.floor(bounds.maxZ + epsilon);
  const pos = new BlockPos.MutableBlockPos();

  for (let y = minY; y <= maxY; y++) {
    for (let z = minZ; z <= maxZ; z++) {
      for (let x = minX; x <= maxX; x++) {
        pos.set(x, y, z);
        const state = level.getBlockState(pos);
        if (state.isAir()) {
          continue;
        }

        const shape = state.getCollisionShape(level, pos);
        if (shape.isEmpty()) {
          continue;
        }

        for (const box of shape.toAabbs()) {
          if (box.move(x, y, z).intersects(bounds)) {
            return true;
          }
        }
      }
    }
  }

  return false;
}
