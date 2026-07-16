export const WORLD_DB_NAME = "mclone-web-worlds";
export const WORLD_DB_VERSION = 4;
export const WORLD_CATALOG_STORE = "worlds";
export const WORLD_CHUNK_STORE = "chunks";
export const WORLD_ENTITY_CHUNK_STORE = "entityChunks";
export const WORLD_PLAYER_STORE = "players";
export const MANAGED_WORLD_METADATA_STORE = "managedWorlds";
export const WORLD_ID_INDEX = "worldId";

type WasmModule = typeof import("mclone-web-client-wasm");

type IndexedDbCatalogPolicy = Pick<
  WasmModule,
  | "mclone_web_catalog_validate_world_id"
  | "mclone_web_catalog_prepare_world_list"
  | "mclone_web_catalog_prepare_create_world"
  | "mclone_web_catalog_prepare_open_world"
  | "mclone_web_catalog_prepare_record_world_played"
  | "mclone_web_catalog_prepare_delete_world"
  | "mclone_web_managed_scenario_prepare_world"
  | "mclone_web_managed_scenario_validate_world"
>;

let indexedDbCatalogPolicy: IndexedDbCatalogPolicy | null = null;
let lastCatalogTimestamp = 0;

export type WebWorldGenerationProfile = "overworld" | "authored-only";

function nextCatalogTimestamp(): number {
  lastCatalogTimestamp = Math.max(Date.now(), lastCatalogTimestamp + 1);
  return lastCatalogTimestamp;
}

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
  generationProfile: WebWorldGenerationProfile;
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
  generationProfile?: WebWorldGenerationProfile;
  requestedId?: string | null;
}

export type ManagedWorldValidationStatus =
  | "missing"
  | "valid"
  | "partial"
  | "incompatible"
  | "corrupt";

export interface ManagedWorldValidation {
  status: ManagedWorldValidationStatus;
  detail: string;
}

export interface ManagedWorldProvisionResult {
  operationToken: string;
  role: string;
  worldId: string;
  status: "provisioned" | "reused";
  priorStatus: ManagedWorldValidationStatus;
  chunkCount: number;
  entityChunkCount: number;
  encodedBytes: number;
  materializeMs: number;
  writeMs: number;
}

export interface ManagedWorldProvisionWorkerRequest {
  workerUrl: string;
  bindgenJsUrl: string;
  bindgenWasmUrl: string;
  operationToken: string;
  scenarioId: string;
  role: string;
}

interface ManagedWorldRecord {
  worldId: string;
  x: number;
  z: number;
  record: Uint8Array;
}

interface ManagedWorldPayload {
  worldId: string;
  role: string;
  metadata: Record<string, unknown>;
  chunks: Array<{ x: number, z: number, record: Uint8Array }>;
  entityChunks: Array<{ x: number, z: number, record: Uint8Array }>;
}

interface ManagedWorldStoredState {
  metadata: unknown | null;
  chunks: ManagedWorldRecord[];
  entityChunks: ManagedWorldRecord[];
}

export function openWorldDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(WORLD_DB_NAME, WORLD_DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      ensureWorldRecordStore(db, request.transaction, WORLD_CHUNK_STORE);
      ensureWorldRecordStore(db, request.transaction, WORLD_ENTITY_CHUNK_STORE);
      ensureWorldPlayerStore(db, request.transaction);
      ensureWorldCatalogStore(db);
      ensureManagedWorldMetadataStore(db);
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
      nextCatalogTimestamp(),
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
      nextCatalogTimestamp(),
    ) as WebLocalWorldSummary;
  await putIndexedDbCatalogSummary(db, opened);
  return opened;
}

export async function recordIndexedDbCatalogWorldPlayed(
  db: IDBDatabase,
  id: string,
): Promise<WebLocalWorldSummary> {
  const policy = requireIndexedDbCatalogPolicy();
  const normalizedId = policy.mclone_web_catalog_validate_world_id(id);
  const summary = await getIndexedDbCatalogWorld(db, normalizedId);
  const recorded = policy.mclone_web_catalog_prepare_record_world_played(
    normalizedId,
    summary,
    nextCatalogTimestamp(),
  ) as WebLocalWorldSummary;
  await putIndexedDbCatalogSummary(db, recorded);
  return recorded;
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

export async function deleteAllIndexedDbCatalogWorlds(
  db: IDBDatabase,
  activeWorldId: string | null = null,
): Promise<{ deletedCount: number }> {
  if (activeWorldId && activeWorldId.trim().length > 0) {
    throw new Error("Quit to title before deleting all local worlds");
  }
  const worlds = await listIndexedDbCatalogWorlds(db);
  for (const world of worlds) {
    await deleteIndexedDbCatalogWorld(db, world.id, null);
  }
  return { deletedCount: worlds.length };
}

export async function factoryResetIndexedDbLocalData(
  db: IDBDatabase,
  activeWorldId: string | null = null,
): Promise<{ deletedCount: number }> {
  const result = await deleteAllIndexedDbCatalogWorlds(db, activeWorldId);
  const stores = [
    MANAGED_WORLD_METADATA_STORE,
    WORLD_CHUNK_STORE,
    WORLD_ENTITY_CHUNK_STORE,
    WORLD_PLAYER_STORE,
  ].filter((storeName) => db.objectStoreNames.contains(storeName));
  if (stores.length > 0) {
    const transaction = db.transaction(stores, "readwrite");
    for (const storeName of stores) {
      transaction.objectStore(storeName).clear();
    }
    await transactionDone(transaction);
  }
  return result;
}

export async function clearIndexedDbWorldRecords(
  db: IDBDatabase,
  worldId: string,
): Promise<void> {
  await Promise.all([
    clearIndexedDbStoreForWorld(db, WORLD_CHUNK_STORE, worldId),
    clearIndexedDbStoreForWorld(db, WORLD_ENTITY_CHUNK_STORE, worldId),
    clearIndexedDbStoreForWorld(db, WORLD_PLAYER_STORE, worldId),
  ]);
}

export async function inspectIndexedDbManagedScenarioWorld(
  db: IDBDatabase,
  scenarioId: string,
  role: string,
): Promise<ManagedWorldValidation & { worldId: string, chunkCount: number, entityChunkCount: number }> {
  const payload = prepareManagedWorldPayload(scenarioId, role);
  const stored = await readIndexedDbManagedWorld(db, payload.worldId);
  const validation = validateManagedWorldPayload(scenarioId, role, stored);
  return {
    ...validation,
    worldId: payload.worldId,
    chunkCount: stored.chunks.length,
    entityChunkCount: stored.entityChunks.length,
  };
}

export async function provisionIndexedDbManagedScenarioWorld(
  db: IDBDatabase,
  operationToken: string,
  scenarioId: string,
  role: string,
  signal?: AbortSignal,
): Promise<ManagedWorldProvisionResult> {
  throwIfAborted(signal);
  const materializeStartedAt = performance.now();
  const payload = prepareManagedWorldPayload(scenarioId, role);
  const materializeMs = performance.now() - materializeStartedAt;
  const stored = await readIndexedDbManagedWorld(db, payload.worldId);
  const validation = validateManagedWorldPayload(scenarioId, role, stored);
  if (validation.status === "valid") {
    return managedWorldProvisionResult(
      operationToken,
      payload,
      "reused",
      validation.status,
      materializeMs,
      0,
    );
  }
  if (validation.status === "corrupt") {
    throw new Error(validation.detail);
  }

  throwIfAborted(signal);
  const writeStartedAt = performance.now();
  try {
    await publishIndexedDbManagedWorld(
      db,
      payload,
      validation.status === "missing",
      signal,
    );
  } catch (error) {
    // Concurrent first publication uses metadata.add(). The losing atomic
    // transaction is expected to abort; accept it only after shared validation
    // confirms the winner published the same valid content.
    const raced = await readIndexedDbManagedWorld(db, payload.worldId);
    const racedValidation = validateManagedWorldPayload(scenarioId, role, raced);
    if (racedValidation.status !== "valid") {
      throw error;
    }
    return managedWorldProvisionResult(
      operationToken,
      payload,
      "reused",
      validation.status,
      materializeMs,
      performance.now() - writeStartedAt,
    );
  }
  return managedWorldProvisionResult(
    operationToken,
    payload,
    "provisioned",
    validation.status,
    materializeMs,
    performance.now() - writeStartedAt,
  );
}

/// Run fixture materialization, validation, and IndexedDB publication away
/// from the browser's uncovered animation-frame thread.
export function provisionIndexedDbManagedScenarioWorldInWorker(
  request: ManagedWorldProvisionWorkerRequest,
  signal?: AbortSignal,
): Promise<ManagedWorldProvisionResult> {
  throwIfAborted(signal);
  return new Promise((resolve, reject) => {
    const worker = new Worker(request.workerUrl, {
      type: "module",
      name: "mclone-managed-content",
    });
    let settled = false;
    const finish = (
      callback: () => void,
    ): void => {
      if (settled) return;
      settled = true;
      signal?.removeEventListener("abort", abort);
      worker.terminate();
      callback();
    };
    const abort = (): void => finish(() => reject(
      signal?.reason ?? new DOMException("Managed provisioning aborted", "AbortError"),
    ));
    signal?.addEventListener("abort", abort, { once: true });
    worker.onmessage = (event: MessageEvent<Record<string, any>>) => {
      if (event.data?.ok === true) {
        finish(() => resolve(event.data.result as ManagedWorldProvisionResult));
      } else {
        finish(() => reject(new Error(String(event.data?.error ?? "managed provision failed"))));
      }
    };
    worker.onerror = (event: ErrorEvent) => {
      finish(() => reject(new Error(event.message || "managed provision Worker failed")));
    };
    worker.postMessage({
      kind: "provision",
      bindgenJsUrl: request.bindgenJsUrl,
      bindgenWasmUrl: request.bindgenWasmUrl,
      operationToken: request.operationToken,
      scenarioId: request.scenarioId,
      role: request.role,
    });
  });
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

function ensureWorldPlayerStore(
  db: IDBDatabase,
  transaction: IDBTransaction | null,
): void {
  if (db.objectStoreNames.contains(WORLD_PLAYER_STORE)) {
    const store = transaction?.objectStore(WORLD_PLAYER_STORE);
    if (store && !store.indexNames.contains(WORLD_ID_INDEX)) {
      store.createIndex(WORLD_ID_INDEX, "worldId", { unique: false });
    }
    return;
  }
  const store = db.createObjectStore(WORLD_PLAYER_STORE, {
    keyPath: ["worldId", "playerKey"],
  });
  store.createIndex(WORLD_ID_INDEX, "worldId", { unique: false });
}

function ensureWorldCatalogStore(db: IDBDatabase): void {
  if (!db.objectStoreNames.contains(WORLD_CATALOG_STORE)) {
    db.createObjectStore(WORLD_CATALOG_STORE, { keyPath: "id" });
  }
}

function ensureManagedWorldMetadataStore(db: IDBDatabase): void {
  if (!db.objectStoreNames.contains(MANAGED_WORLD_METADATA_STORE)) {
    db.createObjectStore(MANAGED_WORLD_METADATA_STORE, { keyPath: "worldId" });
  }
}

function prepareManagedWorldPayload(scenarioId: string, role: string): ManagedWorldPayload {
  const payload = requireIndexedDbCatalogPolicy()
    .mclone_web_managed_scenario_prepare_world(scenarioId, role) as ManagedWorldPayload;
  if (!payload || typeof payload.worldId !== "string" || payload.worldId.length === 0) {
    throw new Error("Rust managed-content policy returned no world identity");
  }
  return payload;
}

function validateManagedWorldPayload(
  scenarioId: string,
  role: string,
  stored: ManagedWorldStoredState,
): ManagedWorldValidation {
  return requireIndexedDbCatalogPolicy().mclone_web_managed_scenario_validate_world(
    scenarioId,
    role,
    stored.metadata,
    stored.chunks,
    stored.entityChunks,
  ) as ManagedWorldValidation;
}

async function readIndexedDbManagedWorld(
  db: IDBDatabase,
  worldId: string,
): Promise<ManagedWorldStoredState> {
  const transaction = db.transaction(
    [MANAGED_WORLD_METADATA_STORE, WORLD_CHUNK_STORE, WORLD_ENTITY_CHUNK_STORE],
    "readonly",
  );
  const metadataRequest = idbRequest<unknown>(
    transaction.objectStore(MANAGED_WORLD_METADATA_STORE).get(worldId),
  );
  const chunksRequest = idbRequest<unknown[]>(
    transaction.objectStore(WORLD_CHUNK_STORE).index(WORLD_ID_INDEX).getAll(
      IDBKeyRange.only(worldId),
    ),
  );
  const entityChunksRequest = idbRequest<unknown[]>(
    transaction.objectStore(WORLD_ENTITY_CHUNK_STORE).index(WORLD_ID_INDEX).getAll(
      IDBKeyRange.only(worldId),
    ),
  );
  const [metadata, chunks, entityChunks] = await Promise.all([
    metadataRequest,
    chunksRequest,
    entityChunksRequest,
  ]);
  await transactionDone(transaction);
  return {
    metadata: metadata ?? null,
    chunks: chunks.map((record) => normalizeManagedWorldRecord(worldId, record)),
    entityChunks: entityChunks.map((record) => normalizeManagedWorldRecord(worldId, record)),
  };
}

async function publishIndexedDbManagedWorld(
  db: IDBDatabase,
  payload: ManagedWorldPayload,
  exclusiveMetadata: boolean,
  signal?: AbortSignal,
): Promise<void> {
  const transaction = db.transaction(
    [MANAGED_WORLD_METADATA_STORE, WORLD_CHUNK_STORE, WORLD_ENTITY_CHUNK_STORE],
    "readwrite",
  );
  const abort = (): void => {
    try {
      transaction.abort();
    } catch {
      // The transaction may already have committed between the signal and
      // this callback. Its completion is then authoritative.
    }
  };
  signal?.addEventListener("abort", abort, { once: true });
  try {
    if (!exclusiveMetadata) {
      await Promise.all([
        clearIndexedDbTransactionStoreForWorld(transaction, WORLD_CHUNK_STORE, payload.worldId),
        clearIndexedDbTransactionStoreForWorld(
          transaction,
          WORLD_ENTITY_CHUNK_STORE,
          payload.worldId,
        ),
      ]);
    }
    throwIfAborted(signal);
    const chunks = transaction.objectStore(WORLD_CHUNK_STORE);
    for (const record of payload.chunks) {
      chunks.put({ ...record, worldId: payload.worldId });
    }
    const entityChunks = transaction.objectStore(WORLD_ENTITY_CHUNK_STORE);
    for (const record of payload.entityChunks) {
      entityChunks.put({ ...record, worldId: payload.worldId });
    }
    const metadata = transaction.objectStore(MANAGED_WORLD_METADATA_STORE);
    if (exclusiveMetadata) {
      metadata.add(payload.metadata);
    } else {
      metadata.put(payload.metadata);
    }
    await transactionDone(transaction);
  } finally {
    signal?.removeEventListener("abort", abort);
  }
}

function clearIndexedDbTransactionStoreForWorld(
  transaction: IDBTransaction,
  storeName: string,
  worldId: string,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const store = transaction.objectStore(storeName);
    const request = store.index(WORLD_ID_INDEX).openKeyCursor(IDBKeyRange.only(worldId));
    request.onsuccess = () => {
      const cursor = request.result;
      if (!cursor) {
        resolve();
        return;
      }
      store.delete(cursor.primaryKey);
      cursor.continue();
    };
    request.onerror = () => reject(request.error ?? new Error("IndexedDB cursor failed"));
  });
}

function normalizeManagedWorldRecord(worldId: string, value: unknown): ManagedWorldRecord {
  const record = (value ?? {}) as Record<string, unknown>;
  return {
    worldId,
    x: Math.trunc(Number(record.x) || 0),
    z: Math.trunc(Number(record.z) || 0),
    record: bytesFromUnknown(record.record),
  };
}

function bytesFromUnknown(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) return value;
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength));
  }
  return new Uint8Array();
}

function managedWorldProvisionResult(
  operationToken: string,
  payload: ManagedWorldPayload,
  status: "provisioned" | "reused",
  priorStatus: ManagedWorldValidationStatus,
  materializeMs: number,
  writeMs: number,
): ManagedWorldProvisionResult {
  return {
    operationToken,
    role: payload.role,
    worldId: payload.worldId,
    status,
    priorStatus,
    chunkCount: payload.chunks.length,
    entityChunkCount: payload.entityChunks.length,
    encodedBytes: [...payload.chunks, ...payload.entityChunks]
      .reduce((total, record) => total + record.record.byteLength, 0),
    materializeMs,
    writeMs,
  };
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) {
    throw signal.reason ?? new DOMException("Managed provisioning aborted", "AbortError");
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
