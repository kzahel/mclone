// 070 Stage 1: the SAB ring control-word ABI is single-sourced in
// ./mclone-runner-shared-abi.js (also imported by the worldgen/light job worker), locked to the
// Rust copy in src/web_server_worker.rs by tests/runner_shared_abi_lock.rs.
import {
  RUNNER_SHARED_STATUS_INDEX,
  RUNNER_SHARED_REQUEST_BYTES_INDEX,
  RUNNER_SHARED_RESPONSE_BYTES_INDEX,
  RUNNER_SHARED_CONTROL_BYTES,
  RUNNER_SHARED_STATUS_COMPLETE,
  RUNNER_SHARED_STATUS_FAILED,
} from "./mclone-runner-shared-abi.js";
import {
  WORLD_CHUNK_STORE,
  WORLD_ENTITY_CHUNK_STORE,
  WORLD_DIMENSION_STORE,
  WORLD_METADATA_STORE,
  WORLD_PLAYER_STORE,
  WORLD_ID_INDEX,
  clearIndexedDbWorldRecords,
  openWorldDb,
} from "./mclone-web-world-catalog.js";
import type { McloneWebIntegratedServerWorker } from "mclone-web-client-wasm";

// The wasm-bindgen module namespace (generated `.d.ts`, emitted by `wasm-bindgen --typescript`).
// Loaded at runtime via a dynamic `import()` of a versioned URL; the bare specifier is path-mapped
// in tsconfig.json and only ever appears in type positions.
type WasmModule = typeof import("mclone-web-client-wasm");

// Inbound postMessage payload for the integrated-server worker. Hand-rolled and `kind`-tagged;
// the `shared-memory` transport carries the SAB ring control/request/response buffers.
interface IntegratedServerWorkerMessage {
  kind?: string;
  requestId?: number;
  seed?: number | string;
  generationProfile?: "overworld" | "authored-only";
  behaviorProfile?: "mutable" | "protected-lobby";
  lightStatusBatchSize?: number;
  jobWorkerUrl?: string;
  bindgenJsUrl?: string;
  bindgenWasmUrl?: string;
  worldStorage?: "transient" | "indexeddb";
  worldId?: string;
  clearWorldStorage?: boolean;
  runnerTransportKind?: string;
  tickIntervalMs?: number;
  freezeScheduledFluidTicks?: boolean;
  debugPassiveShowcase?: boolean;
  debugAuxiliaryPlayerScript?: boolean;
  observerOnly?: boolean;
  frame?: Uint8Array;
  profileId?: Uint8Array;
  displayName?: string;
  transportKind?: "shared-memory" | "message-transfer";
  controlBuffer?: SharedArrayBuffer;
  requestBuffer?: SharedArrayBuffer;
  responseBuffer?: SharedArrayBuffer;
  runnerSharedBufferId?: number;
}

// A pooled shared response slot (worker-local; not part of the cross-boundary control-word ABI).
interface RunnerSharedSlot {
  id: number;
  controlBuffer: SharedArrayBuffer;
  updateBuffer: SharedArrayBuffer;
}

interface RunnerSharedResponseTarget {
  control: Int32Array;
  controlBuffer: SharedArrayBuffer;
  updateBuffer: SharedArrayBuffer;
  pooledResponse: boolean;
  runnerSharedBufferId: number;
}

// Outbound update/response message posted back to the app. A hand-rolled bag read coercion-guarded
// on the main thread; `updates` carries the packed server-update frames.
type RunnerOutboundMessage = Record<string, any> & {
  updates?: unknown[];
};

interface IndexedDbRecord {
  worldId: string;
  dimensionKey: string;
  x: number;
  z: number;
  record: Uint8Array;
}

interface IndexedDbWorldRecords {
  chunks: IndexedDbRecord[];
  entityChunks: IndexedDbRecord[];
  worldMetadataRecord?: Uint8Array;
  legacyRecordsPresent: boolean;
}

interface IndexedDbWorldMetadata {
  worldMetadataRecord?: Uint8Array;
  legacyRecordsPresent: boolean;
}

interface IndexedDbLoadRequest {
  kind: "chunk" | "entityChunk" | "player";
  requestId: number;
  x?: number;
  z?: number;
  dimensionKey?: string;
  playerKey?: string;
}

interface IndexedDbLoadCompletion extends IndexedDbLoadRequest {
  found: boolean;
  record?: Uint8Array;
  error?: string;
}

interface ServicedServerResult {
  result: Record<string, any>;
  updates: unknown[];
  diagnostics: any;
}

let wasmModulePromise: Promise<WasmModule> | null = null;
// The resolved wasm module, captured once `startServer` loads it. The packed-frame codec
// (070 Stage 3) lives in Rust and is reached through this handle, so the SAB packing format has a
// single owner. Non-null for the lifetime of any update-posting path (they run only after the
// server is started).
let wasmModule: WasmModule | null = null;
let server: McloneWebIntegratedServerWorker | null = null;
let indexedDbWorldId: string | null = null;
let tickTimer: ReturnType<typeof setInterval> | 0 = 0;
let tickInFlight = false;
let runnerTransportKind: "shared-memory" | "message-transfer" = "message-transfer";
let nextRunnerSharedBufferId = 1;
const runnerSharedPool: RunnerSharedSlot[] = [];
const runnerSharedInflight = new Map<number, RunnerSharedSlot>();
const workerSelf = self as unknown as DedicatedWorkerGlobalScope;

// Pool-tuning knobs are worker-local — not part of the cross-boundary control-word ABI (which
// is imported above). The worker sizes its own response-buffer pool independently of the Rust
// runner's pool, so these need not agree with any Rust constant.
const MAX_RUNNER_SHARED_POOL_SLOTS = 2;
const DEFAULT_RUNNER_SHARED_RESPONSE_BYTES = 2 * 1024 * 1024;
workerSelf.onmessage = async (event: MessageEvent) => {
  const message = (event.data ?? {}) as IntegratedServerWorkerMessage;
  try {
    switch (message.kind) {
      case "start":
        await startServer(message);
        break;
      case "command":
        await handleCommand(message);
        break;
      case "release-shared-buffer":
        releaseRunnerSharedBuffer(message);
        break;
      case "shutdown":
        await shutdown(message);
        break;
      case "flush-persistence":
        await flushPersistence(message);
        break;
      case "promote-observer":
        await promoteObserver(message);
        break;
      case "demote-player":
        await demotePlayer(message);
        break;
      default:
        postFailure(
          message.requestId,
          `unexpected integrated server worker message kind ${String(message.kind)}`,
        );
        break;
    }
  } catch (error) {
    markRunnerSharedFailure(message);
    postFailure(message.requestId, stringifyError(error));
  }
};

async function startServer(message: IntegratedServerWorkerMessage): Promise<void> {
  if (server) {
    postFailure(message.requestId, "integrated server worker was already started");
    return;
  }

  const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
  wasmModule = module;
  const seed = toBigIntSeed(message.seed);
  const workerConstructor = module.McloneWebIntegratedServerWorker as any;
  const hasJobWorkers = Boolean(message.jobWorkerUrl)
    && typeof workerConstructor.withJobWorkers === "function";
  const indexedDbMode = message.worldStorage === "indexeddb";
  if (indexedDbMode) {
    if (
      hasJobWorkers
      && typeof workerConstructor.withJobWorkersAndIndexedDbExternalLoads === "function"
    ) {
      const metadata = await prepareIndexedDbWorldForStart(message);
      server = workerConstructor.withJobWorkersAndIndexedDbExternalLoads(
        seed,
        String(message.jobWorkerUrl),
        String(message.bindgenJsUrl),
        String(message.bindgenWasmUrl),
        metadata.worldMetadataRecord,
        metadata.legacyRecordsPresent,
      );
    } else if (typeof workerConstructor.withIndexedDbExternalLoads === "function") {
      const metadata = await prepareIndexedDbWorldForStart(message);
      server = workerConstructor.withIndexedDbExternalLoads(
        seed,
        metadata.worldMetadataRecord,
        metadata.legacyRecordsPresent,
      );
    } else {
      const indexedRecords = await loadIndexedDbWorldForStart(message);
      if (
        hasJobWorkers
        && typeof workerConstructor.withJobWorkersAndIndexedDbRecords === "function"
      ) {
        server = workerConstructor.withJobWorkersAndIndexedDbRecords(
          seed,
          String(message.jobWorkerUrl),
          String(message.bindgenJsUrl),
          String(message.bindgenWasmUrl),
          indexedRecords.chunks,
          indexedRecords.entityChunks,
          indexedRecords.worldMetadataRecord,
          indexedRecords.legacyRecordsPresent,
        );
      } else if (typeof workerConstructor.withIndexedDbRecords === "function") {
        server = workerConstructor.withIndexedDbRecords(
          seed,
          indexedRecords.chunks,
          indexedRecords.entityChunks,
          indexedRecords.worldMetadataRecord,
          indexedRecords.legacyRecordsPresent,
        );
      } else {
        throw new Error("wasm module does not expose IndexedDB integrated-server constructors");
      }
    }
  } else {
    indexedDbWorldId = null;
    server = hasJobWorkers
      ? workerConstructor.withJobWorkers(
        seed,
        String(message.jobWorkerUrl),
        String(message.bindgenJsUrl),
        String(message.bindgenWasmUrl),
      )
      : new workerConstructor(seed);
  }
  const generationProfile = String(message.generationProfile ?? "overworld");
  if (typeof (server as any).setWorldGenerationProfile === "function") {
    (server as any).setWorldGenerationProfile(generationProfile);
  }
  const behaviorProfile = String(message.behaviorProfile ?? "mutable");
  if (typeof (server as any).setWorldBehaviorProfile === "function") {
    (server as any).setWorldBehaviorProfile(behaviorProfile);
  }
  if (indexedDbMode) {
    if (typeof (server as any).initializeWorldMetadata !== "function") {
      throw new Error("wasm module does not expose IndexedDB world metadata initialization");
    }
    (server as any).initializeWorldMetadata();
  }
  if (message.observerOnly) {
    if (typeof (server as any).enableObserverMode !== "function") {
      throw new Error("wasm module does not expose observer-only local sessions");
    }
    (server as any).enableObserverMode();
  }
  if (typeof (server as any).setLocalPlayerIdentity === "function") {
    if (!message.profileId || !message.displayName) {
      throw new Error("integrated server start is missing local player identity");
    }
    (server as any).setLocalPlayerIdentity(message.profileId, message.displayName);
  }
  if (typeof (server as any).setScheduledFluidTicksFrozen === "function") {
    (server as any).setScheduledFluidTicksFrozen(Boolean(message.freezeScheduledFluidTicks));
  }
  if (typeof (server as any).setDebugPassiveShowcaseEnabled === "function") {
    (server as any).setDebugPassiveShowcaseEnabled(message.debugPassiveShowcase !== false);
  }
  if (typeof (server as any).setDebugAuxiliaryPlayerScriptEnabled === "function") {
    (server as any).setDebugAuxiliaryPlayerScriptEnabled(
      Boolean(message.debugAuxiliaryPlayerScript),
    );
  }
  const rawLightStatusBatchSize = Number(message.lightStatusBatchSize);
  const lightStatusBatchSize = Number.isFinite(rawLightStatusBatchSize)
    ? Math.trunc(rawLightStatusBatchSize)
    : 0;
  if (lightStatusBatchSize > 0 && typeof (server as any).setLightStatusBatchSize === "function") {
    (server as any).setLightStatusBatchSize(lightStatusBatchSize);
  }
  runnerTransportKind = message.runnerTransportKind === "shared-memory" && sharedTransportAvailable()
    ? "shared-memory"
    : "message-transfer";
  const intervalMs = Math.max(1, Number(message.tickIntervalMs) || 50);
  tickTimer = setInterval(() => {
    void tickServer();
  }, intervalMs);
  if (!server) {
    throw new Error("integrated server worker did not start");
  }
  const diagnostics = server.diagnostics();
  workerSelf.postMessage({
    ok: true,
    kind: "ready",
    requestId: Number(message.requestId) || 0,
    updates: [],
    diagnostics,
  });
}

async function handleCommand(message: IntegratedServerWorkerMessage): Promise<void> {
  if (!server) {
    postFailure(message.requestId, "integrated server worker is not started");
    return;
  }
  const activeServer = server;
  const frame = commandFrame(message);
  let serviced = await serviceIndexedDbResultForCurrentWorld(
    activeServer,
    activeServer.handleCommandFrame(frame),
  );
  const updates = [...serviced.updates];
  let diagnostics = serviced.diagnostics;
  for (let attempt = 0; hasPendingServerJobs(diagnostics) && attempt < 60000; attempt += 1) {
    await waitForJobTurn();
    serviced = await serviceIndexedDbResultForCurrentWorld(activeServer, activeServer.poll());
    updates.push(...serviced.updates);
    diagnostics = serviced.diagnostics;
  }
  if (hasPendingServerJobs(diagnostics)) {
    postFailure(message.requestId, "timed out waiting for web integrated server jobs");
    return;
  }
  postUpdates({
    ok: true,
    kind: "command-result",
    requestId: Number(message.requestId) || 0,
    updates,
    diagnostics,
  }, message);
}

async function tickServer(): Promise<void> {
  if (!server || tickInFlight) return;
  const activeServer = server;
  tickInFlight = true;
  try {
    const serviced = await serviceIndexedDbResultForCurrentWorld(activeServer, activeServer.tick());
    if (serviced.updates.length > 0) {
      postUpdates({
        ok: true,
        kind: "updates",
        requestId: 0,
        updates: serviced.updates,
        diagnostics: serviced.diagnostics,
      }, null);
    }
  } catch (error) {
    postFailure(0, stringifyError(error));
  } finally {
    tickInFlight = false;
  }
}

async function shutdown(message: IntegratedServerWorkerMessage): Promise<void> {
  if (tickTimer) {
    clearInterval(tickTimer);
    tickTimer = 0;
  }
  const result = server?.shutdown?.() ?? { updates: [], diagnostics: null };
  server = null;
  await saveIndexedDbDirtyRecordsForCurrentWorld(result);
  indexedDbWorldId = null;
  workerSelf.postMessage({
    ok: true,
    kind: "shutdown-complete",
    requestId: Number(message.requestId) || 0,
    updates: [],
    diagnostics: result.diagnostics,
  });
  workerSelf.close();
}

async function flushPersistence(message: IntegratedServerWorkerMessage): Promise<void> {
  if (!server || typeof (server as any).flushPersistence !== "function") {
    postFailure(message.requestId, "integrated server worker cannot flush persistence");
    return;
  }
  const activeServer = server;
  const serviced = await serviceIndexedDbResultForCurrentWorld(
    activeServer,
    (activeServer as any).flushPersistence(),
  );
  postUpdates({
    ok: true,
    kind: "flush-complete",
    requestId: Number(message.requestId) || 0,
    updates: serviced.updates,
    diagnostics: serviced.diagnostics,
  });
}

async function promoteObserver(message: IntegratedServerWorkerMessage): Promise<void> {
  if (!server || typeof (server as any).promoteObserverToPlayer !== "function") {
    postFailure(message.requestId, "integrated server worker cannot promote its observer");
    return;
  }
  const activeServer = server;
  const serviced = await serviceIndexedDbResultForCurrentWorld(
    activeServer,
    (activeServer as any).promoteObserverToPlayer(),
  );
  postUpdates({
    ok: true,
    kind: "observer-promoted",
    requestId: Number(message.requestId) || 0,
    updates: serviced.updates,
    diagnostics: serviced.diagnostics,
  }, null);
}

async function demotePlayer(message: IntegratedServerWorkerMessage): Promise<void> {
  if (!server || typeof (server as any).demotePlayerToObserver !== "function") {
    postFailure(message.requestId, "integrated server worker cannot demote its player");
    return;
  }
  const activeServer = server;
  const serviced = await serviceIndexedDbResultForCurrentWorld(
    activeServer,
    (activeServer as any).demotePlayerToObserver(),
  );
  postUpdates({
    ok: true,
    kind: "player-demoted",
    requestId: Number(message.requestId) || 0,
    updates: serviced.updates,
    diagnostics: serviced.diagnostics,
  }, null);
}

async function serviceIndexedDbResultForCurrentWorld(
  activeServer: McloneWebIntegratedServerWorker,
  initialResult: Record<string, any>,
): Promise<ServicedServerResult> {
  let result = initialResult;
  const updates: unknown[] = [];
  let diagnostics = result?.diagnostics ?? null;
  for (let attempt = 0; attempt < 60000; attempt += 1) {
    if (Array.isArray(result?.updates)) {
      updates.push(...result.updates);
    }
    diagnostics = result?.diagnostics ?? diagnostics;
    await saveIndexedDbDirtyRecordsForCurrentWorld(result);

    const requests = indexedDbLoadRequestsFromResult(result);
    if (requests.length === 0) {
      return { result, updates, diagnostics };
    }
    if (!indexedDbWorldId) {
      throw new Error("IndexedDB load requests were emitted without an active browser world");
    }
    const completions = await loadIndexedDbCompletions(indexedDbWorldId, requests);
    result = (activeServer as any).completeIndexedDbLoadRecords(completions) as Record<string, any>;
  }
  throw new Error("timed out servicing IndexedDB persistence load requests");
}

async function prepareIndexedDbWorldForStart(
  message: IntegratedServerWorkerMessage,
): Promise<IndexedDbWorldMetadata> {
  const worldId = indexedDbWorldIdFromMessage(message);
  indexedDbWorldId = worldId;
  const db = await openWorldDb();
  try {
    if (message.clearWorldStorage) {
      await clearIndexedDbWorldRecords(db, worldId);
    }
    return await loadIndexedDbWorldMetadata(db, worldId);
  } finally {
    db.close();
  }
}

async function loadIndexedDbWorldForStart(
  message: IntegratedServerWorkerMessage,
): Promise<IndexedDbWorldRecords> {
  const worldId = indexedDbWorldIdFromMessage(message);
  indexedDbWorldId = worldId;
  const db = await openWorldDb();
  try {
    if (message.clearWorldStorage) {
      await clearIndexedDbWorldRecords(db, worldId);
    }
    const [chunks, entityChunks, metadata] = await Promise.all([
      loadIndexedDbRecords(db, WORLD_CHUNK_STORE, worldId),
      loadIndexedDbRecords(db, WORLD_ENTITY_CHUNK_STORE, worldId),
      loadIndexedDbWorldMetadata(db, worldId),
    ]);
    return { chunks, entityChunks, ...metadata };
  } finally {
    db.close();
  }
}

async function loadIndexedDbWorldMetadata(
  db: IDBDatabase,
  worldId: string,
): Promise<IndexedDbWorldMetadata> {
  const [stored, chunkCount, entityChunkCount, playerCount] = await Promise.all([
    loadIndexedDbWorldMetadataRecord(db, worldId),
    countIndexedDbRecords(db, WORLD_CHUNK_STORE, worldId),
    countIndexedDbRecords(db, WORLD_ENTITY_CHUNK_STORE, worldId),
    countIndexedDbRecords(db, WORLD_PLAYER_STORE, worldId),
  ]);
  return {
    worldMetadataRecord: stored,
    legacyRecordsPresent: chunkCount + entityChunkCount + playerCount > 0,
  };
}

async function loadIndexedDbWorldMetadataRecord(
  db: IDBDatabase,
  worldId: string,
): Promise<Uint8Array | undefined> {
  const transaction = db.transaction(WORLD_METADATA_STORE, "readonly");
  const value = await idbRequest<unknown>(
    transaction.objectStore(WORLD_METADATA_STORE).get(worldId),
  );
  await transactionDone(transaction);
  if (!value) return undefined;
  const record = (value ?? {}) as Record<string, unknown>;
  return uint8ArrayFromUnknown(record.record);
}

async function countIndexedDbRecords(
  db: IDBDatabase,
  storeName: string,
  worldId: string,
): Promise<number> {
  const transaction = db.transaction(storeName, "readonly");
  const count = await idbRequest<number>(
    transaction.objectStore(storeName).index(WORLD_ID_INDEX).count(IDBKeyRange.only(worldId)),
  );
  await transactionDone(transaction);
  return count;
}

function indexedDbWorldIdFromMessage(message: IntegratedServerWorkerMessage): string {
  const explicit = String(message.worldId ?? "").trim();
  if (explicit.length > 0) {
    return explicit;
  }
  return `seed-${String(message.seed ?? "0")}`;
}

async function loadIndexedDbRecords(
  db: IDBDatabase,
  storeName: string,
  worldId: string,
): Promise<IndexedDbRecord[]> {
  const transaction = db.transaction(storeName, "readonly");
  const request = transaction
    .objectStore(storeName)
    .index(WORLD_ID_INDEX)
    .getAll(IDBKeyRange.only(worldId));
  const records = await idbRequest<unknown[]>(request);
  await transactionDone(transaction);
  return records.map((record) => normalizeIndexedDbRecord(worldId, record));
}

async function saveIndexedDbDirtyRecords(worldId: string, result: Record<string, any>): Promise<void> {
  const chunks = indexedDbRecordsFromResult(worldId, result.indexedDbChunks);
  const entityChunks = indexedDbRecordsFromResult(worldId, result.indexedDbEntityChunks);
  const dimensions = indexedDbDimensionRecordsFromResult(worldId, result.indexedDbDimensions);
  const players = indexedDbPlayerRecordsFromResult(worldId, result.indexedDbPlayers);
  const worldMetadata = indexedDbWorldMetadataFromResult(
    worldId,
    result.indexedDbWorldMetadata,
  );
  if (
    chunks.length === 0
    && entityChunks.length === 0
    && dimensions.length === 0
    && players.length === 0
    && !worldMetadata
  ) {
    return;
  }
  const db = await openWorldDb();
  try {
    await Promise.all([
      putIndexedDbRecords(db, WORLD_CHUNK_STORE, chunks),
      putIndexedDbRecords(db, WORLD_ENTITY_CHUNK_STORE, entityChunks),
      putIndexedDbDimensionRecords(db, dimensions),
      putIndexedDbPlayerRecords(db, players),
      putIndexedDbWorldMetadata(db, worldMetadata),
    ]);
  } finally {
    db.close();
  }
}

interface IndexedDbWorldMetadataRecord {
  worldId: string;
  record: Uint8Array;
}

function indexedDbWorldMetadataFromResult(
  worldId: string,
  value: unknown,
): IndexedDbWorldMetadataRecord | null {
  if (value === null || value === undefined) return null;
  return { worldId, record: uint8ArrayFromUnknown(value) };
}

async function putIndexedDbWorldMetadata(
  db: IDBDatabase,
  metadata: IndexedDbWorldMetadataRecord | null,
): Promise<void> {
  if (!metadata) return;
  const transaction = db.transaction(WORLD_METADATA_STORE, "readwrite");
  transaction.objectStore(WORLD_METADATA_STORE).put(metadata);
  await transactionDone(transaction);
}

async function saveIndexedDbDirtyRecordsForCurrentWorld(result: Record<string, any>): Promise<void> {
  if (!indexedDbWorldId) {
    return;
  }
  await saveIndexedDbDirtyRecords(indexedDbWorldId, result);
}

function indexedDbLoadRequestsFromResult(result: Record<string, any>): IndexedDbLoadRequest[] {
  const value = result?.indexedDbLoadRequests;
  if (!Array.isArray(value)) {
    return [];
  }
  return value.map((request) => {
    const record = (request ?? {}) as Record<string, unknown>;
    const kind = String(record.kind ?? "");
    if (kind !== "chunk" && kind !== "entityChunk" && kind !== "player") {
      throw new Error(`unsupported IndexedDB load request kind ${kind}`);
    }
    return {
      kind,
      requestId: Math.trunc(Number(record.requestId) || 0),
      x: Math.trunc(Number(record.x) || 0),
      z: Math.trunc(Number(record.z) || 0),
      dimensionKey: String(record.dimensionKey ?? "minecraft:overworld"),
      playerKey: String(record.playerKey ?? ""),
    };
  });
}

async function loadIndexedDbCompletions(
  worldId: string,
  requests: IndexedDbLoadRequest[],
): Promise<IndexedDbLoadCompletion[]> {
  if (requests.length === 0) {
    return [];
  }
  const db = await openWorldDb();
  try {
    return await Promise.all(
      requests.map((request) => loadIndexedDbCompletion(db, worldId, request)),
    );
  } finally {
    db.close();
  }
}

async function loadIndexedDbCompletion(
  db: IDBDatabase,
  worldId: string,
  request: IndexedDbLoadRequest,
): Promise<IndexedDbLoadCompletion> {
  const storeName = request.kind === "chunk"
    ? WORLD_CHUNK_STORE
    : request.kind === "entityChunk"
      ? WORLD_ENTITY_CHUNK_STORE
      : WORLD_PLAYER_STORE;
  const transaction = db.transaction(storeName, "readonly");
  let key: IDBValidKey;
  if (request.kind === "player") {
    if (typeof request.playerKey !== "string") {
      throw new Error("IndexedDB player load request is missing playerKey");
    }
    key = [worldId, request.playerKey];
  } else {
    if (typeof request.x !== "number" || typeof request.z !== "number") {
      throw new Error(`IndexedDB ${request.kind} load request is missing coordinates`);
    }
    key = [
      worldId,
      request.dimensionKey ?? "minecraft:overworld",
      request.x,
      request.z,
    ];
  }
  const requestHandle = transaction.objectStore(storeName).get(key);
  const record = await idbRequest<unknown>(requestHandle);
  await transactionDone(transaction);
  if (!record) {
    return { ...request, found: false };
  }
  return {
    ...request,
    found: true,
    record: request.kind === "player"
      ? normalizeIndexedDbPlayerRecord(worldId, record).record
      : normalizeIndexedDbRecord(worldId, record).record,
  };
}

async function putIndexedDbRecords(
  db: IDBDatabase,
  storeName: string,
  records: IndexedDbRecord[],
): Promise<void> {
  if (records.length === 0) return;
  const transaction = db.transaction(storeName, "readwrite");
  const store = transaction.objectStore(storeName);
  for (const record of records) {
    store.put(record);
  }
  await transactionDone(transaction);
}

interface IndexedDbPlayerRecord {
  worldId: string;
  playerKey: string;
  record: Uint8Array;
}

interface IndexedDbDimensionRecord {
  worldId: string;
  dimensionKey: string;
  record: Uint8Array;
}

async function putIndexedDbDimensionRecords(
  db: IDBDatabase,
  records: IndexedDbDimensionRecord[],
): Promise<void> {
  if (records.length === 0) return;
  const transaction = db.transaction(WORLD_DIMENSION_STORE, "readwrite");
  const store = transaction.objectStore(WORLD_DIMENSION_STORE);
  for (const record of records) store.put(record);
  await transactionDone(transaction);
}

function indexedDbDimensionRecordsFromResult(
  worldId: string,
  value: unknown,
): IndexedDbDimensionRecord[] {
  if (!Array.isArray(value)) return [];
  return value.map((entry) => {
    const record = (entry ?? {}) as Record<string, unknown>;
    return {
      worldId,
      dimensionKey: String(record.dimensionKey ?? "minecraft:overworld"),
      record: uint8ArrayFromUnknown(record.record),
    };
  });
}

async function putIndexedDbPlayerRecords(
  db: IDBDatabase,
  records: IndexedDbPlayerRecord[],
): Promise<void> {
  if (records.length === 0) return;
  const transaction = db.transaction(WORLD_PLAYER_STORE, "readwrite");
  const store = transaction.objectStore(WORLD_PLAYER_STORE);
  for (const record of records) store.put(record);
  await transactionDone(transaction);
}

function indexedDbPlayerRecordsFromResult(
  worldId: string,
  value: unknown,
): IndexedDbPlayerRecord[] {
  if (!Array.isArray(value)) return [];
  return value.map((record) => normalizeIndexedDbPlayerRecord(worldId, record));
}

function normalizeIndexedDbPlayerRecord(
  worldId: string,
  value: unknown,
): IndexedDbPlayerRecord {
  const record = (value ?? {}) as Record<string, unknown>;
  return {
    worldId,
    playerKey: String(record.playerKey ?? ""),
    record: uint8ArrayFromUnknown(record.record),
  };
}

function indexedDbRecordsFromResult(worldId: string, value: unknown): IndexedDbRecord[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.map((record) => normalizeIndexedDbRecord(worldId, record));
}

function normalizeIndexedDbRecord(worldId: string, value: unknown): IndexedDbRecord {
  const record = (value ?? {}) as Record<string, unknown>;
  return {
    worldId,
    dimensionKey: String(record.dimensionKey ?? "minecraft:overworld"),
    x: Math.trunc(Number(record.x) || 0),
    z: Math.trunc(Number(record.z) || 0),
    record: uint8ArrayFromUnknown(record.record),
  };
}

function uint8ArrayFromUnknown(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (ArrayBuffer.isView(value)) {
    const view = value as ArrayBufferView;
    return new Uint8Array(view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength));
  }
  return new Uint8Array();
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

function postUpdates(
  message: RunnerOutboundMessage,
  requestMessage: IntegratedServerWorkerMessage | null = null,
): void {
  const updates = Array.isArray(message.updates) ? message.updates : [];
  if (runnerTransportKind === "shared-memory" && sharedTransportAvailable()) {
    postSharedUpdates(message, requestMessage, updates);
    return;
  }
  const transfers: Transferable[] = [];
  for (const update of updates) {
    if (update instanceof Uint8Array) {
      transfers.push(update.buffer as ArrayBuffer);
    }
  }
  workerSelf.postMessage(
    {
      ...message,
      updates,
      updateCount: updates.length,
    },
    transfers,
  );
}

function postSharedUpdates(
  message: RunnerOutboundMessage,
  requestMessage: IntegratedServerWorkerMessage | null,
  updates: unknown[],
): void {
  // 070 Stage 3: the packed-frame codec lives in Rust (decode side:
  // unpack_runner_update_frames in src/web_server_worker.rs). Compute the size, size/select
  // the SAB, then let Rust pack the frames in; JS still arms the doorbell below.
  const module = requireWasmModule();
  const packedBytes = module.mcloneWebPackedRunnerUpdateByteLength(updates);
  const shared = sharedRunnerResponseTarget(requestMessage, packedBytes);
  module.mcloneWebWritePackedRunnerUpdates(updates, shared.updateBuffer, packedBytes);
  Atomics.store(shared.control, RUNNER_SHARED_RESPONSE_BYTES_INDEX, packedBytes);
  Atomics.store(shared.control, RUNNER_SHARED_STATUS_INDEX, RUNNER_SHARED_STATUS_COMPLETE);
  Atomics.notify(shared.control, RUNNER_SHARED_STATUS_INDEX, 1);
  const { updates: _updates, ...rest } = message;
  const response: Record<string, any> = {
    ...rest,
    transportKind: "shared-memory",
    updateCount: updates.length,
    controlBuffer: shared.controlBuffer,
    updateBuffer: shared.updateBuffer,
    packedUpdateBytes: packedBytes,
    pooledResponse: shared.pooledResponse,
    responseCapacity: shared.updateBuffer.byteLength,
  };
  if (shared.runnerSharedBufferId > 0) {
    response.runnerSharedBufferId = shared.runnerSharedBufferId;
  }
  workerSelf.postMessage(response);
}

function commandFrame(message: IntegratedServerWorkerMessage): Uint8Array {
  if (message.transportKind !== "shared-memory") {
    return message.frame instanceof Uint8Array ? message.frame : new Uint8Array();
  }
  const control = sharedControlView(message.controlBuffer);
  const requestBytes = Atomics.load(control, RUNNER_SHARED_REQUEST_BYTES_INDEX);
  const requestBuffer = sharedBuffer(message.requestBuffer, "requestBuffer");
  if (!Number.isInteger(requestBytes) || requestBytes < 0 || requestBytes > requestBuffer.byteLength) {
    throw new Error(
      `shared runner command byte count ${requestBytes} exceeds request buffer capacity ${requestBuffer.byteLength}`,
    );
  }
  return new Uint8Array(requestBuffer, 0, requestBytes);
}

function sharedRunnerResponseTarget(
  requestMessage: IntegratedServerWorkerMessage | null,
  packedBytes: number,
): RunnerSharedResponseTarget {
  if (requestMessage?.transportKind === "shared-memory") {
    const control = sharedControlView(requestMessage.controlBuffer);
    const responseBuffer = sharedBuffer(requestMessage.responseBuffer, "responseBuffer");
    const pooledResponse = packedBytes <= responseBuffer.byteLength;
    return {
      control,
      // Validated as a SharedArrayBuffer by `sharedControlView` just above (it throws otherwise).
      controlBuffer: requestMessage.controlBuffer as SharedArrayBuffer,
      updateBuffer: pooledResponse ? responseBuffer : new SharedArrayBuffer(sharedCapacityFor(packedBytes)),
      pooledResponse,
      runnerSharedBufferId: 0,
    };
  }

  const slot = acquireRunnerSharedResponseSlot(packedBytes);
  return {
    control: new Int32Array(slot.controlBuffer),
    controlBuffer: slot.controlBuffer,
    updateBuffer: slot.updateBuffer,
    pooledResponse: true,
    runnerSharedBufferId: slot.id,
  };
}

function acquireRunnerSharedResponseSlot(packedBytes: number): RunnerSharedSlot {
  const capacity = sharedCapacityFor(packedBytes);
  let slot: RunnerSharedSlot | undefined;
  for (let index = runnerSharedPool.length - 1; index >= 0; index -= 1) {
    if (runnerSharedPool[index].updateBuffer.byteLength >= packedBytes) {
      [slot] = runnerSharedPool.splice(index, 1);
      break;
    }
  }
  if (!slot) {
    slot = {
      id: nextRunnerSharedBufferId++,
      controlBuffer: new SharedArrayBuffer(RUNNER_SHARED_CONTROL_BYTES),
      updateBuffer: new SharedArrayBuffer(capacity),
    };
  }
  if (slot.updateBuffer.byteLength < packedBytes) {
    slot.updateBuffer = new SharedArrayBuffer(capacity);
  }
  const control = new Int32Array(slot.controlBuffer);
  Atomics.store(control, RUNNER_SHARED_STATUS_INDEX, 0);
  Atomics.store(control, RUNNER_SHARED_REQUEST_BYTES_INDEX, 0);
  Atomics.store(control, RUNNER_SHARED_RESPONSE_BYTES_INDEX, 0);
  runnerSharedInflight.set(slot.id, slot);
  return slot;
}

function releaseRunnerSharedBuffer(message: IntegratedServerWorkerMessage): void {
  const bufferId = Number(message.runnerSharedBufferId) || 0;
  if (bufferId <= 0) return;
  const slot = runnerSharedInflight.get(bufferId);
  if (!slot) return;
  runnerSharedInflight.delete(bufferId);
  if (runnerSharedPool.length < MAX_RUNNER_SHARED_POOL_SLOTS) {
    runnerSharedPool.push(slot);
  }
}

function sharedCapacityFor(byteLength: number): number {
  let capacity = DEFAULT_RUNNER_SHARED_RESPONSE_BYTES;
  while (capacity < byteLength) {
    capacity *= 2;
  }
  return capacity;
}

function markRunnerSharedFailure(message: IntegratedServerWorkerMessage): void {
  if (message?.transportKind !== "shared-memory") return;
  try {
    const control = sharedControlView(message.controlBuffer);
    Atomics.store(control, RUNNER_SHARED_STATUS_INDEX, RUNNER_SHARED_STATUS_FAILED);
    Atomics.notify(control, RUNNER_SHARED_STATUS_INDEX, 1);
  } catch {}
}

function postFailure(requestId: number | undefined, reason: string): void {
  workerSelf.postMessage({
    ok: false,
    kind: "error",
    requestId: Number(requestId) || 0,
    reason,
  });
}

function loadWasmModule(
  bindgenJsUrl: string | undefined,
  bindgenWasmUrl: string | undefined,
): Promise<WasmModule> {
  wasmModulePromise ??= import(bindgenJsUrl as string).then(async (module: WasmModule) => {
    await module.default(bindgenWasmUrl);
    return module;
  });
  return wasmModulePromise;
}

/**
 * The resolved wasm module, which owns the packed-frame codec (070 Stage 3). Update-posting
 * paths run only after {@link startServer} has set it, so a null here is a programming error.
 */
function requireWasmModule(): WasmModule {
  if (!wasmModule) {
    throw new Error("integrated server worker wasm module is not loaded");
  }
  return wasmModule;
}

function hasPendingServerJobs(diagnostics: any): boolean {
  if (!diagnostics) return false;
  return (
    Number(diagnostics.pendingJobs) > 0
    || Number(diagnostics.pendingPublications) > 0
    || Number(diagnostics.pendingPersistenceLoads) > 0
    || Number(diagnostics.pendingPersistenceSaves) > 0
    || Number(diagnostics.worldgenMailboxPendingJobs) > 0
    || Number(diagnostics.lightStatusMailboxPendingStatuses) > 0
  );
}

function waitForJobTurn(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function sharedControlView(buffer: unknown): Int32Array {
  const shared = sharedBuffer(buffer, "controlBuffer");
  return new Int32Array(shared);
}

function sharedBuffer(buffer: unknown, name: string): SharedArrayBuffer {
  if (typeof SharedArrayBuffer !== "function" || !(buffer instanceof SharedArrayBuffer)) {
    throw new Error(`shared runner ${name} was not a SharedArrayBuffer`);
  }
  return buffer;
}

function sharedTransportAvailable(): boolean {
  return (
    typeof SharedArrayBuffer === "function"
    && typeof Atomics === "object"
    && typeof Atomics.load === "function"
    && typeof Atomics.store === "function"
    && typeof Atomics.notify === "function"
  );
}

function toBigIntSeed(seed: number | string | undefined): bigint {
  const number = Number(seed);
  return BigInt(Number.isFinite(number) ? Math.trunc(number) : 0);
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
