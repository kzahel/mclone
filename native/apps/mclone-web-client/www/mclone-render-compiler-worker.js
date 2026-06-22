const RENDER_COMPILER_TRANSPORT_KIND = "message-transfer";

let wasmModulePromise = null;
let compilerSession = null;
let workerWasmInitCount = 0;
let workerCompileCount = 0;
let workerAssetLoadCount = 0;
let workerAssetPackInitByteLength = 0;
let workerAssetPackFileCount = 0;

self.onmessage = async (event) => {
  const message = event.data ?? {};
  if (message.kind === "init-render-compiler") {
    await handleInit(message);
    return;
  }
  if (message.kind === "compile-render-sections") {
    await handleCompile(message);
    return;
  }

  self.postMessage({
    ok: false,
    requestId: message.requestId,
    kind: "render-compiler-error",
    reason: `unexpected render compiler message kind ${String(message.kind)}`,
  });
};

async function handleInit(message) {
  try {
    const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
    const assetPackByteLength = byteLengthOf(message.assetPack);
    compilerSession = new module.WebRenderCompilerSession(message.assetPack);
    workerAssetLoadCount = Number(compilerSession.assetLoadCount?.()) || 1;
    workerAssetPackInitByteLength = Number(compilerSession.assetPackByteLength?.())
      || assetPackByteLength;
    workerAssetPackFileCount = Number(compilerSession.assetPackFileCount?.()) || 0;
    self.postMessage({
      ok: true,
      kind: "render-compiler-ready",
      requestId: message.requestId,
      transportKind: RENDER_COMPILER_TRANSPORT_KIND,
      sharedMemorySupported: renderCompilerSharedMemorySupported(),
      workerWasmInitCount,
      workerCompileCount,
      workerAssetLoadCount,
      workerAssetPackInitByteLength,
      workerAssetPackFileCount,
      persistentAssetCatalog: true,
    });
  } catch (error) {
    self.postMessage({
      ok: false,
      kind: "render-compiler-ready",
      requestId: message.requestId,
      transportKind: RENDER_COMPILER_TRANSPORT_KIND,
      sharedMemorySupported: renderCompilerSharedMemorySupported(),
      workerWasmInitCount,
      workerCompileCount,
      workerAssetLoadCount,
      workerAssetPackInitByteLength,
      workerAssetPackFileCount,
      persistentAssetCatalog: false,
      reason: stringifyError(error),
    });
  }
}

async function handleCompile(message) {
  try {
    workerCompileCount += 1;
    const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
    const targetSections = normalizeTargetSections(message.targetSections);
    const centerX = Number(message.centerX) || 0;
    const centerZ = Number(message.centerZ) || 0;
    const radiusChunks = Number(message.radiusChunks) || 0;
    const requestAssetPackByteLength = byteLengthOf(message.assetPack);
    const requestTargetSectionsByteLength = targetSections.byteLength;
    const hasPersistentCompiler =
      compilerSession !== null
      && targetSections.length > 0
      && typeof compilerSession.compileGeneratedChunkSectionsForTargets === "function";
    const hasPersistentFullCompiler =
      compilerSession !== null
      && typeof compilerSession.compileGeneratedChunkSections === "function";
    const hasTargetedCompiler =
      targetSections.length > 0
      && (
        hasPersistentCompiler
        || typeof module.mclone_web_compile_generated_chunk_sections_for_targets === "function"
      );
    const packed = hasPersistentCompiler
      ? compilerSession.compileGeneratedChunkSectionsForTargets(
          centerX,
          centerZ,
          radiusChunks,
          targetSections,
        )
      : hasPersistentFullCompiler
        ? compilerSession.compileGeneratedChunkSections(centerX, centerZ, radiusChunks)
        : hasTargetedCompiler
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
        transportKind: RENDER_COMPILER_TRANSPORT_KIND,
        sharedMemorySupported: renderCompilerSharedMemorySupported(),
        workerWasmInitCount,
        workerCompileCount,
        workerAssetLoadCount,
        workerAssetPackInitByteLength,
        workerAssetPackFileCount,
        persistentAssetCatalog: compilerSession !== null,
        requestAssetPackByteLength,
        requestTargetSectionsByteLength,
        requestByteLength: requestAssetPackByteLength + requestTargetSectionsByteLength,
        transferredRequestByteLength: requestAssetPackByteLength,
        transferredResponseByteLength: packed.byteLength,
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
      transportKind: RENDER_COMPILER_TRANSPORT_KIND,
      sharedMemorySupported: renderCompilerSharedMemorySupported(),
      workerWasmInitCount,
      workerCompileCount,
      workerAssetLoadCount,
      workerAssetPackInitByteLength,
      workerAssetPackFileCount,
      persistentAssetCatalog: compilerSession !== null,
      reason: stringifyError(error),
    });
  }
}

function loadWasmModule(bindgenJsUrl, bindgenWasmUrl) {
  wasmModulePromise ??= import(bindgenJsUrl).then(async (module) => {
    await module.default(bindgenWasmUrl);
    workerWasmInitCount += 1;
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

function byteLengthOf(value) {
  if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer) {
    return value.byteLength;
  }
  return 0;
}

function renderCompilerSharedMemorySupported() {
  return typeof SharedArrayBuffer === "function"
    && typeof Atomics === "object"
    && typeof Atomics.load === "function"
    && typeof Atomics.store === "function"
    && typeof Atomics.notify === "function";
}

function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
