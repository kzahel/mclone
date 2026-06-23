// 070 Stage 1: the SAB ring control-word ABI is single-sourced in
// ./mclone-runner-shared-abi.js (also imported by the integrated-server worker), locked to the
// Rust copy in src/web_server_worker.rs by tests/runner_shared_abi_lock.rs. These constants were
// formerly hand-declared here under a divergent `SHARED_*` prefix.
import {
  RUNNER_SHARED_STATUS_INDEX,
  RUNNER_SHARED_REQUEST_BYTES_INDEX,
  RUNNER_SHARED_RESPONSE_BYTES_INDEX,
  RUNNER_SHARED_STATUS_COMPLETE,
  RUNNER_SHARED_STATUS_FAILED,
} from "./mclone-runner-shared-abi.js";

let wasmModulePromise = null;
// 069 Stage 1: the worldgen worker holds a resident session across jobs (exactly
// as the render-compiler worker holds `compilerSession`), so its
// OverworldFeatureDependencyCache persists and each job applies only the request
// delta to it. This worker instance only ever receives "worldgen" jobs (the
// light-status worker is a separate instance), so the session is never created in
// the light worker. Light-status stays stateless (its free function).
let worldgenSession = null;

self.onmessage = async (event) => {
  const message = event.data ?? {};
  try {
    const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
    if (message.transportKind === "shared-memory") {
      handleSharedMemoryJob(module, message);
    } else {
      handleTransferredJob(module, message);
    }
  } catch (error) {
    markSharedFailure(message);
    self.postMessage({
      ok: false,
      kind: "error",
      requestId: Number(message.requestId) || 0,
      reason: stringifyError(error),
    });
  }
};

function handleTransferredJob(module, message) {
  const frame = message.frame instanceof Uint8Array ? message.frame : new Uint8Array();
  const response = computeJobFrame(module, message.kind, frame);
  self.postMessage(
    {
      ok: true,
      kind: `${String(message.kind)}-result`,
      requestId: Number(message.requestId) || 0,
      frame: response,
    },
    [response.buffer],
  );
}

function handleSharedMemoryJob(module, message) {
  const control = sharedControlView(message.controlBuffer);
  const requestBytes = Atomics.load(control, RUNNER_SHARED_REQUEST_BYTES_INDEX);
  const requestBuffer = sharedBuffer(message.requestBuffer, "requestBuffer");
  if (!Number.isInteger(requestBytes) || requestBytes < 0 || requestBytes > requestBuffer.byteLength) {
    throw new Error(
      `shared server job request byte count ${requestBytes} exceeds request buffer capacity ${requestBuffer.byteLength}`,
    );
  }
  const request = new Uint8Array(requestBuffer, 0, requestBytes);
  const response = computeJobFrame(module, message.kind, request);
  const pooledResponseBuffer = sharedBuffer(message.responseBuffer, "responseBuffer");
  const pooledResponse = response.byteLength <= pooledResponseBuffer.byteLength;
  const responseBuffer = pooledResponse
    ? pooledResponseBuffer
    : new SharedArrayBuffer(response.byteLength);
  new Uint8Array(responseBuffer, 0, response.byteLength).set(response);
  Atomics.store(control, RUNNER_SHARED_RESPONSE_BYTES_INDEX, response.byteLength);
  Atomics.store(control, RUNNER_SHARED_STATUS_INDEX, RUNNER_SHARED_STATUS_COMPLETE);
  Atomics.notify(control, RUNNER_SHARED_STATUS_INDEX, 1);
  self.postMessage({
    ok: true,
    kind: `${String(message.kind)}-result`,
    requestId: Number(message.requestId) || 0,
    transportKind: "shared-memory",
    controlBuffer: message.controlBuffer,
    frameBuffer: responseBuffer,
    frameBytes: response.byteLength,
    pooledResponse,
    responseCapacity: pooledResponseBuffer.byteLength,
  });
}

function computeJobFrame(module, kind, frame) {
  switch (kind) {
    case "worldgen":
      worldgenSession ??= new module.WebWorldgenJobSession();
      return worldgenSession.computeWorldgenJobFrame(frame);
    case "light-status":
      return module.mclone_web_compute_light_status_job_frame(frame);
    default:
      throw new Error(`unexpected server job worker message kind ${String(kind)}`);
  }
}

function loadWasmModule(bindgenJsUrl, bindgenWasmUrl) {
  wasmModulePromise ??= import(bindgenJsUrl).then(async (module) => {
    await module.default(bindgenWasmUrl);
    return module;
  });
  return wasmModulePromise;
}

function markSharedFailure(message) {
  if (message?.transportKind !== "shared-memory") return;
  try {
    const control = sharedControlView(message.controlBuffer);
    Atomics.store(control, RUNNER_SHARED_STATUS_INDEX, RUNNER_SHARED_STATUS_FAILED);
    Atomics.notify(control, RUNNER_SHARED_STATUS_INDEX, 1);
  } catch {}
}

function sharedControlView(buffer) {
  const shared = sharedBuffer(buffer, "controlBuffer");
  return new Int32Array(shared);
}

function sharedBuffer(buffer, name) {
  if (typeof SharedArrayBuffer !== "function" || !(buffer instanceof SharedArrayBuffer)) {
    throw new Error(`shared server job ${name} was not a SharedArrayBuffer`);
  }
  return buffer;
}

function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
