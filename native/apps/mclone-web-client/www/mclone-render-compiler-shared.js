// 067 Stage 5: the residual render-compile JS glue shared by mclone-web-app.js (the live
// streaming app) and mclone-web-smoke.js (the deterministic overview pump). Both used to
// hand-duplicate the RenderSectionWorkerCompiler doorbell relay, the SAB arena wrappers, and
// the metrics helpers; they now import them from here. URL/cache-bust policy stays per-caller
// (app cache-busts via `?v=`, smoke uses bare `new URL(...)`), so the worker/bindgen URLs and
// the worker name are passed in as constructor args rather than baked into this module.
//
// The render-compile worker also imports the three predicate/measurement helpers below; this
// module is therefore worker-safe (no window/document references). The ABI constants live in
// ./mclone-render-compiler-abi.js and are re-exported here for one-stop consumer imports.
export * from "./mclone-render-compiler-abi.js";
import {
  RENDER_COMPILER_TRANSPORT_KIND,
  RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX,
  RENDER_COMPILER_DEFAULT_SHARED_RESULT_CAPACITY,
} from "./mclone-render-compiler-abi.js";

export async function fetchAssetPack(assetPackUrl) {
  const response = await fetch(assetPackUrl);
  if (!response.ok) {
    throw new Error(`failed to fetch ${assetPackUrl.pathname}: ${response.status} ${response.statusText}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

export class RenderSectionWorkerCompiler {
  constructor(assetPack, options = {}) {
    const {
      workerUrl,
      bindgenJsUrl,
      bindgenWasmUrl,
      workerName = "mclone-render-compiler",
    } = options;
    this.workerUrl = workerUrl;
    this.bindgenJsUrl = bindgenJsUrl;
    this.bindgenWasmUrl = bindgenWasmUrl;
    this.pending = new Map();
    this.metrics = {
      transportKind: RENDER_COMPILER_TRANSPORT_KIND,
      sharedMemorySupported: renderCompilerSharedMemorySupported(),
      workerInitCount: 1,
      workerWasmInitCount: 0,
      workerAssetLoadCount: 0,
      workerAssetPackInitByteLength: 0,
      workerAssetPackFileCount: 0,
      persistentAssetCatalog: false,
      compileCount: 0,
      assetPackSendCount: 0,
      transferredRequestByteCount: 0,
      transferredResponseByteCount: 0,
      sharedResultResponseCount: 0,
      sharedResultByteCount: 0,
      sharedResultOverflowCount: 0,
      sharedResultBufferCapacityBytes: renderCompilerSharedMemorySupported()
        ? RENDER_COMPILER_DEFAULT_SHARED_RESULT_CAPACITY
        : 0,
      sharedInputGrowCount: 0,
    };
    // 067 Stage 3/5: the app's streaming loop and the smoke's overview pump both drive
    // compiles through the Rust-owned resident SAB ring handed over on each doorbell, so
    // this worker shim no longer owns its own SAB arenas — it is a doorbell + lifecycle
    // relay only.
    this.ready = new Promise((resolve, reject) => {
      this.resolveReady = resolve;
      this.rejectReady = reject;
    });
    this.initTimeout = setTimeout(() => {
      this.rejectReady(new Error("timed out initializing render compiler worker"));
    }, 20_000);
    this.worker = new Worker(this.workerUrl.href, {
      type: "module",
      name: workerName,
    });
    this.worker.onmessage = (event) => {
      const data = event.data ?? {};
      if (data.kind === "render-compiler-ready") {
        this.handleReady(data);
        return;
      }
      const requestId = Number(data.requestId) || 0;
      const pending = this.pending.get(requestId);
      if (!pending) return;
      this.pending.delete(requestId);
      clearTimeout(pending.timeout);
      const response = renderCompilerPackedResponse(data, pending);
      const packed = response.packed;
      const packedByteLength = response.packedByteLength;
      if (response.sharedResultBufferUsed) {
        this.metrics.sharedResultResponseCount += 1;
        this.metrics.sharedResultByteCount += response.sharedResultByteLength;
        this.metrics.sharedResultBufferCapacityBytes = response.sharedResultBufferCapacityBytes;
        if (response.sharedResultOverflow) {
          this.metrics.sharedResultOverflowCount += 1;
        }
      } else {
        this.metrics.transferredResponseByteCount += response.transferredResponseByteLength;
      }
      this.metrics.workerWasmInitCount = Number(data.workerWasmInitCount)
        || this.metrics.workerWasmInitCount;
      this.metrics.workerAssetLoadCount = Number(data.workerAssetLoadCount)
        || this.metrics.workerAssetLoadCount;
      this.metrics.workerAssetPackInitByteLength = Number(data.workerAssetPackInitByteLength)
        || this.metrics.workerAssetPackInitByteLength;
      this.metrics.workerAssetPackFileCount = Number(data.workerAssetPackFileCount)
        || this.metrics.workerAssetPackFileCount;
      this.metrics.persistentAssetCatalog = Boolean(data.persistentAssetCatalog);
      data.sharedResultBufferUsed = response.sharedResultBufferUsed;
      data.sharedResultByteLength = response.sharedResultByteLength;
      data.sharedResultBufferCapacityBytes = response.sharedResultBufferCapacityBytes;
      data.sharedResultOverflow = response.sharedResultOverflow;
      data.transferredResponseByteLength = response.transferredResponseByteLength;
      const renderCompilerMetrics = renderCompilerMetricsForResponse(
        this.metrics,
        pending.requestMetrics,
        data,
        packedByteLength,
      );
      delete data.packed;
      delete data.sharedResultBuffer;
      pending.resolve({
        report: {
          ...data,
          packedByteLength,
          renderCompilerMetrics,
          transportKind: renderCompilerMetrics.transportKind,
          sharedMemorySupported: renderCompilerMetrics.sharedMemorySupported,
          workerInitCount: renderCompilerMetrics.workerInitCount,
          workerWasmInitCount: renderCompilerMetrics.workerWasmInitCount,
          workerAssetLoadCount: renderCompilerMetrics.workerAssetLoadCount,
          workerAssetPackInitByteLength: renderCompilerMetrics.workerAssetPackInitByteLength,
          workerAssetPackFileCount: renderCompilerMetrics.workerAssetPackFileCount,
          persistentAssetCatalog: renderCompilerMetrics.persistentAssetCatalog,
          compileCount: renderCompilerMetrics.compileCount,
          workerCompileCount: renderCompilerMetrics.workerCompileCount,
          assetPackSendCount: renderCompilerMetrics.assetPackSendCount,
          requestAssetPackByteLength: renderCompilerMetrics.requestAssetPackByteLength,
          requestTargetSectionsByteLength: renderCompilerMetrics.requestTargetSectionsByteLength,
          requestSnapshotInputByteLength: renderCompilerMetrics.requestSnapshotInputByteLength,
          requestByteLength: renderCompilerMetrics.requestByteLength,
          transferredRequestByteLength: renderCompilerMetrics.transferredRequestByteLength,
          transferredResponseByteLength: renderCompilerMetrics.transferredResponseByteLength,
          sharedInputBufferUsed: renderCompilerMetrics.sharedInputBufferUsed,
          sharedInputByteLength: renderCompilerMetrics.sharedInputByteLength,
          sharedInputBufferCapacityBytes: renderCompilerMetrics.sharedInputBufferCapacityBytes,
          snapshotInputChunkCount: renderCompilerMetrics.snapshotInputChunkCount,
          snapshotInputCompileUsed: renderCompilerMetrics.snapshotInputCompileUsed,
          generatedViewFallbackUsed: renderCompilerMetrics.generatedViewFallbackUsed,
          sharedResultBufferUsed: renderCompilerMetrics.sharedResultBufferUsed,
          sharedResultByteLength: renderCompilerMetrics.sharedResultByteLength,
          sharedResultBufferCapacityBytes: renderCompilerMetrics.sharedResultBufferCapacityBytes,
          sharedResultOverflow: renderCompilerMetrics.sharedResultOverflow,
          sharedResultResponseCount: renderCompilerMetrics.sharedResultResponseCount,
          sharedResultByteCount: renderCompilerMetrics.sharedResultByteCount,
          sharedResultOverflowCount: renderCompilerMetrics.sharedResultOverflowCount,
        },
        packed,
      });
    };
    this.worker.onerror = (event) => {
      clearTimeout(this.initTimeout);
      this.rejectReady(new Error(event.message || "render compiler worker failed"));
      this.rejectAll(new Error(event.message || "render compiler worker failed"));
    };
    this.initialize(assetPack);
  }

  initialize(assetPack) {
    const initAssetPack = assetPack.slice();
    this.metrics.assetPackSendCount += 1;
    this.metrics.workerAssetPackInitByteLength = initAssetPack.byteLength;
    this.metrics.transferredRequestByteCount += initAssetPack.byteLength;
    this.worker.postMessage(
      {
        kind: "init-render-compiler",
        requestId: 0,
        bindgenJsUrl: this.bindgenJsUrl.href,
        bindgenWasmUrl: this.bindgenWasmUrl.href,
        assetPack: initAssetPack,
      },
      [initAssetPack.buffer],
    );
  }

  handleReady(data) {
    clearTimeout(this.initTimeout);
    this.metrics.workerWasmInitCount = Number(data.workerWasmInitCount)
      || this.metrics.workerWasmInitCount;
    this.metrics.workerAssetLoadCount = Number(data.workerAssetLoadCount)
      || this.metrics.workerAssetLoadCount;
    this.metrics.workerAssetPackInitByteLength = Number(data.workerAssetPackInitByteLength)
      || this.metrics.workerAssetPackInitByteLength;
    this.metrics.workerAssetPackFileCount = Number(data.workerAssetPackFileCount)
      || this.metrics.workerAssetPackFileCount;
    this.metrics.persistentAssetCatalog = Boolean(data.persistentAssetCatalog);
    if (data.ok) {
      this.resolveReady(data);
    } else {
      this.rejectReady(new Error(data.reason || "render compiler worker initialization failed"));
    }
  }

  // 067 Stage 2/3: post a compile against the Rust-owned resident shared ring. Main
  // wasm has already filled the input arena and armed both control words; JS only
  // relays the doorbell (the worker writes the result into the same buffers main wasm
  // polls). Resolves with the worker metrics report; the section data path stays in
  // Rust via the next frame's poll, so JS never decodes the packed bytes.
  async compileWithDoorbell(doorbell) {
    await this.ready;
    const requestId = Number(doorbell.requestId) || 0;
    if (requestId <= 0) {
      throw new Error(`invalid render compile doorbell id ${String(doorbell.requestId)}`);
    }
    const sharedResult = sharedResultArenaFromDoorbell(doorbell);
    const sharedInput = sharedInputArenaFromDoorbell(doorbell);
    return this.dispatchCompile({
      requestId,
      centerX: doorbell.centerX,
      centerZ: doorbell.centerZ,
      radiusChunks: doorbell.radiusChunks,
      targetSections: doorbell.targetSections,
      snapshotInputChunkCount: Number(doorbell.snapshotInputChunkCount) || 0,
      sharedResult,
      sharedInput,
      requestSource: {
        snapshotInputByteLength: Number(doorbell.sharedInputByteLength) || 0,
        snapshotInputChunkCount: Number(doorbell.snapshotInputChunkCount) || 0,
      },
    });
  }

  dispatchCompile({
    requestId,
    centerX,
    centerZ,
    radiusChunks,
    targetSections,
    snapshotInputChunkCount,
    sharedResult,
    sharedInput,
    requestSource,
  }) {
    return new Promise((resolve, reject) => {
      const requestMetrics = renderCompilerRequestMetrics(
        targetSections,
        sharedResult,
        sharedInput,
        requestSource,
      );
      this.metrics.compileCount += 1;
      this.metrics.transferredRequestByteCount += requestMetrics.transferredRequestByteLength;
      const timeout = setTimeout(() => {
        if (!this.pending.has(requestId)) return;
        this.pending.delete(requestId);
        reject(new Error(`timed out waiting for render compiler worker request ${requestId}`));
      }, 20_000);
      this.pending.set(requestId, {
        resolve,
        reject,
        timeout,
        requestMetrics,
        sharedResult,
        sharedInput,
      });
      const message = {
        kind: "compile-render-sections",
        requestId,
        bindgenJsUrl: this.bindgenJsUrl.href,
        bindgenWasmUrl: this.bindgenWasmUrl.href,
        centerX,
        centerZ,
        radiusChunks,
        targetSections,
        snapshotInputChunkCount,
      };
      if (sharedInput !== null) {
        message.sharedInputControlBuffer = sharedInput.controlBuffer;
        message.sharedInputBuffer = sharedInput.inputBuffer;
        message.sharedInputByteLength = sharedInput.inputByteLength;
        message.sharedInputBufferCapacityBytes = sharedInput.inputCapacity;
      }
      if (sharedResult !== null) {
        message.sharedResultControlBuffer = sharedResult.controlBuffer;
        message.sharedResultResponseBuffer = sharedResult.responseBuffer;
        message.sharedResultBufferCapacityBytes = sharedResult.responseCapacity;
      }
      this.worker.postMessage(message);
    });
  }

  pendingJobCount() {
    return this.pending.size;
  }

  terminate() {
    this.worker.terminate();
    this.rejectAll(new Error("render compiler worker terminated"));
  }

  rejectAll(error) {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pending.clear();
  }
}

// 067 Stage 2: wrap the Rust-owned resident result buffers (handed over on the
// submit doorbell) in the same arena shape the worker protocol + metrics expect.
// JS does not arm these — main wasm already filled/armed them — it only reads byte
// counts back for diagnostics.
export function sharedResultArenaFromDoorbell(doorbell) {
  const controlBuffer = doorbell.sharedResultControlBuffer;
  const responseBuffer = doorbell.sharedResultResponseBuffer;
  if (!isSharedArrayBuffer(controlBuffer) || !isSharedArrayBuffer(responseBuffer)) {
    return null;
  }
  return {
    controlBuffer,
    control: new Int32Array(controlBuffer),
    responseBuffer,
    responseCapacity: Number(doorbell.sharedResultBufferCapacityBytes) || responseBuffer.byteLength,
  };
}

export function sharedInputArenaFromDoorbell(doorbell) {
  const controlBuffer = doorbell.sharedInputControlBuffer;
  const inputBuffer = doorbell.sharedInputBuffer;
  if (!isSharedArrayBuffer(controlBuffer) || !isSharedArrayBuffer(inputBuffer)) {
    return null;
  }
  return {
    controlBuffer,
    control: new Int32Array(controlBuffer),
    inputBuffer,
    inputByteLength: Number(doorbell.sharedInputByteLength) || 0,
    inputCapacity: Number(doorbell.sharedInputBufferCapacityBytes) || inputBuffer.byteLength,
  };
}

export function renderCompilerPackedResponse(workerReport, pending) {
  const sharedResult = pending?.sharedResult ?? null;
  const sharedResultBuffer = isSharedArrayBuffer(workerReport.sharedResultBuffer)
    ? workerReport.sharedResultBuffer
    : sharedResult?.responseBuffer;
  const control = sharedResult?.control;
  const controlByteLength = control instanceof Int32Array
    ? Atomics.load(control, RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX)
    : 0;
  const sharedResultByteLength = Number(workerReport.sharedResultByteLength)
    || controlByteLength
    || 0;
  const sharedResultBufferUsed = Boolean(workerReport.sharedResultBufferUsed)
    && isSharedArrayBuffer(sharedResultBuffer)
    && sharedResultByteLength >= 0
    && sharedResultByteLength <= sharedResultBuffer.byteLength;
  if (sharedResultBufferUsed) {
    return {
      packed: new Uint8Array(sharedResultBuffer, 0, sharedResultByteLength),
      packedByteLength: sharedResultByteLength,
      sharedResultBufferUsed: true,
      sharedResultByteLength,
      sharedResultBufferCapacityBytes: Number(workerReport.sharedResultBufferCapacityBytes)
        || sharedResultBuffer.byteLength,
      sharedResultOverflow: Boolean(workerReport.sharedResultOverflow),
      transferredResponseByteLength: Number(workerReport.transferredResponseByteLength) || 0,
    };
  }

  const packed = workerReport.packed instanceof Uint8Array ? workerReport.packed : new Uint8Array();
  return {
    packed,
    packedByteLength: packed.byteLength,
    sharedResultBufferUsed: false,
    sharedResultByteLength: 0,
    sharedResultBufferCapacityBytes: sharedResult?.responseCapacity ?? 0,
    sharedResultOverflow: false,
    transferredResponseByteLength: Number(workerReport.transferredResponseByteLength)
      || packed.byteLength,
  };
}

export function renderCompilerRequestMetrics(targetSections, sharedResult, sharedInput, request) {
  const requestAssetPackByteLength = 0;
  const requestTargetSectionsByteLength = byteLengthOfTargetSections(targetSections);
  const requestSnapshotInputByteLength = Number(request?.snapshotInputByteLength)
    || byteLengthOf(request?.snapshotInputBytes)
    || sharedInput?.inputByteLength
    || 0;
  return {
    requestAssetPackByteLength,
    requestTargetSectionsByteLength,
    requestSnapshotInputByteLength,
    requestByteLength: requestAssetPackByteLength
      + requestTargetSectionsByteLength
      + requestSnapshotInputByteLength,
    transferredRequestByteLength: 0,
    sharedInputBufferUsed: sharedInput !== null,
    sharedInputByteLength: sharedInput?.inputByteLength ?? 0,
    sharedInputBufferCapacityBytes: sharedInput?.inputCapacity ?? 0,
    snapshotInputChunkCount: Number(request?.snapshotInputChunkCount) || 0,
    sharedResultBufferUsed: sharedResult !== null,
    sharedResultBufferCapacityBytes: sharedResult?.responseCapacity ?? 0,
  };
}

export function renderCompilerMetricsForResponse(cumulativeMetrics, requestMetrics, workerReport, packedByteLength) {
  const sharedResultBufferUsed = Boolean(workerReport.sharedResultBufferUsed)
    || Boolean(requestMetrics.sharedResultBufferUsed && Number(workerReport.sharedResultByteLength) > 0);
  const requestSnapshotInputByteLength = Number(requestMetrics.requestSnapshotInputByteLength)
    || Number(workerReport.requestSnapshotInputByteLength)
    || 0;
  const sharedInputBufferUsed = Boolean(workerReport.sharedInputBufferUsed)
    || Boolean(requestMetrics.sharedInputBufferUsed && requestSnapshotInputByteLength > 0);
  const sharedInputByteLength = Number(workerReport.sharedInputByteLength)
    || Number(requestMetrics.sharedInputByteLength)
    || (sharedInputBufferUsed ? requestSnapshotInputByteLength : 0);
  const sharedResultByteLength = sharedResultBufferUsed
    ? Number(workerReport.sharedResultByteLength) || Number(packedByteLength) || 0
    : 0;
  const transferredResponseByteLength = Number(workerReport.transferredResponseByteLength)
    || (sharedResultBufferUsed ? 0 : Number(packedByteLength))
    || 0;
  return {
    transportKind: String(workerReport.transportKind || cumulativeMetrics.transportKind || RENDER_COMPILER_TRANSPORT_KIND),
    sharedMemorySupported: Boolean(workerReport.sharedMemorySupported ?? cumulativeMetrics.sharedMemorySupported),
    workerInitCount: Number(cumulativeMetrics.workerInitCount) || 0,
    workerWasmInitCount: Number(workerReport.workerWasmInitCount)
      || Number(cumulativeMetrics.workerWasmInitCount)
      || 0,
    workerAssetLoadCount: Number(workerReport.workerAssetLoadCount)
      || Number(cumulativeMetrics.workerAssetLoadCount)
      || 0,
    workerAssetPackInitByteLength: Number(workerReport.workerAssetPackInitByteLength)
      || Number(cumulativeMetrics.workerAssetPackInitByteLength)
      || 0,
    workerAssetPackFileCount: Number(workerReport.workerAssetPackFileCount)
      || Number(cumulativeMetrics.workerAssetPackFileCount)
      || 0,
    persistentAssetCatalog: Boolean(workerReport.persistentAssetCatalog ?? cumulativeMetrics.persistentAssetCatalog),
    compileCount: Number(cumulativeMetrics.compileCount) || 0,
    workerCompileCount: Number(workerReport.workerCompileCount) || 0,
    assetPackSendCount: Number(cumulativeMetrics.assetPackSendCount) || 0,
    requestAssetPackByteLength: Number(requestMetrics.requestAssetPackByteLength)
      || Number(workerReport.requestAssetPackByteLength)
      || 0,
    requestTargetSectionsByteLength: Number(requestMetrics.requestTargetSectionsByteLength)
      || Number(workerReport.requestTargetSectionsByteLength)
      || 0,
    requestSnapshotInputByteLength,
    requestByteLength: Number(requestMetrics.requestByteLength)
      || Number(workerReport.requestByteLength)
      || 0,
    transferredRequestByteLength: Number(requestMetrics.transferredRequestByteLength)
      || Number(workerReport.transferredRequestByteLength)
      || 0,
    transferredResponseByteLength,
    transferredRequestByteCount: Number(cumulativeMetrics.transferredRequestByteCount) || 0,
    transferredResponseByteCount: Number(cumulativeMetrics.transferredResponseByteCount) || 0,
    sharedInputBufferUsed,
    sharedInputByteLength,
    sharedInputBufferCapacityBytes: Number(workerReport.sharedInputBufferCapacityBytes)
      || Number(requestMetrics.sharedInputBufferCapacityBytes)
      || 0,
    snapshotInputChunkCount: Number(workerReport.snapshotInputChunkCount)
      || Number(requestMetrics.snapshotInputChunkCount)
      || 0,
    snapshotInputCompileUsed: Boolean(workerReport.snapshotInputCompileUsed),
    generatedViewFallbackUsed: Boolean(workerReport.generatedViewFallbackUsed),
    sharedResultBufferUsed,
    sharedResultByteLength,
    sharedResultBufferCapacityBytes: Number(workerReport.sharedResultBufferCapacityBytes)
      || Number(requestMetrics.sharedResultBufferCapacityBytes)
      || Number(cumulativeMetrics.sharedResultBufferCapacityBytes)
      || 0,
    sharedResultOverflow: Boolean(workerReport.sharedResultOverflow),
    sharedResultResponseCount: Number(cumulativeMetrics.sharedResultResponseCount) || 0,
    sharedResultByteCount: Number(cumulativeMetrics.sharedResultByteCount) || 0,
    sharedResultOverflowCount: Number(cumulativeMetrics.sharedResultOverflowCount) || 0,
  };
}

export function renderCompilerSharedMemorySupported() {
  return typeof SharedArrayBuffer === "function"
    && typeof Atomics === "object"
    && typeof Atomics.load === "function"
    && typeof Atomics.store === "function"
    && typeof Atomics.notify === "function";
}

export function isSharedArrayBuffer(value) {
  return typeof SharedArrayBuffer === "function" && value instanceof SharedArrayBuffer;
}

export function byteLengthOf(value) {
  if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer || isSharedArrayBuffer(value)) {
    return value.byteLength;
  }
  return 0;
}

export function byteLengthOfTargetSections(value) {
  if (Array.isArray(value)) {
    return value.length * 4;
  }
  return byteLengthOf(value);
}
