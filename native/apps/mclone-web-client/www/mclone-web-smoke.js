const WASM_URL = new URL("./mclone_web_client.wasm", import.meta.url);
const BINDGEN_JS_URL = new URL("./pkg/mclone_web_client.js", import.meta.url);
const BINDGEN_WASM_URL = new URL("./pkg/mclone_web_client_bg.wasm", import.meta.url);
const THREAD_WORKER_URL = new URL("./mclone-thread-smoke-worker.js", import.meta.url);
const RENDER_COMPILER_WORKER_URL = new URL("./mclone-render-compiler-worker.js", import.meta.url);
const SERVER_WORKER_URL = new URL("./mclone-integrated-server-worker.js", import.meta.url);
const SERVER_JOB_WORKER_URL = new URL("./mclone-server-job-worker.js", import.meta.url);
const ASSET_PACK_URL = new URL("/reference/minecraft-1.17.1/extracted.zip", import.meta.url);
const RUNTIME_SMOKE_EXPORT = "mclone_web_runtime_smoke_report";

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
      typeof session.beginChunkRenderCompileRequest !== "function"
      || typeof session.finishChunkRenderCompileRequest !== "function"
      || typeof session.pendingChunkRenderCompileJobCount !== "function"
      || typeof session.shutdown !== "function"
    ) {
      return {
        ok: false,
        supported: true,
        status: "export-missing",
        reason: "missing WebChunkRenderSession browser compile lifecycle/shutdown export",
      };
    }

    const compiler = new RenderSectionWorkerCompiler(assetPack);
    try {
      const firstCompileRequest = await session.beginChunkRenderCompileRequest(0, 0, 1);
      const firstCompile = await compiler.compile(firstCompileRequest);
      const renderCompiler = firstCompile.report;
      const firstReport = renderCompiler.ok
        ? session.finishChunkRenderCompileRequest(firstCompileRequest.requestId, firstCompile.packed)
        : { ok: false, reason: "render compiler worker failed before first render" };
      const secondCompileRequest = firstReport.ok
        ? await session.beginChunkRenderCompileRequest(1, 0, 1)
        : { ok: false, reason: "first worker render failed before second compile request" };
      const secondCompile = secondCompileRequest.ok
        ? await compiler.compile(secondCompileRequest)
        : { report: { ok: false, reason: "first worker render failed before second compile" }, packed: new Uint8Array() };
      const secondRenderCompiler = secondCompile.report;
      const report = secondRenderCompiler.ok
        ? session.finishChunkRenderCompileRequest(secondCompileRequest.requestId, secondCompile.packed)
        : { ok: false, reason: "render compiler worker failed before second render" };
      const shutdownReport = session.shutdown();
      const sharedTopologyStress = await runSharedTopologyStress(module);
      return {
        ok: Boolean(
          firstCompileRequest.ok
          && renderCompiler.ok
          && renderCompiler.requestId === firstCompileRequest.requestId
          && firstReport.ok
          && firstReport.workerCompileUsed
          && secondCompileRequest.ok
          && secondRenderCompiler.ok
          && secondRenderCompiler.requestId === secondCompileRequest.requestId
          && report.ok
          && report.workerCompileUsed
          && report.worldgenMailboxKind === "web-worker"
          && report.lightStatusMailboxKind === "web-worker"
          && Number(report.worldgenMailboxPendingJobs) === 0
          && Number(report.lightStatusMailboxPendingStatuses) === 0
          && frameMetricsActive(report.runnerFrameMetrics, "shared-memory")
          && frameMetricsActive(report.worldgenJobFrameMetrics, "shared-memory")
          && frameMetricsActive(report.lightStatusJobFrameMetrics, "shared-memory")
          && sharedBufferPoolActive(report.runnerFrameMetrics)
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
        ),
        supported: true,
        status: report.ok ? "rendered" : "failed",
        firstCompileRequest,
        renderCompiler,
        secondCompileRequest,
        secondRenderCompiler,
        renderCompilerPendingJobCount: compiler.pendingJobCount(),
        sessionPendingCompileJobCount: session.pendingChunkRenderCompileJobCount(),
        firstReport,
        report,
        shutdownReport,
        sharedTopologyStress,
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
    this.assetPack = assetPack;
    this.pending = new Map();
    this.worker = new Worker(RENDER_COMPILER_WORKER_URL.href, {
      type: "module",
      name: "mclone-render-compiler-smoke",
    });
    this.worker.onmessage = (event) => {
      const data = event.data ?? {};
      const requestId = Number(data.requestId) || 0;
      const pending = this.pending.get(requestId);
      if (!pending) return;
      this.pending.delete(requestId);
      clearTimeout(pending.timeout);
      const packed = data.packed instanceof Uint8Array ? data.packed : new Uint8Array();
      const packedByteLength = packed.byteLength;
      delete data.packed;
      pending.resolve({
        report: {
          ...data,
          packedByteLength,
        },
        packed,
      });
    };
    this.worker.onerror = (event) => {
      this.rejectAll(new Error(event.message || "render compiler worker failed"));
    };
  }

  compile(request) {
    return new Promise((resolve, reject) => {
      const requestId = Number(request.requestId) || 0;
      if (requestId <= 0) {
        reject(new Error(`invalid render compile request id ${String(request.requestId)}`));
        return;
      }
      const requestAssetPack = this.assetPack.slice();
      const timeout = setTimeout(() => {
        if (!this.pending.has(requestId)) return;
        this.pending.delete(requestId);
        reject(new Error(`timed out waiting for render compiler worker request ${requestId}`));
      }, 20_000);
      this.pending.set(requestId, { resolve, reject, timeout });
      this.worker.postMessage(
        {
          kind: "compile-render-sections",
          requestId,
          bindgenJsUrl: BINDGEN_JS_URL.href,
          bindgenWasmUrl: BINDGEN_WASM_URL.href,
          assetPack: requestAssetPack,
          centerX: request.centerX,
          centerZ: request.centerZ,
          radiusChunks: request.radiusChunks,
        },
        [requestAssetPack.buffer],
      );
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
