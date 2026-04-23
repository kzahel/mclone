import type { ChunkSnapshot } from "../../world/level/chunk-snapshot";
import {
  createWorldSaveMetadata,
  isWorldSaveMetadataCompatible,
  touchWorldSaveMetadata,
  type ChunkStorage,
  type OpenWorldStorageRequest,
  type WorldSaveMetadata,
  type WorldStorage,
  type WorldStorageSession,
} from "./world-storage";

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

type MemoryChunkRecord = {
  snapshot: ChunkSnapshot;
  savedAtMs: number;
  lastLoadedAtMs?: number;
  lastEvictedAtMs?: number;
};

type MemoryWorldRecord = {
  metadata: WorldSaveMetadata;
  chunks: Map<string, MemoryChunkRecord>;
};

class MemoryChunkStorage implements ChunkStorage {
  public constructor(
    private readonly world: MemoryWorldRecord,
    private readonly now: () => number,
  ) {}

  public async loadChunk(chunkX: number, chunkZ: number): Promise<ChunkSnapshot | undefined> {
    const record = this.world.chunks.get(chunkKey(chunkX, chunkZ));
    if (record === undefined) {
      return undefined;
    }

    record.lastLoadedAtMs = this.now();
    return record.snapshot;
  }

  public async saveChunk(snapshot: ChunkSnapshot): Promise<void> {
    this.world.chunks.set(
      chunkKey(snapshot.chunkX, snapshot.chunkZ),
      {
        snapshot,
        savedAtMs: this.now(),
      },
    );
  }

  public async evictChunk(chunkX: number, chunkZ: number): Promise<void> {
    const record = this.world.chunks.get(chunkKey(chunkX, chunkZ));
    if (record === undefined) {
      return;
    }

    record.lastEvictedAtMs = this.now();
  }
}

class MemoryWorldStorageSession implements WorldStorageSession {
  public readonly chunks: ChunkStorage;

  public constructor(
    public readonly metadata: WorldSaveMetadata,
    world: MemoryWorldRecord,
    now: () => number,
  ) {
    this.chunks = new MemoryChunkStorage(world, now);
  }

  public async close(): Promise<void> {}
}

export class MemoryWorldStorage implements WorldStorage {
  private readonly worlds = new Map<string, MemoryWorldRecord>();

  public constructor(private readonly now: () => number = () => Date.now()) {}

  public async openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession> {
    const existing = this.worlds.get(request.saveId);
    if (existing !== undefined && isWorldSaveMetadataCompatible(existing.metadata, request)) {
      existing.metadata = touchWorldSaveMetadata(existing.metadata, request.openedAtMs);
      return new MemoryWorldStorageSession(existing.metadata, existing, this.now);
    }

    const world: MemoryWorldRecord = {
      metadata: createWorldSaveMetadata(request),
      chunks: new Map<string, MemoryChunkRecord>(),
    };
    this.worlds.set(request.saveId, world);
    return new MemoryWorldStorageSession(world.metadata, world, this.now);
  }

  public getWorldMetadata(saveId: string): WorldSaveMetadata | undefined {
    return this.worlds.get(saveId)?.metadata;
  }

  public getChunkRecord(saveId: string, chunkX: number, chunkZ: number): MemoryChunkRecord | undefined {
    return this.worlds.get(saveId)?.chunks.get(chunkKey(chunkX, chunkZ));
  }
}
