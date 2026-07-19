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
import type { WebServerJobActor } from "mclone-web-client-wasm";

// The wasm-bindgen module namespace (its generated `.d.ts`, emitted by `wasm-bindgen
// --typescript`). Loaded at runtime via a dynamic `import()` of a versioned URL, so this type
// only ever appears in type positions — the bare specifier is path-mapped in tsconfig.json and
// carries no runtime weight.
type WasmModule = typeof import("mclone-web-client-wasm");

// Domain-blind browser envelope for one isolated Rust server-job actor. Main Rust authors the
// opaque init and request frames; shared-memory jobs carry the SAB control/request/response
// buffers used to move those bytes between independent Wasm heaps.
interface ServerJobWorkerMessage {
  actorInitFrame?: Uint8Array;
  requestId?: number;
  bindgenJsUrl?: string;
  bindgenWasmUrl?: string;
  transportKind?: "shared-memory" | "message-transfer";
  frame?: Uint8Array;
  controlBuffer?: SharedArrayBuffer;
  requestBuffer?: SharedArrayBuffer;
  responseBuffer?: SharedArrayBuffer;
}

let wasmModulePromise: Promise<WasmModule> | null = null;
let serverJobActor: WebServerJobActor | null = null;
const workerSelf = self as unknown as DedicatedWorkerGlobalScope;

workerSelf.onmessage = async (event: MessageEvent) => {
  const message = (event.data ?? {}) as ServerJobWorkerMessage;
  try {
    const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
    if (message.transportKind === "shared-memory") {
      handleSharedMemoryJob(module, message);
    } else {
      handleTransferredJob(module, message);
    }
  } catch (error) {
    markSharedFailure(message);
    workerSelf.postMessage({
      ok: false,
      kind: "error",
      requestId: Number(message.requestId) || 0,
      reason: stringifyError(error),
    });
  }
};

function handleTransferredJob(module: WasmModule, message: ServerJobWorkerMessage): void {
  const frame = message.frame instanceof Uint8Array ? message.frame : new Uint8Array();
  const response = computeActorFrame(module, message, frame);
  workerSelf.postMessage(
    {
      ok: true,
      kind: "server-job-result",
      requestId: Number(message.requestId) || 0,
      frame: response,
    },
    [response.buffer as ArrayBuffer],
  );
}

function handleSharedMemoryJob(module: WasmModule, message: ServerJobWorkerMessage): void {
  const control = sharedControlView(message.controlBuffer);
  const requestBytes = Atomics.load(control, RUNNER_SHARED_REQUEST_BYTES_INDEX);
  const requestBuffer = sharedBuffer(message.requestBuffer, "requestBuffer");
  if (!Number.isInteger(requestBytes) || requestBytes < 0 || requestBytes > requestBuffer.byteLength) {
    throw new Error(
      `shared server job request byte count ${requestBytes} exceeds request buffer capacity ${requestBuffer.byteLength}`,
    );
  }
  const request = new Uint8Array(requestBuffer, 0, requestBytes);
  const response = computeActorFrame(module, message, request);
  const pooledResponseBuffer = sharedBuffer(message.responseBuffer, "responseBuffer");
  const pooledResponse = response.byteLength <= pooledResponseBuffer.byteLength;
  const responseBuffer = pooledResponse
    ? pooledResponseBuffer
    : new SharedArrayBuffer(response.byteLength);
  new Uint8Array(responseBuffer, 0, response.byteLength).set(response);
  Atomics.store(control, RUNNER_SHARED_RESPONSE_BYTES_INDEX, response.byteLength);
  Atomics.store(control, RUNNER_SHARED_STATUS_INDEX, RUNNER_SHARED_STATUS_COMPLETE);
  Atomics.notify(control, RUNNER_SHARED_STATUS_INDEX, 1);
  workerSelf.postMessage({
    ok: true,
    kind: "server-job-result",
    requestId: Number(message.requestId) || 0,
    transportKind: "shared-memory",
    controlBuffer: message.controlBuffer,
    frameBuffer: responseBuffer,
    frameBytes: response.byteLength,
    pooledResponse,
    responseCapacity: pooledResponseBuffer.byteLength,
  });
}

function computeActorFrame(
  module: WasmModule,
  message: ServerJobWorkerMessage,
  frame: Uint8Array,
): Uint8Array {
  if (serverJobActor === null) {
    if (!(message.actorInitFrame instanceof Uint8Array)) {
      throw new Error("server-job actor init frame was not a Uint8Array");
    }
    serverJobActor = new module.WebServerJobActor(message.actorInitFrame);
  }
  return serverJobActor.computeFrame(frame);
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

function markSharedFailure(message: ServerJobWorkerMessage): void {
  if (message?.transportKind !== "shared-memory") return;
  try {
    const control = sharedControlView(message.controlBuffer);
    Atomics.store(control, RUNNER_SHARED_STATUS_INDEX, RUNNER_SHARED_STATUS_FAILED);
    Atomics.notify(control, RUNNER_SHARED_STATUS_INDEX, 1);
  } catch {}
}

function sharedControlView(buffer: unknown): Int32Array {
  const shared = sharedBuffer(buffer, "controlBuffer");
  return new Int32Array(shared);
}

function sharedBuffer(buffer: unknown, name: string): SharedArrayBuffer {
  if (typeof SharedArrayBuffer !== "function" || !(buffer instanceof SharedArrayBuffer)) {
    throw new Error(`shared server job ${name} was not a SharedArrayBuffer`);
  }
  return buffer;
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
