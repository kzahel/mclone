let wasmModulePromise = null;

self.onmessage = async (event) => {
  const message = event.data ?? {};
  if (message.kind !== "compile-render-sections") {
    self.postMessage({
      ok: false,
      reason: `unexpected render compiler message kind ${String(message.kind)}`,
    });
    return;
  }

  try {
    const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
    const packed = module.mclone_web_compile_generated_chunk_sections(
      message.assetPack,
      Number(message.centerX) || 0,
      Number(message.centerZ) || 0,
      Number(message.radiusChunks) || 0,
    );
    const summary = module.mclone_web_packed_compile_report_summary(packed);
    self.postMessage(
      {
        ok: true,
        centerX: Number(message.centerX) || 0,
        centerZ: Number(message.centerZ) || 0,
        radiusChunks: Number(message.radiusChunks) || 0,
        summary,
        packed,
      },
      [packed.buffer],
    );
  } catch (error) {
    self.postMessage({
      ok: false,
      reason: stringifyError(error),
    });
  }
};

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
