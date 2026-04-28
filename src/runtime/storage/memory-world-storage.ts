import { clonePackedChunkSnapshot, type PackedChunkSnapshot } from "../../world/level/packed-chunk-snapshot";
import { GeneratedChunkStatus } from "../../world/level/generated-chunk-status";
import {
  GENERATED_PROTO_CHUNK_CONTENT_VERSION,
  cloneGeneratedChunkStorageRecord,
  type GeneratedChunkStorageRecord,
} from "../../world/level/generated-proto-chunk";
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
  snapshot: PackedChunkSnapshot;
  savedAtMs: number;
  lastLoadedAtMs?: number;
  lastEvictedAtMs?: number;
};

type MemoryGeneratedChunkRecord = {
  generatedChunk: GeneratedChunkStorageRecord;
  savedAtMs: number;
  lastLoadedAtMs?: number;
  lastEvictedAtMs?: number;
};

type MemoryWorldRecord = {
  metadata: WorldSaveMetadata;
  chunks: Map<string, MemoryChunkRecord>;
  generatedChunks: Map<string, MemoryGeneratedChunkRecord>;
};

function createFullGeneratedChunkRecord(snapshot: PackedChunkSnapshot): GeneratedChunkStorageRecord {
  return {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    type: "level",
    status: GeneratedChunkStatus.FULL,
    hasBlockSections: true,
    isUnsaved: false,
    contentVersion: GENERATED_PROTO_CHUNK_CONTENT_VERSION,
    snapshot: clonePackedChunkSnapshot(snapshot),
  };
}

class MemoryChunkStorage implements ChunkStorage {
  public constructor(
    private readonly world: MemoryWorldRecord,
    private readonly now: () => number,
  ) {}

  public async loadChunk(chunkX: number, chunkZ: number): Promise<PackedChunkSnapshot | undefined> {
    const record = this.world.chunks.get(chunkKey(chunkX, chunkZ));
    if (record === undefined) {
      return undefined;
    }

    record.lastLoadedAtMs = this.now();
    return clonePackedChunkSnapshot(record.snapshot);
  }

  public async saveChunk(snapshot: PackedChunkSnapshot): Promise<void> {
    const savedAtMs = this.now();
    this.world.chunks.set(
      chunkKey(snapshot.chunkX, snapshot.chunkZ),
      {
        snapshot: clonePackedChunkSnapshot(snapshot),
        savedAtMs,
      },
    );
    this.world.generatedChunks.set(
      chunkKey(snapshot.chunkX, snapshot.chunkZ),
      {
        generatedChunk: createFullGeneratedChunkRecord(snapshot),
        savedAtMs,
      },
    );
  }

  public async loadGeneratedChunk(chunkX: number, chunkZ: number): Promise<GeneratedChunkStorageRecord | undefined> {
    const record = this.world.generatedChunks.get(chunkKey(chunkX, chunkZ));
    if (record === undefined) {
      return undefined;
    }

    record.lastLoadedAtMs = this.now();
    return cloneGeneratedChunkStorageRecord(record.generatedChunk);
  }

  public async saveGeneratedChunk(record: GeneratedChunkStorageRecord): Promise<void> {
    this.world.generatedChunks.set(
      chunkKey(record.chunkX, record.chunkZ),
      {
        generatedChunk: cloneGeneratedChunkStorageRecord(record),
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
    const generatedRecord = this.world.generatedChunks.get(chunkKey(chunkX, chunkZ));
    if (generatedRecord !== undefined) {
      generatedRecord.lastEvictedAtMs = this.now();
    }
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
      generatedChunks: new Map<string, MemoryGeneratedChunkRecord>(),
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

  public getGeneratedChunkRecord(saveId: string, chunkX: number, chunkZ: number): MemoryGeneratedChunkRecord | undefined {
    return this.worlds.get(saveId)?.generatedChunks.get(chunkKey(chunkX, chunkZ));
  }
}
