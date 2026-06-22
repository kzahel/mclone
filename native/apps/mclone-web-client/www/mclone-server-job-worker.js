let wasmModulePromise = null;

const SHARED_STATUS_INDEX = 0;
const SHARED_REQUEST_BYTES_INDEX = 1;
const SHARED_RESPONSE_BYTES_INDEX = 2;
const SHARED_STATUS_COMPLETE = 2;
const SHARED_STATUS_FAILED = -1;

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
  const requestBytes = Atomics.load(control, SHARED_REQUEST_BYTES_INDEX);
  const requestBuffer = sharedBuffer(message.requestBuffer, "requestBuffer");
  const request = new Uint8Array(requestBuffer, 0, requestBytes);
  const response = computeJobFrame(module, message.kind, request);
  const responseBuffer = new SharedArrayBuffer(response.byteLength);
  new Uint8Array(responseBuffer).set(response);
  Atomics.store(control, SHARED_RESPONSE_BYTES_INDEX, response.byteLength);
  Atomics.store(control, SHARED_STATUS_INDEX, SHARED_STATUS_COMPLETE);
  Atomics.notify(control, SHARED_STATUS_INDEX, 1);
  self.postMessage({
    ok: true,
    kind: `${String(message.kind)}-result`,
    requestId: Number(message.requestId) || 0,
    transportKind: "shared-memory",
    controlBuffer: message.controlBuffer,
    frameBuffer: responseBuffer,
    frameBytes: response.byteLength,
  });
}

function computeJobFrame(module, kind, frame) {
  switch (kind) {
    case "worldgen":
      return module.mclone_web_compute_worldgen_job_frame(frame);
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
    Atomics.store(control, SHARED_STATUS_INDEX, SHARED_STATUS_FAILED);
    Atomics.notify(control, SHARED_STATUS_INDEX, 1);
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
