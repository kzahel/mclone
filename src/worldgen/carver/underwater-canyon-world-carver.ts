import type { Biome } from "../biome/biome.ts";
import { ChunkBlockId, MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer.ts";
import { SimpleRandomSource } from "../prng/simple-random-source.ts";
import type { CanyonCarverConfiguration, CarverContext } from "./carver-config.ts";
import { UnderwaterCaveWorldCarver } from "./underwater-cave-world-carver.ts";
import { CanyonWorldCarver } from "./canyon-world-carver.ts";

export class UnderwaterCanyonWorldCarver extends CanyonWorldCarver {
  protected override canReplaceBlock(blockId: ChunkBlockId): boolean {
    return super.canReplaceBlock(blockId) ||
      blockId === ChunkBlockId.SAND ||
      blockId === ChunkBlockId.GRAVEL ||
      blockId === ChunkBlockId.WATER ||
      blockId === ChunkBlockId.LAVA ||
      blockId === ChunkBlockId.OBSIDIAN ||
      blockId === ChunkBlockId.AIR;
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
    config: CanyonCarverConfiguration,
    chunk: MutableChunkBlockBuffer,
    biomeAccessor: (worldX: number, worldY: number, worldZ: number) => Biome,
    random: SimpleRandomSource,
    worldX: number,
    worldY: number,
    worldZ: number,
    reachedSurface: boolean,
  ): boolean {
    return UnderwaterCaveWorldCarver.carveBlock(
      this as unknown as UnderwaterCaveWorldCarver,
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
}
