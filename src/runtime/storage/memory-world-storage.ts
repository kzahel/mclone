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
  type SaveChunkOptions,
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
  sessionEpoch: number;
};

function createFullGeneratedChunkRecord(snapshot: PackedChunkSnapshot, writeVersion: number): GeneratedChunkStorageRecord {
  return {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    type: "level",
    status: GeneratedChunkStatus.FULL,
    hasBlockSections: true,
    isUnsaved: false,
    contentVersion: GENERATED_PROTO_CHUNK_CONTENT_VERSION,
    writeVersion,
    snapshot: clonePackedChunkSnapshot(snapshot),
  };
}

function isStaleGeneratedChunkWrite(
  existing: MemoryGeneratedChunkRecord | undefined,
  incomingWriteVersion: number,
): boolean {
  return existing !== undefined && existing.generatedChunk.writeVersion > incomingWriteVersion;
}

class MemoryChunkStorage implements ChunkStorage {
  public constructor(
    private readonly world: MemoryWorldRecord,
    private readonly sessionEpoch: number,
    private readonly now: () => number,
  ) {}

  private isCurrentSession(): boolean {
    return this.world.sessionEpoch === this.sessionEpoch;
  }

  public async loadChunk(chunkX: number, chunkZ: number): Promise<PackedChunkSnapshot | undefined> {
    if (!this.isCurrentSession()) {
      return undefined;
    }

    const record = this.world.chunks.get(chunkKey(chunkX, chunkZ));
    if (record === undefined) {
      return undefined;
    }

    record.lastLoadedAtMs = this.now();
    return clonePackedChunkSnapshot(record.snapshot);
  }

  public async saveChunk(snapshot: PackedChunkSnapshot, options?: SaveChunkOptions): Promise<void> {
    if (!this.isCurrentSession()) {
      return;
    }

    const savedAtMs = this.now();
    const key = chunkKey(snapshot.chunkX, snapshot.chunkZ);
    this.world.chunks.set(
      key,
      {
        snapshot: clonePackedChunkSnapshot(snapshot),
        savedAtMs,
      },
    );
    const existingGeneratedRecord = this.world.generatedChunks.get(key);
    const writeVersion = options?.generatedRecordWriteVersion
      ?? ((existingGeneratedRecord?.generatedChunk.writeVersion ?? 0) + 1);
    if (isStaleGeneratedChunkWrite(existingGeneratedRecord, writeVersion)) {
      return;
    }

    this.world.generatedChunks.set(
      key,
      {
        generatedChunk: createFullGeneratedChunkRecord(snapshot, writeVersion),
        savedAtMs,
      },
    );
  }

  public async loadGeneratedChunk(chunkX: number, chunkZ: number): Promise<GeneratedChunkStorageRecord | undefined> {
    if (!this.isCurrentSession()) {
      return undefined;
    }

    const record = this.world.generatedChunks.get(chunkKey(chunkX, chunkZ));
    if (record === undefined) {
      return undefined;
    }

    record.lastLoadedAtMs = this.now();
    return cloneGeneratedChunkStorageRecord(record.generatedChunk);
  }

  public async saveGeneratedChunk(record: GeneratedChunkStorageRecord): Promise<void> {
    if (!this.isCurrentSession()) {
      return;
    }

    const existingGeneratedRecord = this.world.generatedChunks.get(chunkKey(record.chunkX, record.chunkZ));
    if (isStaleGeneratedChunkWrite(existingGeneratedRecord, record.writeVersion)) {
      return;
    }

    this.world.generatedChunks.set(
      chunkKey(record.chunkX, record.chunkZ),
      {
        generatedChunk: cloneGeneratedChunkStorageRecord(record),
        savedAtMs: this.now(),
      },
    );
  }

  public async evictChunk(chunkX: number, chunkZ: number): Promise<void> {
    if (!this.isCurrentSession()) {
      return;
    }

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
    private readonly world: MemoryWorldRecord,
    private readonly sessionEpoch: number,
    now: () => number,
  ) {
    this.chunks = new MemoryChunkStorage(world, sessionEpoch, now);
  }

  public async close(): Promise<void> {
    if (this.world.sessionEpoch === this.sessionEpoch) {
      this.world.sessionEpoch++;
    }
  }
}

export class MemoryWorldStorage implements WorldStorage {
  private readonly worlds = new Map<string, MemoryWorldRecord>();

  public constructor(private readonly now: () => number = () => Date.now()) {}

  public async openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession> {
    const existing = this.worlds.get(request.saveId);
    if (existing !== undefined && isWorldSaveMetadataCompatible(existing.metadata, request)) {
      existing.sessionEpoch++;
      existing.metadata = touchWorldSaveMetadata(existing.metadata, request.openedAtMs);
      return new MemoryWorldStorageSession(existing.metadata, existing, existing.sessionEpoch, this.now);
    }

    const previousSessionEpoch = existing?.sessionEpoch ?? 0;
    const world: MemoryWorldRecord = {
      metadata: createWorldSaveMetadata(request),
      chunks: new Map<string, MemoryChunkRecord>(),
      generatedChunks: new Map<string, MemoryGeneratedChunkRecord>(),
      sessionEpoch: previousSessionEpoch + 1,
    };
    this.worlds.set(request.saveId, world);
    return new MemoryWorldStorageSession(world.metadata, world, world.sessionEpoch, this.now);
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
