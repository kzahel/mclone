let wasmModulePromise = null;

self.onmessage = async (event) => {
  const message = event.data ?? {};
  try {
    const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
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
  } catch (error) {
    self.postMessage({
      ok: false,
      kind: "error",
      requestId: Number(message.requestId) || 0,
      reason: stringifyError(error),
    });
  }
};

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

function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
