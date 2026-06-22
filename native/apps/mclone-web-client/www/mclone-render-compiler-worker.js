const RENDER_COMPILER_TRANSPORT_KIND = "shared-result-buffer";
const RENDER_COMPILER_MESSAGE_TRANSFER_KIND = "message-transfer";
const RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS = 4;
const RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX = 0;
const RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX = 1;
const RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX = 2;
const RENDER_COMPILER_SHARED_RESULT_COMPLETE = 2;
const RENDER_COMPILER_SHARED_RESULT_OVERFLOW = 3;
const RENDER_COMPILER_SHARED_RESULT_FAILED = 4;
const RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS = 4;
const RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX = 0;
const RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX = 1;
const RENDER_COMPILER_SHARED_INPUT_CAPACITY_INDEX = 2;
const RENDER_COMPILER_SHARED_INPUT_READY = 2;

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
    const snapshotInput = sharedInputBytes(message);
    const requestSnapshotInputByteLength = snapshotInput?.byteLength ?? 0;
    const sharedInputBufferCapacityBytes = isSharedArrayBuffer(message.sharedInputBuffer)
      ? message.sharedInputBuffer.byteLength
      : 0;
    const hasPersistentSnapshotCompiler =
      compilerSession !== null
      && snapshotInput !== null
      && targetSections.length > 0
      && typeof compilerSession.compileSnapshotSectionsForTargets === "function";
    const hasPersistentGeneratedCompiler =
      compilerSession !== null
      && targetSections.length > 0
      && typeof compilerSession.compileGeneratedChunkSectionsForTargets === "function";
    const hasPersistentFullCompiler =
      compilerSession !== null
      && typeof compilerSession.compileGeneratedChunkSections === "function";
    const hasGeneratedTargetedCompiler =
      targetSections.length > 0
      && (
        hasPersistentGeneratedCompiler
        || typeof module.mclone_web_compile_generated_chunk_sections_for_targets === "function"
      );
    const packed = hasPersistentSnapshotCompiler
      ? compilerSession.compileSnapshotSectionsForTargets(
          snapshotInput,
          targetSections,
        )
      : hasPersistentGeneratedCompiler
        ? compilerSession.compileGeneratedChunkSectionsForTargets(
            centerX,
            centerZ,
            radiusChunks,
            targetSections,
          )
        : hasPersistentFullCompiler
          ? compilerSession.compileGeneratedChunkSections(centerX, centerZ, radiusChunks)
          : hasGeneratedTargetedCompiler
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
    const response = writeSharedCompileResult(message, packed);
    const report = {
      ok: true,
      requestId: message.requestId,
      transportKind: response.transportKind,
      sharedMemorySupported: renderCompilerSharedMemorySupported(),
      workerWasmInitCount,
      workerCompileCount,
      workerAssetLoadCount,
      workerAssetPackInitByteLength,
      workerAssetPackFileCount,
      persistentAssetCatalog: compilerSession !== null,
      requestAssetPackByteLength,
      requestTargetSectionsByteLength,
      requestSnapshotInputByteLength,
      requestByteLength: requestAssetPackByteLength
        + requestTargetSectionsByteLength
        + requestSnapshotInputByteLength,
      transferredRequestByteLength: requestAssetPackByteLength,
      transferredResponseByteLength: response.transferredResponseByteLength,
      sharedInputBufferUsed: snapshotInput !== null,
      sharedInputByteLength: requestSnapshotInputByteLength,
      sharedInputBufferCapacityBytes,
      snapshotInputChunkCount: Number(message.snapshotInputChunkCount) || 0,
      snapshotInputCompileUsed: hasPersistentSnapshotCompiler,
      generatedViewFallbackUsed: !hasPersistentSnapshotCompiler,
      sharedResultBufferUsed: response.sharedResultBufferUsed,
      sharedResultByteLength: response.sharedResultByteLength,
      sharedResultBufferCapacityBytes: response.sharedResultBufferCapacityBytes,
      sharedResultOverflow: response.sharedResultOverflow,
      centerX,
      centerZ,
      radiusChunks,
      targetSectionCount: Math.floor(targetSections.length / 3),
      targetedCompileUsed: hasPersistentSnapshotCompiler || hasGeneratedTargetedCompiler,
      summary,
    };
    if (response.sharedResultBufferUsed) {
      report.sharedResultBuffer = response.sharedResultBuffer;
      self.postMessage(report);
    } else {
      report.packed = packed;
      self.postMessage(report, [packed.buffer]);
    }
  } catch (error) {
    markSharedCompileResultFailed(message);
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

function sharedInputBytes(message) {
  const buffer = message.sharedInputBuffer;
  if (!renderCompilerSharedMemorySupported() || !isSharedArrayBuffer(buffer)) {
    return null;
  }
  const control = sharedInputControl(message);
  const controlByteLength = control instanceof Int32Array
    ? Atomics.load(control, RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX)
    : 0;
  const controlStatus = control instanceof Int32Array
    ? Atomics.load(control, RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX)
    : RENDER_COMPILER_SHARED_INPUT_READY;
  const byteLength = Number(message.sharedInputByteLength) || controlByteLength || 0;
  if (
    controlStatus !== RENDER_COMPILER_SHARED_INPUT_READY
    || byteLength <= 0
    || byteLength > buffer.byteLength
  ) {
    return null;
  }
  return new Uint8Array(buffer, 0, byteLength);
}

function sharedInputControl(message) {
  const buffer = message.sharedInputControlBuffer;
  if (
    !renderCompilerSharedMemorySupported()
    || !isSharedArrayBuffer(buffer)
    || buffer.byteLength < RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS * Int32Array.BYTES_PER_ELEMENT
  ) {
    return null;
  }
  return new Int32Array(buffer);
}

function writeSharedCompileResult(message, packed) {
  const packedByteLength = byteLengthOf(packed);
  const requestedResponseBuffer = message.sharedResultResponseBuffer;
  if (!renderCompilerSharedMemorySupported() || !isSharedArrayBuffer(requestedResponseBuffer)) {
    return {
      transportKind: RENDER_COMPILER_MESSAGE_TRANSFER_KIND,
      transferredResponseByteLength: packedByteLength,
      sharedResultBufferUsed: false,
      sharedResultByteLength: 0,
      sharedResultBufferCapacityBytes: 0,
      sharedResultOverflow: false,
      sharedResultBuffer: null,
    };
  }

  let sharedResultBuffer = requestedResponseBuffer;
  let sharedResultOverflow = false;
  if (sharedResultBuffer.byteLength < packedByteLength) {
    sharedResultBuffer = new SharedArrayBuffer(packedByteLength);
    sharedResultOverflow = true;
  }
  new Uint8Array(sharedResultBuffer, 0, packedByteLength).set(packed);
  const control = sharedResultControl(message);
  if (control !== null) {
    Atomics.store(control, RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX, packedByteLength);
    Atomics.store(control, RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX, sharedResultBuffer.byteLength);
    Atomics.store(
      control,
      RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
      sharedResultOverflow
        ? RENDER_COMPILER_SHARED_RESULT_OVERFLOW
        : RENDER_COMPILER_SHARED_RESULT_COMPLETE,
    );
    Atomics.notify(control, RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX, 1);
  }
  return {
    transportKind: RENDER_COMPILER_TRANSPORT_KIND,
    transferredResponseByteLength: 0,
    sharedResultBufferUsed: true,
    sharedResultByteLength: packedByteLength,
    sharedResultBufferCapacityBytes: sharedResultBuffer.byteLength,
    sharedResultOverflow,
    sharedResultBuffer,
  };
}

function markSharedCompileResultFailed(message) {
  const control = sharedResultControl(message);
  if (control === null) {
    return;
  }
  Atomics.store(control, RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX, RENDER_COMPILER_SHARED_RESULT_FAILED);
  Atomics.notify(control, RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX, 1);
}

function sharedResultControl(message) {
  const buffer = message.sharedResultControlBuffer;
  if (
    !renderCompilerSharedMemorySupported()
    || !isSharedArrayBuffer(buffer)
    || buffer.byteLength < RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS * Int32Array.BYTES_PER_ELEMENT
  ) {
    return null;
  }
  return new Int32Array(buffer);
}

function byteLengthOf(value) {
  if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer || isSharedArrayBuffer(value)) {
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

function isSharedArrayBuffer(value) {
  return typeof SharedArrayBuffer === "function" && value instanceof SharedArrayBuffer;
}

function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
