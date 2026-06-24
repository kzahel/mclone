import { RenderSectionWorkerCompiler, fetchAssetPack } from "./mclone-render-compiler-shared.js";
import {
  INPUT_KEY_NAMES,
  applyHotbarState,
  bindInput,
  defaultInputKeys,
  defaultMovementImpulse,
  sanitizeInputImpulse,
} from "./mclone-web-input.js";
import {
  DEFAULT_LOOK_SENSITIVITY,
  bindMenu,
  formatInteractionStatus,
  loadStoredSettings,
  setHudOpen,
  setMenuOpen,
  updateDom,
} from "./mclone-web-hud.js";
import { TouchControls } from "./mclone-web-touch.js";
import type { WebChunkRenderSession, WebCompileTiming } from "mclone-web-client-wasm";

// The wasm-bindgen module namespace (generated `.d.ts`, emitted by `wasm-bindgen --typescript`).
// Loaded at runtime via a dynamic `import()` of a versioned URL; the bare specifier is path-mapped
// in tsconfig.json and only ever appears in type positions. Casting the dynamic import to this type
// is what makes the live wasm call sites (`session.advanceCameraFrame(...)` &c.) checkable against
// the real export signatures — the "single biggest win" of 070 Stage 2.
type WasmModule = typeof import("mclone-web-client-wasm");

// A wasm-return object — camera/frame/report/target/interaction/doorbell. The generated `.d.ts`
// types every `WebChunkRenderSession` method return as `any` (wasm-bindgen cannot describe the
// serde shape), so these are read coercion-guarded (`Number(...)`/`Boolean(...)`/`?.ok`). Naming
// the boundary documents intent and keeps internal field reads consistent.
type WasmReport = Record<string, any>;

type RenderCompiler = InstanceType<typeof RenderSectionWorkerCompiler>;
type InputKeys = Record<string, boolean>;
type TouchMovementImpulse = ReturnType<typeof defaultMovementImpulse>;

interface PendingCompile {
  timing: WebCompileTiming;
  workerPromise: Promise<any> | null;
  workerError: string | null;
}

interface AppRuntimeState extends Record<string, any> {
  ok: boolean;
  ready: boolean;
  failed: boolean;
  status: string;
  frameCount: number;
  renderCount: number;
  lastFrameGapMs: number;
  maxFrameGapMs: number;
  compileTimingCount: number;
  compileTimings: WasmReport[];
  activeCompileTiming: WasmReport | null;
  lastCompileTiming: WasmReport | null;
}

interface AppRuntime {
  ready: boolean;
  state: AppRuntimeState;
  queueMouseDelta?: (dx: number, dy: number) => void;
  setInputKey?: (name: string, down: boolean) => boolean;
  adjustCameraSpeed?: (amount: number) => WasmReport | null;
  previewBlockTarget?: () => WasmReport | null;
  openNativeTitleUi?: () => WasmReport | null;
  openNativePauseUi?: () => WasmReport | null;
  closeNativeUi?: () => WasmReport | null;
  handleNativeUiKey?: (key: string) => WasmReport | null;
  handleNativeUiPointerMove?: (clientX: number, clientY: number, pointerType?: string) => WasmReport | null;
  handleNativeUiPointerDown?: (clientX: number, clientY: number, pointerType?: string) => WasmReport | null;
  handleNativeUiPointerUp?: (clientX: number, clientY: number, pointerType?: string) => WasmReport | null;
  touchControlState?: () => any;
  setHudOpen?: (open: boolean) => void;
  setMenuOpen?: (open: boolean) => void;
}

declare global {
  var __mcloneWebApp: AppRuntime;
  var __mcloneNativeAppReady: Promise<WasmReport>;
  var __MCLONE_NATIVE_WEB_ASSET_VERSION__: string | undefined;
}

const DEPLOY_ASSET_VERSION = normalizedDeployAssetVersion();
const BINDGEN_JS_URL = versionedUrl("./pkg/mclone_web_client.js");
const BINDGEN_WASM_URL = versionedUrl("./pkg/mclone_web_client_bg.wasm");
const RENDER_COMPILER_WORKER_URL = versionedUrl("./mclone-render-compiler-worker.js");
const SERVER_WORKER_URL = versionedUrl("./mclone-integrated-server-worker.js");
const SERVER_JOB_WORKER_URL = versionedUrl("./mclone-server-job-worker.js");
const ASSET_PACK_URL = versionedUrl("/reference/minecraft-1.17.1/extracted.zip");

const DEFAULT_RADIUS_CHUNKS = 1;
const MIN_RADIUS_CHUNKS = 1;
const MAX_RADIUS_CHUNKS = 16;
const MAX_FRAME_DT_SECONDS = 0.05;

const runtime: AppRuntime = {
  ready: false,
  state: {
    ok: false,
    ready: false,
    failed: false,
    centerX: 0,
    centerZ: 0,
    loadedCenterX: null,
    loadedCenterZ: null,
    radiusChunks: DEFAULT_RADIUS_CHUNKS,
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
    guiCommandCount: 0,
    uiActive: false,
    uiCoversWorld: false,
    nativeUiScreen: "none",
    nativeUiOptionsParent: null,
    lastUiAction: null,
    sectionOcclusionCulling: true,
    forceFullbright: false,
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

async function boot(): Promise<WasmReport> {
  const app = new WebChunkApp();
  runtime.queueMouseDelta = (dx: number, dy: number) => app.queueMouseDelta(dx, dy);
  runtime.setInputKey = (name: string, down: boolean) => app.setInputKey(name, down);
  runtime.adjustCameraSpeed = (amount: number) => app.adjustCameraSpeed(amount);
  runtime.previewBlockTarget = () => runtime.state.currentTarget;
  runtime.openNativeTitleUi = () => app.openNativeTitleUi();
  runtime.openNativePauseUi = () => app.openNativePauseUi();
  runtime.closeNativeUi = () => app.closeNativeUi();
  runtime.handleNativeUiKey = (key: string) => app.handleNativeUiKey(key);
  runtime.handleNativeUiPointerMove = (clientX: number, clientY: number, pointerType?: string) => (
    app.handleNativeUiPointerMove(clientX, clientY, pointerType)
  );
  runtime.handleNativeUiPointerDown = (clientX: number, clientY: number, pointerType?: string) => (
    app.handleNativeUiPointerDown(clientX, clientY, pointerType)
  );
  runtime.handleNativeUiPointerUp = (clientX: number, clientY: number, pointerType?: string) => (
    app.handleNativeUiPointerUp(clientX, clientY, pointerType)
  );
  runtime.touchControlState = () => app.touchControls?.snapshot() ?? null;
  runtime.setHudOpen = (open: boolean) => setHudOpen(runtime.state, Boolean(open));
  runtime.setMenuOpen = (open: boolean) => setMenuOpen(app, runtime.state, Boolean(open));
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
    updateDom(runtime.state);
    return snapshotState();
  }
}

class WebChunkApp {
  canvas: HTMLCanvasElement;
  status: HTMLElement | null;
  hud: HTMLElement | null;
  hudToggle: HTMLElement | null;
  module: WasmModule | null;
  session: WebChunkRenderSession | null;
  compiler: RenderCompiler | null;
  keys: InputKeys;
  touchKeys: InputKeys;
  touchMovementImpulse: TouchMovementImpulse;
  touchControls: TouchControls | null;
  lookSensitivity: number;
  mouseDeltaX: number;
  mouseDeltaY: number;
  compileSequence: number;
  pendingTimings: Map<number, PendingCompile>;
  finalizingCount: number;
  hasRendered: boolean;
  loadedCenter: { centerX: number, centerZ: number } | null;
  pointerDragging: boolean;
  pointerDown: { button: number, enabled: boolean, movement: number } | null;
  radiusChunks: number;
  sectionOcclusionCulling: boolean;
  forceFullbright: boolean;
  animationFrame: number;
  lastFrameTime: number;
  tickFrameBusy: boolean;
  sessionBusy: boolean;

  constructor() {
    // Required for the app to run; `init()` re-validates with `instanceof HTMLCanvasElement` and
    // throws if it is missing, so treating it as a non-null canvas here is sound for the lifecycle.
    this.canvas = document.getElementById("mclone-canvas") as HTMLCanvasElement;
    this.status = document.getElementById("status");
    this.hud = document.getElementById("runtime-hud");
    this.hudToggle = document.getElementById("hud-toggle");
    this.module = null;
    this.session = null;
    this.compiler = null;
    this.keys = defaultInputKeys() as InputKeys;
    this.touchKeys = defaultInputKeys() as InputKeys;
    this.touchMovementImpulse = defaultMovementImpulse();
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
    this.pendingTimings = new Map();
    this.finalizingCount = 0;
    this.hasRendered = false;
    this.loadedCenter = null;
    this.pointerDragging = false;
    this.pointerDown = null;
    this.radiusChunks = DEFAULT_RADIUS_CHUNKS;
    this.sectionOcclusionCulling = true;
    this.forceFullbright = false;
    this.animationFrame = 0;
    this.lastFrameTime = 0;
    this.tickFrameBusy = false;
    this.sessionBusy = false;
  }

  async init(): Promise<void> {
    if (!(this.canvas instanceof HTMLCanvasElement)) {
      throw new Error("missing canvas#mclone-canvas");
    }
    runtime.state.pointerLockSupported = typeof this.canvas.requestPointerLock === "function";
    this.canvas.focus();

    runtime.state.status = "loading wasm";
    updateDom(runtime.state);
    const module = await import(BINDGEN_JS_URL.href) as WasmModule;
    await module.default(BINDGEN_WASM_URL.href);
    this.module = module;
    for (const name of ["mclone_web_create_worker_chunk_render_session"]) {
      if (typeof (module as Record<string, any>)[name] !== "function") {
        throw new Error(`missing ${name} export`);
      }
    }

    runtime.state.status = "loading assets";
    updateDom(runtime.state);
    const assetPack = await fetchAssetPack(ASSET_PACK_URL);
    runtime.state.status = "initializing webgpu";
    updateDom(runtime.state);
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
      "openTitleUi",
      "openPauseUi",
      "closeUi",
      "uiStatus",
      "handleUiKey",
      "handleUiPointerMove",
      "handleUiPointerDown",
      "handleUiPointerUp",
    ]) {
      if (typeof (this.session as unknown as Record<string, any>)[name] !== "function") {
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
    bindMenu(this, runtime.state);
    bindInput(this, runtime.state, () => updateDom(runtime.state));
    this.touchControls = new TouchControls(this, runtime.state);
    this.syncCanvasSize();
    this.applyCameraState(this.session.cameraFrameState());
    this.applyTargetState(this.session.previewBlockTarget());

    runtime.state.status = "rendering";
    updateDom(runtime.state);
    // 067 Stage 3: warm up the streaming loop to idle so the first presented frame has
    // terrain (the web analog of desktop's pre-render `sync_all_render_sections`).
    await this.warmUpStreamingToIdle();
  }

  start(): void {
    this.lastFrameTime = performance.now();
    const frame = (now: number) => {
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

  async tickFrame(now: number): Promise<void> {
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
    const uiActive = runtime.state.uiActive === true;
    const mouseDeltaX = uiActive ? 0 : this.mouseDeltaX;
    const mouseDeltaY = uiActive ? 0 : this.mouseDeltaY;
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.syncCanvasSize();
    const keys = uiActive ? (defaultInputKeys() as InputKeys) : this.currentInputKeys();
    const movementImpulse = uiActive ? defaultMovementImpulse() : this.currentMovementImpulse();

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
  async warmUpStreamingToIdle(): Promise<void> {
    // Called from `init()` only after `this.session` is assigned, so it is non-null here.
    const session = this.session as WebChunkRenderSession;
    const deadline = performance.now() + 30_000;
    while (performance.now() < deadline) {
      const idle = await this.streamFrameOnce({ awaitWorker: true });
      if (idle && this.hasRendered && Number(runtime.state.residentSectionCount) > 0) {
        break;
      }
      await nextAnimationFrame();
    }
    let last: { x: number, z: number } | null = null;
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
  async streamFrameOnce(options: { awaitWorker?: boolean } = {}): Promise<boolean> {
    if (!this.session || !this.compiler) {
      return true;
    }
    const session = this.session;
    const syncStart = performance.now();
    let frame: WasmReport;
    try {
      frame = await this.withSessionAsync(() => session.syncCameraRenderFrame(this.radiusChunks));
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
      updateDom(runtime.state);
      return true;
    }
    const syncMs = performance.now() - syncStart;
    const workerPromise = this.handleStreamingFrame(frame, syncMs);
    if (workerPromise && options.awaitWorker) {
      await workerPromise.catch(() => {});
    }
    return runtime.state.streamingSettled === true;
  }

  handleStreamingFrame(frame: WasmReport, syncMs: number): Promise<any> | null {
    if (!frame?.ok) {
      runtime.state.ok = false;
      runtime.state.status = frame?.reason ?? "streaming render frame failed";
      updateDom(runtime.state);
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
    let workerPromise: Promise<any> | null = null;
    if (frame.doorbell) {
      workerPromise = this.startAndPostCompileTiming(frame.doorbell, syncMs);
    }
    updateDom(runtime.state);
    return workerPromise;
  }

  // Begin a per-compile timing for the doorbell armed this frame and relay it to the
  // worker. The worker writes the packed result into the resident ring (drained by the
  // next frame's poll); the returned promise resolves with the worker metrics report.
  startAndPostCompileTiming(doorbell: WasmReport, syncMs: number): Promise<any> {
    // Reached only from `handleStreamingFrame` via `streamFrameOnce`, which already guards
    // `this.compiler` and (via boot/init) `this.module` non-null.
    const compiler = this.compiler as RenderCompiler;
    const module = this.module as WasmModule;
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
    const pending: PendingCompile = { timing, workerPromise: null, workerError: null };
    const workerStart = performance.now();
    const workerPromise = compiler.compileWithDoorbell(doorbell).then((compiled: any) => {
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
    workerPromise.catch((error: unknown) => {
      pending.workerError = stringifyError(error);
      console.error(error);
    });
    return workerPromise;
  }

  // Finalize the timing for the compile applied this frame: await its worker metrics (so
  // the record carries the transport/byte diagnostics), merge the Rust apply report, and
  // publish it to runtime.state.compileTimings.
  finalizeCompileTiming(frame: WasmReport, syncMs: number): void {
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
      updateDom(runtime.state);
    })();
  }

  applyCameraState(camera: WasmReport): void {
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
    applyHotbarState(camera, runtime.state);
  }

  applyReport(report: WasmReport): void {
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
    runtime.state.guiCommandCount = report.guiCommandCount;
    runtime.state.uiActive = Boolean(report.uiActive);
    runtime.state.uiCoversWorld = Boolean(report.uiCoversWorld);
    runtime.state.nativeUiScreen = String(report.uiScreen ?? runtime.state.nativeUiScreen ?? "none");
    runtime.state.nativeUiOptionsParent = report.uiOptionsParent ?? null;
    if (typeof report.sectionOcclusionCulling !== "undefined") {
      this.sectionOcclusionCulling = Boolean(report.sectionOcclusionCulling);
    }
    if (typeof report.forceFullbright !== "undefined") {
      this.forceFullbright = Boolean(report.forceFullbright);
    }
    runtime.state.sectionOcclusionCulling = this.sectionOcclusionCulling;
    runtime.state.forceFullbright = this.forceFullbright;
    runtime.state.status = "ready";
    runtime.state.lastReport = report;
  }

  applyTargetState(target: WasmReport): void {
    if (!target?.ok) {
      return;
    }
    runtime.state.currentTarget = target;
    applyHotbarState(target, runtime.state);
  }

  async interactBlock(action: string): Promise<WasmReport | null> {
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
      applyHotbarState(interaction, runtime.state);
      // 067 Stage 3: a block edit publishes a SectionBlockUpdates server update, which the
      // streaming loop marks render-dirty on its next drain and recompiles automatically —
      // no explicit compile request needed.
      updateDom(runtime.state);
      return interaction;
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
      updateDom(runtime.state);
      return null;
    }
  }

  selectHotbarSlot(slot: number): boolean {
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
    applyHotbarState(hotbar, runtime.state);
    updateDom(runtime.state);
    return true;
  }

  adjustCameraSpeed(amount: number): WasmReport | null {
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
    updateDom(runtime.state);
    return camera;
  }

  openNativeTitleUi(): WasmReport | null {
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.openNativeTitleUi(), 0);
      return null;
    }
    const report = this.session.openTitleUi();
    this.applyNativeUiReport(report);
    return report;
  }

  openNativePauseUi(): WasmReport | null {
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.openNativePauseUi(), 0);
      return null;
    }
    const report = this.session.openPauseUi();
    this.applyNativeUiReport(report);
    return report;
  }

  closeNativeUi(): WasmReport | null {
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.closeNativeUi(), 0);
      return null;
    }
    const report = this.session.closeUi();
    this.applyNativeUiReport(report);
    return report;
  }

  handleNativeUiKey(key: string): WasmReport | null {
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.handleNativeUiKey(key), 0);
      return deferredUiReport();
    }
    const report = this.session.handleUiKey(key);
    this.applyNativeUiReport(report);
    return report;
  }

  handleNativeUiPointerMove(clientX: number, clientY: number, pointerType = "mouse"): WasmReport | null {
    if (!this.session) {
      return null;
    }
    const point = this.canvasPixelPoint(clientX, clientY);
    if (this.sessionBusy) {
      setTimeout(() => this.handleNativeUiPointerMove(clientX, clientY, pointerType), 0);
      return deferredUiReport();
    }
    const report = this.session.handleUiPointerMove(point.x, point.y, this.radiusChunks);
    this.applyNativeUiReport(report, { pointerType });
    return report;
  }

  handleNativeUiPointerDown(clientX: number, clientY: number, pointerType = "mouse"): WasmReport | null {
    if (!this.session) {
      return null;
    }
    const point = this.canvasPixelPoint(clientX, clientY);
    if (this.sessionBusy) {
      setTimeout(() => this.handleNativeUiPointerDown(clientX, clientY, pointerType), 0);
      return deferredUiReport();
    }
    const report = this.session.handleUiPointerDown(point.x, point.y);
    this.applyNativeUiReport(report, { pointerType });
    return report;
  }

  handleNativeUiPointerUp(clientX: number, clientY: number, pointerType = "mouse"): WasmReport | null {
    if (!this.session) {
      return null;
    }
    const point = this.canvasPixelPoint(clientX, clientY);
    if (this.sessionBusy) {
      setTimeout(() => this.handleNativeUiPointerUp(clientX, clientY, pointerType), 0);
      return deferredUiReport();
    }
    const report = this.session.handleUiPointerUp(point.x, point.y, this.radiusChunks);
    this.applyNativeUiReport(report, { fromPointer: true, pointerType });
    return report;
  }

  applyNativeUiReport(report: WasmReport | null | undefined, options: { fromPointer?: boolean, pointerType?: string } = {}): void {
    if (!report?.ok) {
      return;
    }
    runtime.state.uiActive = Boolean(report.active ?? report.uiActive);
    runtime.state.uiCoversWorld = Boolean(report.coversWorld ?? report.uiCoversWorld);
    runtime.state.nativeUiScreen = String(report.screen ?? report.uiScreen ?? "none");
    runtime.state.nativeUiOptionsParent = report.optionsParent ?? report.uiOptionsParent ?? null;
    this.sectionOcclusionCulling = Boolean(report.sectionOcclusionCulling);
    this.forceFullbright = Boolean(report.forceFullbright);
    runtime.state.sectionOcclusionCulling = this.sectionOcclusionCulling;
    runtime.state.forceFullbright = this.forceFullbright;
    if (typeof report.renderDistance !== "undefined") {
      this.radiusChunks = clampRadiusChunks(report.renderDistance);
      runtime.state.radiusChunks = this.radiusChunks;
    }
    if (report.action) {
      runtime.state.lastUiAction = report;
    }
    if (runtime.state.uiActive) {
      this.releasePointerLockForUi();
      this.clearGameplayInput();
    }
    if (
      (report.action === "startWorld" || report.action === "resume")
      && options.fromPointer
      && options.pointerType !== "touch"
    ) {
      this.requestPointerLock();
    }
    updateDom(runtime.state);
  }

  canvasPixelPoint(clientX: number, clientY: number): { x: number, y: number } {
    const rect = this.canvas.getBoundingClientRect();
    const scaleX = rect.width > 0 ? this.canvas.width / rect.width : 1;
    const scaleY = rect.height > 0 ? this.canvas.height / rect.height : 1;
    return {
      x: (Number(clientX) - rect.left) * scaleX,
      y: (Number(clientY) - rect.top) * scaleY,
    };
  }

  clearGameplayInput(): void {
    for (const name of INPUT_KEY_NAMES) {
      this.keys[name] = false;
      this.touchKeys[name] = false;
    }
    this.touchMovementImpulse = defaultMovementImpulse();
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.pointerDragging = false;
    this.pointerDown = null;
    this.touchControls?.clearAll();
  }

  releasePointerLockForUi(): void {
    if (document.pointerLockElement === this.canvas && typeof document.exitPointerLock === "function") {
      document.exitPointerLock();
    }
    runtime.state.pointerLocked = false;
    runtime.state.pointerLockFallback = false;
  }

  async withSessionAsync<T>(operation: () => T | Promise<T>): Promise<Awaited<T>> {
    await this.waitForSessionIdle();
    this.sessionBusy = true;
    try {
      return await operation();
    } finally {
      this.sessionBusy = false;
    }
  }

  async waitForSessionIdle(): Promise<void> {
    for (let attempt = 0; this.sessionBusy && attempt < 10_000; attempt += 1) {
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
    if (this.sessionBusy) {
      throw new Error("timed out waiting for WebChunkRenderSession async operation");
    }
  }

  setInputKey(name: string, down: boolean): boolean {
    if (!(name in this.keys)) {
      return false;
    }
    this.keys[name] = Boolean(down);
    return true;
  }

  setTouchKey(name: string, down: boolean): boolean {
    if (!(name in this.touchKeys)) {
      return false;
    }
    this.touchKeys[name] = Boolean(down);
    return true;
  }

  setTouchKeys(keys: Record<string, boolean>): void {
    for (const [name, down] of Object.entries(keys)) {
      this.setTouchKey(name, down);
    }
  }

  setTouchMovementImpulse(left: number, forward: number, active: boolean): void {
    this.touchMovementImpulse = {
      active: Boolean(active),
      left: sanitizeInputImpulse(left),
      forward: sanitizeInputImpulse(forward),
    };
  }

  currentInputKeys(): InputKeys {
    const keys = defaultInputKeys() as InputKeys;
    for (const name of INPUT_KEY_NAMES) {
      keys[name] = Boolean(this.keys[name] || this.touchKeys[name]);
    }
    return keys;
  }

  currentMovementImpulse(): TouchMovementImpulse {
    return { ...this.touchMovementImpulse };
  }

  clearTouchKeys(names: readonly string[] = INPUT_KEY_NAMES): void {
    for (const name of names) {
      this.setTouchKey(name, false);
    }
  }

  queueMouseDelta(dx: number, dy: number): void {
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

  currentCameraCenter(): { centerX: number, centerZ: number } {
    return {
      centerX: Number(runtime.state.centerX) || 0,
      centerZ: Number(runtime.state.centerZ) || 0,
    };
  }

  recordFrameGap(frameGapMs: number): void {
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

  requestPointerLock(): void {
    if (runtime.state.uiActive === true) {
      return;
    }
    runtime.state.pointerLockAttempted = true;
    if (typeof this.canvas.requestPointerLock === "function") {
      const result = this.canvas.requestPointerLock() as Promise<void> | undefined;
      if (result && typeof result.catch === "function") {
        result.catch(() => {
          runtime.state.pointerLockFallback = true;
          updateDom(runtime.state);
        });
      }
    } else {
      runtime.state.pointerLockFallback = true;
    }
    updateDom(runtime.state);
  }

  updatePointerLockState(): void {
    runtime.state.pointerLocked = document.pointerLockElement === this.canvas;
    runtime.state.pointerLockFallback =
      runtime.state.pointerLockAttempted && !runtime.state.pointerLocked;
    updateDom(runtime.state);
  }

  syncCanvasSize(): void {
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

function publishActiveCompileTiming(timing: WebCompileTiming): void {
  runtime.state.activeCompileTiming = timing.publicSnapshot(performance.now());
}

function recordCompileTiming(timing: WebCompileTiming): void {
  const snapshot = timing.publicSnapshot(performance.now());
  runtime.state.lastCompileTiming = snapshot;
  runtime.state.compileTimings = [...runtime.state.compileTimings, snapshot].slice(-16);
  runtime.state.compileTimingCount += 1;
  runtime.state.activeCompileTiming = null;
}

function nextAnimationFrame(): Promise<void> {
  return new Promise(
    (resolve: (value?: void) => void) => requestAnimationFrame(() => resolve()),
  );
}

function normalizedDeployAssetVersion(): string | null {
  const version = globalThis.__MCLONE_NATIVE_WEB_ASSET_VERSION__;
  if (
    typeof version === "string"
    && version.length > 0
    && version !== "__MCLONE_NATIVE_WEB_ASSET_VERSION__"
  ) {
    return version;
  }
  return null;
}

function versionedUrl(path: string): URL {
  const url = new URL(path, import.meta.url);
  if (DEPLOY_ASSET_VERSION !== null) {
    url.searchParams.set("v", DEPLOY_ASSET_VERSION);
  }
  return url;
}

function snapshotState(): WasmReport {
  return JSON.parse(JSON.stringify(runtime.state));
}

function clampRadiusChunks(value: unknown): number {
  const radius = Math.round(Number(value));
  if (!Number.isFinite(radius)) {
    return DEFAULT_RADIUS_CHUNKS;
  }
  return Math.min(MAX_RADIUS_CHUNKS, Math.max(MIN_RADIUS_CHUNKS, radius));
}

function deferredUiReport(): WasmReport {
  return {
    ok: true,
    handled: runtime.state.uiActive === true,
    deferred: true,
    active: runtime.state.uiActive === true,
    coversWorld: runtime.state.uiCoversWorld === true,
    screen: runtime.state.nativeUiScreen ?? "none",
  };
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}

globalThis.__mcloneNativeAppReady = boot();
