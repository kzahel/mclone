const WASM_URL = new URL("./mclone_web_client.wasm", import.meta.url);
const BINDGEN_JS_URL = new URL("./pkg/mclone_web_client.js", import.meta.url);
const BINDGEN_WASM_URL = new URL("./pkg/mclone_web_client_bg.wasm", import.meta.url);
const THREAD_WORKER_URL = new URL("./mclone-thread-smoke-worker.js", import.meta.url);
const RENDER_COMPILER_WORKER_URL = new URL("./mclone-render-compiler-worker.js", import.meta.url);
const SERVER_WORKER_URL = new URL("./mclone-integrated-server-worker.js", import.meta.url);
const SERVER_JOB_WORKER_URL = new URL("./mclone-server-job-worker.js", import.meta.url);
const ASSET_PACK_URL = new URL("/reference/minecraft-1.17.1/extracted.zip", import.meta.url);
const RUNTIME_SMOKE_EXPORT = "mclone_web_runtime_smoke_report";
const REMOTE_WS_URL = new URL(globalThis.location.href).searchParams.get("remoteWsUrl") ?? "";
const RENDER_COMPILER_TRANSPORT_KIND = "shared-result-buffer";
const RENDER_COMPILER_MESSAGE_TRANSFER_KIND = "message-transfer";
const RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS = 4;
const RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX = 0;
const RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX = 1;
const RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX = 2;
const RENDER_COMPILER_SHARED_RESULT_PENDING = 1;
const RENDER_COMPILER_SHARED_RESULT_COMPLETE = 2;
const RENDER_COMPILER_SHARED_RESULT_OVERFLOW = 3;
const RENDER_COMPILER_DEFAULT_SHARED_RESULT_CAPACITY = 16 * 1024 * 1024;
const RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS = 4;
const RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX = 0;
const RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX = 1;
const RENDER_COMPILER_SHARED_INPUT_CAPACITY_INDEX = 2;
const RENDER_COMPILER_SHARED_INPUT_READY = 2;
const RENDER_COMPILER_DEFAULT_SHARED_INPUT_CAPACITY = 1 * 1024 * 1024;

const ready = boot();
globalThis.__mcloneNativeReady = ready;

ready.then(
  (result) => renderStatus(result),
  (error) => renderStatus({ ok: false, reason: stringifyError(error) }),
);

async function boot() {
  const threading = await probeThreading();
  const wasm = await loadRuntimeWasm();
  const webGpu = await probeWebGpu();
  const canvas = webGpu.supported
    ? await renderCanvas()
    : {
        ok: true,
        supported: false,
        status: "webgpu-unavailable",
        reason: webGpu.status,
      };
  return {
    ok: threading.ok && wasm.ok && webGpu.ok && canvas.ok,
    threading,
    wasm,
    webGpu,
    canvas,
  };
}

async function probeThreading() {
  const report = {
    ok: false,
    crossOriginIsolated: Boolean(globalThis.crossOriginIsolated),
    sharedArrayBuffer: typeof SharedArrayBuffer === "function",
    wasmSharedMemory: false,
    workerConstructor: typeof Worker === "function",
    atomics: typeof Atomics === "object"
      && typeof Atomics.add === "function"
      && typeof Atomics.load === "function"
      && typeof Atomics.store === "function",
    workerRoundtrip: false,
    initialValue: 0,
    finalValue: 0,
  };

  if (!report.crossOriginIsolated) {
    return {
      ...report,
      reason: "page is not cross-origin isolated; SharedArrayBuffer threading is disabled",
    };
  }
  if (!report.sharedArrayBuffer) {
    return {
      ...report,
      reason: "SharedArrayBuffer is not available",
    };
  }
  if (!report.atomics) {
    return {
      ...report,
      reason: "Atomics operations are not available",
    };
  }
  if (!report.workerConstructor) {
    return {
      ...report,
      reason: "Worker constructor is not available",
    };
  }

  let memory;
  try {
    memory = new WebAssembly.Memory({
      initial: 1,
      maximum: 1,
      shared: true,
    });
    report.wasmSharedMemory = memory.buffer instanceof SharedArrayBuffer;
  } catch (error) {
    return {
      ...report,
      reason: `failed to allocate shared WebAssembly.Memory: ${stringifyError(error)}`,
    };
  }
  if (!report.wasmSharedMemory) {
    return {
      ...report,
      reason: "WebAssembly.Memory did not produce a SharedArrayBuffer",
    };
  }

  const view = new Int32Array(memory.buffer);
  Atomics.store(view, 0, 7);
  Atomics.store(view, 1, 0);
  report.initialValue = Atomics.load(view, 0);

  try {
    const worker = await runThreadWorker(memory, 35);
    report.workerRoundtrip = Boolean(
      worker.ok
      && worker.initialValue === 7
      && worker.finalValue === 42
      && Atomics.load(view, 0) === 42
      && Atomics.load(view, 1) === 1
    );
    report.worker = worker;
    report.finalValue = Atomics.load(view, 0);
  } catch (error) {
    return {
      ...report,
      reason: stringifyError(error),
    };
  }

  return {
    ...report,
    ok: report.workerRoundtrip,
  };
}

function runThreadWorker(memory, addend) {
  return new Promise((resolve, reject) => {
    const worker = new Worker(THREAD_WORKER_URL.href, {
      type: "module",
      name: "mclone-thread-smoke",
    });
    let settled = false;
    const timeout = setTimeout(() => {
      if (settled) return;
      settled = true;
      worker.terminate();
      reject(new Error("timed out waiting for thread smoke worker"));
    }, 5_000);

    worker.onmessage = (event) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      worker.terminate();
      resolve(event.data);
    };
    worker.onerror = (event) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      worker.terminate();
      reject(new Error(event.message || "thread smoke worker failed"));
    };
    worker.postMessage({
      kind: "mclone-thread-smoke",
      memory,
      addend,
    });
  });
}

async function loadRuntimeWasm() {
  try {
    const response = await fetch(WASM_URL);
    if (!response.ok) {
      return {
        ok: false,
        reason: `failed to fetch ${WASM_URL.pathname}: ${response.status} ${response.statusText}`,
      };
    }

    const bytes = await response.arrayBuffer();
    const module = await WebAssembly.compile(bytes);
    const imports = createWasmImports(module);
    const instance = await WebAssembly.instantiate(module, imports);
    const runtimeSmoke = instance.exports[RUNTIME_SMOKE_EXPORT];
    if (typeof runtimeSmoke !== "function") {
      return {
        ok: false,
        reason: `missing wasm export ${RUNTIME_SMOKE_EXPORT}`,
        exports: Object.keys(instance.exports),
      };
    }

    const bits = Number(runtimeSmoke()) >>> 0;
    const report = decodeRuntimeReport(bits);
    return {
      ok: report.ok,
      exportName: RUNTIME_SMOKE_EXPORT,
      report,
    };
  } catch (error) {
    return {
      ok: false,
      reason: stringifyError(error),
    };
  }
}

function createWasmImports(module) {
  const imports = {};
  for (const descriptor of WebAssembly.Module.imports(module)) {
    if (descriptor.kind !== "function") {
      throw new Error(`unsupported wasm import ${descriptor.module}.${descriptor.name} (${descriptor.kind})`);
    }
    imports[descriptor.module] ??= {};
    imports[descriptor.module][descriptor.name] = createWasmImportStub(descriptor);
  }
  return imports;
}

function createWasmImportStub(descriptor) {
  if (descriptor.name.includes("throw")) {
    return () => {
      throw new Error(`wasm import ${descriptor.module}.${descriptor.name} called`);
    };
  }
  if (descriptor.name.includes("table_grow")) {
    return () => -1;
  }
  return () => 0;
}

async function probeWebGpu() {
  if (!("gpu" in navigator) || !navigator.gpu) {
    return {
      ok: true,
      supported: false,
      status: "navigator-gpu-missing",
      reason: "navigator.gpu is not available",
    };
  }

  let adapter;
  try {
    adapter = await navigator.gpu.requestAdapter();
  } catch (error) {
    return {
      ok: true,
      supported: false,
      status: "adapter-request-failed",
      reason: stringifyError(error),
    };
  }

  if (!adapter) {
    return {
      ok: true,
      supported: false,
      status: "adapter-unavailable",
      reason: "requestAdapter returned null",
    };
  }

  let device;
  try {
    device = await adapter.requestDevice();
  } catch (error) {
    return {
      ok: true,
      supported: false,
      status: "device-request-failed",
      reason: stringifyError(error),
    };
  }

  const adapterInfo = await readAdapterInfo(adapter);
  const format = navigator.gpu.getPreferredCanvasFormat();
  device.destroy();
  return {
    ok: true,
    supported: true,
    status: "ready",
    format,
    adapterInfo,
  };
}

async function readAdapterInfo(adapter) {
  try {
    if (typeof adapter.requestAdapterInfo === "function") {
      return await adapter.requestAdapterInfo();
    }
  } catch {}

  return adapter.info ?? {};
}

async function renderCanvas() {
  const canvas = document.getElementById("mclone-canvas");
  if (!(canvas instanceof HTMLCanvasElement)) {
    return {
      ok: false,
      supported: true,
      status: "canvas-missing",
      reason: "missing canvas#mclone-canvas",
    };
  }

  try {
    const module = await import(BINDGEN_JS_URL.href);
    await module.default(BINDGEN_WASM_URL.href);
    if (typeof module.mclone_web_create_worker_chunk_render_session !== "function") {
      return {
        ok: false,
        supported: true,
        status: "export-missing",
        reason: "missing mclone_web_create_worker_chunk_render_session export",
        exports: Object.keys(module),
      };
    }

    const assetPack = await fetchAssetPack();
    const session = await module.mclone_web_create_worker_chunk_render_session(
      canvas,
      assetPack,
      SERVER_WORKER_URL.href,
      SERVER_JOB_WORKER_URL.href,
      BINDGEN_JS_URL.href,
      BINDGEN_WASM_URL.href,
    );
    if (
      typeof session.syncOverviewRenderFrame !== "function"
      || typeof session.pendingChunkRenderCompileJobCount !== "function"
      || typeof session.shutdown !== "function"
    ) {
      return {
        ok: false,
        supported: true,
        status: "export-missing",
        reason: "missing WebChunkRenderSession streaming/shutdown export",
      };
    }

    const compiler = new RenderSectionWorkerCompiler(assetPack);
    try {
      // 067 Stage 3: pump the shared streaming loop to idle for two overview centers
      // (the web analog of desktop `sync_all_render_sections`). Each frame may both apply
      // the previous compile and arm the next; JS only relays the doorbell, the Rust loop
      // owns scheduling/coalescing/acceptance. Center (1,0) reuses the resident worker mesh
      // catalog + ring, so it incrementally streams the shifted view on top of (0,0).
      const firstCenter = await streamOverviewToIdle(session, compiler, 0, 0, 1);
      const secondCenter = await streamOverviewToIdle(session, compiler, 1, 0, 1);
      const firstReport = firstCenter.report;
      const report = secondCenter.report;
      const shutdownReport = session.shutdown();
      const sharedTopologyStress = await runSharedTopologyStress(module);
      const remoteWebSocket = await runRemoteWebSocketSmoke(module);
      return {
        ok: Boolean(
          firstCenter.settled
          && secondCenter.settled
          && Number(firstCenter.compileCount) > 0
          && report.worldgenMailboxKind === "web-worker"
          && report.lightStatusMailboxKind === "web-worker"
          && Number(report.worldgenMailboxPendingJobs) === 0
          && Number(report.lightStatusMailboxPendingStatuses) === 0
          // The streaming runner ships its tiny command/update frames over either
          // shared-memory or message-transfer depending on payload size (non-deterministic
          // run to run); the heavy worldgen/light lanes are deterministically shared-memory,
          // and the shared-memory runner capability is asserted by sharedTopologyStress.
          && frameMetricsActive(report.worldgenJobFrameMetrics, "shared-memory")
          && frameMetricsActive(report.lightStatusJobFrameMetrics, "shared-memory")
          && sharedBufferPoolActive(report.worldgenJobFrameMetrics)
          && sharedBufferPoolActive(report.lightStatusJobFrameMetrics)
          && report.rendered
          && report.configured
          && report.chunkLoaded
          && report.meshBuilt
          && report.assetPackLoaded
          && report.textured
          && shutdownReport.ok
          && sharedTopologyStressActive(sharedTopologyStress)
          && remoteWebSocketActive(remoteWebSocket)
        ),
        supported: true,
        status: report.ok ? "rendered" : "failed",
        firstCenter: publicOverviewCenter(firstCenter),
        secondCenter: publicOverviewCenter(secondCenter),
        renderCompilerPendingJobCount: compiler.pendingJobCount(),
        sessionPendingCompileJobCount: session.pendingChunkRenderCompileJobCount(),
        firstReport,
        report,
        shutdownReport,
        sharedTopologyStress,
        remoteWebSocket,
      };
    } finally {
      compiler.terminate();
    }
  } catch (error) {
    return {
      ok: false,
      supported: true,
      status: "render-failed",
      reason: stringifyError(error),
    };
  }
}

// 067 Stage 3: drive `syncOverviewRenderFrame` to idle at a fixed chunk center, relaying
// each frame's worker doorbell. "Settled" requires render-idle AND the server runner
// drained (queue/job/publication counts zero) AND at least one resident section, because a
// fast center jump can reach render-idle before the moved-to chunks have streamed (no
// snapshots yet => no dirty work => idle, but the view is not actually loaded). A stable-
// frame count and a wall-clock deadline keep the pump robust.
async function streamOverviewToIdle(session, compiler, centerX, centerZ, radius) {
  const deadline = performance.now() + 30_000;
  let frame = null;
  let lastWorkerReport = null;
  let compileCount = 0;
  let stableFrames = 0;
  // A center change requests a new view, but the runner registers the new generation
  // jobs asynchronously. Right after the change the runner can momentarily look drained
  // (jobs not yet propagated, no dirty work yet) and the loop would settle prematurely
  // with no compiles. Require having observed the runner do work for this center — busy
  // queues/jobs/publications, or a compile armed this frame — before accepting "settled".
  let observedRunnerWork = false;
  while (performance.now() < deadline) {
    frame = session.syncOverviewRenderFrame(centerX, centerZ, radius);
    if (!frame?.ok) {
      throw new Error(
        `overview render frame failed at center ${centerX},${centerZ}: ${frame?.reason ?? "unknown"}`,
      );
    }
    if (frame.doorbell) {
      const compiled = await compiler.compileWithDoorbell(frame.doorbell);
      compileCount += 1;
      if (compiled?.report) {
        lastWorkerReport = compiled.report;
      }
    }
    if (overviewRunnerBusy(frame) || frame.doorbell) {
      observedRunnerWork = true;
    }
    if (observedRunnerWork && overviewFrameSettled(frame)) {
      stableFrames += 1;
      if (stableFrames >= 3) {
        break;
      }
    } else {
      stableFrames = 0;
    }
    // Always yield to the event loop (a macrotask, not just a microtask) so the
    // integrated server / worldgen / light workers' messages are processed between
    // frames. Without this, no-doorbell frames (no compile to await) spin the main
    // thread synchronously and starve the runner so it can never drain.
    await yieldToEventLoop();
  }
  const settled = observedRunnerWork && overviewFrameSettled(frame);
  if (!settled) {
    throw new Error(
      `overview render did not settle at center ${centerX},${centerZ} `
        + `(observedRunnerWork=${observedRunnerWork}):\n${JSON.stringify(frame, null, 2)}`,
    );
  }
  return {
    centerX,
    centerZ,
    settled,
    compileCount,
    residentSectionCount: Number(frame.residentSectionCount) || 0,
    report: frame,
    workerReport: lastWorkerReport,
  };
}

function yieldToEventLoop() {
  if (typeof requestAnimationFrame === "function") {
    return new Promise((resolve) => requestAnimationFrame(() => resolve()));
  }
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function overviewRunnerBusy(frame) {
  return Boolean(
    frame
    && (Number(frame.runnerCommandQueueDepth) > 0
      || Number(frame.runnerUpdateQueueDepth) > 0
      || Number(frame.runnerPendingJobs) > 0
      || Number(frame.runnerPendingPublications) > 0),
  );
}

function overviewFrameSettled(frame) {
  return Boolean(
    frame
    && frame.streamingIdle
    && Number(frame.runnerPendingJobs) === 0
    && Number(frame.runnerUpdateQueueDepth) === 0
    && Number(frame.runnerCommandQueueDepth) === 0
    && Number(frame.runnerPendingPublications) === 0
    && Number(frame.residentSectionCount) > 0
  );
}

function publicOverviewCenter(center) {
  return {
    centerX: center.centerX,
    centerZ: center.centerZ,
    settled: center.settled,
    compileCount: center.compileCount,
    residentSectionCount: center.residentSectionCount,
    workerReport: center.workerReport,
  };
}

async function runRemoteWebSocketSmoke(module) {
  if (!REMOTE_WS_URL) {
    return {
      ok: true,
      supported: false,
      status: "not-requested",
    };
  }
  if (typeof module.mclone_web_remote_websocket_smoke_report !== "function") {
    return {
      ok: false,
      supported: true,
      status: "export-missing",
      reason: "missing mclone_web_remote_websocket_smoke_report export",
    };
  }
  try {
    return {
      supported: true,
      status: "connected",
      ...(await module.mclone_web_remote_websocket_smoke_report(REMOTE_WS_URL)),
    };
  } catch (error) {
    return {
      ok: false,
      supported: true,
      status: "remote-failed",
      reason: stringifyError(error),
    };
  }
}

async function runSharedTopologyStress(module) {
  if (typeof module.mclone_web_shared_topology_stress_report !== "function") {
    return {
      ok: false,
      status: "export-missing",
      reason: "missing mclone_web_shared_topology_stress_report export",
    };
  }
  try {
    return await module.mclone_web_shared_topology_stress_report(
      SERVER_WORKER_URL.href,
      SERVER_JOB_WORKER_URL.href,
      BINDGEN_JS_URL.href,
      BINDGEN_WASM_URL.href,
    );
  } catch (error) {
    return {
      ok: false,
      status: "stress-failed",
      reason: stringifyError(error),
    };
  }
}

async function fetchAssetPack() {
  const response = await fetch(ASSET_PACK_URL);
  if (!response.ok) {
    throw new Error(`failed to fetch ${ASSET_PACK_URL.pathname}: ${response.status} ${response.statusText}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

class RenderSectionWorkerCompiler {
  constructor(assetPack) {
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
    // 067 Stage 3: the overview pump drives compiles through the Rust-owned resident SAB
    // ring handed over on each doorbell, so this worker shim no longer owns its own SAB
    // arenas — it is a doorbell + lifecycle relay only.
    this.ready = new Promise((resolve, reject) => {
      this.resolveReady = resolve;
      this.rejectReady = reject;
    });
    this.initTimeout = setTimeout(() => {
      this.rejectReady(new Error("timed out initializing render compiler worker"));
    }, 20_000);
    this.worker = new Worker(RENDER_COMPILER_WORKER_URL.href, {
      type: "module",
      name: "mclone-render-compiler-smoke",
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
        bindgenJsUrl: BINDGEN_JS_URL.href,
        bindgenWasmUrl: BINDGEN_WASM_URL.href,
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

  // 067 Stage 3: post a compile against the Rust-owned resident shared ring. Main
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
        bindgenJsUrl: BINDGEN_JS_URL.href,
        bindgenWasmUrl: BINDGEN_WASM_URL.href,
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

// 067 Stage 3: wrap the Rust-owned resident result buffers (handed over on the
// submit doorbell) in the same arena shape the worker protocol + metrics expect.
// JS does not arm these — main wasm already filled/armed them — it only reads byte
// counts back for diagnostics.
function sharedResultArenaFromDoorbell(doorbell) {
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

function sharedInputArenaFromDoorbell(doorbell) {
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

function renderCompilerPackedResponse(workerReport, pending) {
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

function renderCompilerRequestMetrics(targetSections, sharedResult, sharedInput, request) {
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

function renderCompilerMetricsForResponse(cumulativeMetrics, requestMetrics, workerReport, packedByteLength) {
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

function byteLengthOf(value) {
  if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer || isSharedArrayBuffer(value)) {
    return value.byteLength;
  }
  return 0;
}

function byteLengthOfTargetSections(value) {
  if (Array.isArray(value)) {
    return value.length * 4;
  }
  return byteLengthOf(value);
}

function decodeRuntimeReport(bits) {
  return {
    bits,
    ok: (bits & 0x1) !== 0,
    localHost: (bits & 0x2) !== 0,
    webRenderBackend: (bits & 0x4) !== 0,
    centerChunkLoaded: (bits & 0x8) !== 0,
    transportDrained: (bits & 0x10) !== 0,
    movedChunkLoaded: (bits & 0x20) !== 0,
    previousChunkUnloaded: (bits & 0x40) !== 0,
    protocolCodecRoundtrip: (bits & 0x80) !== 0,
    commandCount: (bits >>> 8) & 0xff,
    updateCount: (bits >>> 16) & 0xff,
    loadedChunkCount: (bits >>> 24) & 0xff,
  };
}

function frameMetricsActive(metrics, transportKind) {
  return Boolean(
    metrics
    && metrics.transportKind === transportKind
    && Number(metrics.requestFrames) > 0
    && Number(metrics.requestBytes) > 0
    && Number(metrics.responseFrames) > 0
    && Number(metrics.responseBytes) > 0
  );
}

function sharedBufferPoolActive(metrics) {
  return Boolean(
    metrics
    && Number(metrics.sharedBufferPoolMisses) > 0
    && Number(metrics.sharedBufferPoolHits) > 0
    && Number(metrics.sharedBufferPoolDrops) === 0
    && Number(metrics.sharedBufferCapacityBytes) > 0
    && Number(metrics.maxSharedBufferCapacityBytes) >= Number(metrics.sharedBufferCapacityBytes)
    && Number(metrics.sharedBufferPooledResponseFrames) > 0
    && Number(metrics.sharedBufferFallbackResponseFrames) === 0
  );
}

function sharedTopologyStressActive(report) {
  return Boolean(
    report
    && report.ok
    && sharedRunnerStressActive(report.sharedRunner)
    && fallbackRunnerStressActive(report.fallbackRunner)
  );
}

function sharedRunnerStressActive(report) {
  const metrics = report?.runnerFrameMetrics;
  return Boolean(
    report
    && report.ok
    && Number(report.commandCount) >= 6
    && Number(report.updateCount) > 0
    && frameMetricsActive(metrics, "shared-memory")
    && Number(metrics.sharedBufferPoolMisses) >= 4
    && Number(metrics.sharedBufferPoolHits) >= 2
    && Number(metrics.sharedBufferPoolDrops) > 0
    && Number(metrics.sharedBufferCapacityBytes) > 0
    && Number(metrics.maxSharedBufferCapacityBytes) >= Number(metrics.sharedBufferCapacityBytes)
    && Number(metrics.sharedBufferPooledResponseFrames) > 0
    && Number(metrics.sharedBufferFallbackResponseFrames) > 0
    && frameMetricsActive(report.worldgenJobFrameMetrics, "shared-memory")
    && frameMetricsActive(report.lightStatusJobFrameMetrics, "shared-memory")
    && report.shutdown
    && report.shutdown.running === false
  );
}

function fallbackRunnerStressActive(report) {
  const metrics = report?.runnerFrameMetrics;
  return Boolean(
    report
    && report.ok
    && Number(report.commandCount) === 1
    && Number(report.updateCount) > 0
    && frameMetricsActive(metrics, "message-transfer")
    && Number(metrics.sharedBufferPoolHits) === 0
    && Number(metrics.sharedBufferPoolMisses) === 0
    && Number(metrics.sharedBufferPoolDrops) === 0
    && Number(metrics.sharedBufferPooledResponseFrames) === 0
    && Number(metrics.sharedBufferFallbackResponseFrames) === 0
    && report.shutdown
    && report.shutdown.running === false
  );
}

function remoteWebSocketActive(report) {
  if (!REMOTE_WS_URL) {
    return Boolean(report?.ok && report.supported === false);
  }
  const metrics = report?.runnerFrameMetrics;
  return Boolean(
    report
    && report.ok
    && report.supported === true
    && report.runnerKind === "remote-websocket"
    && report.clientHost === "remote-dedicated"
    && report.centerChunkLoaded
    && report.movedChunkLoaded
    && report.previousChunkUnloaded
    && report.transportDrained
    && report.protocolCodecRoundtrip
    && Number(report.commandCount) === 2
    && Number(report.updateCount) > 0
    && Number(report.loadedChunkCount) === 1
    && Number(report.runnerCommandQueueDepth) === 0
    && Number(report.runnerUpdateQueueDepth) === 0
    && frameMetricsActive(metrics, "websocket")
    && Number(metrics.requestFrames) >= 3
    && Number(metrics.responseFrames) >= 3
  );
}

function renderStatus(result) {
  const status = document.getElementById("status");
  if (!status) return;
  status.textContent = JSON.stringify(result, null, 2);
  status.dataset.ok = result.ok ? "true" : "false";
}

function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
