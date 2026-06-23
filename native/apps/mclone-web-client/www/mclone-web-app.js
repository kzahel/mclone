import { RenderSectionWorkerCompiler, fetchAssetPack } from "./mclone-render-compiler-shared.js";

/**
 * The wasm-bindgen module namespace (generated `.d.ts`, emitted by `wasm-bindgen --typescript`).
 * Loaded at runtime via a dynamic `import()` of a versioned URL; the bare specifier is path-mapped
 * in tsconfig.json and only ever appears in type positions. Casting the dynamic import to this type
 * is what makes the live wasm call sites (`session.advanceCameraFrame(...)` &c.) checkable against
 * the real export signatures — the "single biggest win" of 070 Stage 2.
 * @typedef {typeof import("mclone-web-client-wasm")} WasmModule
 * @typedef {import("mclone-web-client-wasm").WebChunkRenderSession} WebChunkRenderSession
 */

/**
 * A wasm-return object — camera/frame/report/target/interaction/doorbell. The generated `.d.ts`
 * types every `WebChunkRenderSession` method return as `any` (wasm-bindgen cannot describe the
 * serde shape), so these are read coercion-guarded (`Number(...)`/`Boolean(...)`/`?.ok`). Naming
 * the boundary documents intent and keeps internal field reads consistent.
 * @typedef {Record<string, any>} WasmReport
 */

/**
 * A per-compile timing record. 070 Stage 3 moved the ~90-field instrumentation bag and its
 * coercion-heavy merge/projection logic into Rust (`WebCompileTiming` in the
 * `mclone-web-client` crate); JS now only feeds it report objects + `performance.now()`
 * measurements and reads back `publicSnapshot(...)`.
 * @typedef {import("mclone-web-client-wasm").WebCompileTiming} WebCompileTiming
 */

/**
 * An in-flight compile: the Rust timing handle plus the JS-only worker promise/error the
 * orchestration tracks while the compile round-trips.
 * @typedef {object} PendingCompile
 * @property {WebCompileTiming} timing
 * @property {Promise<any> | null} workerPromise
 * @property {string | null} workerError
 */

/**
 * The global app runtime exposed on `globalThis.__mcloneWebApp` and polled by the smoke harness.
 * `state` is the mutable status snapshot (a loose bag); the optional methods are installed by
 * `boot()` once the {@link WebChunkApp} exists.
 * @typedef {object} AppRuntime
 * @property {boolean} ready
 * @property {Record<string, any>} state
 * @property {(dx: number, dy: number) => void} [queueMouseDelta]
 * @property {(name: string, down: boolean) => boolean} [setInputKey]
 * @property {(amount: number) => any} [adjustCameraSpeed]
 * @property {() => any} [previewBlockTarget]
 * @property {() => any} [touchControlState]
 * @property {(open: boolean) => void} [setHudOpen]
 * @property {(open: boolean) => void} [setMenuOpen]
 */

const DEPLOY_ASSET_VERSION = normalizedDeployAssetVersion();
const BINDGEN_JS_URL = versionedUrl("./pkg/mclone_web_client.js");
const BINDGEN_WASM_URL = versionedUrl("./pkg/mclone_web_client_bg.wasm");
const RENDER_COMPILER_WORKER_URL = versionedUrl("./mclone-render-compiler-worker.js");
const SERVER_WORKER_URL = versionedUrl("./mclone-integrated-server-worker.js");
const SERVER_JOB_WORKER_URL = versionedUrl("./mclone-server-job-worker.js");
const ASSET_PACK_URL = versionedUrl("/reference/minecraft-1.17.1/extracted.zip");

const RADIUS_CHUNKS = 1;
const MAX_FRAME_DT_SECONDS = 0.05;
const INPUT_KEY_NAMES = ["forward", "backward", "left", "right", "jump", "descend", "shift", "sprint"];
const TOUCH_JOYSTICK_MAX_DISTANCE = 50;
const TOUCH_JOYSTICK_DEAD_ZONE = 10;
const TOUCH_AXIS_THRESHOLD = TOUCH_JOYSTICK_DEAD_ZONE / TOUCH_JOYSTICK_MAX_DISTANCE;

// Touch look multiplies the raw drag delta before it reaches the engine's fixed
// mouse sensitivity. A finger drag covers far fewer pixels than a relative mouse
// move (especially on the narrow look-half of a portrait phone), so the default
// boost makes aiming the crosshair toward your travel direction practical.
const LOOK_SENSITIVITY_MIN = 0.5;
const LOOK_SENSITIVITY_MAX = 5;
const DEFAULT_LOOK_SENSITIVITY = 2.4;
const SETTINGS_STORAGE_KEYS = {
  lookSensitivity: "mclone.web.lookSensitivity",
};

/** @type {AppRuntime} */
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
    compileInFlight: false,
    compileInFlightCount: 0,
    compileFinalizingCount: 0,
    streamingSettled: false,
    compileQueued: false,
    compileTargetX: null,
    compileTargetZ: null,
    queuedCompileTargetX: null,
    queuedCompileTargetZ: null,
    activeCompileTiming: null,
    lastCompileTiming: null,
    compileTimings: [],
    compileTimingCount: 0,
    renderCount: 0,
    frameCount: 0,
    lastFrameGapMs: 0,
    maxFrameGapMs: 0,
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
    worldgenMailboxKind: "unknown",
    lightStatusMailboxKind: "unknown",
    worldgenMailboxPendingJobs: 0,
    lightStatusMailboxPendingStatuses: 0,
    runnerFrameMetrics: null,
    worldgenJobFrameMetrics: null,
    lightStatusJobFrameMetrics: null,
    hudOpen: true,
    menuOpen: false,
    settingsOpen: false,
    lookSensitivity: DEFAULT_LOOK_SENSITIVITY,
    touchControlsVisible: false,
    touchJoystickActive: false,
    touchMovementLeftImpulse: 0,
    touchMovementForwardImpulse: 0,
    touchLookActive: false,
    touchButtonActiveCount: 0,
    status: "booting",
  },
};

globalThis.__mcloneWebApp = runtime;

async function boot() {
  const app = new WebChunkApp();
  runtime.queueMouseDelta = (dx, dy) => app.queueMouseDelta(dx, dy);
  runtime.setInputKey = (name, down) => app.setInputKey(name, down);
  runtime.adjustCameraSpeed = (amount) => app.adjustCameraSpeed(amount);
  runtime.previewBlockTarget = () => runtime.state.currentTarget;
  runtime.touchControlState = () => app.touchControls?.snapshot() ?? null;
  runtime.setHudOpen = (open) => setHudOpen(Boolean(open));
  runtime.setMenuOpen = (open) => setMenuOpen(app, Boolean(open));
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
    // Required for the app to run; `init()` re-validates with `instanceof HTMLCanvasElement` and
    // throws if it is missing, so treating it as a non-null canvas here is sound for the lifecycle.
    this.canvas = /** @type {HTMLCanvasElement} */ (document.getElementById("mclone-canvas"));
    this.status = document.getElementById("status");
    this.hud = document.getElementById("runtime-hud");
    this.hudToggle = document.getElementById("hud-toggle");
    /** @type {WasmModule | null} */
    this.module = null;
    /** @type {WebChunkRenderSession | null} */
    this.session = null;
    /** @type {RenderSectionWorkerCompiler | null} */
    this.compiler = null;
    this.keys = defaultInputKeys();
    this.touchKeys = defaultInputKeys();
    this.touchMovementImpulse = defaultMovementImpulse();
    /** @type {TouchControls | null} */
    this.touchControls = null;
    const settings = loadStoredSettings();
    this.lookSensitivity = settings.lookSensitivity;
    runtime.state.lookSensitivity = this.lookSensitivity;
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.compileSequence = 0;
    // 067 Stage 3: per-compile timings keyed by request id while in flight (submitted +
    // worker round-trip) plus a count of timings whose apply finalize is still awaiting
    // the worker metrics. The streaming loop posts a doorbell per compile and finalizes
    // the timing when the next frame's poll applies the result.
    /** @type {Map<number, PendingCompile>} */
    this.pendingTimings = new Map();
    this.finalizingCount = 0;
    this.hasRendered = false;
    /** @type {{ centerX: number, centerZ: number } | null} */
    this.loadedCenter = null;
    this.pointerDragging = false;
    /** @type {{ button: number, enabled: boolean, movement: number } | null} */
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
    const module = /** @type {WasmModule} */ (await import(BINDGEN_JS_URL.href));
    await module.default(BINDGEN_WASM_URL.href);
    this.module = module;
    for (const name of ["mclone_web_create_worker_chunk_render_session"]) {
      if (typeof (/** @type {any} */ (module))[name] !== "function") {
        throw new Error(`missing ${name} export`);
      }
    }

    runtime.state.status = "loading assets";
    updateDom();
    const assetPack = await fetchAssetPack(ASSET_PACK_URL);
    runtime.state.status = "initializing webgpu";
    updateDom();
    this.session = await module.mclone_web_create_worker_chunk_render_session(
      this.canvas,
      assetPack,
      SERVER_WORKER_URL.href,
      SERVER_JOB_WORKER_URL.href,
      BINDGEN_JS_URL.href,
      BINDGEN_WASM_URL.href,
    );
    for (const name of [
      "advanceCameraFrame",
      "renderCompilerSharedSupported",
      "syncCameraRenderFrame",
      "cameraFrameState",
      "resizeCanvas",
      "toggleMovementMode",
      "selectHotbarSlot",
      "previewBlockTarget",
      "interactBlock",
    ]) {
      if (typeof (/** @type {any} */ (this.session))[name] !== "function") {
        throw new Error(`missing WebChunkRenderSession.${name} export`);
      }
    }
    if (!this.session.renderCompilerSharedSupported()) {
      throw new Error(
        "cross-origin isolation (SharedArrayBuffer/Atomics) is required for the streaming render loop",
      );
    }
    this.compiler = new RenderSectionWorkerCompiler(assetPack, {
      workerUrl: RENDER_COMPILER_WORKER_URL,
      bindgenJsUrl: BINDGEN_JS_URL,
      bindgenWasmUrl: BINDGEN_WASM_URL,
      workerName: "mclone-render-compiler-app",
    });
    bindMenu(this);
    bindInput(this);
    this.touchControls = new TouchControls(this);
    this.syncCanvasSize();
    this.applyCameraState(this.session.cameraFrameState());
    this.applyTargetState(this.session.previewBlockTarget());

    runtime.state.status = "rendering";
    updateDom();
    // 067 Stage 3: warm up the streaming loop to idle so the first presented frame has
    // terrain (the web analog of desktop's pre-render `sync_all_render_sections`).
    await this.warmUpStreamingToIdle();
  }

  start() {
    this.lastFrameTime = performance.now();
    const frame = (/** @type {number} */ now) => {
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

  /** @param {number} now */
  async tickFrame(now) {
    if (!this.session) {
      return;
    }
    const session = this.session;
    const frameGapMs = Number.isFinite(now - this.lastFrameTime)
      ? Math.max(0, now - this.lastFrameTime)
      : 0;
    this.recordFrameGap(frameGapMs);
    const rawDt = frameGapMs / 1000;
    this.lastFrameTime = now;
    const dtSeconds = Number.isFinite(rawDt)
      ? Math.max(0, Math.min(rawDt, MAX_FRAME_DT_SECONDS))
      : 0;
    const mouseDeltaX = this.mouseDeltaX;
    const mouseDeltaY = this.mouseDeltaY;
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.syncCanvasSize();
    const keys = this.currentInputKeys();
    const movementImpulse = this.currentMovementImpulse();

    runtime.state.frameCount += 1;
    runtime.state.tickPhase = "advance";
    const camera = await this.withSessionAsync(() => session.advanceCameraFrame(
      dtSeconds,
      mouseDeltaX,
      mouseDeltaY,
      keys.forward,
      keys.backward,
      keys.left,
      keys.right,
      keys.jump,
      keys.descend,
      keys.shift,
      keys.sprint,
      movementImpulse.active,
      movementImpulse.left,
      movementImpulse.forward,
    ));
    runtime.state.tickPhase = "post-advance";
    this.applyCameraState(camera);
    this.applyTargetState(session.previewBlockTarget());

    // 067 Stage 3: one frame of the shared streaming loop. syncCameraRenderFrame drains
    // updates, runs the budget-1 sync over the resident-ring compiler, and renders the
    // current cache; JS only relays the worker doorbell it returns. No coalescing queue,
    // no busy flag, no begin/worker/finish transaction — the loop is dirty-driven.
    runtime.state.tickPhase = "stream";
    await this.streamFrameOnce();
    runtime.state.tickPhase = "idle";
  }

  // 067 Stage 3: warm up the streaming loop to idle before the live rAF loop begins, in
  // two phases, so the boot-time camera fall is reproducible despite per-compile frame
  // timing:
  //   1. stream the client-spawn view to idle (no camera motion);
  //   2. apply the server-spawn snap with ~0 dt (so the camera's horizontal position
  //      lands deterministically without falling) and pre-load the server-spawn chunks
  //      to idle.
  // The live rAF gravity fall then runs on fast, fully-streamed frames and settles the
  // camera at the same sub-block position every run — matching the pre-streaming app,
  // whose render-only frames were uniformly fast. Feeding variable streaming frame gaps
  // into the boot fall otherwise wedged WALK movement against spawn terrain on ~half of
  // runs.
  async warmUpStreamingToIdle() {
    // Called from `init()` only after `this.session` is assigned, so it is non-null here.
    const session = /** @type {WebChunkRenderSession} */ (this.session);
    const deadline = performance.now() + 30_000;
    while (performance.now() < deadline) {
      const idle = await this.streamFrameOnce({ awaitWorker: true });
      if (idle && this.hasRendered && Number(runtime.state.residentSectionCount) > 0) {
        break;
      }
      await nextAnimationFrame();
    }
    /** @type {{ x: number, z: number } | null} */
    let last = null;
    let stableFrames = 0;
    let iterations = 0;
    while (performance.now() < deadline) {
      const camera = await this.withSessionAsync(() => session.advanceCameraFrame(
        1e-4, 0, 0, false, false, false, false, false, false, false, false, false, 0, 0,
      ));
      this.applyCameraState(camera);
      const idle = await this.streamFrameOnce({ awaitWorker: true });
      iterations += 1;
      const x = Number(camera.cameraX);
      const z = Number(camera.cameraZ);
      const settled = last !== null
        && Math.abs(x - last.x) < 1e-4
        && Math.abs(z - last.z) < 1e-4;
      stableFrames = settled ? stableFrames + 1 : 0;
      last = { x, z };
      if (
        idle
        && this.hasRendered
        && Number(runtime.state.residentSectionCount) > 0
        && iterations >= 8
        && stableFrames >= 6
      ) {
        return;
      }
      await nextAnimationFrame();
    }
    throw new Error("timed out warming up the streaming render loop");
  }

  // 067 Stage 3 keystone: one frame of the shared streaming loop. Rust drains updates,
  // runs the budget-1 sync over the resident-ring compiler, and renders the current
  // cache, returning the frame report plus an optional worker `doorbell`. JS relays the
  // doorbell (fire-and-forget in the live loop; awaited during warm-up so the pump
  // converges); the next frame's poll applies the result. Returns whether the loop idled.
  /** @param {{ awaitWorker?: boolean }} [options] */
  async streamFrameOnce(options = {}) {
    if (!this.session || !this.compiler) {
      return true;
    }
    const session = this.session;
    const syncStart = performance.now();
    /** @type {WasmReport} */
    let frame;
    try {
      frame = await this.withSessionAsync(() => session.syncCameraRenderFrame(RADIUS_CHUNKS));
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
      updateDom();
      return true;
    }
    const syncMs = performance.now() - syncStart;
    const workerPromise = this.handleStreamingFrame(frame, syncMs);
    if (workerPromise && options.awaitWorker) {
      await workerPromise.catch(() => {});
    }
    return runtime.state.streamingSettled === true;
  }

  /**
   * @param {WasmReport} frame
   * @param {number} syncMs
   */
  handleStreamingFrame(frame, syncMs) {
    if (!frame?.ok) {
      runtime.state.ok = false;
      runtime.state.status = frame?.reason ?? "streaming render frame failed";
      updateDom();
      return null;
    }
    this.hasRendered = true;
    this.applyReport(frame);
    const pendingJobs = Number(frame.pendingCompileJobCount) || 0;
    runtime.state.pendingCompileJobCount = pendingJobs;
    runtime.state.compileInFlight = pendingJobs > 0;
    runtime.state.compileInFlightCount = this.pendingTimings.size;
    runtime.state.compileFinalizingCount = this.finalizingCount;
    // 067 Stage 3: "settled" must mean the server runner has finished generating +
    // publishing the view AND the render loop has nothing left to compile. Render-idle
    // alone is reached prematurely when a fast camera outruns chunk generation (no
    // snapshots yet => no dirty work => idle, but the view is not actually loaded). Gate
    // loadedCenter on the runner queues draining so movement waits for the moved-to
    // chunks to stream + compile.
    const runnerSettled = Number(frame.runnerCommandQueueDepth) === 0
      && Number(frame.runnerUpdateQueueDepth) === 0
      && Number(frame.runnerPendingJobs) === 0
      && Number(frame.runnerPendingPublications) === 0;
    const streamingSettled = Boolean(frame.streamingIdle)
      && runnerSettled
      && pendingJobs === 0
      && this.pendingTimings.size === 0
      && this.finalizingCount === 0;
    runtime.state.streamingSettled = streamingSettled;
    if (streamingSettled) {
      this.loadedCenter = { centerX: frame.centerX, centerZ: frame.centerZ };
      runtime.state.loadedCenterX = frame.centerX;
      runtime.state.loadedCenterZ = frame.centerZ;
    }
    if (Number(frame.appliedRequestId) > 0) {
      this.finalizeCompileTiming(frame, syncMs);
    }
    let workerPromise = null;
    if (frame.doorbell) {
      workerPromise = this.startAndPostCompileTiming(frame.doorbell, syncMs);
    }
    updateDom();
    return workerPromise;
  }

  // Begin a per-compile timing for the doorbell armed this frame and relay it to the
  // worker. The worker writes the packed result into the resident ring (drained by the
  // next frame's poll); the returned promise resolves with the worker metrics report.
  /**
   * @param {WasmReport} doorbell
   * @param {number} syncMs
   */
  startAndPostCompileTiming(doorbell, syncMs) {
    // Reached only from `handleStreamingFrame` via `streamFrameOnce`, which already guards
    // `this.compiler` and (via boot/init) `this.module` non-null.
    const compiler = /** @type {RenderSectionWorkerCompiler} */ (this.compiler);
    const module = /** @type {WasmModule} */ (this.module);
    const sequence = ++this.compileSequence;
    // 070 Stage 3: the timing bag + its report coercions live in Rust now; `updateFromRequest`
    // sets the target center off the doorbell, so the constructor starts it null.
    const timing = new module.WebCompileTiming(
      sequence,
      "stream",
      this.loadedCenter?.centerX ?? null,
      this.loadedCenter?.centerZ ?? null,
      runtime.state.frameCount,
      runtime.state.renderCount,
      performance.now(),
    );
    timing.updateFromRequest(doorbell);
    timing.setBeginRequestMs(syncMs);
    /** @type {PendingCompile} */
    const pending = { timing, workerPromise: null, workerError: null };
    const workerStart = performance.now();
    const workerPromise = compiler.compileWithDoorbell(doorbell).then((compiled) => {
      timing.setWorkerRoundTripMs(performance.now() - workerStart);
      if (compiled?.report) {
        timing.updateFromWorker(compiled.report);
      }
      return compiled;
    });
    pending.workerPromise = workerPromise;
    this.pendingTimings.set(Number(doorbell.requestId), pending);
    runtime.state.compileInFlightCount = this.pendingTimings.size;
    publishActiveCompileTiming(timing);
    workerPromise.catch((error) => {
      pending.workerError = stringifyError(error);
      console.error(error);
    });
    return workerPromise;
  }

  // Finalize the timing for the compile applied this frame: await its worker metrics (so
  // the record carries the transport/byte diagnostics), merge the Rust apply report, and
  // publish it to runtime.state.compileTimings.
  /**
   * @param {WasmReport} frame
   * @param {number} syncMs
   */
  finalizeCompileTiming(frame, syncMs) {
    const requestId = Number(frame.appliedRequestId);
    const pending = this.pendingTimings.get(requestId);
    if (!pending) {
      return;
    }
    this.pendingTimings.delete(requestId);
    runtime.state.compileInFlightCount = this.pendingTimings.size;
    this.finalizingCount += 1;
    runtime.state.compileFinalizingCount = this.finalizingCount;
    const { timing } = pending;
    void (async () => {
      try {
        await pending.workerPromise;
      } catch (_error) {
        // worker error already recorded on `pending.workerError`
      }
      timing.updateFromReport(frame);
      timing.setDecodeFinishApplyMs(syncMs);
      const ok = Boolean(frame.appliedOk) && !pending.workerError;
      timing.finish(
        ok ? "accepted" : "failed",
        pending.workerError ?? null,
        performance.now(),
        runtime.state.frameCount,
        runtime.state.renderCount,
        runtime.state.lastFrameGapMs,
      );
      recordCompileTiming(timing);
      runtime.state.lastCompileReport = frame;
      this.finalizingCount = Math.max(0, this.finalizingCount - 1);
      runtime.state.compileFinalizingCount = this.finalizingCount;
      updateDom();
    })();
  }

  /** @param {WasmReport} camera */
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

  /** @param {WasmReport} report */
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
    runtime.state.worldgenMailboxKind = report.worldgenMailboxKind;
    runtime.state.lightStatusMailboxKind = report.lightStatusMailboxKind;
    runtime.state.worldgenMailboxPendingJobs = report.worldgenMailboxPendingJobs;
    runtime.state.lightStatusMailboxPendingStatuses = report.lightStatusMailboxPendingStatuses;
    runtime.state.runnerFrameMetrics = report.runnerFrameMetrics ?? null;
    runtime.state.worldgenJobFrameMetrics = report.worldgenJobFrameMetrics ?? null;
    runtime.state.lightStatusJobFrameMetrics = report.lightStatusJobFrameMetrics ?? null;
    runtime.state.renderCount = report.renderCount;
    runtime.state.status = "ready";
    runtime.state.lastReport = report;
  }

  /** @param {WasmReport} target */
  applyTargetState(target) {
    if (!target?.ok) {
      return;
    }
    runtime.state.currentTarget = target;
    applyHotbarState(target);
  }

  /** @param {string} action */
  async interactBlock(action) {
    if (!this.session) {
      return null;
    }
    const session = this.session;
    try {
      await this.waitForSessionIdle();
      const interaction = await this.withSessionAsync(() => session.interactBlock(action));
      if (!interaction?.ok) {
        return null;
      }
      runtime.state.interactionCount += 1;
      runtime.state.lastInteraction = interaction;
      runtime.state.interactionStatus = formatInteractionStatus(interaction);
      applyHotbarState(interaction);
      // 067 Stage 3: a block edit publishes a SectionBlockUpdates server update, which the
      // streaming loop marks render-dirty on its next drain and recompiles automatically —
      // no explicit compile request needed.
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

  /** @param {number} slot */
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

  /** @param {number} amount */
  adjustCameraSpeed(amount) {
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.adjustCameraSpeed(amount), 0);
      return null;
    }
    const camera = this.session.adjustCameraSpeed(amount);
    if (!camera?.ok) {
      return null;
    }
    this.applyCameraState(camera);
    updateDom();
    return camera;
  }

  /**
   * @template T
   * @param {() => T} operation
   * @returns {Promise<Awaited<T>>}
   */
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

  /**
   * @param {string} name
   * @param {boolean} down
   */
  setInputKey(name, down) {
    if (!(name in this.keys)) {
      return false;
    }
    this.keys[name] = Boolean(down);
    return true;
  }

  /**
   * @param {string} name
   * @param {boolean} down
   */
  setTouchKey(name, down) {
    if (!(name in this.touchKeys)) {
      return false;
    }
    this.touchKeys[name] = Boolean(down);
    return true;
  }

  /** @param {Record<string, boolean>} keys */
  setTouchKeys(keys) {
    for (const [name, down] of Object.entries(keys)) {
      this.setTouchKey(name, down);
    }
  }

  /**
   * @param {number} left
   * @param {number} forward
   * @param {boolean} active
   */
  setTouchMovementImpulse(left, forward, active) {
    this.touchMovementImpulse = {
      active: Boolean(active),
      left: sanitizeInputImpulse(left),
      forward: sanitizeInputImpulse(forward),
    };
  }

  currentInputKeys() {
    const keys = defaultInputKeys();
    for (const name of INPUT_KEY_NAMES) {
      keys[name] = Boolean(this.keys[name] || this.touchKeys[name]);
    }
    return keys;
  }

  currentMovementImpulse() {
    return { ...this.touchMovementImpulse };
  }

  /** @param {string[]} [names] */
  clearTouchKeys(names = INPUT_KEY_NAMES) {
    for (const name of names) {
      this.setTouchKey(name, false);
    }
  }

  /**
   * @param {number} dx
   * @param {number} dy
   */
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

  currentCameraCenter() {
    return {
      centerX: Number(runtime.state.centerX) || 0,
      centerZ: Number(runtime.state.centerZ) || 0,
    };
  }

  /** @param {number} frameGapMs */
  recordFrameGap(frameGapMs) {
    if (!Number.isFinite(frameGapMs)) {
      return;
    }
    runtime.state.lastFrameGapMs = frameGapMs;
    runtime.state.maxFrameGapMs = Math.max(runtime.state.maxFrameGapMs, frameGapMs);
    // 067 Stage 3: many compiles can be in flight across frames; charge the frame gap to
    // every pending timing so each compile's window reflects presentation stalls.
    for (const pending of this.pendingTimings.values()) {
      pending.timing.observeFrameGap(frameGapMs);
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

/** @param {WebChunkApp} app */
function bindInput(app) {
  window.addEventListener("keydown", (event) => {
    if (isPhysicalKey(event, "KeyN", "n") && !event.repeat) {
      event.preventDefault();
      const camera = app.session?.toggleMovementMode?.();
      if (camera) app.applyCameraState(camera);
      updateDom();
      return;
    }
    const hotbarSlot = hotbarSlotForEvent(event);
    if (hotbarSlot !== null) {
      event.preventDefault();
      if (!event.repeat) {
        app.selectHotbarSlot(hotbarSlot);
      }
      return;
    }
    const key = inputNameForEvent(event);
    if (!key) return;
    event.preventDefault();
    app.setInputKey(key, true);
  });

  window.addEventListener("keyup", (event) => {
    const key = inputNameForEvent(event);
    if (!key) return;
    event.preventDefault();
    app.setInputKey(key, false);
  });

  app.canvas.addEventListener("click", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      event.preventDefault();
      return;
    }
    app.canvas.focus();
    app.requestPointerLock();
  });

  app.canvas.addEventListener("mousedown", (event) => {
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      event.preventDefault();
      return;
    }
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
    if (app.touchControls?.shouldIgnoreMouseEvent()) {
      return;
    }
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

/** @param {WebChunkApp} app */
function bindMenu(app) {
  // The hamburger now opens the main menu; the runtime/debug stats live behind
  // the menu's "Debug info" entry. Stats stay open by default on desktop so the
  // debug HUD remains a glance away, while mobile boots into the clean view.
  setHudOpen(defaultHudOpen());
  setMenuOpen(app, false);
  setSettingsOpen(false);
  syncSettingsControls(app);

  app.hudToggle?.addEventListener("click", () => setMenuOpen(app, !isMenuOpen()));

  document.getElementById("menu-resume")?.addEventListener("click", () => {
    setMenuOpen(app, false);
  });

  const debugButton = document.getElementById("menu-debug");
  debugButton?.addEventListener("click", () => {
    const open = !isHudOpen();
    setHudOpen(open);
    setMenuOpen(app, false);
  });

  document.getElementById("menu-settings")?.addEventListener("click", () => {
    setSettingsOpen(!isSettingsOpen());
  });

  const lookInput = /** @type {HTMLInputElement | null} */ (
    document.getElementById("setting-look-sensitivity")
  );
  lookInput?.addEventListener("input", () => {
    setLookSensitivity(app, Number(lookInput.value));
  });

  window.addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || document.pointerLockElement === app.canvas) {
      return;
    }
    if (isSettingsOpen()) {
      setSettingsOpen(false);
    } else if (isMenuOpen()) {
      setMenuOpen(app, false);
    } else if (isHudOpen()) {
      setHudOpen(false);
    }
  });
}

/**
 * @param {WebChunkApp} app
 * @param {boolean} open
 */
function setMenuOpen(app, open) {
  const menu = document.getElementById("main-menu");
  const toggle = document.getElementById("hud-toggle");
  if (menu) {
    menu.hidden = !open;
  }
  if (toggle) {
    toggle.setAttribute("aria-expanded", open ? "true" : "false");
  }
  if (!open) {
    setSettingsOpen(false);
  } else {
    // Releasing held touch input keeps the player from drifting while the modal
    // menu is consuming the screen.
    app?.touchControls?.clearAll();
    syncSettingsControls(app);
  }
  const debugButton = document.getElementById("menu-debug");
  debugButton?.setAttribute("aria-pressed", isHudOpen() ? "true" : "false");
  runtime.state.menuOpen = Boolean(open);
}

function isMenuOpen() {
  return document.getElementById("main-menu")?.hidden === false;
}

/** @param {boolean} open */
function setSettingsOpen(open) {
  const panel = document.getElementById("settings-panel");
  const button = document.getElementById("menu-settings");
  if (panel) {
    panel.hidden = !open;
  }
  if (button) {
    button.setAttribute("aria-expanded", open ? "true" : "false");
  }
  runtime.state.settingsOpen = Boolean(open);
}

function isSettingsOpen() {
  return document.getElementById("settings-panel")?.hidden === false;
}

/**
 * @param {WebChunkApp} app
 * @param {number} value
 */
function setLookSensitivity(app, value) {
  const clamped = clampLookSensitivity(value);
  app.lookSensitivity = clamped;
  runtime.state.lookSensitivity = clamped;
  storeSetting(SETTINGS_STORAGE_KEYS.lookSensitivity, String(clamped));
  syncSettingsControls(app);
}

/** @param {WebChunkApp} app */
function syncSettingsControls(app) {
  const lookInput = /** @type {HTMLInputElement | null} */ (
    document.getElementById("setting-look-sensitivity")
  );
  if (lookInput && document.activeElement !== lookInput) {
    lookInput.value = String(app.lookSensitivity);
  }
  const lookValue = document.getElementById("setting-look-sensitivity-value");
  if (lookValue) {
    lookValue.textContent = `${app.lookSensitivity.toFixed(1)}×`;
  }
}

/** @param {unknown} value */
function clampLookSensitivity(value) {
  const sensitivity = Number(value);
  if (!Number.isFinite(sensitivity)) {
    return DEFAULT_LOOK_SENSITIVITY;
  }
  return Math.min(LOOK_SENSITIVITY_MAX, Math.max(LOOK_SENSITIVITY_MIN, sensitivity));
}

function loadStoredSettings() {
  return {
    lookSensitivity: clampLookSensitivity(
      readStoredSetting(SETTINGS_STORAGE_KEYS.lookSensitivity) ?? DEFAULT_LOOK_SENSITIVITY,
    ),
  };
}

/** @param {string} key */
function readStoredSetting(key) {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch (_error) {
    return null;
  }
}

/**
 * @param {string} key
 * @param {string} value
 */
function storeSetting(key, value) {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch (_error) {
    // Private browsing / disabled storage: keep the in-memory setting only.
  }
}

class TouchControls {
  /** @param {WebChunkApp} app */
  constructor(app) {
    this.app = app;
    this.canvas = app.canvas;
    this.root = document.getElementById("touch-controls");
    this.joystick = document.getElementById("touch-joystick");
    this.thumb = document.getElementById("touch-joystick-thumb");
    this.buttons = /** @type {HTMLElement[]} */ (Array.from(document.querySelectorAll("[data-touch-key]")));
    /** @type {number | null} */
    this.movementPointerId = null;
    /** @type {number | null} */
    this.lookPointerId = null;
    this.movementBaseX = 0;
    this.movementBaseY = 0;
    this.movementThumbX = 0;
    this.movementThumbY = 0;
    this.lookLastX = 0;
    this.lookLastY = 0;
    /** @type {Map<number, string>} */
    this.buttonPointers = new Map();
    this.lastTouchAt = 0;

    this.setVisible(hasTouchInput());
    this.bindCanvas();
    this.bindButtons();
    window.addEventListener("blur", () => this.clearAll());
  }

  bindCanvas() {
    this.canvas.addEventListener("pointerdown", (event) => this.onCanvasPointerDown(event), { passive: false });
    this.canvas.addEventListener("pointermove", (event) => this.onCanvasPointerMove(event), { passive: false });
    this.canvas.addEventListener("pointerup", (event) => this.onCanvasPointerEnd(event), { passive: false });
    this.canvas.addEventListener("pointercancel", (event) => this.onCanvasPointerEnd(event), { passive: false });
    this.canvas.addEventListener("lostpointercapture", (event) => this.onCanvasPointerEnd(event), { passive: false });
  }

  bindButtons() {
    for (const button of this.buttons) {
      button.addEventListener("pointerdown", (event) => this.onButtonPointerDown(event), { passive: false });
      button.addEventListener("pointerup", (event) => this.onButtonPointerEnd(event), { passive: false });
      button.addEventListener("pointercancel", (event) => this.onButtonPointerEnd(event), { passive: false });
      button.addEventListener("lostpointercapture", (event) => this.onButtonPointerEnd(event), { passive: false });
    }
  }

  /** @param {PointerEvent} event */
  onCanvasPointerDown(event) {
    if (!isTouchPointer(event)) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    this.setVisible(true);
    this.canvas.focus();
    const rect = this.canvas.getBoundingClientRect();
    const onLeft = event.clientX < rect.left + rect.width * 0.5;
    if (onLeft && this.movementPointerId === null) {
      this.startMovement(event);
    } else if (this.lookPointerId === null) {
      this.startLook(event);
    }
  }

  /** @param {PointerEvent} event */
  onCanvasPointerMove(event) {
    if (event.pointerId === this.movementPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.updateMovement(event.clientX, event.clientY);
    } else if (event.pointerId === this.lookPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.updateLook(event.clientX, event.clientY);
    }
  }

  /** @param {PointerEvent} event */
  onCanvasPointerEnd(event) {
    if (event.pointerId === this.movementPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.clearMovement();
    } else if (event.pointerId === this.lookPointerId) {
      this.markTouchEvent();
      event.preventDefault();
      this.clearLook();
    }
  }

  /** @param {PointerEvent} event */
  onButtonPointerDown(event) {
    // currentTarget is the bound `[data-touch-key]` button (an HTMLElement) for the duration of
    // its own dispatch.
    const target = /** @type {HTMLElement} */ (event.currentTarget);
    const key = target?.dataset?.touchKey;
    if (!key) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    event.stopPropagation();
    this.setVisible(true);
    trySetPointerCapture(target, event.pointerId);
    this.buttonPointers.set(event.pointerId, key);
    target.dataset.active = "true";
    this.app.setTouchKey(key, true);
    this.updateRuntimeState();
  }

  /** @param {PointerEvent} event */
  onButtonPointerEnd(event) {
    const key = this.buttonPointers.get(event.pointerId);
    if (!key) {
      return;
    }
    this.markTouchEvent();
    event.preventDefault();
    event.stopPropagation();
    this.buttonPointers.delete(event.pointerId);
    const button = this.buttons.find((candidate) => candidate.dataset.touchKey === key);
    if (button) {
      button.dataset.active = "false";
    }
    this.app.setTouchKey(key, false);
    this.updateRuntimeState();
  }

  /** @param {PointerEvent} event */
  startMovement(event) {
    this.movementPointerId = event.pointerId;
    this.movementBaseX = event.clientX;
    this.movementBaseY = event.clientY;
    this.movementThumbX = event.clientX;
    this.movementThumbY = event.clientY;
    trySetPointerCapture(this.canvas, event.pointerId);
    this.updateJoystickVisual();
    this.updateMovement(event.clientX, event.clientY);
    this.updateRuntimeState();
  }

  /**
   * @param {number} clientX
   * @param {number} clientY
   */
  updateMovement(clientX, clientY) {
    const dx = clientX - this.movementBaseX;
    const dy = clientY - this.movementBaseY;
    const distance = Math.hypot(dx, dy);
    const scale = distance > TOUCH_JOYSTICK_MAX_DISTANCE
      ? TOUCH_JOYSTICK_MAX_DISTANCE / distance
      : 1;
    const clampedX = dx * scale;
    const clampedY = dy * scale;
    this.movementThumbX = this.movementBaseX + clampedX;
    this.movementThumbY = this.movementBaseY + clampedY;
    const axisX = clampedX / TOUCH_JOYSTICK_MAX_DISTANCE;
    const axisY = -clampedY / TOUCH_JOYSTICK_MAX_DISTANCE;
    const magnitude = Math.hypot(axisX, axisY);
    const active = magnitude >= TOUCH_AXIS_THRESHOLD;
    let leftImpulse = 0;
    let forwardImpulse = 0;
    if (active) {
      const adjustedMagnitude = (magnitude - TOUCH_AXIS_THRESHOLD) / (1 - TOUCH_AXIS_THRESHOLD);
      const impulseScale = adjustedMagnitude / magnitude;
      leftImpulse = -axisX * impulseScale;
      forwardImpulse = axisY * impulseScale;
    }
    this.app.setTouchMovementImpulse(leftImpulse, forwardImpulse, true);
    this.app.setTouchKeys({
      forward: active && axisY > TOUCH_AXIS_THRESHOLD,
      backward: active && axisY < -TOUCH_AXIS_THRESHOLD,
      left: active && axisX < -TOUCH_AXIS_THRESHOLD,
      right: active && axisX > TOUCH_AXIS_THRESHOLD,
    });
    this.updateJoystickVisual();
    this.updateRuntimeState();
  }

  clearMovement() {
    this.movementPointerId = null;
    this.app.setTouchMovementImpulse(0, 0, false);
    this.app.setTouchKeys({
      forward: false,
      backward: false,
      left: false,
      right: false,
    });
    if (this.joystick) {
      this.joystick.dataset.active = "false";
    }
    this.updateRuntimeState();
  }

  /** @param {PointerEvent} event */
  startLook(event) {
    this.lookPointerId = event.pointerId;
    this.lookLastX = event.clientX;
    this.lookLastY = event.clientY;
    trySetPointerCapture(this.canvas, event.pointerId);
    this.updateRuntimeState();
  }

  /**
   * @param {number} clientX
   * @param {number} clientY
   */
  updateLook(clientX, clientY) {
    const sensitivity = Number.isFinite(this.app.lookSensitivity) ? this.app.lookSensitivity : 1;
    this.app.queueMouseDelta(
      (clientX - this.lookLastX) * sensitivity,
      (clientY - this.lookLastY) * sensitivity,
    );
    this.lookLastX = clientX;
    this.lookLastY = clientY;
    this.updateRuntimeState();
  }

  clearLook() {
    this.lookPointerId = null;
    this.updateRuntimeState();
  }

  clearAll() {
    this.clearMovement();
    this.clearLook();
    for (const pointerId of Array.from(this.buttonPointers.keys())) {
      const key = this.buttonPointers.get(pointerId);
      if (key) {
        this.app.setTouchKey(key, false);
      }
      this.buttonPointers.delete(pointerId);
    }
    for (const button of this.buttons) {
      button.dataset.active = "false";
    }
    this.updateRuntimeState();
  }

  updateJoystickVisual() {
    if (!this.joystick || !this.thumb) {
      return;
    }
    const rect = this.canvas.getBoundingClientRect();
    this.joystick.style.left = `${this.movementBaseX - rect.left}px`;
    this.joystick.style.top = `${this.movementBaseY - rect.top}px`;
    this.joystick.dataset.active = this.movementPointerId === null ? "false" : "true";
    this.thumb.style.left = `${50 + (this.movementThumbX - this.movementBaseX)}px`;
    this.thumb.style.top = `${50 + (this.movementThumbY - this.movementBaseY)}px`;
  }

  /** @param {boolean} visible */
  setVisible(visible) {
    if (this.root) {
      this.root.dataset.visible = visible ? "true" : "false";
      this.root.setAttribute("aria-hidden", visible ? "false" : "true");
    }
    runtime.state.touchControlsVisible = Boolean(visible);
  }

  markTouchEvent() {
    this.lastTouchAt = performance.now();
  }

  shouldIgnoreMouseEvent() {
    return performance.now() - this.lastTouchAt < 800;
  }

  updateRuntimeState() {
    runtime.state.touchControlsVisible = this.root?.dataset.visible === "true";
    runtime.state.touchJoystickActive = this.movementPointerId !== null;
    runtime.state.touchMovementLeftImpulse = this.app.touchMovementImpulse.left;
    runtime.state.touchMovementForwardImpulse = this.app.touchMovementImpulse.forward;
    runtime.state.touchLookActive = this.lookPointerId !== null;
    runtime.state.touchButtonActiveCount = this.buttonPointers.size;
  }

  snapshot() {
    return {
      visible: this.root?.dataset.visible === "true",
      joystickActive: this.movementPointerId !== null,
      lookActive: this.lookPointerId !== null,
      buttonActiveCount: this.buttonPointers.size,
      keys: { ...this.app.touchKeys },
      movementImpulse: this.app.currentMovementImpulse(),
    };
  }
}

/** @param {KeyboardEvent} event */
function inputNameForEvent(event) {
  const code = keyboardCode(event);
  switch (code) {
    case "ArrowUp":
    case "KeyW":
      return "forward";
    case "ArrowDown":
    case "KeyS":
      return "backward";
    case "ArrowLeft":
    case "KeyA":
      return "left";
    case "ArrowRight":
    case "KeyD":
      return "right";
    case "Space":
      return "jump";
    case "KeyX":
    case "KeyQ":
      return "descend";
    case "ShiftLeft":
    case "ShiftRight":
      return "shift";
    case "ControlLeft":
    case "ControlRight":
      return "sprint";
    default:
      return code === null ? inputNameForLegacyKey(event.key) : null;
  }
}

/** @param {string} key */
function inputNameForLegacyKey(key) {
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

/**
 * @param {KeyboardEvent} event
 * @param {string} code
 * @param {string} legacyKey
 */
function isPhysicalKey(event, code, legacyKey) {
  if (keyboardCode(event) === code) {
    return true;
  }
  return keyboardCode(event) === null && (
    event.key === legacyKey || event.key === legacyKey.toUpperCase()
  );
}

/** @param {KeyboardEvent} event */
function keyboardCode(event) {
  return typeof event.code === "string" && event.code.length > 0 && event.code !== "Unidentified"
    ? event.code
    : null;
}

/** @param {WebCompileTiming} timing */
function publishActiveCompileTiming(timing) {
  runtime.state.activeCompileTiming = timing.publicSnapshot(performance.now());
}

/** @param {WebCompileTiming} timing */
function recordCompileTiming(timing) {
  const snapshot = timing.publicSnapshot(performance.now());
  runtime.state.lastCompileTiming = snapshot;
  runtime.state.compileTimings = [...runtime.state.compileTimings, snapshot].slice(-16);
  runtime.state.compileTimingCount += 1;
  runtime.state.activeCompileTiming = null;
}

/** @returns {Promise<void>} */
function nextAnimationFrame() {
  return new Promise(
    /** @param {(value?: void) => void} resolve */
    (resolve) => requestAnimationFrame(() => resolve()),
  );
}

function updateDom() {
  const state = runtime.state;
  if (state.failed || (state.ready && !state.ok)) {
    setHudOpen(true);
  }
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
  setText("compile", formatCompileTiming(state));
  setText("frames", String(state.frameCount));
  setText("lock", state.pointerLocked ? "on" : state.pointerLockFallback ? "fallback" : "off");
  const status = document.getElementById("status");
  if (status) {
    status.textContent = state.status;
    status.dataset.ok = state.ok ? "true" : "false";
    status.dataset.ready = state.ready && state.status === "ready" ? "true" : "false";
  }
}

/** @param {Record<string, any>} state */
function formatCompileTiming(state) {
  const timing = state.activeCompileTiming ?? state.lastCompileTiming;
  if (!timing) {
    return "-";
  }
  const label = state.activeCompileTiming ? "active" : timing.status;
  const target = `${timing.targetCenterX},${timing.targetCenterZ}`;
  return `${label} ${target} ${Number(timing.totalMs || 0).toFixed(0)}ms gap ${Number(timing.maxFrameGapMs || 0).toFixed(0)}ms`;
}

/**
 * @param {string} id
 * @param {string} value
 */
function setText(id, value) {
  const element = document.getElementById(id);
  if (element) element.textContent = value;
}

function defaultInputKeys() {
  return Object.fromEntries(INPUT_KEY_NAMES.map((name) => [name, false]));
}

function normalizedDeployAssetVersion() {
  const version = /** @type {any} */ (globalThis).__MCLONE_NATIVE_WEB_ASSET_VERSION__;
  if (
    typeof version === "string"
    && version.length > 0
    && version !== "__MCLONE_NATIVE_WEB_ASSET_VERSION__"
  ) {
    return version;
  }
  return null;
}

/** @param {string} path */
function versionedUrl(path) {
  const url = new URL(path, import.meta.url);
  if (DEPLOY_ASSET_VERSION !== null) {
    url.searchParams.set("v", DEPLOY_ASSET_VERSION);
  }
  return url;
}

function defaultMovementImpulse() {
  return {
    active: false,
    left: 0,
    forward: 0,
  };
}

/** @param {unknown} value */
function sanitizeInputImpulse(value) {
  const impulse = Number(value);
  if (!Number.isFinite(impulse)) {
    return 0;
  }
  return Math.max(-1, Math.min(1, impulse));
}

function isHudOpen() {
  return document.getElementById("runtime-hud")?.hidden === false;
}

/** @param {boolean} open */
function setHudOpen(open) {
  const hud = document.getElementById("runtime-hud");
  const toggle = document.getElementById("hud-toggle");
  if (hud) {
    hud.hidden = !open;
  }
  if (toggle) {
    toggle.setAttribute("aria-expanded", open ? "true" : "false");
  }
  runtime.state.hudOpen = Boolean(open);
}

function defaultHudOpen() {
  return !hasTouchInput() && window.matchMedia("(min-width: 681px)").matches;
}

function hasTouchInput() {
  return (typeof navigator !== "undefined" && navigator.maxTouchPoints > 0)
    || window.matchMedia("(pointer: coarse)").matches;
}

/** @param {PointerEvent} event */
function isTouchPointer(event) {
  return event.pointerType === "touch" || event.pointerType === "pen";
}

/**
 * @param {EventTarget | null} target
 * @param {number} pointerId
 */
function trySetPointerCapture(target, pointerId) {
  if (!target || !("setPointerCapture" in target) || typeof target.setPointerCapture !== "function") {
    return;
  }
  try {
    target.setPointerCapture(pointerId);
  } catch (_error) {
    // Some synthetic or browser-generated pointer streams cannot be captured.
  }
}

/** @param {WasmReport} interaction */
function formatInteractionStatus(interaction) {
  if (!interaction?.ok) {
    return "idle";
  }
  if (!interaction.hit) {
    return `${interaction.action}: miss`;
  }
  return `${interaction.action}: ${interaction.changed ? "changed" : "same"}`;
}

/** @param {Record<string, any> | null | undefined} value */
function applyHotbarState(value) {
  if (!value || typeof value.selectedHotbarSlot === "undefined") {
    return;
  }
  const slot = Number(value.selectedHotbarSlot);
  if (Number.isInteger(slot) && slot >= 0 && slot < 9) {
    runtime.state.selectedHotbarSlot = slot;
  }
}

/** @param {KeyboardEvent} event */
function hotbarSlotForEvent(event) {
  const code = keyboardCode(event);
  if (code?.startsWith("Digit") || code?.startsWith("Numpad")) {
    const digit = Number(code.slice(-1));
    if (Number.isInteger(digit) && digit >= 1 && digit <= 9) {
      return digit - 1;
    }
    return null;
  }
  return code === null ? hotbarSlotForLegacyKey(event.key) : null;
}

/** @param {string} key */
function hotbarSlotForLegacyKey(key) {
  if (key.length !== 1) {
    return null;
  }
  const digit = Number(key);
  if (Number.isInteger(digit) && digit >= 1 && digit <= 9) {
    return digit - 1;
  }
  return null;
}

/** @param {WasmReport} interaction */
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

/** @param {unknown} error */
function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}

globalThis.__mcloneNativeAppReady = boot();
