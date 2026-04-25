import { BlockPos } from "../../../core/block-pos";
import { SectionPos } from "../../../core/section-pos";

export interface EntityChunkPos {
  readonly chunkX: number;
  readonly chunkZ: number;
}

export interface ChunkEntities<T> extends EntityChunkPos {
  readonly entities: readonly T[];
}

export function entityChunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

export function parseEntityChunkKey(key: string): EntityChunkPos {
  const [rawX, rawZ] = key.split(",");
  if (rawX === undefined || rawZ === undefined) {
    throw new Error(`Invalid entity chunk key: ${key}`);
  }

  return {
    chunkX: Number(rawX),
    chunkZ: Number(rawZ),
  };
}

export function entityChunkKeyFromBlockPos(pos: BlockPos): string {
  return entityChunkKey(SectionPos.blockToSectionCoord(pos.getX()), SectionPos.blockToSectionCoord(pos.getZ()));
}

export function entityChunkKeyFromSectionKey(sectionKey: bigint): string {
  return entityChunkKey(SectionPos.x(sectionKey), SectionPos.z(sectionKey));
}

export function createChunkEntities<T>(
  chunkX: number,
  chunkZ: number,
  entities: readonly T[] = [],
): ChunkEntities<T> {
  return { chunkX, chunkZ, entities: [...entities] };
}

export function isChunkEntitiesEmpty<T>(chunk: ChunkEntities<T>): boolean {
  return chunk.entities.length === 0;
}
