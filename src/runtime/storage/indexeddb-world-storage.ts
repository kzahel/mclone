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

const DATABASE_NAME = "mclone-world-storage";
const DATABASE_VERSION = 1;
const WORLDS_STORE = "worlds";
const CHUNKS_STORE = "chunks";
const CHUNKS_BY_SAVE_INDEX = "chunks_by_save";

type IndexedDbChunkRecord = {
  readonly saveId: string;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly snapshot: ChunkSnapshot;
  readonly savedAtMs: number;
  readonly lastLoadedAtMs?: number;
  readonly lastEvictedAtMs?: number;
};

function openIndexedDb(factory: IDBFactory): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = factory.open(DATABASE_NAME, DATABASE_VERSION);
    request.onerror = () => reject(request.error ?? new Error(`Unable to open ${DATABASE_NAME}`));
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
    };
    request.onsuccess = () => resolve(request.result);
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
  const transaction = database.transaction(CHUNKS_STORE, "readwrite");
  const store = transaction.objectStore(CHUNKS_STORE);
  const index = store.index(CHUNKS_BY_SAVE_INDEX);
  const keys = await requestToPromise(index.getAllKeys(saveId));
  for (const key of keys) {
    store.delete(key);
  }

  await waitForTransaction(transaction);
}

class IndexedDbChunkStorage implements ChunkStorage {
  public constructor(
    private readonly database: IDBDatabase,
    private readonly saveId: string,
    private readonly now: () => number,
  ) {}

  public async loadChunk(chunkX: number, chunkZ: number): Promise<ChunkSnapshot | undefined> {
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
    return record?.snapshot;
  }

  public async saveChunk(snapshot: ChunkSnapshot): Promise<void> {
    const transaction = this.database.transaction(CHUNKS_STORE, "readwrite");
    transaction.objectStore(CHUNKS_STORE).put({
      saveId: this.saveId,
      chunkX: snapshot.chunkX,
      chunkZ: snapshot.chunkZ,
      snapshot,
      savedAtMs: this.now(),
    } satisfies IndexedDbChunkRecord);
    await waitForTransaction(transaction);
  }

  public async evictChunk(chunkX: number, chunkZ: number): Promise<void> {
    const transaction = this.database.transaction(CHUNKS_STORE, "readwrite");
    const store = transaction.objectStore(CHUNKS_STORE);
    const key = [this.saveId, chunkX, chunkZ];
    const record = (await requestToPromise(store.get(key))) as IndexedDbChunkRecord | undefined;
    if (record !== undefined) {
      store.put({
        ...record,
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
