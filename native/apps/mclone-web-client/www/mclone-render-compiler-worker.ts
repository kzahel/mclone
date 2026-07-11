// 067 Stage 5: the SAB ring ABI constants and the shared predicate helpers now live in
// ./mclone-render-compiler-abi.js (the single JS source, locked to the Rust copy by a host
// test) and ./mclone-render-compiler-shared.js. This worker is the producer; it imports the
// constants it writes/reads plus the measurement predicates the consumers also use.
import {
  RENDER_COMPILER_TRANSPORT_KIND,
  RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS,
  RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX,
  RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX,
  RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX,
  RENDER_COMPILER_SHARED_RESULT_COMPLETE,
  RENDER_COMPILER_SHARED_RESULT_OVERFLOW,
  RENDER_COMPILER_SHARED_RESULT_FAILED,
  RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS,
  RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX,
  RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX,
  RENDER_COMPILER_SHARED_INPUT_READY,
} from "./mclone-render-compiler-abi.js";
import {
  byteLengthOf,
  isSharedArrayBuffer,
  renderCompilerSharedMemorySupported,
} from "./mclone-render-compiler-shared.js";
import type { WebRenderCompilerSession } from "mclone-web-client-wasm";

// The wasm-bindgen module namespace (generated `.d.ts`, emitted by `wasm-bindgen --typescript`).
// Loaded at runtime via a dynamic `import()` of a versioned URL; the bare specifier is path-mapped
// in tsconfig.json and only ever appears in type positions.
type WasmModule = typeof import("mclone-web-client-wasm");

// Inbound postMessage payload for the render-compile worker. `init-render-compiler` carries the
// asset pack; `compile-render-sections` carries the compile targets plus, on the SAB path, the
// resident input/result ring buffers (mirror of `RenderCompileWorkerRequest` on the consumer side
// in mclone-render-compiler-shared.js).
interface RenderCompileWorkerInbound {
  kind?: string;
  requestId?: number;
  workKind?: "render-sections" | "far-lod";
  bindgenJsUrl?: string;
  bindgenWasmUrl?: string;
  assetPack?: Uint8Array;
  authoredPack?: Uint8Array;
  referencePack?: Uint8Array;
  fallbackPack?: Uint8Array;
  authoredEnabled?: boolean;
  referenceEnabled?: boolean;
  assetEpoch?: number;
  targetSections?: Int32Array | number[] | ArrayBufferView | ArrayBuffer;
  centerX?: number;
  centerZ?: number;
  radiusChunks?: number;
  snapshotInputChunkCount?: number;
  snapshotInputClonedColumnCount?: number;
  farLodSeed?: string;
  farLodChunkX?: number;
  farLodChunkZ?: number;
  farLodLevel?: number;
  farLodSampleSpacingBlocks?: number;
  sharedInputBuffer?: SharedArrayBuffer;
  sharedInputByteLength?: number;
  sharedInputControlBuffer?: SharedArrayBuffer;
  sharedResultResponseBuffer?: SharedArrayBuffer;
  sharedResultControlBuffer?: SharedArrayBuffer;
}

interface SharedCompileResult {
  transportKind: string;
  transferredResponseByteLength: number;
  sharedResultBufferUsed: boolean;
  sharedResultByteLength: number;
  sharedResultBufferCapacityBytes: number;
  sharedResultOverflow: boolean;
  sharedResultBuffer: SharedArrayBuffer;
}

let wasmModulePromise = null;
let compilerSession: WebRenderCompilerSession | null = null;
let workerWasmInitCount = 0;
let workerCompileCount = 0;
let workerAssetLoadCount = 0;
let workerAssetPackInitByteLength = 0;
let workerAssetPackFileCount = 0;
let workerAssetEpoch = 0;
const workerSelf = self as unknown as DedicatedWorkerGlobalScope;

workerSelf.onmessage = async (event: MessageEvent) => {
  const message = (event.data ?? {}) as RenderCompileWorkerInbound;
  if (message.kind === "init-render-compiler") {
    await handleInit(message);
    return;
  }
  if (message.kind === "compile-render-sections") {
    await handleCompile(message);
    return;
  }

  workerSelf.postMessage({
    ok: false,
    requestId: message.requestId,
    kind: "render-compiler-error",
    reason: `unexpected render compiler message kind ${String(message.kind)}`,
  });
};

async function handleInit(message: RenderCompileWorkerInbound): Promise<void> {
  try {
    const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
    const selected = message.authoredPack instanceof Uint8Array
      && message.referencePack instanceof Uint8Array
      && message.fallbackPack instanceof Uint8Array;
    const assetPackByteLength = selected
      ? byteLengthOf(message.authoredPack)
        + byteLengthOf(message.referencePack)
        + byteLengthOf(message.fallbackPack)
      : byteLengthOf(message.assetPack);
    compilerSession = selected
      ? module.WebRenderCompilerSession.newSelected(
          message.authoredPack as Uint8Array,
          message.referencePack as Uint8Array,
          message.fallbackPack as Uint8Array,
          Boolean(message.authoredEnabled),
          Boolean(message.referenceEnabled),
        )
      : new module.WebRenderCompilerSession(message.assetPack as Uint8Array);
    workerAssetLoadCount = Number(compilerSession.assetLoadCount?.()) || 1;
    workerAssetPackInitByteLength = Number(compilerSession.assetPackByteLength?.())
      || assetPackByteLength;
    workerAssetPackFileCount = Number(compilerSession.assetPackFileCount?.()) || 0;
    workerAssetEpoch = Number(message.assetEpoch) || 0;
    workerSelf.postMessage({
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
      assetEpoch: workerAssetEpoch,
    });
  } catch (error) {
    workerSelf.postMessage({
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
      assetEpoch: workerAssetEpoch,
      persistentAssetCatalog: false,
      reason: stringifyError(error),
    });
  }
}

async function handleCompile(message: RenderCompileWorkerInbound): Promise<void> {
  try {
    workerCompileCount += 1;
    const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
    const workKind = message.workKind === "far-lod" ? "far-lod" : "render-sections";
    const farLodCompile = workKind === "far-lod";
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
    const hasPersistentFarLodCompiler =
      compilerSession !== null
      && typeof compilerSession.compileFarLodTile === "function";
    const hasGeneratedTargetedCompiler =
      targetSections.length > 0
      && (
        hasPersistentGeneratedCompiler
        || typeof module.mclone_web_compile_generated_chunk_sections_for_targets === "function"
      );
    // The `hasPersistent*` flags above already gate on `compilerSession !== null` (and the
    // snapshot flag on `snapshotInput !== null`), but those guards live in stored booleans that
    // TS cannot use to narrow the module-level `compilerSession`. The persistent branches below
    // are only reached when the flag is set, so this non-null view is sound.
    const session = compilerSession as WebRenderCompilerSession;
    if (farLodCompile && !hasPersistentFarLodCompiler) {
      throw new Error("resident render compiler does not expose compileFarLodTile");
    }
    const packed = farLodCompile
      ? session.compileFarLodTile(
          String(message.farLodSeed ?? "0"),
          Number(message.farLodChunkX) || 0,
          Number(message.farLodChunkZ) || 0,
          Number(message.farLodLevel) || 1,
          Number(message.farLodSampleSpacingBlocks) || 4,
        )
      : hasPersistentSnapshotCompiler
        ? session.compileSnapshotSectionsForTargets(
            snapshotInput as Uint8Array,
            targetSections,
          )
        : hasPersistentGeneratedCompiler
          ? session.compileGeneratedChunkSectionsForTargets(
              centerX,
              centerZ,
              radiusChunks,
              targetSections,
            )
          : hasPersistentFullCompiler
            ? session.compileGeneratedChunkSections(centerX, centerZ, radiusChunks)
            : hasGeneratedTargetedCompiler
            ? module.mclone_web_compile_generated_chunk_sections_for_targets(
                message.assetPack as Uint8Array,
                centerX,
                centerZ,
                radiusChunks,
                targetSections,
              )
              : module.mclone_web_compile_generated_chunk_sections(
                  message.assetPack as Uint8Array,
                  centerX,
                  centerZ,
                  radiusChunks,
                );
    const summary = farLodCompile
      ? { farLodTile: true, packedByteLength: packed.byteLength }
      : module.mclone_web_packed_compile_report_summary(packed);
    const response = writeSharedCompileResult(message, packed);
    const report = {
      ok: true,
      requestId: message.requestId,
      workKind,
      transportKind: response.transportKind,
      sharedMemorySupported: renderCompilerSharedMemorySupported(),
      workerWasmInitCount,
      workerCompileCount,
      workerAssetLoadCount,
      workerAssetPackInitByteLength,
      workerAssetPackFileCount,
      assetEpoch: workerAssetEpoch,
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
      // 067 follow-up 1: columns the main thread actually cloned for this submit (delta-only now),
      // echoed straight from the doorbell so the perf fence can assert it equals the upserts shipped
      // — a whole-world clone that ships only a delta would make the two diverge.
      snapshotInputClonedColumnCount: Number(message.snapshotInputClonedColumnCount) || 0,
      // 067 Stage 4: the input is a delta against the worker's resident snapshot mirror.
      // Report the delta width and resident mirror size straight from the persistent Rust
      // session so the perf fence can see input shrink to upserts-only and confirm the
      // mirror stays bounded to the loaded view (evictions track client unloads).
      snapshotInputUpsertCount: Number(compilerSession?.lastDeltaUpsertCount?.()) || 0,
      snapshotInputEvictionCount: Number(compilerSession?.lastDeltaEvictionCount?.()) || 0,
      snapshotMirrorChunkCount: Number(compilerSession?.mirrorChunkCount?.()) || 0,
      snapshotInputCompileUsed: !farLodCompile && hasPersistentSnapshotCompiler,
      generatedViewFallbackUsed: !farLodCompile && !hasPersistentSnapshotCompiler,
      sharedResultBufferUsed: response.sharedResultBufferUsed,
      sharedResultByteLength: response.sharedResultByteLength,
      sharedResultBufferCapacityBytes: response.sharedResultBufferCapacityBytes,
      sharedResultOverflow: response.sharedResultOverflow,
      centerX,
      centerZ,
      radiusChunks,
      targetSectionCount: Math.floor(targetSections.length / 3),
      targetedCompileUsed: !farLodCompile
        && (hasPersistentSnapshotCompiler || hasGeneratedTargetedCompiler),
      farLodCompileUsed: farLodCompile,
      summary,
      // 067 Stage 3 removed the non-SAB fallback from the live path; the app requires
      // cross-origin isolation, so the result always rides the resident shared buffer and
      // the transferable-postMessage branch is gone (067 Stage 5).
      sharedResultBuffer: response.sharedResultBuffer,
    };
    workerSelf.postMessage(report);
  } catch (error) {
    markSharedCompileResultFailed(message);
    workerSelf.postMessage({
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

function loadWasmModule(
  bindgenJsUrl: string | undefined,
  bindgenWasmUrl: string | undefined,
): Promise<WasmModule> {
  wasmModulePromise ??= import(bindgenJsUrl as string).then(async (module: WasmModule) => {
    await module.default(bindgenWasmUrl as string);
    workerWasmInitCount += 1;
    return module;
  });
  return wasmModulePromise;
}

function normalizeTargetSections(value: unknown): Int32Array {
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

function sharedInputBytes(message: RenderCompileWorkerInbound): Uint8Array | null {
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

function sharedInputControl(message: RenderCompileWorkerInbound): Int32Array | null {
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

function writeSharedCompileResult(
  message: RenderCompileWorkerInbound,
  packed: Uint8Array,
): SharedCompileResult {
  // 067 Stage 5: the non-SAB transferable fallback is deleted — the live path requires
  // cross-origin isolation, so the result always rides the resident shared buffer. The
  // returned `transferredResponseByteLength: 0` field is kept because the app/smoke metrics
  // and browser-smoke.mjs still read it (asserting it stays 0 on the shared path).
  const packedByteLength = byteLengthOf(packed);
  // Always present on the live SAB path (the caller hands over the resident response buffer).
  let sharedResultBuffer = message.sharedResultResponseBuffer as SharedArrayBuffer;
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

function markSharedCompileResultFailed(message: RenderCompileWorkerInbound): void {
  const control = sharedResultControl(message);
  if (control === null) {
    return;
  }
  Atomics.store(control, RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX, RENDER_COMPILER_SHARED_RESULT_FAILED);
  Atomics.notify(control, RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX, 1);
}

function sharedResultControl(message: RenderCompileWorkerInbound): Int32Array | null {
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

function stringifyError(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
