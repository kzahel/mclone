import type { Biome } from "../biome/biome.ts";
import { ChunkBlockId, MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer.ts";
import { SimpleRandomSource } from "../prng/simple-random-source.ts";
import type { CarverConfiguration, CarverContext, CaveCarverConfiguration } from "./carver-config.ts";
import { CaveWorldCarver } from "./cave-world-carver.ts";

function localCoordinate(worldCoordinate: number): number {
  return worldCoordinate & 15;
}

export class UnderwaterCaveWorldCarver extends CaveWorldCarver {
  protected override canReplaceBlock(blockId: ChunkBlockId): boolean {
    return super.canReplaceBlock(blockId) ||
      blockId === ChunkBlockId.SAND ||
      blockId === ChunkBlockId.GRAVEL ||
      blockId === ChunkBlockId.WATER ||
      blockId === ChunkBlockId.LAVA ||
      blockId === ChunkBlockId.OBSIDIAN;
  }

  protected override hasDisallowedLiquid(
    _chunk: MutableChunkBlockBuffer,
    _minX: number,
    _maxX: number,
    _minY: number,
    _maxY: number,
    _minZ: number,
    _maxZ: number,
  ): boolean {
    return false;
  }

  protected override carveBlock(
    context: CarverContext,
    config: CaveCarverConfiguration,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    random: SimpleRandomSource,
    worldX: number,
    worldY: number,
    worldZ: number,
    reachedSurface: boolean,
  ): boolean {
    return UnderwaterCaveWorldCarver.carveBlock(
      this,
      context,
      config,
      chunk,
      biomeAccessor,
      random,
      worldX,
      worldY,
      worldZ,
      reachedSurface,
    );
  }

  public static carveBlock(
    carver: UnderwaterCaveWorldCarver,
    context: CarverContext,
    _config: CarverConfiguration,
    chunk: MutableChunkBlockBuffer,
    _biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    random: SimpleRandomSource,
    worldX: number,
    worldY: number,
    worldZ: number,
    _reachedSurface: boolean,
  ): boolean {
    const seaLevel = context.seaLevel ?? 63;
    if (worldY >= seaLevel) {
      return false;
    }

    const localX = localCoordinate(worldX);
    const localZ = localCoordinate(worldZ);
    const blockId = chunk.getBlockAtY(localX, worldY, localZ);
    if (!carver.canReplaceBlock(blockId)) {
      return false;
    }

    if (worldY === 10) {
      chunk.setBlockAtY(
        localX,
        worldY,
        localZ,
        random.nextFloat() < 0.25 ? ChunkBlockId.MAGMA_BLOCK : ChunkBlockId.OBSIDIAN,
      );
      return true;
    }

    if (worldY < 10) {
      chunk.setBlockAtY(localX, worldY, localZ, ChunkBlockId.LAVA);
      return false;
    }

    chunk.setBlockAtY(localX, worldY, localZ, ChunkBlockId.WATER);
    return true;
  }
}
