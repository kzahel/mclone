import { BlockPos } from "../../../core/block-pos";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { Heightmap } from "../heightmap";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import { OreConfiguration } from "./configurations/ore-configuration";

function isOutsideBuildHeight(minBuildHeight: number, maxBuildHeight: number, y: number): boolean {
  return y < minBuildHeight || y >= maxBuildHeight;
}

export class OreFeature extends Feature<OreConfiguration> {
  public override place(context: FeaturePlaceContext<OreConfiguration>): boolean {
    const random = context.random();
    const origin = context.origin();
    const level = context.level();
    const config = context.config();
    const angle = random.nextFloat() * Math.PI;
    const size = config.size / 8.0;
    const radius = Math.ceil((((config.size / 16.0) * 2.0) + 1.0) / 2.0);
    const startX = origin.getX() + (Math.sin(angle) * size);
    const endX = origin.getX() - (Math.sin(angle) * size);
    const startZ = origin.getZ() + (Math.cos(angle) * size);
    const endZ = origin.getZ() - (Math.cos(angle) * size);
    const startY = origin.getY() + random.nextInt(3) - 2;
    const endY = origin.getY() + random.nextInt(3) - 2;
    const minX = origin.getX() - Math.ceil(size) - radius;
    const minY = origin.getY() - 2 - radius;
    const minZ = origin.getZ() - Math.ceil(size) - radius;
    const width = 2 * (Math.ceil(size) + radius);
    const height = 2 * (2 + radius);

    for (let x = minX; x <= minX + width; x++) {
      for (let z = minZ; z <= minZ + width; z++) {
        if (minY <= level.getHeight(Heightmap.Types.OCEAN_FLOOR_WG, x, z)) {
          return this.doPlace(level, random, config, startX, endX, startZ, endZ, startY, endY, minX, minY, minZ, width, height);
        }
      }
    }

    return false;
  }

  protected doPlace(
    level: WorldGenLevel,
    random: SimpleRandomSource,
    config: OreConfiguration,
    startX: number,
    endX: number,
    startZ: number,
    endZ: number,
    startY: number,
    endY: number,
    minX: number,
    minY: number,
    minZ: number,
    width: number,
    height: number,
  ): boolean {
    let placed = 0;
    const visited = new Uint8Array(width * height * width);
    const mutablePos = new BlockPos.MutableBlockPos();
    const veinSize = config.size;
    const veinPoints = new Array<number>(veinSize * 4).fill(0);
    const getBlockState = (pos: BlockPos) => level.getBlockState(pos);
    const minBuildHeight = level.getMinBuildHeight();
    const maxBuildHeight = level.getMaxBuildHeight();

    for (let index = 0; index < veinSize; index++) {
      const progress = index / veinSize;
      const x = startX + (progress * (endX - startX));
      const y = startY + (progress * (endY - startY));
      const z = startZ + (progress * (endZ - startZ));
      const randomSize = random.nextDouble() * veinSize / 16.0;
      const radius = (((Math.sin(Math.PI * progress) + 1.0) * randomSize) + 1.0) / 2.0;
      veinPoints[(index * 4) + 0] = x;
      veinPoints[(index * 4) + 1] = y;
      veinPoints[(index * 4) + 2] = z;
      veinPoints[(index * 4) + 3] = radius;
    }

    for (let left = 0; left < veinSize - 1; left++) {
      if (veinPoints[(left * 4) + 3]! <= 0.0) {
        continue;
      }

      for (let right = left + 1; right < veinSize; right++) {
        if (veinPoints[(right * 4) + 3]! <= 0.0) {
          continue;
        }

        const deltaX = veinPoints[(left * 4) + 0]! - veinPoints[(right * 4) + 0]!;
        const deltaY = veinPoints[(left * 4) + 1]! - veinPoints[(right * 4) + 1]!;
        const deltaZ = veinPoints[(left * 4) + 2]! - veinPoints[(right * 4) + 2]!;
        const deltaRadius = veinPoints[(left * 4) + 3]! - veinPoints[(right * 4) + 3]!;
        if ((deltaRadius * deltaRadius) > ((deltaX * deltaX) + (deltaY * deltaY) + (deltaZ * deltaZ))) {
          if (deltaRadius > 0.0) {
            veinPoints[(right * 4) + 3] = -1.0;
          } else {
            veinPoints[(left * 4) + 3] = -1.0;
          }
        }
      }
    }

    // WorldGenLevel: direct block-state reads/writes replace BulkSectionAccess and ensureCanWrite here.
    for (let index = 0; index < veinSize; index++) {
      const radius = veinPoints[(index * 4) + 3]!;
      if (radius < 0.0) {
        continue;
      }

      const centerX = veinPoints[(index * 4) + 0]!;
      const centerY = veinPoints[(index * 4) + 1]!;
      const centerZ = veinPoints[(index * 4) + 2]!;
      const startBlockX = Math.max(Math.floor(centerX - radius), minX);
      const startBlockY = Math.max(Math.floor(centerY - radius), minY);
      const startBlockZ = Math.max(Math.floor(centerZ - radius), minZ);
      const endBlockX = Math.max(Math.floor(centerX + radius), startBlockX);
      const endBlockY = Math.max(Math.floor(centerY + radius), startBlockY);
      const endBlockZ = Math.max(Math.floor(centerZ + radius), startBlockZ);

      for (let x = startBlockX; x <= endBlockX; x++) {
        const normalizedX = ((x + 0.5) - centerX) / radius;
        if ((normalizedX * normalizedX) >= 1.0) {
          continue;
        }

        for (let y = startBlockY; y <= endBlockY; y++) {
          const normalizedY = ((y + 0.5) - centerY) / radius;
          if ((normalizedX * normalizedX) + (normalizedY * normalizedY) >= 1.0) {
            continue;
          }

          for (let z = startBlockZ; z <= endBlockZ; z++) {
            const normalizedZ = ((z + 0.5) - centerZ) / radius;
            if ((normalizedX * normalizedX) + (normalizedY * normalizedY) + (normalizedZ * normalizedZ) >= 1.0) {
              continue;
            }

            if (isOutsideBuildHeight(minBuildHeight, maxBuildHeight, y)) {
              continue;
            }

            const visitedIndex = (x - minX) + ((y - minY) * width) + ((z - minZ) * width * height);
            if (visited[visitedIndex] !== 0) {
              continue;
            }

            visited[visitedIndex] = 1;
            mutablePos.set(x, y, z);
            const state = level.getBlockState(mutablePos);

            for (const targetState of config.targetStates) {
              if (!OreFeature.canPlaceOre(state, getBlockState, random, config, targetState, mutablePos)) {
                continue;
              }

              if (level.setBlock(mutablePos, targetState.state, 2)) {
                placed++;
              }
              break;
            }
          }
        }
      }
    }

    return placed > 0;
  }

  public static canPlaceOre(
    state: BlockState,
    getBlockState: (pos: BlockPos) => BlockState,
    random: SimpleRandomSource,
    config: OreConfiguration,
    targetState: OreConfiguration.TargetBlockState,
    pos: BlockPos.MutableBlockPos,
  ): boolean {
    if (!targetState.target.test(state, random)) {
      return false;
    }

    return OreFeature.shouldSkipAirCheck(random, config.discardChanceOnAirExposure) || !Feature.isAdjacentToAir(getBlockState, pos);
  }

  protected static shouldSkipAirCheck(random: SimpleRandomSource, discardChanceOnAirExposure: number): boolean {
    if (discardChanceOnAirExposure <= 0.0) {
      return true;
    }

    if (discardChanceOnAirExposure >= 1.0) {
      return false;
    }

    return random.nextFloat() >= discardChanceOnAirExposure;
  }
}
