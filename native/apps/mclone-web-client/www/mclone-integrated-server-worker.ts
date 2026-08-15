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
  clearIndexedDbWorldRecords,
  openWorldDb,
} from "./mclone-web-world-catalog.js";
import {
  executeIndexedDbRecordRequests,
} from "./mclone-web-persistence-executor.js";
import type { PersistenceRecordRequest } from "./mclone-web-persistence-executor.js";
import { acquireWorldWriterLease } from "./mclone-web-world-lease.js";
import type { HeldWorldWriterLease } from "./mclone-web-world-lease.js";
import type {
  WebIntegratedServerActor,
  WebIntegratedServerStartup,
} from "mclone-web-client-wasm";

// The wasm-bindgen module namespace (generated `.d.ts`, emitted by `wasm-bindgen --typescript`).
// Loaded at runtime via a dynamic `import()` of a versioned URL; the bare specifier is path-mapped
// in tsconfig.json and only ever appears in type positions.
type WasmModule = typeof import("mclone-web-client-wasm");

// Inbound postMessage payload for the integrated-server worker. Hand-rolled and `kind`-tagged;
// the `shared-memory` transport carries the SAB ring control/request/response buffers.
interface IntegratedServerWorkerMessage {
  kind?: string;
  requestId?: number;
  startupFrame?: Uint8Array;
  jobWorkerUrl?: string;
  bindgenJsUrl?: string;
  bindgenWasmUrl?: string;
  worldStorage?: "transient" | "indexeddb";
  worldId?: string;
  clearWorldStorage?: boolean;
  runnerTransportKind?: string;
  tickIntervalMs?: number;
  frame?: Uint8Array;
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

interface ServicedServerResult {
  result: Record<string, any>;
  updates: unknown[];
}

// Adapter-only circuit breaker against a malformed continuation that never
// quiesces. Rust still authors every request and decides whether another
// continuation exists.
const MAX_BROWSER_PERSISTENCE_CONTINUATIONS = 60_000;

let wasmModulePromise: Promise<WasmModule> | null = null;
// The resolved wasm module, captured once `startServer` loads it. The packed-frame codec
// (070 Stage 3) lives in Rust and is reached through this handle, so the SAB packing format has a
// single owner. Non-null for the lifetime of any update-posting path (they run only after the
// server is started).
let wasmModule: WasmModule | null = null;
let server: WebIntegratedServerActor | null = null;
let indexedDbWorldId: string | null = null;
let indexedDbWriterLease: HeldWorldWriterLease | null = null;
let tickTimer: ReturnType<typeof setInterval> | 0 = 0;
let tickInFlight = false;
let serverOperationInFlight = false;
let persistenceFenceDepth = 0;
let persistenceContinuationTail: Promise<void> = Promise.resolve();
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
    if (message.kind === "start") {
      await startServer(message);
    } else if (message.kind === "release-shared-buffer") {
      releaseRunnerSharedBuffer(message);
    } else {
      await driveActorMessage(message);
    }
  } catch (error) {
    markRunnerSharedFailure(message);
    postActorFailure(message.requestId, error);
  }
};

async function startServer(message: IntegratedServerWorkerMessage): Promise<void> {
  if (server) {
    postFailure(message.requestId, "integrated server worker was already started");
    return;
  }

  const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
  wasmModule = module;
  if (!(message.startupFrame instanceof Uint8Array)) {
    throw new Error("integrated server start is missing its opaque Rust startup frame");
  }
  const startup: WebIntegratedServerStartup = new module.WebIntegratedServerStartup(
    message.startupFrame,
  );
  const indexedDbMode = message.worldStorage === "indexeddb";
  try {
    if (indexedDbMode) {
      const worldId = indexedDbWorldIdFromMessage(message);
      indexedDbWorldId = worldId;
      indexedDbWriterLease = await acquireWorldWriterLease(
        startup.indexedDbWriterLeaseName(worldId),
      );
      await prepareIndexedDbWorldForStart(message);
      const bootstrap = await executeIndexedDbRecordRequests(
        worldId,
        persistenceRecordRequestsFromValue(startup.indexedDbBootstrapRequests()),
      );
      server = startup.createIndexedDb(
        bootstrap,
        String(message.jobWorkerUrl ?? ""),
        String(message.bindgenJsUrl ?? ""),
        String(message.bindgenWasmUrl ?? ""),
      );
    } else {
      indexedDbWorldId = null;
      server = startup.createTransient(
        String(message.jobWorkerUrl ?? ""),
        String(message.bindgenJsUrl ?? ""),
        String(message.bindgenWasmUrl ?? ""),
      );
    }
  } catch (error) {
    await releaseIndexedDbWriterLease();
    indexedDbWorldId = null;
    throw error;
  } finally {
    startup.free();
  }
  runnerTransportKind = message.runnerTransportKind === "shared-memory" && sharedTransportAvailable()
    ? "shared-memory"
    : "message-transfer";
  const intervalMs = Number(message.tickIntervalMs);
  if (!Number.isFinite(intervalMs) || intervalMs < 1) {
    throw new Error("integrated server start has an invalid Rust-authored tick interval");
  }
  tickTimer = setInterval(() => {
    void tickServer();
  }, Math.trunc(intervalMs));
  if (!server) {
    throw new Error("integrated server worker did not start");
  }
  workerSelf.postMessage(server.readyReport(Number(message.requestId) || 0));
}

async function driveActorMessage(message: IntegratedServerWorkerMessage): Promise<void> {
  if (!server) {
    postFailure(message.requestId, "integrated server worker is not started");
    return;
  }
  const persistenceFence = message.kind === "flush-persistence" || message.kind === "shutdown";
  if (persistenceFence) {
    persistenceFenceDepth += 1;
    await persistenceContinuationTail;
  } else {
    while (persistenceFenceDepth > 0) {
      await waitForJobTurn();
    }
  }
  await acquireServerOperation();
  let detachedPersistenceRequests: PersistenceRecordRequest[] = [];
  try {
    const activeServer = server;
    if (!activeServer) {
      postFailure(message.requestId, "integrated server worker stopped before actor dispatch");
      return;
    }
    const initial = activeServer.beginMessage(message, commandFrame(message));
    if (persistenceFence) {
      await drivePersistenceFenceOperation(activeServer, initial as Record<string, any>, message);
    } else {
      detachedPersistenceRequests = finishActorOperation(
        activeServer,
        initial as Record<string, any>,
        message,
      );
    }
  } catch (error) {
    markRunnerSharedFailure(message);
    postActorFailure(message.requestId, error);
  } finally {
    serverOperationInFlight = false;
    if (persistenceFence) persistenceFenceDepth -= 1;
  }
  schedulePersistenceRequests(detachedPersistenceRequests);
}

async function tickServer(): Promise<void> {
  if (!server || tickInFlight || serverOperationInFlight || persistenceFenceDepth > 0) return;
  const activeServer = server;
  tickInFlight = true;
  serverOperationInFlight = true;
  try {
    const initial = activeServer.beginTick();
    const requests = finishActorOperation(activeServer, initial as Record<string, any>, null);
    schedulePersistenceRequests(requests);
  } catch (error) {
    postActorFailure(0, error);
  } finally {
    tickInFlight = false;
    serverOperationInFlight = false;
  }
}

async function acquireServerOperation(): Promise<void> {
  while (serverOperationInFlight) {
    await waitForJobTurn();
  }
  serverOperationInFlight = true;
}

function finishActorOperation(
  activeServer: WebIntegratedServerActor,
  initialResult: Record<string, any>,
  requestMessage: IntegratedServerWorkerMessage | null,
): PersistenceRecordRequest[] {
  const updates = Array.isArray(initialResult?.updates) ? [...initialResult.updates] : [];
  const report = activeServer.finishOperation(initialResult, updates) as Record<string, any>;
  if (report.closeWorker === true) {
    throw new Error("durable integrated-server close bypassed its persistence fence");
  }
  if (report.postMessage === true) {
    postUpdates(report.message as RunnerOutboundMessage, requestMessage);
  }
  return persistenceRecordRequestsFromValue(report.message?.persistenceRecordRequests);
}

async function drivePersistenceFenceOperation(
  activeServer: WebIntegratedServerActor,
  initialResult: Record<string, any>,
  requestMessage: IntegratedServerWorkerMessage,
): Promise<void> {
  const serviced = await servicePersistenceFenceResultForCurrentWorld(activeServer, initialResult);
  const report = activeServer.finishOperation(serviced.result, serviced.updates) as Record<string, any>;
  if (report.closeWorker === true) {
    if (tickTimer) {
      clearInterval(tickTimer);
      tickTimer = 0;
    }
    server = null;
    // `shutdown-complete` is the browser host's durable retirement fence. Release
    // the world writer lease before publishing it so a same-page replacement can
    // distinguish graceful handoff from genuine cross-tab contention.
    await releaseIndexedDbWriterLease();
    indexedDbWorldId = null;
  }
  if (report.postMessage === true) {
    postUpdates(report.message as RunnerOutboundMessage, requestMessage);
  }
  if (report.closeWorker === true) {
    workerSelf.close();
  }
}

function schedulePersistenceRequests(requests: PersistenceRecordRequest[]): void {
  if (requests.length === 0) return;
  const scheduled = persistenceContinuationTail.then(() => drivePersistenceRequests(requests));
  persistenceContinuationTail = scheduled.catch((error) => {
    postActorFailure(0, error);
  });
}

async function drivePersistenceRequests(initialRequests: PersistenceRecordRequest[]): Promise<void> {
  let requests = initialRequests;
  for (let attempt = 0; attempt < MAX_BROWSER_PERSISTENCE_CONTINUATIONS; attempt += 1) {
    const worldId = indexedDbWorldId;
    if (!worldId) {
      throw new Error("persistence record requests were emitted without an active browser world");
    }
    const completions = await executeIndexedDbRecordRequests(worldId, requests);
    await acquireServerOperation();
    try {
      const activeServer = server;
      if (!activeServer) {
        throw new Error("integrated server stopped before persistence completion");
      }
      const initial = activeServer.beginPersistenceCompletion(completions) as Record<string, any>;
      requests = finishActorOperation(activeServer, initial, null);
    } catch (error) {
      postActorFailure(0, error);
      return;
    } finally {
      serverOperationInFlight = false;
    }
    if (requests.length === 0) return;
    await waitForJobTurn();
  }
  throw new Error("timed out servicing detached IndexedDB persistence record requests");
}

async function servicePersistenceFenceResultForCurrentWorld(
  activeServer: WebIntegratedServerActor,
  initialResult: Record<string, any>,
): Promise<ServicedServerResult> {
  let result = initialResult;
  const updates: unknown[] = [];
  for (let attempt = 0; attempt < MAX_BROWSER_PERSISTENCE_CONTINUATIONS; attempt += 1) {
    if (Array.isArray(result?.updates)) {
      updates.push(...result.updates);
    }
    const requests = persistenceRecordRequestsFromValue(result?.persistenceRecordRequests);
    if (requests.length === 0) {
      return { result, updates };
    }
    if (!indexedDbWorldId) {
      throw new Error("persistence record requests were emitted without an active browser world");
    }
    const completions = await executeIndexedDbRecordRequests(indexedDbWorldId, requests);
    result = activeServer.continuePersistenceFence(completions) as Record<string, any>;
    if (Array.isArray(result?.updates)) {
      updates.push(...result.updates);
      result.updates = [];
    }
    await waitForJobTurn();
  }
  throw new Error("timed out servicing IndexedDB persistence record requests");
}

async function prepareIndexedDbWorldForStart(
  message: IntegratedServerWorkerMessage,
): Promise<void> {
  const worldId = indexedDbWorldIdFromMessage(message);
  const db = await openWorldDb();
  try {
    if (message.clearWorldStorage) {
      await clearIndexedDbWorldRecords(db, worldId);
    }
  } finally {
    db.close();
  }
}

function indexedDbWorldIdFromMessage(message: IntegratedServerWorkerMessage): string {
  const explicit = String(message.worldId ?? "").trim();
  if (explicit.length > 0) {
    return explicit;
  }
  throw new Error("IndexedDB integrated server start is missing its Rust-authored world id");
}

function persistenceRecordRequestsFromValue(value: unknown): PersistenceRecordRequest[] {
  if (!Array.isArray(value)) return [];
  return value as PersistenceRecordRequest[];
}

async function releaseIndexedDbWriterLease(): Promise<void> {
  const lease = indexedDbWriterLease;
  indexedDbWriterLease = null;
  if (lease) await lease.release();
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

function postActorFailure(requestId: number | undefined, error: unknown): void {
  const reason = stringifyError(error);
  const activeServer = server;
  if (!activeServer) {
    postFailure(requestId, reason);
    return;
  }
  try {
    workerSelf.postMessage(activeServer.failOperation(Number(requestId) || 0, reason));
  } catch (actorError) {
    postFailure(
      requestId,
      `${reason}\nfailed to author Rust actor failure: ${stringifyError(actorError)}`,
    );
  }
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

function stringifyError(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
