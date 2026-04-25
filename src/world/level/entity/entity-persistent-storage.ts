import {
  createChunkEntities,
  entityChunkKey,
  type ChunkEntities,
} from "./chunk-entities";

export interface EntityPersistentStorage<T> {
  loadEntities(chunkX: number, chunkZ: number): Promise<ChunkEntities<T>>;
  storeEntities(chunk: ChunkEntities<T>): void | Promise<void>;
  flush(sync: boolean): Promise<void>;
  close(): Promise<void>;
}

export class MemoryEntityPersistentStorage<T> implements EntityPersistentStorage<T> {
  private readonly chunks = new Map<string, ChunkEntities<T>>();

  public async loadEntities(chunkX: number, chunkZ: number): Promise<ChunkEntities<T>> {
    const chunk = this.chunks.get(entityChunkKey(chunkX, chunkZ));
    return createChunkEntities(chunkX, chunkZ, chunk?.entities ?? []);
  }

  public storeEntities(chunk: ChunkEntities<T>): void {
    this.chunks.set(entityChunkKey(chunk.chunkX, chunk.chunkZ), createChunkEntities(chunk.chunkX, chunk.chunkZ, chunk.entities));
  }

  public async flush(_sync: boolean): Promise<void> {}

  public async close(): Promise<void> {}

  public seedChunk(chunk: ChunkEntities<T>): void {
    this.storeEntities(chunk);
  }

  public getStoredChunk(chunkX: number, chunkZ: number): ChunkEntities<T> | undefined {
    const chunk = this.chunks.get(entityChunkKey(chunkX, chunkZ));
    return chunk === undefined ? undefined : createChunkEntities(chunk.chunkX, chunk.chunkZ, chunk.entities);
  }
}
