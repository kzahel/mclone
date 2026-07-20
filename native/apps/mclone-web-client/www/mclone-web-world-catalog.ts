export const WORLD_DB_NAME = "mclone-web-worlds";
export const WORLD_DB_VERSION = 6;
export const WORLD_CATALOG_STORE = "worlds";
export const WORLD_CHUNK_STORE = "dimensionChunks";
export const WORLD_ENTITY_CHUNK_STORE = "dimensionEntityChunks";
export const LEGACY_WORLD_CHUNK_STORE = "chunks";
export const LEGACY_WORLD_ENTITY_CHUNK_STORE = "entityChunks";
export const WORLD_DIMENSION_STORE = "dimensions";
export const WORLD_PLAYER_STORE = "players";
export const WORLD_METADATA_STORE = "worldMetadata";
export const MANAGED_WORLD_METADATA_STORE = "managedWorlds";
export const WORLD_ID_INDEX = "worldId";

import type { WebCatalogExecution } from "mclone-web-client-wasm";
import { acquireWorldWriterLease } from "./mclone-web-world-lease.js";
import type { HeldWorldWriterLease } from "./mclone-web-world-lease.js";

type WasmModule = typeof import("mclone-web-client-wasm");

type IndexedDbCatalogPolicy = Pick<
  WasmModule,
  | "mclone_web_managed_scenario_prepare_world"
  | "mclone_web_managed_scenario_validate_world"
>;

let indexedDbCatalogPolicy: IndexedDbCatalogPolicy | null = null;
let lastCatalogTimestamp = 0;

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

interface CatalogStorageStep {
  stepId: number;
  transactions: CatalogStorageTransaction[];
}

interface CatalogStorageTransaction {
  stores: string[];
  mode: IDBTransactionMode;
  optionalStores: boolean;
  actions: CatalogStorageAction[];
}

interface CatalogStorageAction extends Record<string, unknown> {
  actionId: number;
  kind: string;
  store: string;
  needsTimestamp?: boolean;
  key?: string;
  index?: string;
  value?: unknown;
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
      ensureDimensionWorldRecordStore(db, request.transaction, WORLD_CHUNK_STORE);
      ensureDimensionWorldRecordStore(db, request.transaction, WORLD_ENTITY_CHUNK_STORE);
      ensureWorldDimensionStore(db, request.transaction);
      ensureWorldPlayerStore(db, request.transaction);
      ensureWorldMetadataStore(db, request.transaction);
      ensureWorldCatalogStore(db);
      ensureManagedWorldMetadataStore(db);
      migrateLegacyWorldRecordsToOverworld(db, request.transaction);
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("failed to open IndexedDB world store"));
    request.onblocked = () => reject(new Error("IndexedDB world store upgrade was blocked"));
  });
}

export async function executeIndexedDbCatalogExecution(
  db: IDBDatabase,
  execution: WebCatalogExecution,
): Promise<void> {
  const leases = new Map<string, HeldWorldWriterLease>();
  try {
    while (!execution.isComplete()) {
      const requiredLeases = (execution.requiredWriterLeaseNames() as string[]).sort();
      for (const name of requiredLeases) {
        if (!leases.has(name)) {
          leases.set(name, await acquireWorldWriterLease(name));
        }
      }
      const step = execution.nextStorageStep() as CatalogStorageStep | undefined;
      if (!step) {
        if (execution.isComplete()) break;
        throw new Error("Rust catalog continuation returned no storage step");
      }
      await Promise.all(step.transactions.map((transaction) => (
        executeCatalogStorageTransaction(db, execution, step.stepId, transaction)
      )));
      execution.completeStorageStep(step.stepId);
    }
  } finally {
    for (const lease of [...leases.values()].reverse()) await lease.release();
  }
}

function executeCatalogStorageTransaction(
  db: IDBDatabase,
  execution: WebCatalogExecution,
  stepId: number,
  plan: CatalogStorageTransaction,
): Promise<void> {
  const availableStores = plan.stores
    .map((store) => ({ stable: store, browser: catalogBrowserStoreName(store) }))
    .filter(({ browser }) => db.objectStoreNames.contains(browser));
  if (!plan.optionalStores && availableStores.length !== plan.stores.length) {
    const missing = plan.stores.filter((store) => (
      !db.objectStoreNames.contains(catalogBrowserStoreName(store))
    ));
    return Promise.reject(new Error(`IndexedDB catalog stores are missing: ${missing.join(", ")}`));
  }
  if (availableStores.length === 0) return Promise.resolve();
  const available = new Set(availableStores.map(({ stable }) => stable));
  const browserStores = [...new Set(availableStores.map(({ browser }) => browser))];

  return new Promise((resolve, reject) => {
    const transaction = db.transaction(browserStores, plan.mode);
    let settled = false;
    const fail = (error: unknown): void => {
      if (settled) return;
      settled = true;
      try {
        transaction.abort();
      } catch {
        // A request can fail after the transaction has already aborted.
      }
      reject(error);
    };
    transaction.oncomplete = () => {
      if (settled) return;
      settled = true;
      resolve();
    };
    transaction.onerror = () => fail(
      transaction.error ?? new Error("IndexedDB catalog transaction failed"),
    );
    transaction.onabort = () => fail(
      transaction.error ?? new Error("IndexedDB catalog transaction aborted"),
    );
    try {
      for (const action of plan.actions) {
        if (available.has(action.store)) {
          enqueueCatalogStorageAction(transaction, execution, stepId, action, fail);
        }
      }
    } catch (error) {
      fail(error);
    }
  });
}

function enqueueCatalogStorageAction(
  transaction: IDBTransaction,
  execution: WebCatalogExecution,
  stepId: number,
  action: CatalogStorageAction,
  fail: (error: unknown) => void,
): void {
  const store = transaction.objectStore(catalogBrowserStoreName(action.store));
  switch (action.kind) {
    case "get-all":
      enqueueCatalogRead(store.getAll(), transaction, execution, stepId, action, fail);
      return;
    case "get":
      enqueueCatalogRead(store.get(String(action.key ?? "")), transaction, execution, stepId, action, fail);
      return;
    case "add":
      observeCatalogWrite(store.add(action.value), fail);
      return;
    case "put":
      observeCatalogWrite(store.put(action.value), fail);
      return;
    case "delete-key":
      observeCatalogWrite(store.delete(String(action.key ?? "")), fail);
      return;
    case "delete-index-range": {
      const index = catalogBrowserIndexName(String(action.index ?? ""));
      const request = store.index(index).openKeyCursor(IDBKeyRange.only(String(action.key ?? "")));
      request.onsuccess = () => {
        try {
          const cursor = request.result;
          if (!cursor) return;
          observeCatalogWrite(store.delete(cursor.primaryKey), fail);
          cursor.continue();
        } catch (error) {
          fail(error);
        }
      };
      request.onerror = () => fail(request.error ?? new Error("IndexedDB catalog cursor failed"));
      return;
    }
    case "clear":
      observeCatalogWrite(store.clear(), fail);
      return;
    default:
      throw new Error(`unsupported Rust catalog storage action ${JSON.stringify(action.kind)}`);
  }
}

function enqueueCatalogRead<T>(
  request: IDBRequest<T>,
  transaction: IDBTransaction,
  execution: WebCatalogExecution,
  stepId: number,
  action: CatalogStorageAction,
  fail: (error: unknown) => void,
): void {
  request.onsuccess = () => {
    try {
      const now = action.needsTimestamp === true ? nextCatalogTimestamp() : 0;
      const followups = execution.acceptStorageRead(
        stepId,
        action.actionId,
        request.result ?? null,
        now,
      ) as CatalogStorageAction[];
      // Enqueue read-dependent writes synchronously inside this success
      // callback. In particular, record-played get/put must remain in one
      // read/write transaction so a concurrent delete cannot resurrect a row.
      for (const followup of followups) {
        enqueueCatalogStorageAction(transaction, execution, stepId, followup, fail);
      }
    } catch (error) {
      fail(error);
    }
  };
  request.onerror = () => fail(request.error ?? new Error("IndexedDB catalog read failed"));
}

function observeCatalogWrite<T>(request: IDBRequest<T>, fail: (error: unknown) => void): void {
  request.onerror = () => fail(request.error ?? new Error("IndexedDB catalog write failed"));
}

function catalogBrowserStoreName(stableName: string): string {
  switch (stableName) {
    case "catalog": return WORLD_CATALOG_STORE;
    case "dimension-chunks": return WORLD_CHUNK_STORE;
    case "dimension-entity-chunks": return WORLD_ENTITY_CHUNK_STORE;
    case "legacy-chunks": return LEGACY_WORLD_CHUNK_STORE;
    case "legacy-entity-chunks": return LEGACY_WORLD_ENTITY_CHUNK_STORE;
    case "dimensions": return WORLD_DIMENSION_STORE;
    case "players": return WORLD_PLAYER_STORE;
    case "world-metadata": return WORLD_METADATA_STORE;
    case "managed-world-metadata": return MANAGED_WORLD_METADATA_STORE;
    default: throw new Error(`unsupported Rust catalog store ${JSON.stringify(stableName)}`);
  }
}

function catalogBrowserIndexName(stableName: string): string {
  if (stableName === "world-id") return WORLD_ID_INDEX;
  throw new Error(`unsupported Rust catalog index ${JSON.stringify(stableName)}`);
}

export async function clearIndexedDbWorldRecords(
  db: IDBDatabase,
  worldId: string,
): Promise<void> {
  await Promise.all([
    clearIndexedDbStoreForWorld(db, WORLD_CHUNK_STORE, worldId),
    clearIndexedDbStoreForWorld(db, WORLD_ENTITY_CHUNK_STORE, worldId),
    clearIndexedDbStoreForWorld(db, LEGACY_WORLD_CHUNK_STORE, worldId),
    clearIndexedDbStoreForWorld(db, LEGACY_WORLD_ENTITY_CHUNK_STORE, worldId),
    clearIndexedDbStoreForWorld(db, WORLD_DIMENSION_STORE, worldId),
    clearIndexedDbStoreForWorld(db, WORLD_PLAYER_STORE, worldId),
    clearIndexedDbStoreForWorld(db, WORLD_METADATA_STORE, worldId),
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

function ensureDimensionWorldRecordStore(
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
  const store = db.createObjectStore(storeName, {
    keyPath: ["worldId", "dimensionKey", "x", "z"],
  });
  store.createIndex(WORLD_ID_INDEX, "worldId", { unique: false });
}

function ensureWorldDimensionStore(
  db: IDBDatabase,
  transaction: IDBTransaction | null,
): void {
  if (db.objectStoreNames.contains(WORLD_DIMENSION_STORE)) {
    const store = transaction?.objectStore(WORLD_DIMENSION_STORE);
    if (store && !store.indexNames.contains(WORLD_ID_INDEX)) {
      store.createIndex(WORLD_ID_INDEX, "worldId", { unique: false });
    }
    return;
  }
  const store = db.createObjectStore(WORLD_DIMENSION_STORE, {
    keyPath: ["worldId", "dimensionKey"],
  });
  store.createIndex(WORLD_ID_INDEX, "worldId", { unique: false });
}

function migrateLegacyWorldRecordsToOverworld(
  db: IDBDatabase,
  transaction: IDBTransaction | null,
): void {
  if (!transaction) return;
  migrateLegacyWorldRecordStore(
    db,
    transaction,
    LEGACY_WORLD_CHUNK_STORE,
    WORLD_CHUNK_STORE,
  );
  migrateLegacyWorldRecordStore(
    db,
    transaction,
    LEGACY_WORLD_ENTITY_CHUNK_STORE,
    WORLD_ENTITY_CHUNK_STORE,
  );
}

function migrateLegacyWorldRecordStore(
  db: IDBDatabase,
  transaction: IDBTransaction,
  legacyStoreName: string,
  destinationStoreName: string,
): void {
  if (!db.objectStoreNames.contains(legacyStoreName)) return;
  const destination = transaction.objectStore(destinationStoreName);
  const request = transaction.objectStore(legacyStoreName).openCursor();
  request.onsuccess = () => {
    const cursor = request.result;
    if (!cursor) return;
    const value = (cursor.value ?? {}) as Record<string, unknown>;
    destination.put({ ...value, dimensionKey: "minecraft:overworld" });
    cursor.continue();
  };
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

function ensureWorldMetadataStore(
  db: IDBDatabase,
  transaction: IDBTransaction | null,
): void {
  if (db.objectStoreNames.contains(WORLD_METADATA_STORE)) {
    const store = transaction?.objectStore(WORLD_METADATA_STORE);
    if (store && !store.indexNames.contains(WORLD_ID_INDEX)) {
      store.createIndex(WORLD_ID_INDEX, "worldId", { unique: true });
    }
    return;
  }
  const store = db.createObjectStore(WORLD_METADATA_STORE, { keyPath: "worldId" });
  store.createIndex(WORLD_ID_INDEX, "worldId", { unique: true });
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
      chunks.put({
        ...record,
        worldId: payload.worldId,
        dimensionKey: "minecraft:overworld",
      });
    }
    const entityChunks = transaction.objectStore(WORLD_ENTITY_CHUNK_STORE);
    for (const record of payload.entityChunks) {
      entityChunks.put({
        ...record,
        worldId: payload.worldId,
        dimensionKey: "minecraft:overworld",
      });
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

async function clearIndexedDbStoreForWorld(
  db: IDBDatabase,
  storeName: string,
  worldId: string,
): Promise<void> {
  if (!db.objectStoreNames.contains(storeName)) return;
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
