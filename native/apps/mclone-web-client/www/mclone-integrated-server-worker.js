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

/**
 * The wasm-bindgen module namespace (generated `.d.ts`, emitted by `wasm-bindgen --typescript`).
 * Loaded at runtime via a dynamic `import()` of a versioned URL; the bare specifier is path-mapped
 * in tsconfig.json and only ever appears in type positions.
 * @typedef {typeof import("mclone-web-client-wasm")} WasmModule
 * @typedef {import("mclone-web-client-wasm").McloneWebIntegratedServerWorker} IntegratedServer
 */

/**
 * Inbound postMessage payload for the integrated-server worker. Hand-rolled and `kind`-tagged;
 * the `shared-memory` transport carries the SAB ring control/request/response buffers.
 * @typedef {object} IntegratedServerWorkerMessage
 * @property {string} [kind]
 * @property {number} [requestId]
 * @property {number | string} [seed]
 * @property {string} [jobWorkerUrl]
 * @property {string} [bindgenJsUrl]
 * @property {string} [bindgenWasmUrl]
 * @property {string} [runnerTransportKind]
 * @property {number} [tickIntervalMs]
 * @property {Uint8Array} [frame]
 * @property {"shared-memory" | "message-transfer"} [transportKind]
 * @property {SharedArrayBuffer} [controlBuffer]
 * @property {SharedArrayBuffer} [requestBuffer]
 * @property {SharedArrayBuffer} [responseBuffer]
 * @property {number} [runnerSharedBufferId]
 */

/**
 * A pooled shared response slot (worker-local; not part of the cross-boundary control-word ABI).
 * @typedef {object} RunnerSharedSlot
 * @property {number} id
 * @property {SharedArrayBuffer} controlBuffer
 * @property {SharedArrayBuffer} updateBuffer
 */

/**
 * Outbound update/response message posted back to the app. A hand-rolled bag read coercion-guarded
 * on the main thread; `updates` carries the packed server-update frames.
 * @typedef {Record<string, any>} RunnerOutboundMessage
 */

/** @type {Promise<WasmModule> | null} */
let wasmModulePromise = null;
/**
 * The resolved wasm module, captured once {@link startServer} loads it. The packed-frame
 * codec (070 Stage 3) lives in Rust and is reached through this handle, so the SAB packing
 * format has a single owner. Non-null for the lifetime of any update-posting path (they run
 * only after the server is started).
 * @type {WasmModule | null}
 */
let wasmModule = null;
/** @type {IntegratedServer | null} */
let server = null;
/** @type {ReturnType<typeof setInterval> | number} */
let tickTimer = 0;
let runnerTransportKind = "message-transfer";
let nextRunnerSharedBufferId = 1;
/** @type {RunnerSharedSlot[]} */
const runnerSharedPool = [];
/** @type {Map<number, RunnerSharedSlot>} */
const runnerSharedInflight = new Map();

// Pool-tuning knobs are worker-local — not part of the cross-boundary control-word ABI (which
// is imported above). The worker sizes its own response-buffer pool independently of the Rust
// runner's pool, so these need not agree with any Rust constant.
const MAX_RUNNER_SHARED_POOL_SLOTS = 2;
const DEFAULT_RUNNER_SHARED_RESPONSE_BYTES = 2 * 1024 * 1024;

self.onmessage = async (event) => {
  const message = /** @type {IntegratedServerWorkerMessage} */ (event.data ?? {});
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
        shutdown(message);
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

/** @param {IntegratedServerWorkerMessage} message */
async function startServer(message) {
  if (server) {
    postFailure(message.requestId, "integrated server worker was already started");
    return;
  }

  const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
  wasmModule = module;
  server = message.jobWorkerUrl && typeof module.McloneWebIntegratedServerWorker.withJobWorkers === "function"
    ? module.McloneWebIntegratedServerWorker.withJobWorkers(
      toBigIntSeed(message.seed),
      String(message.jobWorkerUrl),
      String(message.bindgenJsUrl),
      String(message.bindgenWasmUrl),
    )
    : new module.McloneWebIntegratedServerWorker(toBigIntSeed(message.seed));
  runnerTransportKind = message.runnerTransportKind === "shared-memory" && sharedTransportAvailable()
    ? "shared-memory"
    : "message-transfer";
  const intervalMs = Math.max(1, Number(message.tickIntervalMs) || 50);
  tickTimer = setInterval(tickServer, intervalMs);
  const diagnostics = server.diagnostics();
  self.postMessage({
    ok: true,
    kind: "ready",
    requestId: Number(message.requestId) || 0,
    updates: [],
    diagnostics,
  });
}

/** @param {IntegratedServerWorkerMessage} message */
async function handleCommand(message) {
  if (!server) {
    postFailure(message.requestId, "integrated server worker is not started");
    return;
  }
  const frame = commandFrame(message);
  const result = server.handleCommandFrame(frame);
  const updates = Array.isArray(result.updates) ? [...result.updates] : [];
  let diagnostics = result.diagnostics;
  for (let attempt = 0; hasPendingServerJobs(diagnostics) && attempt < 60000; attempt += 1) {
    await waitForJobTurn();
    const poll = server.poll();
    if (Array.isArray(poll.updates)) {
      updates.push(...poll.updates);
    }
    diagnostics = poll.diagnostics;
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

function tickServer() {
  if (!server) return;
  try {
    const result = server.tick();
    if (Number(result.updateCount) > 0) {
      postUpdates({
        ok: true,
        kind: "updates",
        requestId: 0,
        updates: result.updates,
        diagnostics: result.diagnostics,
      }, null);
    }
  } catch (error) {
    postFailure(0, stringifyError(error));
  }
}

/** @param {IntegratedServerWorkerMessage} message */
function shutdown(message) {
  if (tickTimer) {
    clearInterval(tickTimer);
    tickTimer = 0;
  }
  const result = server?.shutdown?.() ?? { updates: [], diagnostics: null };
  server = null;
  self.postMessage({
    ok: true,
    kind: "shutdown-complete",
    requestId: Number(message.requestId) || 0,
    updates: [],
    diagnostics: result.diagnostics,
  });
}

/**
 * @param {RunnerOutboundMessage} message
 * @param {IntegratedServerWorkerMessage | null} [requestMessage]
 */
function postUpdates(message, requestMessage = null) {
  const updates = Array.isArray(message.updates) ? message.updates : [];
  if (runnerTransportKind === "shared-memory" && sharedTransportAvailable()) {
    postSharedUpdates(message, requestMessage, updates);
    return;
  }
  const transfers = [];
  for (const update of updates) {
    if (update instanceof Uint8Array) {
      transfers.push(update.buffer);
    }
  }
  self.postMessage(
    {
      ...message,
      updates,
      updateCount: updates.length,
    },
    transfers,
  );
}

/**
 * @param {RunnerOutboundMessage} message
 * @param {IntegratedServerWorkerMessage | null} requestMessage
 * @param {unknown[]} updates
 */
function postSharedUpdates(message, requestMessage, updates) {
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
  /** @type {Record<string, any>} */
  const response = {
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
  self.postMessage(response);
}

/**
 * @param {IntegratedServerWorkerMessage} message
 * @returns {Uint8Array}
 */
function commandFrame(message) {
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

/**
 * @param {IntegratedServerWorkerMessage | null} requestMessage
 * @param {number} packedBytes
 * @returns {{ control: Int32Array, controlBuffer: SharedArrayBuffer, updateBuffer: SharedArrayBuffer, pooledResponse: boolean, runnerSharedBufferId: number }}
 */
function sharedRunnerResponseTarget(requestMessage, packedBytes) {
  if (requestMessage?.transportKind === "shared-memory") {
    const control = sharedControlView(requestMessage.controlBuffer);
    const responseBuffer = sharedBuffer(requestMessage.responseBuffer, "responseBuffer");
    const pooledResponse = packedBytes <= responseBuffer.byteLength;
    return {
      control,
      // Validated as a SharedArrayBuffer by `sharedControlView` just above (it throws otherwise).
      controlBuffer: /** @type {SharedArrayBuffer} */ (requestMessage.controlBuffer),
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

/**
 * @param {number} packedBytes
 * @returns {RunnerSharedSlot}
 */
function acquireRunnerSharedResponseSlot(packedBytes) {
  const capacity = sharedCapacityFor(packedBytes);
  /** @type {RunnerSharedSlot | null} */
  let slot = null;
  for (let index = runnerSharedPool.length - 1; index >= 0; index -= 1) {
    if (runnerSharedPool[index].updateBuffer.byteLength >= packedBytes) {
      slot = runnerSharedPool.splice(index, 1)[0];
      break;
    }
  }
  slot ??= {
    id: nextRunnerSharedBufferId++,
    controlBuffer: new SharedArrayBuffer(RUNNER_SHARED_CONTROL_BYTES),
    updateBuffer: new SharedArrayBuffer(capacity),
  };
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

/** @param {IntegratedServerWorkerMessage} message */
function releaseRunnerSharedBuffer(message) {
  const bufferId = Number(message.runnerSharedBufferId) || 0;
  if (bufferId <= 0) return;
  const slot = runnerSharedInflight.get(bufferId);
  if (!slot) return;
  runnerSharedInflight.delete(bufferId);
  if (runnerSharedPool.length < MAX_RUNNER_SHARED_POOL_SLOTS) {
    runnerSharedPool.push(slot);
  }
}

/** @param {number} byteLength */
function sharedCapacityFor(byteLength) {
  let capacity = DEFAULT_RUNNER_SHARED_RESPONSE_BYTES;
  while (capacity < byteLength) {
    capacity *= 2;
  }
  return capacity;
}

/** @param {IntegratedServerWorkerMessage} message */
function markRunnerSharedFailure(message) {
  if (message?.transportKind !== "shared-memory") return;
  try {
    const control = sharedControlView(message.controlBuffer);
    Atomics.store(control, RUNNER_SHARED_STATUS_INDEX, RUNNER_SHARED_STATUS_FAILED);
    Atomics.notify(control, RUNNER_SHARED_STATUS_INDEX, 1);
  } catch {}
}

/**
 * @param {number | undefined} requestId
 * @param {string} reason
 */
function postFailure(requestId, reason) {
  self.postMessage({
    ok: false,
    kind: "error",
    requestId: Number(requestId) || 0,
    reason,
  });
}

/**
 * @param {string | undefined} bindgenJsUrl
 * @param {string | undefined} bindgenWasmUrl
 * @returns {Promise<WasmModule>}
 */
function loadWasmModule(bindgenJsUrl, bindgenWasmUrl) {
  wasmModulePromise ??= import(/** @type {string} */ (bindgenJsUrl)).then(/** @param {any} module */ async (module) => {
    await module.default(bindgenWasmUrl);
    return /** @type {WasmModule} */ (module);
  });
  return wasmModulePromise;
}

/**
 * The resolved wasm module, which owns the packed-frame codec (070 Stage 3). Update-posting
 * paths run only after {@link startServer} has set it, so a null here is a programming error.
 * @returns {WasmModule}
 */
function requireWasmModule() {
  if (!wasmModule) {
    throw new Error("integrated server worker wasm module is not loaded");
  }
  return wasmModule;
}

/** @param {any} diagnostics */
function hasPendingServerJobs(diagnostics) {
  if (!diagnostics) return false;
  return (
    Number(diagnostics.pendingJobs) > 0
    || Number(diagnostics.pendingPublications) > 0
    || Number(diagnostics.worldgenMailboxPendingJobs) > 0
    || Number(diagnostics.lightStatusMailboxPendingStatuses) > 0
  );
}

function waitForJobTurn() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

/** @param {unknown} buffer */
function sharedControlView(buffer) {
  const shared = sharedBuffer(buffer, "controlBuffer");
  return new Int32Array(shared);
}

/**
 * @param {unknown} buffer
 * @param {string} name
 * @returns {SharedArrayBuffer}
 */
function sharedBuffer(buffer, name) {
  if (typeof SharedArrayBuffer !== "function" || !(buffer instanceof SharedArrayBuffer)) {
    throw new Error(`shared runner ${name} was not a SharedArrayBuffer`);
  }
  return buffer;
}

function sharedTransportAvailable() {
  return (
    typeof SharedArrayBuffer === "function"
    && typeof Atomics === "object"
    && typeof Atomics.load === "function"
    && typeof Atomics.store === "function"
    && typeof Atomics.notify === "function"
  );
}

/** @param {number | string | undefined} seed */
function toBigIntSeed(seed) {
  const number = Number(seed);
  return BigInt(Number.isFinite(number) ? Math.trunc(number) : 0);
}

/** @param {unknown} error */
function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
