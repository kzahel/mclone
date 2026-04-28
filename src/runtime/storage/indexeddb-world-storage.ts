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

export const INDEXED_DB_WORLD_STORAGE_DATABASE_NAME = "mclone-world-storage";
const DATABASE_VERSION = 2;
const WORLDS_STORE = "worlds";
const CHUNKS_STORE = "chunks";
const CHUNKS_BY_SAVE_INDEX = "chunks_by_save";
const GENERATED_CHUNKS_STORE = "generated_chunks";
const GENERATED_CHUNKS_BY_SAVE_INDEX = "generated_chunks_by_save";

type IndexedDbChunkRecord = {
  readonly saveId: string;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly snapshot: PackedChunkSnapshot;
  readonly savedAtMs: number;
  readonly lastLoadedAtMs?: number;
  readonly lastEvictedAtMs?: number;
};

type IndexedDbGeneratedChunkRecord = {
  readonly saveId: string;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly generatedChunk: GeneratedChunkStorageRecord;
  readonly savedAtMs: number;
  readonly lastLoadedAtMs?: number;
  readonly lastEvictedAtMs?: number;
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

function openIndexedDb(factory: IDBFactory): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = factory.open(INDEXED_DB_WORLD_STORAGE_DATABASE_NAME, DATABASE_VERSION);
    request.onerror = () => reject(request.error ?? new Error(`Unable to open ${INDEXED_DB_WORLD_STORAGE_DATABASE_NAME}`));
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(WORLDS_STORE)) {
        database.createObjectStore(WORLDS_STORE, { keyPath: "saveId" });
      }

      if (!database.objectStoreNames.contains(CHUNKS_STORE)) {
        const store = database.createObjectStore(CHUNKS_STORE, {
          keyPath: ["saveId", "chunkX", "chunkZ"],
        });
        store.createIndex(CHUNKS_BY_SAVE_INDEX, "saveId", { unique: false });
      }

      if (!database.objectStoreNames.contains(GENERATED_CHUNKS_STORE)) {
        const store = database.createObjectStore(GENERATED_CHUNKS_STORE, {
          keyPath: ["saveId", "chunkX", "chunkZ"],
        });
        store.createIndex(GENERATED_CHUNKS_BY_SAVE_INDEX, "saveId", { unique: false });
      }
    };
    request.onsuccess = () => resolve(request.result);
  });
}

export function deleteIndexedDbWorldStorage(factory: IDBFactory): Promise<void> {
  return new Promise((resolve, reject) => {
    let blockedTimeout: ReturnType<typeof setTimeout> | undefined;
    const clearBlockedTimeout = (): void => {
      if (blockedTimeout !== undefined) {
        clearTimeout(blockedTimeout);
        blockedTimeout = undefined;
      }
    };
    const request = factory.deleteDatabase(INDEXED_DB_WORLD_STORAGE_DATABASE_NAME);
    request.onerror = () => {
      clearBlockedTimeout();
      reject(request.error ?? new Error(`Unable to delete ${INDEXED_DB_WORLD_STORAGE_DATABASE_NAME}`));
    };
    request.onblocked = () => {
      clearBlockedTimeout();
      blockedTimeout = setTimeout(() => {
        reject(new Error(`Unable to delete ${INDEXED_DB_WORLD_STORAGE_DATABASE_NAME}: open connections are still using it`));
      }, 5_000);
    };
    request.onsuccess = () => {
      clearBlockedTimeout();
      resolve();
    };
  });
}

function waitForTransaction(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  });
}

function requestToPromise<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

async function clearSaveChunks(database: IDBDatabase, saveId: string): Promise<void> {
  const transaction = database.transaction([CHUNKS_STORE, GENERATED_CHUNKS_STORE], "readwrite");
  for (const [storeName, indexName] of [
    [CHUNKS_STORE, CHUNKS_BY_SAVE_INDEX],
    [GENERATED_CHUNKS_STORE, GENERATED_CHUNKS_BY_SAVE_INDEX],
  ] as const) {
    const store = transaction.objectStore(storeName);
    const index = store.index(indexName);
    const keys = await requestToPromise(index.getAllKeys(saveId));
    for (const key of keys) {
      store.delete(key);
    }
  }

  await waitForTransaction(transaction);
}

class IndexedDbChunkStorage implements ChunkStorage {
  public constructor(
    private readonly database: IDBDatabase,
    private readonly saveId: string,
    private readonly now: () => number,
  ) {}

  public async loadChunk(chunkX: number, chunkZ: number): Promise<PackedChunkSnapshot | undefined> {
    const transaction = this.database.transaction(CHUNKS_STORE, "readwrite");
    const store = transaction.objectStore(CHUNKS_STORE);
    const key = [this.saveId, chunkX, chunkZ];
    const record = (await requestToPromise(store.get(key))) as IndexedDbChunkRecord | undefined;
    if (record !== undefined) {
      store.put({
        ...record,
        lastLoadedAtMs: this.now(),
      });
    }

    await waitForTransaction(transaction);
    return record === undefined ? undefined : clonePackedChunkSnapshot(record.snapshot);
  }

  public async saveChunk(snapshot: PackedChunkSnapshot): Promise<void> {
    const transaction = this.database.transaction([CHUNKS_STORE, GENERATED_CHUNKS_STORE], "readwrite");
    const savedAtMs = this.now();
    transaction.objectStore(CHUNKS_STORE).put({
      saveId: this.saveId,
      chunkX: snapshot.chunkX,
      chunkZ: snapshot.chunkZ,
      snapshot: clonePackedChunkSnapshot(snapshot),
      savedAtMs,
    } satisfies IndexedDbChunkRecord);
    transaction.objectStore(GENERATED_CHUNKS_STORE).put({
      saveId: this.saveId,
      chunkX: snapshot.chunkX,
      chunkZ: snapshot.chunkZ,
      generatedChunk: createFullGeneratedChunkRecord(snapshot),
      savedAtMs,
    } satisfies IndexedDbGeneratedChunkRecord);
    await waitForTransaction(transaction);
  }

  public async loadGeneratedChunk(chunkX: number, chunkZ: number): Promise<GeneratedChunkStorageRecord | undefined> {
    const transaction = this.database.transaction(GENERATED_CHUNKS_STORE, "readwrite");
    const store = transaction.objectStore(GENERATED_CHUNKS_STORE);
    const key = [this.saveId, chunkX, chunkZ];
    const record = (await requestToPromise(store.get(key))) as IndexedDbGeneratedChunkRecord | undefined;
    if (record !== undefined) {
      store.put({
        ...record,
        lastLoadedAtMs: this.now(),
      });
    }

    await waitForTransaction(transaction);
    return record === undefined ? undefined : cloneGeneratedChunkStorageRecord(record.generatedChunk);
  }

  public async saveGeneratedChunk(record: GeneratedChunkStorageRecord): Promise<void> {
    const transaction = this.database.transaction(GENERATED_CHUNKS_STORE, "readwrite");
    transaction.objectStore(GENERATED_CHUNKS_STORE).put({
      saveId: this.saveId,
      chunkX: record.chunkX,
      chunkZ: record.chunkZ,
      generatedChunk: cloneGeneratedChunkStorageRecord(record),
      savedAtMs: this.now(),
    } satisfies IndexedDbGeneratedChunkRecord);
    await waitForTransaction(transaction);
  }

  public async evictChunk(chunkX: number, chunkZ: number): Promise<void> {
    const transaction = this.database.transaction([CHUNKS_STORE, GENERATED_CHUNKS_STORE], "readwrite");
    const key = [this.saveId, chunkX, chunkZ];
    const store = transaction.objectStore(CHUNKS_STORE);
    const record = (await requestToPromise(store.get(key))) as IndexedDbChunkRecord | undefined;
    if (record !== undefined) {
      store.put({
        ...record,
        lastEvictedAtMs: this.now(),
      });
    }
    const generatedStore = transaction.objectStore(GENERATED_CHUNKS_STORE);
    const generatedRecord = (await requestToPromise(generatedStore.get(key))) as IndexedDbGeneratedChunkRecord | undefined;
    if (generatedRecord !== undefined) {
      generatedStore.put({
        ...generatedRecord,
        lastEvictedAtMs: this.now(),
      });
    }

    await waitForTransaction(transaction);
  }
}

class IndexedDbWorldStorageSession implements WorldStorageSession {
  public readonly chunks: ChunkStorage;

  public constructor(
    public readonly metadata: WorldSaveMetadata,
    database: IDBDatabase,
    now: () => number,
  ) {
    this.chunks = new IndexedDbChunkStorage(database, metadata.saveId, now);
  }

  public async close(): Promise<void> {}
}

export class IndexedDbWorldStorage implements WorldStorage {
  private databasePromise: Promise<IDBDatabase> | undefined;

  public constructor(
    private readonly factory: IDBFactory,
    private readonly now: () => number = () => Date.now(),
  ) {}

  public async openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession> {
    const database = await this.getDatabase();
    const worldsTransaction = database.transaction(WORLDS_STORE, "readwrite");
    const worldsStore = worldsTransaction.objectStore(WORLDS_STORE);
    const existing = (await requestToPromise(worldsStore.get(request.saveId))) as WorldSaveMetadata | undefined;

    let metadata: WorldSaveMetadata;
    if (existing !== undefined && isWorldSaveMetadataCompatible(existing, request)) {
      metadata = touchWorldSaveMetadata(existing, request.openedAtMs);
      worldsStore.put(metadata);
      await waitForTransaction(worldsTransaction);
    } else {
      worldsTransaction.abort();
      await clearSaveChunks(database, request.saveId);
      metadata = createWorldSaveMetadata(request);
      const createTransaction = database.transaction(WORLDS_STORE, "readwrite");
      createTransaction.objectStore(WORLDS_STORE).put(metadata);
      await waitForTransaction(createTransaction);
    }

    return new IndexedDbWorldStorageSession(metadata, database, this.now);
  }

  private getDatabase(): Promise<IDBDatabase> {
    this.databasePromise ??= openIndexedDb(this.factory);
    return this.databasePromise;
  }
}
