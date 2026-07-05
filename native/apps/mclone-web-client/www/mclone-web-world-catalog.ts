export const WORLD_DB_NAME = "mclone-web-worlds";
export const WORLD_DB_VERSION = 2;
export const WORLD_CATALOG_STORE = "worlds";
export const WORLD_CHUNK_STORE = "chunks";
export const WORLD_ENTITY_CHUNK_STORE = "entityChunks";
export const WORLD_ID_INDEX = "worldId";

type WasmModule = typeof import("mclone-web-client-wasm");

type IndexedDbCatalogPolicy = Pick<
  WasmModule,
  | "mclone_web_catalog_validate_world_id"
  | "mclone_web_catalog_prepare_world_list"
  | "mclone_web_catalog_prepare_create_world"
  | "mclone_web_catalog_prepare_open_world"
  | "mclone_web_catalog_prepare_delete_world"
>;

let indexedDbCatalogPolicy: IndexedDbCatalogPolicy | null = null;

export function setIndexedDbCatalogPolicy(policy: IndexedDbCatalogPolicy): void {
  indexedDbCatalogPolicy = policy;
}

function requireIndexedDbCatalogPolicy(): IndexedDbCatalogPolicy {
  if (!indexedDbCatalogPolicy) {
    throw new Error("Rust world catalog policy is not initialized");
  }
  return indexedDbCatalogPolicy;
}

export interface WebLocalWorldSummary {
  id: string;
  displayName: string;
  seed: number;
  createdUnixMillis: number;
  lastPlayedUnixMillis: number | null;
  storageSchemaVersion: number;
  targetMinecraftVersion: string;
  mcloneVersion: string | null;
  backendLabel: string | null;
  locked: boolean;
  compatible: boolean;
}

export interface WebLocalWorldCreateOptions {
  displayName: string;
  seed: number;
  requestedId?: string | null;
}

export function openWorldDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(WORLD_DB_NAME, WORLD_DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      ensureWorldRecordStore(db, request.transaction, WORLD_CHUNK_STORE);
      ensureWorldRecordStore(db, request.transaction, WORLD_ENTITY_CHUNK_STORE);
      ensureWorldCatalogStore(db);
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("failed to open IndexedDB world store"));
    request.onblocked = () => reject(new Error("IndexedDB world store upgrade was blocked"));
  });
}

export async function listIndexedDbCatalogWorlds(
  db: IDBDatabase,
): Promise<WebLocalWorldSummary[]> {
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readonly");
  const records = await idbRequest<unknown[]>(
    transaction.objectStore(WORLD_CATALOG_STORE).getAll(),
  );
  await transactionDone(transaction);
  return requireIndexedDbCatalogPolicy()
    .mclone_web_catalog_prepare_world_list(records) as WebLocalWorldSummary[];
}

export async function createIndexedDbCatalogWorld(
  db: IDBDatabase,
  options: WebLocalWorldCreateOptions,
): Promise<WebLocalWorldSummary> {
  const existingRecords = await readIndexedDbCatalogRecords(db);
  const summary = requireIndexedDbCatalogPolicy()
    .mclone_web_catalog_prepare_create_world(
      options,
      existingRecords,
      Date.now(),
    ) as WebLocalWorldSummary;

  const transaction = db.transaction(WORLD_CATALOG_STORE, "readwrite");
  transaction.objectStore(WORLD_CATALOG_STORE).add(summary);
  await transactionDone(transaction);
  return summary;
}

export async function openIndexedDbCatalogWorld(
  db: IDBDatabase,
  id: string,
): Promise<WebLocalWorldSummary> {
  const normalizedId = requireIndexedDbCatalogPolicy()
    .mclone_web_catalog_validate_world_id(id);
  const summary = await getIndexedDbCatalogWorld(db, normalizedId);
  const opened = requireIndexedDbCatalogPolicy()
    .mclone_web_catalog_prepare_open_world(
      normalizedId,
      summary,
      Date.now(),
    ) as WebLocalWorldSummary;
  await putIndexedDbCatalogSummary(db, opened);
  return opened;
}

export async function deleteIndexedDbCatalogWorld(
  db: IDBDatabase,
  id: string,
  activeWorldId: string | null = null,
): Promise<WebLocalWorldSummary> {
  const policy = requireIndexedDbCatalogPolicy();
  const normalizedId = policy.mclone_web_catalog_validate_world_id(id);
  const summary = await getIndexedDbCatalogWorld(db, normalizedId);
  const deleted = policy.mclone_web_catalog_prepare_delete_world(
    normalizedId,
    activeWorldId ?? "",
    summary,
  ) as WebLocalWorldSummary;

  await clearIndexedDbWorldRecords(db, normalizedId);
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readwrite");
  transaction.objectStore(WORLD_CATALOG_STORE).delete(normalizedId);
  await transactionDone(transaction);
  return deleted;
}

export async function clearIndexedDbWorldRecords(
  db: IDBDatabase,
  worldId: string,
): Promise<void> {
  await Promise.all([
    clearIndexedDbStoreForWorld(db, WORLD_CHUNK_STORE, worldId),
    clearIndexedDbStoreForWorld(db, WORLD_ENTITY_CHUNK_STORE, worldId),
  ]);
}

function ensureWorldRecordStore(
  db: IDBDatabase,
  transaction: IDBTransaction | null,
  storeName: string,
): void {
  if (db.objectStoreNames.contains(storeName)) {
    const store = transaction?.objectStore(storeName);
    if (store && !store.indexNames.contains(WORLD_ID_INDEX)) {
      store.createIndex(WORLD_ID_INDEX, "worldId", { unique: false });
    }
    return;
  }
  const store = db.createObjectStore(storeName, { keyPath: ["worldId", "x", "z"] });
  store.createIndex(WORLD_ID_INDEX, "worldId", { unique: false });
}

function ensureWorldCatalogStore(db: IDBDatabase): void {
  if (!db.objectStoreNames.contains(WORLD_CATALOG_STORE)) {
    db.createObjectStore(WORLD_CATALOG_STORE, { keyPath: "id" });
  }
}

async function getIndexedDbCatalogWorld(
  db: IDBDatabase,
  id: string,
): Promise<unknown | null> {
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readonly");
  const record = await idbRequest<unknown>(
    transaction.objectStore(WORLD_CATALOG_STORE).get(id),
  );
  await transactionDone(transaction);
  return record ?? null;
}

async function readIndexedDbCatalogRecords(db: IDBDatabase): Promise<unknown[]> {
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readonly");
  const records = await idbRequest<unknown[]>(
    transaction.objectStore(WORLD_CATALOG_STORE).getAll(),
  );
  await transactionDone(transaction);
  return records;
}

async function putIndexedDbCatalogSummary(
  db: IDBDatabase,
  summary: WebLocalWorldSummary,
): Promise<void> {
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readwrite");
  transaction.objectStore(WORLD_CATALOG_STORE).put(summary);
  await transactionDone(transaction);
}

async function clearIndexedDbStoreForWorld(
  db: IDBDatabase,
  storeName: string,
  worldId: string,
): Promise<void> {
  const transaction = db.transaction(storeName, "readwrite");
  const store = transaction.objectStore(storeName);
  const request = store.index(WORLD_ID_INDEX).openKeyCursor(IDBKeyRange.only(worldId));
  request.onsuccess = () => {
    const cursor = request.result;
    if (!cursor) return;
    store.delete(cursor.primaryKey);
    cursor.continue();
  };
  await transactionDone(transaction);
}

function idbRequest<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function transactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  });
}
