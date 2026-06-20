const BINDGEN_JS_URL = new URL("./pkg/mclone_web_client.js", import.meta.url);
const BINDGEN_WASM_URL = new URL("./pkg/mclone_web_client_bg.wasm", import.meta.url);
const RENDER_COMPILER_WORKER_URL = new URL("./mclone-render-compiler-worker.js", import.meta.url);
const ASSET_PACK_URL = new URL("/reference/minecraft-1.17.1/extracted.zip", import.meta.url);

const RADIUS_CHUNKS = 1;

const runtime = {
  ready: false,
  state: {
    ok: false,
    ready: false,
    failed: false,
    centerX: 0,
    centerZ: 0,
    radiusChunks: RADIUS_CHUNKS,
    loadedChunkCount: 0,
    residentSectionCount: 0,
    pendingCompileJobCount: 0,
    renderCount: 0,
    frameCount: 0,
    status: "booting",
  },
};

globalThis.__mcloneWebApp = runtime;

async function boot() {
  const app = new WebChunkApp();
  try {
    await app.init();
    runtime.ready = true;
    runtime.state.ready = true;
    runtime.state.ok = true;
    runtime.state.failed = false;
    runtime.state.status = "ready";
    app.start();
    return snapshotState();
  } catch (error) {
    runtime.ready = false;
    runtime.state.ok = false;
    runtime.state.failed = true;
    runtime.state.status = stringifyError(error);
    updateDom();
    return snapshotState();
  }
}

class WebChunkApp {
  constructor() {
    this.canvas = document.getElementById("mclone-canvas");
    this.status = document.getElementById("status");
    this.session = null;
    this.compiler = null;
    this.pending = false;
    this.queuedCenter = null;
    this.animationFrame = 0;
  }

  async init() {
    if (!(this.canvas instanceof HTMLCanvasElement)) {
      throw new Error("missing canvas#mclone-canvas");
    }
    this.canvas.focus();

    runtime.state.status = "loading wasm";
    updateDom();
    const module = await import(BINDGEN_JS_URL.href);
    await module.default(BINDGEN_WASM_URL.href);
    if (typeof module.mclone_web_create_chunk_render_session !== "function") {
      throw new Error("missing mclone_web_create_chunk_render_session export");
    }

    runtime.state.status = "loading assets";
    updateDom();
    const assetPack = await fetchAssetPack();
    runtime.state.status = "initializing webgpu";
    updateDom();
    this.session = await module.mclone_web_create_chunk_render_session(this.canvas, assetPack);
    this.compiler = new RenderSectionWorkerCompiler(assetPack);
    bindInput(this);
    runtime.state.status = "rendering 0, 0";
    updateDom();
    await this.renderCenter(0, 0);
  }

  start() {
    const frame = () => {
      runtime.state.frameCount += 1;
      updateDom();
      this.animationFrame = requestAnimationFrame(frame);
    };
    this.animationFrame = requestAnimationFrame(frame);
  }

  moveBy(dx, dz) {
    this.queueCenter(runtime.state.centerX + dx, runtime.state.centerZ + dz);
  }

  queueCenter(centerX, centerZ) {
    if (centerX === runtime.state.centerX && centerZ === runtime.state.centerZ) {
      return;
    }
    this.queuedCenter = { centerX, centerZ };
    void this.drainQueue();
  }

  async drainQueue() {
    if (this.pending || !this.queuedCenter) {
      return;
    }
    const next = this.queuedCenter;
    this.queuedCenter = null;
    try {
      await this.renderCenter(next.centerX, next.centerZ);
    } catch (error) {
      console.error(error);
    }
    if (this.queuedCenter) {
      void this.drainQueue();
    }
  }

  async renderCenter(centerX, centerZ) {
    if (!this.session || !this.compiler) {
      throw new Error("web chunk app is not initialized");
    }
    this.pending = true;
    runtime.state.status = `loading ${centerX}, ${centerZ}`;
    runtime.state.pendingCompileJobCount = this.session.pendingChunkRenderCompileJobCount();
    updateDom();

    try {
      const request = this.session.beginChunkRenderCompileRequest(centerX, centerZ, RADIUS_CHUNKS);
      if (!request?.ok) {
        throw new Error(request?.reason ?? "failed to begin render compile request");
      }
      runtime.state.pendingCompileJobCount = request.pendingCompileJobCount ?? 1;
      updateDom();

      const compiled = await this.compiler.compile(request);
      if (!compiled.report?.ok) {
        throw new Error(compiled.report?.reason ?? "render compiler worker failed");
      }

      const report = this.session.finishChunkRenderCompileRequest(request.requestId, compiled.packed);
      if (!report?.ok) {
        throw new Error(report?.reason ?? "failed to render compiled chunk sections");
      }
      runtime.state.ok = true;
      runtime.state.centerX = report.centerX;
      runtime.state.centerZ = report.centerZ;
      runtime.state.radiusChunks = report.radiusChunks;
      runtime.state.loadedChunkCount = report.loadedChunkCount;
      runtime.state.residentSectionCount = report.residentSectionCount;
      runtime.state.pendingCompileJobCount = report.pendingCompileJobCount;
      runtime.state.renderCount = report.renderCount;
      runtime.state.status = "ready";
      runtime.state.lastReport = report;
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      throw error;
    } finally {
      this.pending = false;
      updateDom();
    }
  }
}

function bindInput(app) {
  window.addEventListener("keydown", (event) => {
    const move = keyMove(event.key);
    if (!move) return;
    event.preventDefault();
    if (event.repeat) return;
    app.moveBy(move.dx, move.dz);
  });

  for (const button of document.querySelectorAll("button[data-dx][data-dz]")) {
    button.addEventListener("click", () => {
      app.moveBy(
        Number.parseInt(button.dataset.dx ?? "0", 10),
        Number.parseInt(button.dataset.dz ?? "0", 10),
      );
      app.canvas?.focus();
    });
  }
}

function keyMove(key) {
  switch (key) {
    case "ArrowUp":
    case "w":
    case "W":
      return { dx: 0, dz: -1 };
    case "ArrowDown":
    case "s":
    case "S":
      return { dx: 0, dz: 1 };
    case "ArrowLeft":
    case "a":
    case "A":
      return { dx: -1, dz: 0 };
    case "ArrowRight":
    case "d":
    case "D":
      return { dx: 1, dz: 0 };
    default:
      return null;
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
      name: "mclone-render-compiler-app",
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

  rejectAll(error) {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pending.clear();
  }
}

function updateDom() {
  const state = runtime.state;
  setText("center", `${state.centerX}, ${state.centerZ}`);
  setText("chunks", String(state.loadedChunkCount));
  setText("sections", String(state.residentSectionCount));
  setText("pending", String(state.pendingCompileJobCount));
  setText("frames", String(state.frameCount));
  const status = document.getElementById("status");
  if (status) {
    status.textContent = state.status;
    status.dataset.ok = state.ok ? "true" : "false";
  }
}

function setText(id, value) {
  const element = document.getElementById(id);
  if (element) element.textContent = value;
}

function snapshotState() {
  return JSON.parse(JSON.stringify(runtime.state));
}

function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}

globalThis.__mcloneNativeAppReady = boot();
