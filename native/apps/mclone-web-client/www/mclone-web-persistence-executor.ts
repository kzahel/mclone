import {
  WORLD_CHUNK_STORE,
  WORLD_DIMENSION_STORE,
  WORLD_ENTITY_CHUNK_STORE,
  WORLD_ID_INDEX,
  WORLD_METADATA_STORE,
  WORLD_PLAYER_STORE,
  WORLD_SAVED_DATA_STORE,
  openWorldDb,
} from "./mclone-web-world-catalog.js";

export interface PersistenceRecordKeyPart {
  kind: "text" | "i32";
  value: string | number;
}

export interface PersistenceRecordAddress {
  namespace: number;
  key: PersistenceRecordKeyPart[];
}

export interface PersistenceRecordMutation extends PersistenceRecordAddress {
  kind: "put" | "delete";
  codecVersion?: number;
  revision?: string;
  record?: Uint8Array;
}

export interface PersistenceRecordRequest extends Partial<PersistenceRecordAddress> {
  requestId: string;
  kind: "read" | "probeAny" | "commit" | "flush" | "close";
  namespaces?: number[];
  mutations?: PersistenceRecordMutation[];
}

export interface PersistenceRecordCompletion extends Partial<PersistenceRecordAddress> {
  requestId: string;
  kind: PersistenceRecordRequest["kind"];
  ok: boolean;
  found?: boolean;
  value?: boolean;
  codecVersion?: number;
  revision?: string;
  record?: Uint8Array;
  errorKind?: string;
  error?: string;
}

interface NamespaceSpec {
  store: string;
  keyKinds: PersistenceRecordKeyPart["kind"][];
  valueFields: string[];
}

export interface IndexedDbRecordExecutorMetrics {
  requestBatchCount: number;
  requestCount: number;
  readCount: number;
  probeCount: number;
  commitCount: number;
  flushCount: number;
  closeCount: number;
  failedCount: number;
  transactionCount: number;
  committedMutationCount: number;
  maxRequestBatchLength: number;
  maxCommitMutationCount: number;
  rustToBrowserOpaqueBytes: number;
  browserToRustOpaqueBytes: number;
  totalRequestLatencyMs: number;
  maxRequestLatencyMs: number;
}

export interface BrowserStoragePersistenceStatus {
  storageManagerAvailable: boolean;
  persisted: boolean | null;
  persistenceRequestAvailable: boolean;
  usageBytes: number | null;
  quotaBytes: number | null;
}

const metrics: IndexedDbRecordExecutorMetrics = emptyMetrics();

const NAMESPACE_SPECS = new Map<number, NamespaceSpec>([
  [0, { store: WORLD_METADATA_STORE, keyKinds: [], valueFields: [] }],
  [1, { store: WORLD_DIMENSION_STORE, keyKinds: ["text"], valueFields: ["dimensionKey"] }],
  [2, {
    store: WORLD_CHUNK_STORE,
    keyKinds: ["text", "i32", "i32"],
    valueFields: ["dimensionKey", "x", "z"],
  }],
  [3, {
    store: WORLD_ENTITY_CHUNK_STORE,
    keyKinds: ["text", "i32", "i32"],
    valueFields: ["dimensionKey", "x", "z"],
  }],
  [4, { store: WORLD_PLAYER_STORE, keyKinds: ["text"], valueFields: ["playerKey"] }],
  [5, { store: WORLD_SAVED_DATA_STORE, keyKinds: ["text"], valueFields: ["savedDataKey"] }],
]);

export async function executeIndexedDbRecordRequests(
  worldId: string,
  requests: PersistenceRecordRequest[],
): Promise<PersistenceRecordCompletion[]> {
  if (requests.length === 0) return [];
  metrics.requestBatchCount += 1;
  metrics.maxRequestBatchLength = Math.max(metrics.maxRequestBatchLength, requests.length);
  let db: IDBDatabase | null = null;
  const completions: PersistenceRecordCompletion[] = [];
  try {
    db = await openWorldDb();
    for (const request of requests) {
      const startedAt = performance.now();
      observeRequest(request);
      try {
        const completion = await executeIndexedDbRecordRequest(db, worldId, request);
        observeCompletion(completion);
        completions.push(completion);
      } catch (error) {
        const completion = failedCompletion(request, error);
        observeCompletion(completion);
        completions.push(completion);
      } finally {
        const elapsedMs = performance.now() - startedAt;
        metrics.totalRequestLatencyMs += elapsedMs;
        metrics.maxRequestLatencyMs = Math.max(metrics.maxRequestLatencyMs, elapsedMs);
      }
    }
  } catch (error) {
    for (const request of requests) {
      observeRequest(request);
      const completion = failedCompletion(request, error);
      observeCompletion(completion);
      completions.push(completion);
    }
  } finally {
    db?.close();
  }
  return completions;
}

export function indexedDbRecordExecutorMetricsSnapshot(): IndexedDbRecordExecutorMetrics {
  return { ...metrics };
}

export function resetIndexedDbRecordExecutorMetrics(): void {
  Object.assign(metrics, emptyMetrics());
}

export async function browserStoragePersistenceStatus(): Promise<BrowserStoragePersistenceStatus> {
  const storage = navigator.storage;
  if (!storage) {
    return {
      storageManagerAvailable: false,
      persisted: null,
      persistenceRequestAvailable: false,
      usageBytes: null,
      quotaBytes: null,
    };
  }
  const [persisted, estimate] = await Promise.all([
    typeof storage.persisted === "function" ? storage.persisted() : Promise.resolve(null),
    typeof storage.estimate === "function"
      ? storage.estimate()
      : Promise.resolve({} as StorageEstimate),
  ]);
  return {
    storageManagerAvailable: true,
    persisted,
    persistenceRequestAvailable: typeof storage.persist === "function",
    usageBytes: Number.isFinite(estimate.usage) ? Number(estimate.usage) : null,
    quotaBytes: Number.isFinite(estimate.quota) ? Number(estimate.quota) : null,
  };
}

/** Product policy may call this from a user gesture; startup never calls it implicitly. */
export async function requestBrowserStoragePersistence(): Promise<boolean | null> {
  const storage = navigator.storage;
  return storage && typeof storage.persist === "function" ? await storage.persist() : null;
}

export function browserPersistenceErrorKind(error: unknown): string {
  return classifyBrowserStorageError(error).kind;
}

async function executeIndexedDbRecordRequest(
  db: IDBDatabase,
  worldId: string,
  request: PersistenceRecordRequest,
): Promise<PersistenceRecordCompletion> {
  switch (request.kind) {
    case "read": return readRecord(db, worldId, request);
    case "probeAny": return probeAnyRecord(db, worldId, request);
    case "commit": return commitRecords(db, worldId, request);
    case "flush":
    case "close":
      return successfulCompletion(request);
  }
}

async function readRecord(
  db: IDBDatabase,
  worldId: string,
  request: PersistenceRecordRequest,
): Promise<PersistenceRecordCompletion> {
  metrics.transactionCount += 1;
  const address = requiredAddress(request);
  const spec = namespaceSpec(address.namespace);
  const transaction = db.transaction(spec.store, "readonly");
  const value = await idbRequest<unknown>(
    transaction.objectStore(spec.store).get(physicalKey(worldId, address, spec)),
  );
  await transactionDone(transaction);
  if (!value) {
    return { ...successfulCompletion(request), ...address, found: false };
  }
  const stored = value as Record<string, unknown>;
  return {
    ...successfulCompletion(request),
    ...address,
    found: true,
    codecVersion: storedCodecVersion(stored.codecVersion),
    revision: storedRevision(stored.revision),
    record: uint8ArrayFromUnknown(stored.record),
  };
}

async function probeAnyRecord(
  db: IDBDatabase,
  worldId: string,
  request: PersistenceRecordRequest,
): Promise<PersistenceRecordCompletion> {
  const specs = [...new Set((request.namespaces ?? []).map((namespace) => namespaceSpec(namespace)))];
  if (specs.length === 0) {
    return { ...successfulCompletion(request), value: false };
  }
  metrics.transactionCount += 1;
  const transaction = db.transaction(specs.map((spec) => spec.store), "readonly");
  const counts = await Promise.all(specs.map((spec) => idbRequest<number>(
    transaction.objectStore(spec.store).index(WORLD_ID_INDEX).count(IDBKeyRange.only(worldId)),
  )));
  await transactionDone(transaction);
  return { ...successfulCompletion(request), value: counts.some((count) => count > 0) };
}

async function commitRecords(
  db: IDBDatabase,
  worldId: string,
  request: PersistenceRecordRequest,
): Promise<PersistenceRecordCompletion> {
  const mutations = request.mutations ?? [];
  if (mutations.length === 0) return successfulCompletion(request);
  const addressed = mutations.map((mutation) => ({
    mutation,
    address: requiredAddress(mutation),
    spec: namespaceSpec(mutation.namespace),
  }));
  const stores = [...new Set(addressed.map(({ spec }) => spec.store))];
  metrics.transactionCount += 1;
  const transaction = db.transaction(stores, "readwrite");
  for (const { mutation, address, spec } of addressed) {
    const store = transaction.objectStore(spec.store);
    if (mutation.kind === "delete") {
      store.delete(physicalKey(worldId, address, spec));
    } else {
      store.put(physicalValue(worldId, address, spec, mutation));
    }
  }
  await transactionDone(transaction);
  metrics.committedMutationCount += mutations.length;
  return successfulCompletion(request);
}

function observeRequest(request: PersistenceRecordRequest): void {
  metrics.requestCount += 1;
  switch (request.kind) {
    case "read": metrics.readCount += 1; break;
    case "probeAny": metrics.probeCount += 1; break;
    case "commit": {
      metrics.commitCount += 1;
      const mutations = request.mutations ?? [];
      metrics.maxCommitMutationCount = Math.max(metrics.maxCommitMutationCount, mutations.length);
      metrics.rustToBrowserOpaqueBytes += mutations.reduce(
        (total, mutation) => total + (mutation.record?.byteLength ?? 0),
        0,
      );
      break;
    }
    case "flush": metrics.flushCount += 1; break;
    case "close": metrics.closeCount += 1; break;
  }
}

function observeCompletion(completion: PersistenceRecordCompletion): void {
  if (!completion.ok) metrics.failedCount += 1;
  metrics.browserToRustOpaqueBytes += completion.record?.byteLength ?? 0;
}

function emptyMetrics(): IndexedDbRecordExecutorMetrics {
  return {
    requestBatchCount: 0,
    requestCount: 0,
    readCount: 0,
    probeCount: 0,
    commitCount: 0,
    flushCount: 0,
    closeCount: 0,
    failedCount: 0,
    transactionCount: 0,
    committedMutationCount: 0,
    maxRequestBatchLength: 0,
    maxCommitMutationCount: 0,
    rustToBrowserOpaqueBytes: 0,
    browserToRustOpaqueBytes: 0,
    totalRequestLatencyMs: 0,
    maxRequestLatencyMs: 0,
  };
}

function requiredAddress(value: Partial<PersistenceRecordAddress>): PersistenceRecordAddress {
  if (!Number.isInteger(value.namespace) || !Array.isArray(value.key)) {
    throw classifiedError("invalid-data", "persistence record request has an invalid address");
  }
  const address = { namespace: value.namespace as number, key: value.key };
  const spec = namespaceSpec(address.namespace);
  if (address.key.length !== spec.keyKinds.length) {
    throw classifiedError(
      "invalid-data",
      `namespace ${address.namespace} expected ${spec.keyKinds.length} key parts`,
    );
  }
  for (let index = 0; index < address.key.length; index += 1) {
    const part = address.key[index];
    if (part.kind !== spec.keyKinds[index]
      || (part.kind === "text" && typeof part.value !== "string")
      || (part.kind === "i32" && (!Number.isInteger(part.value)
        || Number(part.value) < -2147483648
        || Number(part.value) > 2147483647))) {
      throw classifiedError(
        "invalid-data",
        `namespace ${address.namespace} has an invalid key part ${index}`,
      );
    }
  }
  return address;
}

function namespaceSpec(namespace: number): NamespaceSpec {
  const spec = NAMESPACE_SPECS.get(namespace);
  if (!spec) {
    throw classifiedError(
      "unavailable",
      `IndexedDB schema does not implement persistence namespace ${namespace}`,
    );
  }
  return spec;
}

function physicalKey(
  worldId: string,
  address: PersistenceRecordAddress,
  spec: NamespaceSpec,
): IDBValidKey {
  const parts = address.key.map((part) => part.value as string | number);
  return spec.keyKinds.length === 0 ? worldId : [worldId, ...parts];
}

function physicalValue(
  worldId: string,
  address: PersistenceRecordAddress,
  spec: NamespaceSpec,
  mutation: PersistenceRecordMutation,
): Record<string, unknown> {
  const record = mutation.record;
  if (!(record instanceof Uint8Array)) {
    throw classifiedError("invalid-data", "persistence put is missing opaque record bytes");
  }
  const codecVersion = mutation.codecVersion ?? 0;
  if (!Number.isInteger(codecVersion) || codecVersion < 0 || codecVersion > 0xffff_ffff) {
    throw classifiedError("invalid-data", "persistence put has an invalid codec version");
  }
  const revision = storedRevision(mutation.revision);
  const value: Record<string, unknown> = { worldId, codecVersion, revision, record };
  for (let index = 0; index < spec.valueFields.length; index += 1) {
    value[spec.valueFields[index]] = address.key[index].value;
  }
  return value;
}

function storedCodecVersion(value: unknown): number {
  const codecVersion = value === undefined ? 0 : Number(value);
  if (!Number.isInteger(codecVersion) || codecVersion < 0 || codecVersion > 0xffff_ffff) {
    throw classifiedError("corrupt", "IndexedDB record has an invalid codec version");
  }
  return codecVersion;
}

function storedRevision(value: unknown): string {
  const revision = value === undefined ? "0" : String(value);
  if (!/^(0|[1-9][0-9]*)$/.test(revision)
    || BigInt(revision) > 0xffff_ffff_ffff_ffffn) {
    throw classifiedError("corrupt", "IndexedDB record has an invalid revision");
  }
  return revision;
}

function successfulCompletion(request: PersistenceRecordRequest): PersistenceRecordCompletion {
  return { requestId: request.requestId, kind: request.kind, ok: true };
}

function failedCompletion(
  request: PersistenceRecordRequest,
  error: unknown,
): PersistenceRecordCompletion {
  const classified = classifyBrowserStorageError(error);
  return {
    requestId: request.requestId,
    kind: request.kind,
    ok: false,
    ...(request.kind === "read" ? requiredAddressForFailure(request) : {}),
    errorKind: classified.kind,
    error: classified.message,
  };
}

function requiredAddressForFailure(
  request: PersistenceRecordRequest,
): Partial<PersistenceRecordAddress> {
  return Number.isInteger(request.namespace) && Array.isArray(request.key)
    ? { namespace: request.namespace, key: request.key }
    : {};
}

function classifyBrowserStorageError(error: unknown): { kind: string; message: string } {
  if (error && typeof error === "object" && "persistenceKind" in error) {
    return {
      kind: String((error as { persistenceKind: unknown }).persistenceKind),
      message: stringifyError(error),
    };
  }
  const name = error instanceof DOMException ? error.name : "";
  const kind = name === "QuotaExceededError"
    ? "quota"
    : name === "AbortError"
      ? "cancelled"
      : name === "DataCloneError"
        ? "invalid-data"
      : name === "SecurityError" || name === "NotAllowedError" || name === "InvalidStateError"
        ? "unavailable"
        : "backend";
  return { kind, message: stringifyError(error) };
}

function classifiedError(kind: string, message: string): Error & { persistenceKind: string } {
  return Object.assign(new Error(message), { persistenceKind: kind });
}

function uint8ArrayFromUnknown(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) return value;
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) {
    const view = value as ArrayBufferView;
    return new Uint8Array(view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength));
  }
  throw classifiedError("corrupt", "IndexedDB record bytes are not a byte array");
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
    transaction.onerror = () => reject(
      transaction.error ?? new Error("IndexedDB transaction failed"),
    );
    transaction.onabort = () => reject(
      transaction.error ?? new DOMException("IndexedDB transaction aborted", "AbortError"),
    );
  });
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) {
    const stack = error.stack?.trim();
    if (stack) return stack;
    const label = error.name.trim();
    const message = error.message.trim();
    return label && message ? `${label}: ${message}` : label || message || "browser storage error";
  }
  return String(error);
}
