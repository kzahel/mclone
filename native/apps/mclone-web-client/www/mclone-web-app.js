const BINDGEN_JS_URL = new URL("./pkg/mclone_web_client.js", import.meta.url);
const BINDGEN_WASM_URL = new URL("./pkg/mclone_web_client_bg.wasm", import.meta.url);
const RENDER_COMPILER_WORKER_URL = new URL("./mclone-render-compiler-worker.js", import.meta.url);
const SERVER_WORKER_URL = new URL("./mclone-integrated-server-worker.js", import.meta.url);
const ASSET_PACK_URL = new URL("/reference/minecraft-1.17.1/extracted.zip", import.meta.url);

const RADIUS_CHUNKS = 1;
const MAX_FRAME_DT_SECONDS = 0.05;

const runtime = {
  ready: false,
  state: {
    ok: false,
    ready: false,
    failed: false,
    centerX: 0,
    centerZ: 0,
    loadedCenterX: null,
    loadedCenterZ: null,
    radiusChunks: RADIUS_CHUNKS,
    cameraX: 0,
    cameraY: 0,
    cameraZ: 0,
    cameraYawRadians: 0,
    cameraPitchRadians: 0,
    cameraSpeedBlocksPerSecond: 0,
    movementMode: "WALK",
    onGround: false,
    horizontalCollision: false,
    verticalCollision: false,
    selectedHotbarSlot: 0,
    currentTarget: null,
    interactionCount: 0,
    interactionStatus: "idle",
    lastInteraction: null,
    width: 0,
    height: 0,
    dayTime: 0,
    timeOfDay: 0,
    skyRendered: false,
    actorCount: 0,
    drawnActorCount: 0,
    loadedChunkCount: 0,
    residentSectionCount: 0,
    pendingCompileJobCount: 0,
    renderCount: 0,
    frameCount: 0,
    pointerLockSupported: false,
    pointerLockAttempted: false,
    pointerLocked: false,
    pointerLockFallback: false,
    tickFrameBusy: false,
    tickPhase: "idle",
    runnerKind: "unknown",
    runnerCommandQueueDepth: 0,
    runnerUpdateQueueDepth: 0,
    runnerPendingJobs: 0,
    runnerPendingPublications: 0,
    status: "booting",
  },
};

globalThis.__mcloneWebApp = runtime;

async function boot() {
  const app = new WebChunkApp();
  runtime.queueMouseDelta = (dx, dy) => app.queueMouseDelta(dx, dy);
  runtime.setInputKey = (name, down) => app.setInputKey(name, down);
  runtime.previewBlockTarget = () => runtime.state.currentTarget;
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
    this.keys = {
      forward: false,
      backward: false,
      left: false,
      right: false,
      jump: false,
      descend: false,
      shift: false,
      sprint: false,
    };
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.pendingCompile = false;
    this.renderFrameBusy = false;
    this.hasRendered = false;
    this.loadedCenter = null;
    this.pointerDragging = false;
    this.pointerDown = null;
    this.animationFrame = 0;
    this.lastFrameTime = 0;
    this.tickFrameBusy = false;
    this.sessionBusy = false;
  }

  async init() {
    if (!(this.canvas instanceof HTMLCanvasElement)) {
      throw new Error("missing canvas#mclone-canvas");
    }
    runtime.state.pointerLockSupported = typeof this.canvas.requestPointerLock === "function";
    this.canvas.focus();

    runtime.state.status = "loading wasm";
    updateDom();
    const module = await import(BINDGEN_JS_URL.href);
    await module.default(BINDGEN_WASM_URL.href);
    for (const name of ["mclone_web_create_worker_chunk_render_session"]) {
      if (typeof module[name] !== "function") {
        throw new Error(`missing ${name} export`);
      }
    }

    runtime.state.status = "loading assets";
    updateDom();
    const assetPack = await fetchAssetPack();
    runtime.state.status = "initializing webgpu";
    updateDom();
    this.session = await module.mclone_web_create_worker_chunk_render_session(
      this.canvas,
      assetPack,
      SERVER_WORKER_URL.href,
      BINDGEN_JS_URL.href,
      BINDGEN_WASM_URL.href,
    );
    for (const name of [
      "advanceCameraFrame",
      "beginCameraRenderCompileRequest",
      "finishCameraRenderCompileRequest",
      "renderCameraFrame",
      "cameraFrameState",
      "resizeCanvas",
      "toggleMovementMode",
      "selectHotbarSlot",
      "previewBlockTarget",
      "interactBlock",
    ]) {
      if (typeof this.session[name] !== "function") {
        throw new Error(`missing WebChunkRenderSession.${name} export`);
      }
    }
    this.compiler = new RenderSectionWorkerCompiler(assetPack);
    bindInput(this);
    this.syncCanvasSize();
    this.applyCameraState(this.session.cameraFrameState());
    this.applyTargetState(this.session.previewBlockTarget());

    runtime.state.status = "rendering";
    updateDom();
    await this.compileCameraView();
  }

  start() {
    this.lastFrameTime = performance.now();
    const frame = (now) => {
      if (!this.tickFrameBusy) {
        this.tickFrameBusy = true;
        runtime.state.tickFrameBusy = true;
        void this.tickFrame(now).finally(() => {
          this.tickFrameBusy = false;
          runtime.state.tickFrameBusy = false;
          runtime.state.tickPhase = "idle";
        });
      }
      this.animationFrame = requestAnimationFrame(frame);
    };
    this.animationFrame = requestAnimationFrame(frame);
  }

  async tickFrame(now) {
    if (!this.session) {
      return;
    }
    const rawDt = (now - this.lastFrameTime) / 1000;
    this.lastFrameTime = now;
    const dtSeconds = Number.isFinite(rawDt)
      ? Math.max(0, Math.min(rawDt, MAX_FRAME_DT_SECONDS))
      : 0;
    const mouseDeltaX = this.mouseDeltaX;
    const mouseDeltaY = this.mouseDeltaY;
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.syncCanvasSize();

    runtime.state.frameCount += 1;
    runtime.state.tickPhase = "advance";
    const camera = await this.withSessionAsync(() => this.session.advanceCameraFrame(
      dtSeconds,
      mouseDeltaX,
      mouseDeltaY,
      this.keys.forward,
      this.keys.backward,
      this.keys.left,
      this.keys.right,
      this.keys.jump,
      this.keys.descend,
      this.keys.shift,
      this.keys.sprint,
    ));
    runtime.state.tickPhase = "post-advance";
    this.applyCameraState(camera);
    this.applyTargetState(this.session.previewBlockTarget());

    if (this.hasRendered && this.loadedCenter && !this.pendingCompile) {
      if (
        camera.centerX !== this.loadedCenter.centerX
        || camera.centerZ !== this.loadedCenter.centerZ
      ) {
        runtime.state.tickPhase = "compile-requested";
        await this.compileCameraView();
      }
    }

    if (this.hasRendered && !this.renderFrameBusy) {
      runtime.state.tickPhase = "render";
      this.renderCameraFrame();
    } else {
      updateDom();
    }
  }

  async compileCameraView() {
    if (!this.session || !this.compiler || this.pendingCompile) {
      return;
    }
    this.pendingCompile = true;
    runtime.state.status = "compiling";
    runtime.state.pendingCompileJobCount = this.session.pendingChunkRenderCompileJobCount();
    updateDom();

    try {
      const request = await this.withSessionAsync(() => (
        this.session.beginCameraRenderCompileRequest(RADIUS_CHUNKS)
      ));
      if (!request?.ok) {
        throw new Error(request?.reason ?? "failed to begin camera render compile request");
      }
      runtime.state.pendingCompileJobCount = request.pendingCompileJobCount ?? 1;
      updateDom();

      const compiled = await this.compiler.compile(request);
      if (!compiled.report?.ok) {
        throw new Error(compiled.report?.reason ?? "render compiler worker failed");
      }

      const report = this.session.finishCameraRenderCompileRequest(
        request.requestId,
        compiled.packed,
      );
      if (!report?.ok) {
        throw new Error(report?.reason ?? "failed to finish camera render compile request");
      }
      this.hasRendered = true;
      this.loadedCenter = { centerX: report.centerX, centerZ: report.centerZ };
      runtime.state.loadedCenterX = report.centerX;
      runtime.state.loadedCenterZ = report.centerZ;
      runtime.state.lastCompileReport = report;
      this.applyReport(report);
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      throw error;
    } finally {
      this.pendingCompile = false;
      runtime.state.pendingCompileJobCount = this.session?.pendingChunkRenderCompileJobCount?.() ?? 0;
      updateDom();
    }
  }

  renderCameraFrame() {
    if (!this.session || !this.hasRendered) {
      return;
    }
    this.renderFrameBusy = true;
    try {
      const report = this.session.renderCameraFrame(RADIUS_CHUNKS);
      if (!report?.ok) {
        throw new Error(report?.reason ?? "failed to render camera frame");
      }
      this.applyReport(report);
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
    } finally {
      this.renderFrameBusy = false;
      updateDom();
    }
  }

  applyCameraState(camera) {
    if (!camera?.ok) {
      return;
    }
    runtime.state.centerX = Number(camera.centerX) || 0;
    runtime.state.centerZ = Number(camera.centerZ) || 0;
    runtime.state.cameraX = Number(camera.cameraX) || 0;
    runtime.state.cameraY = Number(camera.cameraY) || 0;
    runtime.state.cameraZ = Number(camera.cameraZ) || 0;
    runtime.state.cameraYawRadians = Number(camera.cameraYawRadians) || 0;
    runtime.state.cameraPitchRadians = Number(camera.cameraPitchRadians) || 0;
    runtime.state.cameraSpeedBlocksPerSecond = Number(camera.cameraSpeedBlocksPerSecond) || 0;
    runtime.state.movementMode = String(camera.movementMode || runtime.state.movementMode || "WALK");
    runtime.state.onGround = Boolean(camera.onGround);
    runtime.state.horizontalCollision = Boolean(camera.horizontalCollision);
    runtime.state.verticalCollision = Boolean(camera.verticalCollision);
    applyHotbarState(camera);
  }

  applyReport(report) {
    this.applyCameraState(report);
    runtime.state.ok = true;
    runtime.state.radiusChunks = report.radiusChunks;
    runtime.state.width = report.width;
    runtime.state.height = report.height;
    runtime.state.dayTime = report.dayTime;
    runtime.state.timeOfDay = report.timeOfDay;
    runtime.state.skyRendered = Boolean(report.skyRendered);
    runtime.state.actorCount = report.actorCount;
    runtime.state.drawnActorCount = report.drawnActorCount;
    runtime.state.loadedChunkCount = report.loadedChunkCount;
    runtime.state.residentSectionCount = report.residentSectionCount;
    runtime.state.pendingCompileJobCount = report.pendingCompileJobCount;
    runtime.state.runnerKind = report.runnerKind;
    runtime.state.runnerCommandQueueDepth = report.runnerCommandQueueDepth;
    runtime.state.runnerUpdateQueueDepth = report.runnerUpdateQueueDepth;
    runtime.state.runnerPendingJobs = report.runnerPendingJobs;
    runtime.state.runnerPendingPublications = report.runnerPendingPublications;
    runtime.state.renderCount = report.renderCount;
    runtime.state.status = "ready";
    runtime.state.lastReport = report;
  }

  applyTargetState(target) {
    if (!target?.ok) {
      return;
    }
    runtime.state.currentTarget = target;
    applyHotbarState(target);
  }

  async interactBlock(action) {
    if (!this.session) {
      return null;
    }
    try {
      await this.waitForSessionIdle();
      const interaction = await this.withSessionAsync(() => this.session.interactBlock(action));
      if (!interaction?.ok) {
        return null;
      }
      runtime.state.interactionCount += 1;
      runtime.state.lastInteraction = interaction;
      runtime.state.interactionStatus = formatInteractionStatus(interaction);
      applyHotbarState(interaction);
      if (interaction.changed && this.hasRendered && !this.pendingCompile) {
        await this.compileCameraView();
      }
      updateDom();
      return interaction;
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
      updateDom();
      return null;
    }
  }

  selectHotbarSlot(slot) {
    if (!this.session) {
      return false;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.selectHotbarSlot(slot), 0);
      return true;
    }
    const hotbar = this.session.selectHotbarSlot(slot);
    if (!hotbar?.ok) {
      return false;
    }
    applyHotbarState(hotbar);
    updateDom();
    return true;
  }

  async withSessionAsync(operation) {
    await this.waitForSessionIdle();
    this.sessionBusy = true;
    try {
      return await operation();
    } finally {
      this.sessionBusy = false;
    }
  }

  async waitForSessionIdle() {
    for (let attempt = 0; this.sessionBusy && attempt < 10_000; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    if (this.sessionBusy) {
      throw new Error("timed out waiting for WebChunkRenderSession async operation");
    }
  }

  setInputKey(name, down) {
    if (!(name in this.keys)) {
      return false;
    }
    this.keys[name] = Boolean(down);
    return true;
  }

  queueMouseDelta(dx, dy) {
    const x = Number(dx);
    const y = Number(dy);
    if (Number.isFinite(x)) {
      this.mouseDeltaX += x;
    }
    if (Number.isFinite(y)) {
      this.mouseDeltaY += y;
    }
    if (this.pointerDown) {
      this.pointerDown.movement += Math.abs(Number.isFinite(x) ? x : 0)
        + Math.abs(Number.isFinite(y) ? y : 0);
    }
  }

  requestPointerLock() {
    runtime.state.pointerLockAttempted = true;
    if (typeof this.canvas.requestPointerLock === "function") {
      const result = this.canvas.requestPointerLock();
      if (result && typeof result.catch === "function") {
        result.catch(() => {
          runtime.state.pointerLockFallback = true;
          updateDom();
        });
      }
    } else {
      runtime.state.pointerLockFallback = true;
    }
    updateDom();
  }

  updatePointerLockState() {
    runtime.state.pointerLocked = document.pointerLockElement === this.canvas;
    runtime.state.pointerLockFallback =
      runtime.state.pointerLockAttempted && !runtime.state.pointerLocked;
    updateDom();
  }

  syncCanvasSize() {
    if (!this.session) {
      return;
    }
    const rect = this.canvas.getBoundingClientRect();
    const scale = Number.isFinite(window.devicePixelRatio) ? window.devicePixelRatio : 1;
    const width = Math.max(1, Math.round(rect.width * scale));
    const height = Math.max(1, Math.round(rect.height * scale));
    if (width === runtime.state.width && height === runtime.state.height) {
      return;
    }
    const report = this.session.resizeCanvas(width, height);
    if (report?.ok) {
      runtime.state.width = Number(report.width) || width;
      runtime.state.height = Number(report.height) || height;
    }
  }
}

function bindInput(app) {
  window.addEventListener("keydown", (event) => {
    if ((event.key === "n" || event.key === "N") && !event.repeat) {
      event.preventDefault();
      const camera = app.session?.toggleMovementMode?.();
      if (camera) app.applyCameraState(camera);
      updateDom();
      return;
    }
    const hotbarSlot = hotbarSlotForKey(event.key);
    if (hotbarSlot !== null) {
      event.preventDefault();
      if (!event.repeat) {
        app.selectHotbarSlot(hotbarSlot);
      }
      return;
    }
    const key = inputNameForKey(event.key);
    if (!key) return;
    event.preventDefault();
    app.setInputKey(key, true);
  });

  window.addEventListener("keyup", (event) => {
    const key = inputNameForKey(event.key);
    if (!key) return;
    event.preventDefault();
    app.setInputKey(key, false);
  });

  app.canvas.addEventListener("click", () => {
    app.canvas.focus();
    app.requestPointerLock();
  });

  app.canvas.addEventListener("mousedown", (event) => {
    app.pointerDragging = true;
    app.pointerDown = {
      button: event.button,
      enabled: runtime.state.pointerLockAttempted,
      movement: 0,
    };
    app.canvas.focus();
    if (event.button === 2) {
      event.preventDefault();
    }
  });

  window.addEventListener("mouseup", (event) => {
    const pointerDown = app.pointerDown;
    app.pointerDragging = false;
    app.pointerDown = null;
    if (!pointerDown?.enabled || pointerDown.button !== event.button || pointerDown.movement > 4) {
      return;
    }
    if (event.button === 0) {
      event.preventDefault();
      app.interactBlock("break");
    } else if (event.button === 2) {
      event.preventDefault();
      app.interactBlock("place");
    }
  });

  app.canvas.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });

  window.addEventListener("mousemove", (event) => {
    if (document.pointerLockElement === app.canvas) {
      app.queueMouseDelta(event.movementX, event.movementY);
    } else if (app.pointerDragging) {
      app.queueMouseDelta(event.movementX, event.movementY);
    }
  });

  app.canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    const amount = event.deltaMode === WheelEvent.DOM_DELTA_PIXEL
      ? -event.deltaY * 0.001
      : -event.deltaY * 0.12;
    const camera = app.session?.adjustCameraSpeed?.(amount);
    if (camera) app.applyCameraState(camera);
    updateDom();
  }, { passive: false });

  document.addEventListener("pointerlockchange", () => app.updatePointerLockState());
  document.addEventListener("pointerlockerror", () => {
    runtime.state.pointerLockFallback = true;
    updateDom();
  });
  window.addEventListener("resize", () => {
    app.syncCanvasSize();
    updateDom();
  });
}

function inputNameForKey(key) {
  switch (key) {
    case "ArrowUp":
    case "w":
    case "W":
      return "forward";
    case "ArrowDown":
    case "s":
    case "S":
      return "backward";
    case "ArrowLeft":
    case "a":
    case "A":
      return "left";
    case "ArrowRight":
    case "d":
    case "D":
      return "right";
    case " ":
    case "Spacebar":
      return "jump";
    case "x":
    case "X":
    case "q":
    case "Q":
      return "descend";
    case "Shift":
      return "shift";
    case "Control":
      return "sprint";
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
  setText("camera", `${state.cameraX.toFixed(1)}, ${state.cameraY.toFixed(1)}, ${state.cameraZ.toFixed(1)}`);
  setText("mode", state.movementMode);
  setText("slot", String(Number(state.selectedHotbarSlot || 0) + 1));
  setText("ground", state.onGround ? "ground" : state.verticalCollision ? "blocked" : state.horizontalCollision ? "wall" : "air");
  setText("target", formatTarget(state.currentTarget));
  setText("action", state.interactionStatus);
  setText("chunks", String(state.loadedChunkCount));
  setText("sections", String(state.residentSectionCount));
  setText("time", `${Number(state.dayTime || 0).toFixed(0)} / ${Number(state.timeOfDay || 0).toFixed(3)}`);
  setText("actors", `${state.drawnActorCount}/${state.actorCount}`);
  setText("pending", String(state.pendingCompileJobCount));
  setText("frames", String(state.frameCount));
  setText("lock", state.pointerLocked ? "on" : state.pointerLockFallback ? "fallback" : "off");
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

function formatInteractionStatus(interaction) {
  if (!interaction?.ok) {
    return "idle";
  }
  if (!interaction.hit) {
    return `${interaction.action}: miss`;
  }
  return `${interaction.action}: ${interaction.changed ? "changed" : "same"}`;
}

function applyHotbarState(value) {
  if (!value || typeof value.selectedHotbarSlot === "undefined") {
    return;
  }
  const slot = Number(value.selectedHotbarSlot);
  if (Number.isInteger(slot) && slot >= 0 && slot < 9) {
    runtime.state.selectedHotbarSlot = slot;
  }
}

function hotbarSlotForKey(key) {
  if (key.length !== 1) {
    return null;
  }
  const digit = Number(key);
  if (Number.isInteger(digit) && digit >= 1 && digit <= 9) {
    return digit - 1;
  }
  return null;
}

function formatTarget(interaction) {
  if (!interaction?.ok) {
    return "-";
  }
  if (!interaction.hit) {
    return "miss";
  }
  return `${interaction.blockX}, ${interaction.blockY}, ${interaction.blockZ}`;
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
