import { BlockPos } from "../../core/block-pos";
import type { BlockGetter } from "../../world/level/block-getter";
import { AABB } from "../../world/phys/aabb";
import { Vec3 } from "../../world/phys/vec3";

export interface LoadedCollisionQuery {
  readonly type: "loaded";
  readonly boxes: readonly AABB[];
}

export interface MissingCollisionQuery {
  readonly type: "missing";
  readonly reason?: string;
}

export type CollisionQuery = LoadedCollisionQuery | MissingCollisionQuery;

export interface CollisionWorld {
  queryBlockCollisions(bounds: AABB): CollisionQuery;
}

export function createStaticCollisionWorld(boxes: readonly AABB[]): CollisionWorld {
  return {
    queryBlockCollisions(bounds) {
      return {
        type: "loaded",
        boxes: boxes.filter((box) => box.intersects(bounds)),
      };
    },
  };
}

export function createMissingCollisionWorld(reason: string): CollisionWorld {
  return {
    queryBlockCollisions() {
      return { type: "missing", reason };
    },
  };
}

export function blockGetterCollisionWorld(level: BlockGetter): CollisionWorld {
  return {
    queryBlockCollisions(bounds) {
      const boxes: AABB[] = [];
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
            if (state.isAir() || !state.getBlock().hasCollision || !state.isCollisionShapeFullBlock(level, pos)) {
              continue;
            }

            const blockBox = AABB.unitCubeFromLowerCorner(new Vec3(x, y, z));
            if (blockBox.intersects(bounds)) {
              boxes.push(blockBox);
            }
          }
        }
      }

      return { type: "loaded", boxes };
    },
  };
}
