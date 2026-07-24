import { fetchAssetPack } from "./mclone-render-compiler-shared.js";
import { PolledWorkerTransport } from "./mclone-worker-transport.js";
import {
  WORLD_CHUNK_STORE,
  WORLD_CATALOG_STORE,
  WORLD_ENTITY_CHUNK_STORE,
  WORLD_ID_INDEX,
  executeIndexedDbCatalogExecution,
  openWorldDb,
} from "./mclone-web-world-catalog.js";

/**
 * The wasm-bindgen module namespace (generated `.d.ts`, emitted by `wasm-bindgen --typescript`).
 * Loaded at runtime via a dynamic `import()` of a versioned URL; the bare specifier is path-mapped
 * in tsconfig.json and only ever appears in type positions.
 * @typedef {typeof import("mclone-web-client-wasm")} WasmModule
 * @typedef {import("mclone-web-client-wasm").WebSceneHost} WebSceneHost
 */

/**
 * A wasm-return per-frame overview report / diagnostic metrics bag, read coercion-guarded
 * (`Number(...)`/`Boolean(...)`); the underlying wasm exports are typed `any` in the `.d.ts`.
 * @typedef {Record<string, any>} RenderFrameReport
 */

const WASM_URL = new URL("./mclone_web_client.wasm", import.meta.url);
const BINDGEN_JS_URL = new URL("./pkg/mclone_web_client.js", import.meta.url);
const BINDGEN_WASM_URL = new URL("./pkg/mclone_web_client_bg.wasm", import.meta.url);
const THREAD_WORKER_URL = new URL("./mclone-thread-smoke-worker.js", import.meta.url);
const RENDER_COMPILER_WORKER_URL = new URL("./mclone-render-compiler-worker.js", import.meta.url);
const SERVER_WORKER_URL = new URL("./mclone-integrated-server-worker.js", import.meta.url);
const SERVER_JOB_WORKER_URL = new URL("./mclone-server-job-worker.js", import.meta.url);
const RUNTIME_SMOKE_EXPORT = "mclone_web_runtime_smoke_report";
const SCENE_ADAPTER_CONTRACT_EXPORT = "mclone_web_scene_adapter_contract_report";
const REMOTE_WS_URL = new URL(globalThis.location.href).searchParams.get("remoteWsUrl") ?? "";
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
  /**
   * @type {{ ok: boolean, crossOriginIsolated: boolean, sharedArrayBuffer: boolean,
   *   wasmSharedMemory: boolean, workerConstructor: boolean, atomics: boolean,
   *   workerRoundtrip: boolean, initialValue: number, finalValue: number, worker?: any }}
   */
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

/**
 * @param {WebAssembly.Memory} memory
 * @param {number} addend
 * @returns {Promise<any>}
 */
function runThreadWorker(memory, addend) {
  return new Promise((resolve, reject) => {
    const transport = new PolledWorkerTransport(
      THREAD_WORKER_URL,
      "mclone-thread-smoke",
    );
    let settled = false;
    const timeout = setTimeout(() => {
      if (settled) return;
      settled = true;
      transport.terminate();
      reject(new Error("timed out waiting for thread smoke worker"));
    }, 5_000);

    const poll = () => {
      if (settled) return;
      const event = transport.poll();
      if (event === null) {
        setTimeout(poll, 0);
        return;
      }
      settled = true;
      clearTimeout(timeout);
      transport.terminate();
      if (event.kind === "error") {
        reject(new Error(event.message));
      } else {
        resolve(event.data);
      }
    };
    transport.post({
      kind: "mclone-thread-smoke",
      memory,
      addend,
    });
    poll();
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
    const sceneAdapterContract = instance.exports[SCENE_ADAPTER_CONTRACT_EXPORT];
    if (typeof sceneAdapterContract !== "function") {
      return {
        ok: false,
        reason: `missing wasm export ${SCENE_ADAPTER_CONTRACT_EXPORT}`,
        exports: Object.keys(instance.exports),
      };
    }

    const bits = Number(runtimeSmoke()) >>> 0;
    const report = decodeRuntimeReport(bits);
    const sceneAdapter = decodeSceneAdapterContract(
      Number(sceneAdapterContract()) >>> 0,
    );
    return {
      ok: report.ok && sceneAdapter.ok,
      exportName: RUNTIME_SMOKE_EXPORT,
      report,
      sceneAdapter,
    };
  } catch (error) {
    return {
      ok: false,
      reason: stringifyError(error),
    };
  }
}

/**
 * @param {WebAssembly.Module} module
 * @returns {WebAssembly.Imports}
 */
function createWasmImports(module) {
  /** @type {WebAssembly.Imports} */
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

/** @param {WebAssembly.ModuleImportDescriptor} descriptor */
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

/** @param {GPUAdapter & { requestAdapterInfo?: () => Promise<any>, info?: any }} adapter */
async function readAdapterInfo(adapter) {
  try {
    if (typeof adapter.requestAdapterInfo === "function") {
      return await adapter.requestAdapterInfo();
    }
  } catch {}

  return adapter.info ?? {};
}

/**
 * Execute Rust-authored resource requests without assigning resource roles in
 * the browser harness.
 * @param {WasmModule} module
 * @param {{ browserPlan: () => any }} startup
 */
async function fetchBootstrapResources(module, startup) {
  const plan = startup.browserPlan();
  if (!Array.isArray(plan?.resources)) {
    throw new Error("browser bootstrap plan has no resource requests");
  }
  const fetched = await Promise.all(plan.resources.map(async (
    /** @type {{ requestId?: unknown, url?: unknown }} */ request,
  ) => {
    const requestId = Number(request?.requestId);
    const url = typeof request?.url === "string" ? request.url : "";
    if (!Number.isInteger(requestId) || requestId < 0 || url.length === 0) {
      throw new Error("invalid browser bootstrap resource request");
    }
    return {
      requestId,
      bytes: await fetchAssetPack(new URL(url, import.meta.url)),
    };
  }));
  const resources = new module.WebBootstrapResources();
  for (const response of fetched) {
    resources.add(response.requestId, response.bytes);
  }
  return resources;
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
    const module = /** @type {WasmModule} */ (await import(BINDGEN_JS_URL.href));
    await module.default(BINDGEN_WASM_URL.href);
    if (
      typeof module.mclone_web_create_scene_host_with_startup !== "function"
      || typeof module.WebBootstrapResources !== "function"
      || typeof module.WebHostCapabilities !== "function"
    ) {
      return {
        ok: false,
        supported: true,
        status: "export-missing",
        reason: "missing unified browser bootstrap export",
        exports: Object.keys(module),
      };
    }

    const renderWorkerTransportFactory = () => new PolledWorkerTransport(
      RENDER_COMPILER_WORKER_URL,
      "mclone-render-compiler-smoke",
    );
    const startup = module.mclone_web_startup_options_from_query(
      "?startInWorld=true&renderDistance=1&movementMode=fly",
    );
    const resources = await fetchBootstrapResources(module, startup);
    const session = await module.mclone_web_create_scene_host_with_startup(
      canvas,
      resources,
      browserHostCapabilities(module, canvas),
      startup,
      SERVER_WORKER_URL.href,
      SERVER_JOB_WORKER_URL.href,
      BINDGEN_JS_URL.href,
      BINDGEN_WASM_URL.href,
      renderWorkerTransportFactory,
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
        reason: "missing WebSceneHost streaming/shutdown export",
      };
    }

    try {
      // 067 Stage 3: pump the shared streaming loop to idle for two overview centers
      // (the web analog of desktop `sync_all_render_sections`). Each frame may both apply
      // the previous compile and arm the next; JS only relays the doorbell, the Rust loop
      // owns scheduling/coalescing/acceptance. Center (1,0) reuses the resident worker mesh
      // catalog + ring, so it incrementally streams the shifted view on top of (0,0).
      const firstCenter = await streamOverviewToIdle(session, 0, 0, 1);
      const secondCenter = await streamOverviewToIdle(session, 1, 0, 1);
      const firstReport = firstCenter.report;
      const report = secondCenter.report;
      const shutdownReport = session.shutdown();
      const indexedDbPersistence = await runIndexedDbPersistenceSmoke(
        module,
        canvas,
      );
      const indexedDbCatalog = await runIndexedDbCatalogSmoke(module);
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
          && sharedBufferPoolUsed(report.worldgenJobFrameMetrics)
          && sharedBufferPoolUsed(report.lightStatusJobFrameMetrics)
          && report.rendered
          && report.configured
          && report.chunkLoaded
          && report.meshBuilt
          && report.assetPackLoaded
          && report.textured
          && shutdownReport.ok
          && indexedDbPersistence.ok
          && indexedDbCatalog.ok
          && sharedTopologyStressActive(sharedTopologyStress)
          && remoteWebSocketActive(remoteWebSocket)
        ),
        supported: true,
        status: report.ok ? "rendered" : "failed",
        firstCenter: publicOverviewCenter(firstCenter),
        secondCenter: publicOverviewCenter(secondCenter),
        renderCompilerPendingJobCount: Number(report.renderWorkerPendingRequestCount) || 0,
        sessionPendingCompileJobCount: session.pendingChunkRenderCompileJobCount(),
        firstReport,
        report,
        shutdownReport,
        indexedDbPersistence,
        indexedDbCatalog,
        sharedTopologyStress,
        remoteWebSocket,
      };
    } finally {
      try {
        session.shutdown();
      } catch {}
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

/**
 * @param {WasmModule} module
 * @param {HTMLCanvasElement} canvas
 */
async function runIndexedDbPersistenceSmoke(module, canvas) {
  if (typeof module.mclone_web_create_scene_host_with_startup !== "function") {
    return {
      ok: false,
      reason: "missing mclone_web_create_scene_host_with_startup export",
    };
  }
  const worldId = `smoke-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;
  const first = await createIndexedDbSmokeSession(
    module, canvas, worldId, true,
  );
  if (typeof first.shutdown !== "function") {
    return {
      ok: false,
      reason: "missing WebSceneHost.shutdown export",
    };
  }
  try {
    const firstCenter = await streamOverviewToIdle(first, 0, 0, 1);
    const firstShutdown = first.shutdown();
    const afterFirst = await waitForIndexedDbWorldRecords(worldId, 1);
    const second = await createIndexedDbSmokeSession(
      module, canvas, worldId, false,
    );
    if (typeof second.shutdown !== "function") {
      return {
        ok: false,
        reason: "missing WebSceneHost.shutdown export on restart",
      };
    }
    try {
      const secondCenter = await streamOverviewToIdle(second, 0, 0, 1);
      const secondShutdown = second.shutdown();
      const afterSecond = await waitForIndexedDbWorldRecords(worldId, afterFirst.chunks);
      return {
        ok: Boolean(
          firstCenter.settled
          && secondCenter.settled
          && firstShutdown?.ok
          && secondShutdown?.ok
          && afterFirst.chunks > 0
          && afterSecond.chunks >= afterFirst.chunks
        ),
        worldId,
        afterFirst,
        afterSecond,
        firstShutdown,
        secondShutdown,
        firstCenter: publicOverviewCenter(firstCenter),
        secondCenter: publicOverviewCenter(secondCenter),
      };
    } finally {
      try {
        second.shutdown();
      } catch {}
    }
  } finally {
    try {
      first.shutdown();
    } catch {}
  }
}

/**
 * @param {WasmModule} module
 * @param {HTMLCanvasElement} canvas
 * @param {string} worldId
 * @param {boolean} clearWorldStorage
 */
async function createIndexedDbSmokeSession(
  module, canvas, worldId, clearWorldStorage,
) {
  const startup = module.mclone_web_startup_options_from_query(
    `?startInWorld=true&seed=424242&renderDistance=1&worldStorage=indexeddb&worldId=${encodeURIComponent(worldId)}&clearWorldStorage=${clearWorldStorage}`,
  );
  const resources = await fetchBootstrapResources(module, startup);
  return await module.mclone_web_create_scene_host_with_startup(
    canvas,
    resources,
    browserHostCapabilities(module, canvas),
    startup,
    SERVER_WORKER_URL.href,
    SERVER_JOB_WORKER_URL.href,
    BINDGEN_JS_URL.href,
    BINDGEN_WASM_URL.href,
    () => new PolledWorkerTransport(
      RENDER_COMPILER_WORKER_URL,
      "mclone-render-compiler-indexeddb-smoke",
    ),
  );
}

/**
 * @param {WasmModule} module
 * @param {HTMLCanvasElement} canvas
 */
function browserHostCapabilities(module, canvas) {
  const touchInputAvailable = navigator.maxTouchPoints > 0
    || window.matchMedia("(pointer: coarse)").matches;
  return new module.WebHostCapabilities(
    touchInputAvailable,
    canvas.getBoundingClientRect().width,
  );
}

/** @param {WasmModule} module */
async function runIndexedDbCatalogSmoke(module) {
  const worldId = `catalog-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;
  const legacyWorldId = `${worldId}-legacy`;
  const constraintWorldId = `${worldId}-constraint`;
  const nonResurrectionWorldId = `${worldId}-non-resurrection`;
  const db = await openWorldDb();
  /** @type {Record<string, number>} */
  const operationTimingsMs = {};
  /**
   * @param {string} label
   * @param {string} operation
   * @param {Record<string, unknown>} [options]
   * @param {string} [activeWorldId]
   */
  const timedCatalogOperation = async (
    label,
    operation,
    options = {},
    activeWorldId = "",
  ) => {
    const startedAt = performance.now();
    try {
      return await executeCatalogSmoke(
        module,
        db,
        operation,
        options,
        activeWorldId,
      );
    } finally {
      operationTimingsMs[label] = performance.now() - startedAt;
    }
  };
  try {
    const before = await timedCatalogOperation("listBefore", "listWorlds");
    const legacyTransaction = db.transaction(WORLD_CATALOG_STORE, "readwrite");
    legacyTransaction.objectStore(WORLD_CATALOG_STORE).add({
      id: legacyWorldId,
      displayName: "Legacy Clear Catalog World",
      seed: 17,
      generationProfile: "flat-grass-v1",
      createdUnixMillis: Date.now() - 1,
      lastPlayedUnixMillis: null,
      storageSchemaVersion: 1,
      targetMinecraftVersion: "1.17.1",
      mcloneVersion: null,
      backendLabel: "web-indexeddb",
      locked: false,
      compatible: true,
    });
    await transactionDone(legacyTransaction);
    const legacyListed = (await executeCatalogSmoke(module, db, "listWorlds"))
      .some((/** @type {any} */ world) => world.id === legacyWorldId);
    const legacyOpened = await executeCatalogSmoke(
      module,
      db,
      "openWorld",
      { id: legacyWorldId },
    );
    const legacyReadTransaction = db.transaction(WORLD_CATALOG_STORE, "readonly");
    const legacyUpgradedRecord = await new Promise((resolve, reject) => {
      const request = legacyReadTransaction
        .objectStore(WORLD_CATALOG_STORE)
        .get(legacyWorldId);
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error ?? new Error("failed to read legacy catalog row"));
    });
    await transactionDone(legacyReadTransaction);
    const legacyUpgraded = Boolean(
      legacyUpgradedRecord
      && legacyUpgradedRecord.descriptor instanceof Uint8Array
      && !("displayName" in legacyUpgradedRecord),
    );
    await executeCatalogSmoke(module, db, "deleteWorld", { id: legacyWorldId });

    const created = await timedCatalogOperation("create", "createWorld", {
      displayName: "Smoke Catalog World",
      seed: 424242,
      requestedId: worldId,
    });
    const duplicateRejected = await rejectsWithMessage(
      () => executeCatalogSmoke(module, db, "createWorld", {
        displayName: "Duplicate Smoke Catalog World",
        seed: 424242,
        requestedId: worldId,
      }),
      "already exists",
    );
    const activeDeleteRejected = await rejectsWithMessage(
      () => executeCatalogSmoke(module, db, "deleteWorld", { id: worldId }, worldId),
      "cannot delete active local world",
    );

    // Pause one create after its read, insert the same id through an
    // independent execution, then resume it. The stale add must abort its
    // transaction, and the next ordinary operation must still succeed.
    const staleCreate = new module.WebCatalogSmokeExecution(
      "createWorld",
      {
        displayName: "Stale Constraint Probe",
        seed: 424242,
        requestedId: constraintWorldId,
      },
      "",
    );
    let constraintAbortError = "";
    let continuedAfterAbort = false;
    try {
      await advanceCatalogSmokeCreateRead(db, staleCreate);
      await executeCatalogSmoke(module, db, "createWorld", {
        displayName: "Constraint Winner",
        seed: 424242,
        requestedId: constraintWorldId,
      });
      try {
        await executeIndexedDbCatalogExecution(db, staleCreate);
      } catch (error) {
        constraintAbortError = stringifyError(error);
      }
      continuedAfterAbort = (await executeCatalogSmoke(module, db, "listWorlds"))
        .some((/** @type {any} */ world) => world.id === constraintWorldId);
      await executeCatalogSmoke(module, db, "deleteWorld", { id: constraintWorldId });
    } finally {
      staleCreate.free();
    }
    const constraintAbortRejected = /constraint|key already exists/i.test(
      constraintAbortError,
    );

    await executeCatalogSmoke(module, db, "createWorld", {
      displayName: "Non-resurrection Probe",
      seed: 424242,
      requestedId: nonResurrectionWorldId,
    });
    await Promise.all([
      executeCatalogSmoke(
        module,
        db,
        "recordWorldPlayed",
        { id: nonResurrectionWorldId },
      ),
      executeCatalogSmoke(
        module,
        db,
        "deleteWorld",
        { id: nonResurrectionWorldId },
      ),
    ]);
    const afterNonResurrectionRace = await executeCatalogSmoke(module, db, "listWorlds");
    const nonResurrection = afterNonResurrectionRace.every(
      (/** @type {any} */ world) => world.id !== nonResurrectionWorldId,
    );

    await putIndexedDbSmokeRecord(db, WORLD_CHUNK_STORE, worldId, 0, 0);
    await putIndexedDbSmokeRecord(db, WORLD_ENTITY_CHUNK_STORE, worldId, 0, 0);
    const recordsBeforeDelete = await countIndexedDbWorldRecords(worldId);

    const afterCreate = await timedCatalogOperation(
      "listAfterCreate",
      "listWorlds",
    );
    const opened = await timedCatalogOperation(
      "open",
      "openWorld",
      { id: worldId },
    );
    const recorded = await timedCatalogOperation(
      "recordPlayed",
      "recordWorldPlayed",
      { id: worldId },
    );
    const deleted = await timedCatalogOperation(
      "delete",
      "deleteWorld",
      { id: worldId },
    );
    const afterDelete = await timedCatalogOperation(
      "listAfterDelete",
      "listWorlds",
    );
    const openDeletedRejected = await rejectsWithMessage(
      () => executeCatalogSmoke(module, db, "openWorld", { id: worldId }),
      "was not found",
    );
    const recordsAfterDelete = await countIndexedDbWorldRecords(worldId);

    return {
      ok: Boolean(
        created.id === worldId
        && created.displayName === "Smoke Catalog World"
        && legacyListed
        && legacyOpened.id === legacyWorldId
        && legacyUpgraded
        && opened.id === worldId
        && Number(opened.lastPlayedUnixMillis) >= Number(created.lastPlayedUnixMillis)
        && Number(recorded.lastPlayedUnixMillis) > Number(opened.lastPlayedUnixMillis)
        && deleted.id === worldId
        && duplicateRejected
        && activeDeleteRejected
        && constraintAbortRejected
        && continuedAfterAbort
        && nonResurrection
        && openDeletedRejected
        && recordsBeforeDelete.chunks === 1
        && recordsBeforeDelete.entityChunks === 1
        && recordsAfterDelete.total === 0
        && afterCreate.some((/** @type {any} */ world) => world.id === worldId)
        && afterDelete.every((/** @type {any} */ world) => world.id !== worldId)
      ),
      worldId,
      beforeCount: before.length,
      afterCreateCount: afterCreate.length,
      afterDeleteCount: afterDelete.length,
      created,
      opened,
      recorded,
      deleted,
      legacyListed,
      legacyOpened,
      legacyUpgraded,
      recordsBeforeDelete,
      recordsAfterDelete,
      duplicateRejected,
      activeDeleteRejected,
      constraintAbortRejected,
      constraintAbortError,
      continuedAfterAbort,
      nonResurrection,
      openDeletedRejected,
      operationTimingsMs,
    };
  } finally {
    db.close();
  }
}

/**
 * Advance a smoke-only create continuation through its initial getAll so its
 * later add can be raced deterministically against another committed create.
 *
 * @param {IDBDatabase} db
 * @param {import("mclone-web-client-wasm").WebCatalogSmokeExecution} execution
 */
async function advanceCatalogSmokeCreateRead(db, execution) {
  const step = execution.nextStorageStep();
  const transactionPlan = step?.transactions?.[0];
  const action = transactionPlan?.actions?.[0];
  if (
    transactionPlan?.mode !== "readonly"
    || action?.kind !== "get-all"
    || action?.store !== "catalog"
  ) {
    throw new Error(`unexpected catalog create read plan ${JSON.stringify(step)}`);
  }
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readonly");
  const request = transaction.objectStore(WORLD_CATALOG_STORE).getAll();
  const rows = await new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("catalog create read failed"));
  });
  await transactionDone(transaction);
  execution.acceptStorageRead(step.stepId, action.actionId, rows, Date.now());
  execution.completeStorageStep(step.stepId);
}

/**
 * @param {WasmModule} module
 * @param {IDBDatabase} db
 * @param {string} operation
 * @param {Record<string, unknown>} [options]
 * @param {string} [activeWorldId]
 * @returns {Promise<any>}
 */
async function executeCatalogSmoke(
  module,
  db,
  operation,
  options = {},
  activeWorldId = "",
) {
  const execution = new module.WebCatalogSmokeExecution(
    operation,
    options,
    activeWorldId,
  );
  try {
    await executeIndexedDbCatalogExecution(db, execution);
    return execution.responseForSmoke();
  } finally {
    execution.free();
  }
}

/**
 * @param {() => Promise<unknown>} operation
 * @param {string} message
 * @returns {Promise<boolean>}
 */
async function rejectsWithMessage(operation, message) {
  try {
    await operation();
    return false;
  } catch (error) {
    return stringifyError(error).includes(message);
  }
}

/**
 * @param {IDBDatabase} db
 * @param {string} storeName
 * @param {string} worldId
 * @param {number} x
 * @param {number} z
 */
async function putIndexedDbSmokeRecord(db, storeName, worldId, x, z) {
  const transaction = db.transaction(storeName, "readwrite");
  transaction.objectStore(storeName).put({
    worldId,
    dimensionKey: "minecraft:overworld",
    x,
    z,
    record: new Uint8Array([1, 2, 3, 4]),
  });
  await transactionDone(transaction);
}

/** @param {string} worldId */
async function countIndexedDbWorldRecords(worldId) {
  const db = await openSmokeWorldDb();
  try {
    const [chunks, entityChunks] = await Promise.all([
      countIndexedDbStoreForWorld(db, WORLD_CHUNK_STORE, worldId),
      countIndexedDbStoreForWorld(db, WORLD_ENTITY_CHUNK_STORE, worldId),
    ]);
    return {
      chunks,
      entityChunks,
      total: chunks + entityChunks,
    };
  } finally {
    db.close();
  }
}

/**
 * @param {string} worldId
 * @param {number} minChunks
 */
async function waitForIndexedDbWorldRecords(worldId, minChunks) {
  const deadline = performance.now() + 10_000;
  let last = { chunks: 0, entityChunks: 0, total: 0 };
  while (performance.now() < deadline) {
    last = await countIndexedDbWorldRecords(worldId);
    if (last.chunks >= minChunks) {
      return last;
    }
    await yieldToEventLoop();
  }
  throw new Error(
    `IndexedDB world ${worldId} did not reach ${minChunks} chunk records: `
      + JSON.stringify(last),
  );
}

/** @returns {Promise<IDBDatabase>} */
function openSmokeWorldDb() {
  return openWorldDb();
}

/**
 * @param {IDBDatabase} db
 * @param {string} storeName
 * @param {string} worldId
 * @returns {Promise<number>}
 */
function countIndexedDbStoreForWorld(db, storeName, worldId) {
  return new Promise((resolve, reject) => {
    const transaction = db.transaction(storeName, "readonly");
    const request = transaction
      .objectStore(storeName)
      .index(WORLD_ID_INDEX)
      .count(IDBKeyRange.only(worldId));
    request.onsuccess = () => resolve(Number(request.result) || 0);
    request.onerror = () => reject(request.error ?? new Error(`failed to count ${storeName}`));
    transaction.onerror = () => reject(
      transaction.error ?? new Error(`failed to count ${storeName}`),
    );
  });
}

/**
 * @param {IDBTransaction} transaction
 * @returns {Promise<void>}
 */
function transactionDone(transaction) {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve(undefined);
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  });
}

// Drive the production shared host to idle at a fixed chunk center. The Rust
// runtime service and render-worker coordinator own admission and lifecycle;
// JavaScript observes only the frame report.
/**
 * @param {WebSceneHost} session
 * @param {number} centerX
 * @param {number} centerZ
 * @param {number} radius
 */
async function streamOverviewToIdle(session, centerX, centerZ, radius) {
  const deadline = performance.now() + 30_000;
  /** @type {RenderFrameReport | null} */
  let frame = null;
  const baseline = session.cameraFrameState();
  const compileCountBefore = Number(baseline?.lastCompileReport?.compileCount) || 0;
  let compileCount = compileCountBefore;
  let lastWorkerReport = baseline?.lastCompileReport ?? null;
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
    const workerReport = frame.lastCompileReport ?? null;
    const observedCompileCount = Number(workerReport?.compileCount) || 0;
    compileCount = Math.max(compileCount, observedCompileCount);
    if (workerReport) lastWorkerReport = workerReport;
    if (overviewRunnerBusy(frame) || compileCount > compileCountBefore) {
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
  // `settled` implies `overviewFrameSettled(frame)` was true, which requires a non-null frame.
  const settledFrame = /** @type {RenderFrameReport} */ (frame);
  return {
    centerX,
    centerZ,
    settled,
    compileCount: compileCount - compileCountBefore,
    residentSectionCount: Number(settledFrame.residentSectionCount) || 0,
    report: settledFrame,
    workerReport: lastWorkerReport,
  };
}

/** @returns {Promise<void>} */
function yieldToEventLoop() {
  if (typeof requestAnimationFrame === "function") {
    return new Promise(
      /** @param {(value?: void) => void} resolve */
      (resolve) => requestAnimationFrame(() => resolve()),
    );
  }
  return new Promise((resolve) => setTimeout(resolve, 0));
}

/** @param {RenderFrameReport | null} frame */
function overviewRunnerBusy(frame) {
  return Boolean(
    frame
    && (Number(frame.runnerCommandQueueDepth) > 0
      || Number(frame.runnerUpdateQueueDepth) > 0
      || Number(frame.runnerPendingJobs) > 0
      || Number(frame.runnerPendingPublications) > 0
      || Number(frame.runnerPendingPersistenceLoads) > 0
      || Number(frame.runnerPendingPersistenceSaves) > 0),
  );
}

/** @param {RenderFrameReport | null} frame */
function overviewFrameSettled(frame) {
  return Boolean(
    frame
    && frame.streamingIdle
    && Number(frame.runnerPendingJobs) === 0
    && Number(frame.runnerUpdateQueueDepth) === 0
    && Number(frame.runnerCommandQueueDepth) === 0
    && Number(frame.runnerPendingPublications) === 0
    && Number(frame.runnerPendingPersistenceLoads) === 0
    && Number(frame.runnerPendingPersistenceSaves) === 0
    && Number(frame.renderWorkerPendingRequestCount) === 0
    && Number(frame.residentSectionCount) > 0
  );
}

/**
 * @param {{ centerX: number, centerZ: number, settled: boolean, compileCount: number,
 *   residentSectionCount: number, workerReport: any }} center
 */
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

/** @param {WasmModule} module */
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

/** @param {WasmModule} module */
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

/** @param {number} bits */
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

/** @param {number} bits */
function decodeSceneAdapterContract(bits) {
  return {
    bits,
    ok: (bits & 0x1f) === 0x1f,
    monotonicClockClamped: (bits & 0x1) !== 0,
    sharedSessionCoordinator: (bits & 0x2) !== 0,
    staleOperationRejected: (bits & 0x4) !== 0,
    catalogCompletionTyped: (bits & 0x8) !== 0,
    catalogEpochRejectsLateCompletion: (bits & 0x10) !== 0,
  };
}

/**
 * @param {any} metrics
 * @param {string} transportKind
 */
function frameMetricsActive(metrics, transportKind) {
  return Boolean(
    metrics
    && metrics.transportKind === transportKind
    && Number(metrics.requestFrames) > 0
    && Number(metrics.requestBytes) > 0
    && Number(metrics.inboundFrames) > 0
    && Number(metrics.inboundBytes) > 0
  );
}

/** @param {any} metrics */
function sharedBufferPoolUsed(metrics) {
  return Boolean(
    metrics
    && Number(metrics.sharedBufferCapacityBytes) > 0
    && Number(metrics.maxSharedBufferCapacityBytes) >= Number(metrics.sharedBufferCapacityBytes)
    && Number(metrics.sharedBufferPooledInboundFrames) > 0
    && Number(metrics.sharedBufferFallbackInboundFrames) === 0
  );
}

/** @param {any} report */
function sharedTopologyStressActive(report) {
  return Boolean(
    report
    && report.ok
    && sharedRunnerStressActive(report.sharedRunner)
    && fallbackRunnerStressActive(report.fallbackRunner)
  );
}

/** @param {any} report */
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
    && Number(metrics.sharedBufferPooledInboundFrames) > 0
    && Number(metrics.sharedBufferFallbackInboundFrames) > 0
    && frameMetricsActive(report.worldgenJobFrameMetrics, "shared-memory")
    && frameMetricsActive(report.lightStatusJobFrameMetrics, "shared-memory")
    && report.shutdown
    && report.shutdown.running === false
  );
}

/** @param {any} report */
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
    && Number(metrics.sharedBufferPooledInboundFrames) === 0
    && Number(metrics.sharedBufferFallbackInboundFrames) === 0
    && report.shutdown
    && report.shutdown.running === false
  );
}

/** @param {any} report */
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
    && Number(metrics.inboundFrames) >= 3
  );
}

/** @param {any} result */
function renderStatus(result) {
  const status = document.getElementById("status");
  if (!status) return;
  status.textContent = JSON.stringify(result, null, 2);
  status.dataset.ok = result.ok ? "true" : "false";
}

/** @param {unknown} error */
function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
