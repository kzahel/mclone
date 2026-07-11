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

// Constructor options for {@link RenderSectionWorkerCompiler}. URL/cache-bust policy is the
// caller's, so the worker/bindgen URLs are passed in as `URL`s rather than baked in here.
export interface RenderSectionWorkerCompilerOptions {
  workerUrl?: URL;
  bindgenJsUrl?: URL;
  bindgenWasmUrl?: URL;
  workerName?: string;
}

export interface RenderCompilerAssetSelection {
  authoredPack: Uint8Array;
  referencePack: Uint8Array;
  fallbackPack: Uint8Array;
  authoredEnabled: boolean;
  referenceEnabled: boolean;
  epoch: number;
}

// The SAB "doorbell" handed from Rust main-wasm to JS when a compile is armed for a frame
// (see `WebSceneHost.syncCameraRenderFrame`). JS relays it to the worker and reads
// byte counts back for diagnostics; it never decodes the packed section bytes itself.
export interface RenderCompileDoorbell {
  requestId?: number;
  workKind?: "render-sections" | "far-lod";
  centerX?: number;
  centerZ?: number;
  radiusChunks?: number;
  targetSections?: Int32Array | number[];
  snapshotInputChunkCount?: number;
  snapshotInputClonedColumnCount?: number;
  farLodSeed?: string;
  farLodChunkX?: number;
  farLodChunkZ?: number;
  farLodLevel?: number;
  farLodSampleSpacingBlocks?: number;
  farLodWestSampleSpacingBlocks?: number;
  farLodEastSampleSpacingBlocks?: number;
  farLodNorthSampleSpacingBlocks?: number;
  farLodSouthSampleSpacingBlocks?: number;
  sharedInputByteLength?: number;
  sharedInputBufferCapacityBytes?: number;
  sharedInputControlBuffer?: SharedArrayBuffer;
  sharedInputBuffer?: SharedArrayBuffer;
  sharedResultBufferCapacityBytes?: number;
  sharedResultControlBuffer?: SharedArrayBuffer;
  sharedResultResponseBuffer?: SharedArrayBuffer;
}

// Wrapper around the Rust-owned resident shared result buffers handed over on a doorbell.
export interface RenderCompileSharedResultArena {
  controlBuffer: SharedArrayBuffer;
  control: Int32Array;
  responseBuffer: SharedArrayBuffer;
  responseCapacity: number;
}

// Wrapper around the Rust-owned resident shared input buffers handed over on a doorbell.
export interface RenderCompileSharedInputArena {
  controlBuffer: SharedArrayBuffer;
  control: Int32Array;
  inputBuffer: SharedArrayBuffer;
  inputByteLength: number;
  inputCapacity: number;
}

// Outbound `compile-render-sections` postMessage payload posted to the render-compile worker.
// The `shared*` fields are attached only on the cross-origin-isolated SAB path.
export interface RenderCompileWorkerRequest {
  kind: string;
  requestId: number;
  workKind?: "render-sections" | "far-lod";
  bindgenJsUrl: string;
  bindgenWasmUrl: string;
  centerX?: number;
  centerZ?: number;
  radiusChunks?: number;
  targetSections?: Int32Array | number[];
  snapshotInputChunkCount?: number;
  snapshotInputClonedColumnCount?: number;
  farLodSeed?: string;
  farLodChunkX?: number;
  farLodChunkZ?: number;
  farLodLevel?: number;
  farLodSampleSpacingBlocks?: number;
  farLodWestSampleSpacingBlocks?: number;
  farLodEastSampleSpacingBlocks?: number;
  farLodNorthSampleSpacingBlocks?: number;
  farLodSouthSampleSpacingBlocks?: number;
  sharedInputControlBuffer?: SharedArrayBuffer;
  sharedInputBuffer?: SharedArrayBuffer;
  sharedInputByteLength?: number;
  sharedInputBufferCapacityBytes?: number;
  sharedResultControlBuffer?: SharedArrayBuffer;
  sharedResultResponseBuffer?: SharedArrayBuffer;
  sharedResultBufferCapacityBytes?: number;
}

// The render-compile worker's `render-compiler-ready` / per-compile report payload. This is a
// hand-rolled metrics/diagnostics bag echoed across the worker postMessage boundary and read
// back coercion-guarded (`Number(...)`/`Boolean(...)`), so it is intentionally permissive; the
// packed section bytes ride a separate SAB and are not part of this object on the live path.
export type RenderCompileWorkerReport = Record<string, any>;

// The per-request `pending` record the consumer holds while a compile is in flight.
export interface RenderCompilePending {
  resolve: (value: any) => void;
  reject: (reason?: any) => void;
  timeout: ReturnType<typeof setTimeout>;
  requestMetrics: Record<string, any>;
  sharedResult: RenderCompileSharedResultArena | null;
  sharedInput: RenderCompileSharedInputArena | null;
}

interface RenderCompilerDispatchArgs {
  requestId: number;
  workKind?: "render-sections" | "far-lod";
  centerX?: number;
  centerZ?: number;
  radiusChunks?: number;
  targetSections?: Int32Array | number[];
  snapshotInputChunkCount?: number;
  snapshotInputClonedColumnCount?: number;
  farLodSeed?: string;
  farLodChunkX?: number;
  farLodChunkZ?: number;
  farLodLevel?: number;
  farLodSampleSpacingBlocks?: number;
  farLodWestSampleSpacingBlocks?: number;
  farLodEastSampleSpacingBlocks?: number;
  farLodNorthSampleSpacingBlocks?: number;
  farLodSouthSampleSpacingBlocks?: number;
  sharedResult: RenderCompileSharedResultArena | null;
  sharedInput: RenderCompileSharedInputArena | null;
  requestSource?: {
    snapshotInputByteLength?: number;
    snapshotInputChunkCount?: number;
    snapshotInputBytes?: ArrayBufferView;
  };
}

export async function fetchAssetPack(assetPackUrl: URL): Promise<Uint8Array> {
  const response = await fetch(assetPackUrl);
  if (!response.ok) {
    throw new Error(`failed to fetch ${assetPackUrl.pathname}: ${response.status} ${response.statusText}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

export class RenderSectionWorkerCompiler {
  workerUrl: URL;
  bindgenJsUrl: URL;
  bindgenWasmUrl: URL;
  pending: Map<number, RenderCompilePending>;
  metrics: Record<string, any>;
  ready: Promise<RenderCompileWorkerReport>;
  resolveReady!: (value: RenderCompileWorkerReport) => void;
  rejectReady!: (reason?: any) => void;
  initTimeout: ReturnType<typeof setTimeout>;
  worker: Worker;

  constructor(
    assetPack: Uint8Array | RenderCompilerAssetSelection,
    options: RenderSectionWorkerCompilerOptions = {},
  ) {
    const {
      workerUrl,
      bindgenJsUrl,
      bindgenWasmUrl,
      workerName = "mclone-render-compiler",
    } = options;
    // Required in practice — every caller (app + smoke) passes all three URLs; the worker cannot
    // run without them, so a missing one throws at `.href` either way.
    this.workerUrl = workerUrl as URL;
    this.bindgenJsUrl = bindgenJsUrl as URL;
    this.bindgenWasmUrl = bindgenWasmUrl as URL;
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
    this.ready = new Promise<RenderCompileWorkerReport>((resolve, reject) => {
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
    this.worker.onmessage = (event: MessageEvent) => {
      const data = (event.data ?? {}) as RenderCompileWorkerReport;
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

  initialize(assetPack: Uint8Array | RenderCompilerAssetSelection): void {
    if (!(assetPack instanceof Uint8Array)) {
      const authoredPack = assetPack.authoredPack.slice();
      const referencePack = assetPack.referencePack.slice();
      const fallbackPack = assetPack.fallbackPack.slice();
      const byteLength = authoredPack.byteLength
        + referencePack.byteLength
        + fallbackPack.byteLength;
      this.metrics.assetPackSendCount += 1;
      this.metrics.workerAssetPackInitByteLength = byteLength;
      this.metrics.transferredRequestByteCount += byteLength;
      this.worker.postMessage(
        {
          kind: "init-render-compiler",
          requestId: 0,
          bindgenJsUrl: this.bindgenJsUrl.href,
          bindgenWasmUrl: this.bindgenWasmUrl.href,
          authoredPack,
          referencePack,
          fallbackPack,
          authoredEnabled: assetPack.authoredEnabled,
          referenceEnabled: assetPack.referenceEnabled,
          assetEpoch: assetPack.epoch,
        },
        [authoredPack.buffer, referencePack.buffer, fallbackPack.buffer],
      );
      return;
    }
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

  handleReady(data: RenderCompileWorkerReport): void {
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
  async compileWithDoorbell(doorbell: RenderCompileDoorbell): Promise<any> {
    await this.ready;
    const requestId = Number(doorbell.requestId) || 0;
    if (requestId <= 0) {
      throw new Error(`invalid render compile doorbell id ${String(doorbell.requestId)}`);
    }
    const sharedResult = sharedResultArenaFromDoorbell(doorbell);
    const sharedInput = sharedInputArenaFromDoorbell(doorbell);
    return this.dispatchCompile({
      requestId,
      workKind: doorbell.workKind,
      centerX: doorbell.centerX,
      centerZ: doorbell.centerZ,
      radiusChunks: doorbell.radiusChunks,
      targetSections: doorbell.targetSections,
      snapshotInputChunkCount: Number(doorbell.snapshotInputChunkCount) || 0,
      snapshotInputClonedColumnCount: Number(doorbell.snapshotInputClonedColumnCount) || 0,
      farLodSeed: doorbell.farLodSeed,
      farLodChunkX: doorbell.farLodChunkX,
      farLodChunkZ: doorbell.farLodChunkZ,
      farLodLevel: doorbell.farLodLevel,
      farLodSampleSpacingBlocks: doorbell.farLodSampleSpacingBlocks,
      farLodWestSampleSpacingBlocks: doorbell.farLodWestSampleSpacingBlocks,
      farLodEastSampleSpacingBlocks: doorbell.farLodEastSampleSpacingBlocks,
      farLodNorthSampleSpacingBlocks: doorbell.farLodNorthSampleSpacingBlocks,
      farLodSouthSampleSpacingBlocks: doorbell.farLodSouthSampleSpacingBlocks,
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
    workKind,
    centerX,
    centerZ,
    radiusChunks,
    targetSections,
    snapshotInputChunkCount,
    snapshotInputClonedColumnCount,
    farLodSeed,
    farLodChunkX,
    farLodChunkZ,
    farLodLevel,
    farLodSampleSpacingBlocks,
    farLodWestSampleSpacingBlocks,
    farLodEastSampleSpacingBlocks,
    farLodNorthSampleSpacingBlocks,
    farLodSouthSampleSpacingBlocks,
    sharedResult,
    sharedInput,
    requestSource,
  }: RenderCompilerDispatchArgs): Promise<any> {
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
      const message: RenderCompileWorkerRequest = {
        kind: "compile-render-sections",
        requestId,
        workKind,
        bindgenJsUrl: this.bindgenJsUrl.href,
        bindgenWasmUrl: this.bindgenWasmUrl.href,
        centerX,
        centerZ,
        radiusChunks,
        targetSections,
        snapshotInputChunkCount,
        snapshotInputClonedColumnCount,
        farLodSeed,
        farLodChunkX,
        farLodChunkZ,
        farLodLevel,
        farLodSampleSpacingBlocks,
        farLodWestSampleSpacingBlocks,
        farLodEastSampleSpacingBlocks,
        farLodNorthSampleSpacingBlocks,
        farLodSouthSampleSpacingBlocks,
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

  pendingJobCount(): number {
    return this.pending.size;
  }

  terminate(): void {
    this.worker.terminate();
    this.rejectAll(new Error("render compiler worker terminated"));
  }

  rejectAll(error: Error): void {
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
export function sharedResultArenaFromDoorbell(
  doorbell: RenderCompileDoorbell,
): RenderCompileSharedResultArena | null {
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

export function sharedInputArenaFromDoorbell(
  doorbell: RenderCompileDoorbell,
): RenderCompileSharedInputArena | null {
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

export function renderCompilerPackedResponse(
  workerReport: RenderCompileWorkerReport,
  pending: RenderCompilePending | undefined,
) {
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

export function renderCompilerRequestMetrics(
  targetSections: Int32Array | number[] | undefined,
  sharedResult: RenderCompileSharedResultArena | null,
  sharedInput: RenderCompileSharedInputArena | null,
  request: RenderCompilerDispatchArgs["requestSource"] | undefined,
) {
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

export function renderCompilerMetricsForResponse(
  cumulativeMetrics: Record<string, any>,
  requestMetrics: Record<string, any>,
  workerReport: RenderCompileWorkerReport,
  packedByteLength: number,
) {
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
    // 067 follow-up 1: main-thread columns cloned for this submit; the worker echoes it from the
    // doorbell. The fence asserts it equals snapshotInputChunkCount (the upserts shipped).
    snapshotInputClonedColumnCount: Number(workerReport.snapshotInputClonedColumnCount) || 0,
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

export function renderCompilerSharedMemorySupported(): boolean {
  return typeof SharedArrayBuffer === "function"
    && typeof Atomics === "object"
    && typeof Atomics.load === "function"
    && typeof Atomics.store === "function"
    && typeof Atomics.notify === "function";
}

export function isSharedArrayBuffer(value: unknown): value is SharedArrayBuffer {
  return typeof SharedArrayBuffer === "function" && value instanceof SharedArrayBuffer;
}

export function byteLengthOf(value: unknown): number {
  if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer || isSharedArrayBuffer(value)) {
    return value.byteLength;
  }
  return 0;
}

export function byteLengthOfTargetSections(value: unknown): number {
  if (Array.isArray(value)) {
    return value.length * 4;
  }
  return byteLengthOf(value);
}
