import type { Biome } from "../biome/biome.ts";
import { ChunkBlockId, MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer.ts";
import { SimpleRandomSource } from "../prng/simple-random-source.ts";
import { getOverworldSurfaceTopMaterial } from "../surface/surface-builders.ts";
import { floor } from "./carver-math.ts";
import type { CarverConfiguration, CarverContext } from "./carver-config.ts";

export interface ChunkPosLike {
  readonly chunkX: number;
  readonly chunkZ: number;
}

export type CarveSkipChecker = (
  context: CarverContext,
  relativeX: number,
  relativeY: number,
  relativeZ: number,
  y: number,
) => boolean;

function getMinBlockX(chunkX: number): number {
  return chunkX << 4;
}

function getMinBlockZ(chunkZ: number): number {
  return chunkZ << 4;
}

function getMiddleBlockX(chunkX: number): number {
  return getMinBlockX(chunkX) + 8;
}

function getMiddleBlockZ(chunkZ: number): number {
  return getMinBlockZ(chunkZ) + 8;
}

function localX(worldX: number): number {
  return worldX & 15;
}

function localZ(worldZ: number): number {
  return worldZ & 15;
}

export abstract class WorldCarver<C extends CarverConfiguration> {
  public getRange(): number {
    return 4;
  }

  public abstract isStartChunk(config: C, random: SimpleRandomSource): boolean;

  public abstract carve(
    context: CarverContext,
    config: C,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    random: SimpleRandomSource,
    chunkPos: ChunkPosLike,
    carvingMask: Uint8Array,
  ): boolean;

  protected canReplaceBlock(blockId: ChunkBlockId): boolean {
    switch (blockId) {
      case ChunkBlockId.STONE:
      case ChunkBlockId.DIRT:
      case ChunkBlockId.GRASS_BLOCK:
      case ChunkBlockId.SNOW:
        return true;
      default:
        return false;
    }
  }

  protected canReplaceBlockWithAbove(blockId: ChunkBlockId, aboveBlockId: ChunkBlockId): boolean {
    return this.canReplaceBlock(blockId) ||
      ((blockId === ChunkBlockId.SAND || blockId === ChunkBlockId.GRAVEL) && aboveBlockId !== ChunkBlockId.WATER);
  }

  protected carveEllipsoid(
    context: CarverContext,
    config: C,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    seed: bigint,
    x: number,
    y: number,
    z: number,
    horizontalRadius: number,
    verticalRadius: number,
    carvingMask: Uint8Array,
    skipChecker: CarveSkipChecker,
  ): boolean {
    const chunkX = chunk.chunkX;
    const chunkZ = chunk.chunkZ;
    const random = new SimpleRandomSource(seed + BigInt(chunkX) + BigInt(chunkZ));
    const centerX = getMiddleBlockX(chunkX);
    const centerZ = getMiddleBlockZ(chunkZ);
    const maxDistance = 16.0 + (horizontalRadius * 2.0);
    if (Math.abs(x - centerX) > maxDistance || Math.abs(z - centerZ) > maxDistance) {
      return false;
    }

    const minBlockX = getMinBlockX(chunkX);
    const minBlockZ = getMinBlockZ(chunkZ);
    const minX = Math.max(floor(x - horizontalRadius) - minBlockX - 1, 0);
    const maxX = Math.min(floor(x + horizontalRadius) - minBlockX, 15);
    const minY = Math.max(floor(y - verticalRadius) - 1, context.minY + 1);
    const maxY = Math.min(floor(y + verticalRadius) + 1, (context.minY + context.genDepth) - 8);
    const minZ = Math.max(floor(z - horizontalRadius) - minBlockZ - 1, 0);
    const maxZ = Math.min(floor(z + horizontalRadius) - minBlockZ, 15);

    if (!config.aquifersEnabled && this.hasDisallowedLiquid(chunk, minX, maxX, minY, maxY, minZ, maxZ)) {
      return false;
    }

    let carvedAny = false;

    for (let localBlockX = minX; localBlockX <= maxX; localBlockX++) {
      const worldX = minBlockX + localBlockX;
      const relativeX = (worldX + 0.5 - x) / horizontalRadius;

      for (let localBlockZ = minZ; localBlockZ <= maxZ; localBlockZ++) {
        const worldZ = minBlockZ + localBlockZ;
        const relativeZ = (worldZ + 0.5 - z) / horizontalRadius;
        if ((relativeX * relativeX) + (relativeZ * relativeZ) >= 1.0) {
          continue;
        }

        let reachedSurface = false;

        for (let worldY = maxY; worldY > minY; worldY--) {
          const relativeY = (worldY - 0.5 - y) / verticalRadius;
          if (skipChecker(context, relativeX, relativeY, relativeZ, worldY)) {
            continue;
          }

          const maskIndex = localBlockX | (localBlockZ << 4) | ((worldY - context.minY) << 8);
          if (carvingMask[maskIndex] !== 0) {
            continue;
          }

          const currentBlock = chunk.getBlockAtY(localBlockX, worldY, localBlockZ);
          if (currentBlock === ChunkBlockId.GRASS_BLOCK) {
            reachedSurface = true;
          }

          carvingMask[maskIndex] = 1;
          carvedAny = this.carveBlock(
            context,
            config,
            chunk,
            biomeAccessor,
            random,
            worldX,
            worldY,
            worldZ,
            reachedSurface,
          ) || carvedAny;
        }
      }
    }

    return carvedAny;
  }

  protected carveBlock(
    context: CarverContext,
    config: C,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    _random: SimpleRandomSource,
    worldX: number,
    worldY: number,
    worldZ: number,
    reachedSurface: boolean,
  ): boolean {
    const localBlockX = localX(worldX);
    const localBlockZ = localZ(worldZ);
    const blockId = chunk.getBlockAtY(localBlockX, worldY, localBlockZ);
    const aboveBlockId = worldY + 1 >= chunk.minY + chunk.height
      ? ChunkBlockId.AIR
      : chunk.getBlockAtY(localBlockX, worldY + 1, localBlockZ);

    if (!this.canReplaceBlockWithAbove(blockId, aboveBlockId)) {
      return false;
    }

    const carveState = this.getCarveState(context, config, worldY);
    chunk.setBlockAtY(localBlockX, worldY, localBlockZ, carveState);

    if (reachedSurface && worldY - 1 >= chunk.minY) {
      const belowBlockId = chunk.getBlockAtY(localBlockX, worldY - 1, localBlockZ);
      if (belowBlockId === ChunkBlockId.DIRT) {
        chunk.setBlockAtY(
          localBlockX,
          worldY - 1,
          localBlockZ,
          getOverworldSurfaceTopMaterial(biomeAccessor(worldX, worldY, worldZ)),
        );
      }
    }

    return true;
  }

  protected hasDisallowedLiquid(
    chunk: MutableChunkBlockBuffer,
    minX: number,
    maxX: number,
    minY: number,
    maxY: number,
    minZ: number,
    maxZ: number,
  ): boolean {
    for (let localBlockX = minX; localBlockX <= maxX; localBlockX++) {
      for (let localBlockZ = minZ; localBlockZ <= maxZ; localBlockZ++) {
        for (let worldY = minY - 1; worldY <= maxY + 1; worldY++) {
          if (worldY >= chunk.minY && worldY < chunk.minY + chunk.height) {
            if (chunk.getBlockAtY(localBlockX, worldY, localBlockZ) === ChunkBlockId.WATER) {
              return true;
            }
          }

          if (worldY !== maxY + 1 && !isEdge(localBlockX, localBlockZ, minX, maxX, minZ, maxZ)) {
            worldY = maxY;
          }
        }
      }
    }

    return false;
  }

  protected getCarveState(context: CarverContext, config: C, worldY: number): ChunkBlockId {
    return worldY <= config.lavaLevel.resolveY(context) ? ChunkBlockId.LAVA : ChunkBlockId.AIR;
  }

  protected static canReach(
    chunkPos: ChunkPosLike,
    x: number,
    z: number,
    branchIndex: number,
    branchCount: number,
    width: number,
  ): boolean {
    const deltaX = x - getMiddleBlockX(chunkPos.chunkX);
    const deltaZ = z - getMiddleBlockZ(chunkPos.chunkZ);
    const remainingBranches = branchCount - branchIndex;
    const maxDistance = width + 18.0;
    return ((deltaX * deltaX) + (deltaZ * deltaZ)) - (remainingBranches * remainingBranches) <= (maxDistance * maxDistance);
  }
}

function isEdge(x: number, z: number, minX: number, maxX: number, minZ: number, maxZ: number): boolean {
  return x === minX || x === maxX || z === minZ || z === maxZ;
}
