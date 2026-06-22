let wasmModulePromise = null;

self.onmessage = async (event) => {
  const message = event.data ?? {};
  if (message.kind !== "compile-render-sections") {
    self.postMessage({
      ok: false,
      requestId: message.requestId,
      reason: `unexpected render compiler message kind ${String(message.kind)}`,
    });
    return;
  }

  try {
    const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
    const targetSections = normalizeTargetSections(message.targetSections);
    const centerX = Number(message.centerX) || 0;
    const centerZ = Number(message.centerZ) || 0;
    const radiusChunks = Number(message.radiusChunks) || 0;
    const hasTargetedCompiler =
      targetSections.length > 0
      && typeof module.mclone_web_compile_generated_chunk_sections_for_targets === "function";
    const packed = hasTargetedCompiler
      ? module.mclone_web_compile_generated_chunk_sections_for_targets(
          message.assetPack,
          centerX,
          centerZ,
          radiusChunks,
          targetSections,
        )
      : module.mclone_web_compile_generated_chunk_sections(
          message.assetPack,
          centerX,
          centerZ,
          radiusChunks,
        );
    const summary = module.mclone_web_packed_compile_report_summary(packed);
    self.postMessage(
      {
        ok: true,
        requestId: message.requestId,
        centerX,
        centerZ,
        radiusChunks,
        targetSectionCount: Math.floor(targetSections.length / 3),
        targetedCompileUsed: hasTargetedCompiler,
        summary,
        packed,
      },
      [packed.buffer],
    );
  } catch (error) {
    self.postMessage({
      ok: false,
      requestId: message.requestId,
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

function normalizeTargetSections(value) {
  if (value instanceof Int32Array) {
    return value;
  }
  if (Array.isArray(value)) {
    return new Int32Array(value.map((entry) => Number(entry) || 0));
  }
  if (ArrayBuffer.isView(value)) {
    return new Int32Array(value.buffer, value.byteOffset, Math.floor(value.byteLength / 4));
  }
  if (value instanceof ArrayBuffer) {
    return new Int32Array(value);
  }
  return new Int32Array();
}

function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
