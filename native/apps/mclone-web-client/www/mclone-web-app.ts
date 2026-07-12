import { RenderSectionWorkerCompiler, fetchAssetPack } from "./mclone-render-compiler-shared.js";
import type { RenderCompilerAssetSelection } from "./mclone-render-compiler-shared.js";
import {
  INPUT_KEY_NAMES,
  applyHotbarState,
  bindInput,
  defaultInputKeys,
  defaultMovementImpulse,
  sanitizeInputImpulse,
} from "./mclone-web-input.js";
import {
  clampLookSensitivity,
  DEFAULT_LOOK_SENSITIVITY,
  formatInteractionStatus,
  loadStoredSettings,
  storeLookSensitivity,
  storeTouchControlsMode,
} from "./mclone-web-settings.js";
import type { TouchControlsMode } from "./mclone-web-settings.js";
import { TouchControls, hasTouchInput } from "./mclone-web-touch.js";
import type { TouchOverlayState } from "./mclone-web-touch.js";
import {
  createIndexedDbCatalogWorld,
  deleteIndexedDbCatalogWorld,
  listIndexedDbCatalogWorlds,
  openIndexedDbCatalogWorld,
  openWorldDb,
  setIndexedDbCatalogPolicy,
} from "./mclone-web-world-catalog.js";
import type { WebLocalWorldSummary } from "./mclone-web-world-catalog.js";
import type { WebSceneHost, WebCompileTiming } from "mclone-web-client-wasm";

// The wasm-bindgen module namespace (generated `.d.ts`, emitted by `wasm-bindgen --typescript`).
// Loaded at runtime via a dynamic `import()` of a versioned URL; the bare specifier is path-mapped
// in tsconfig.json and only ever appears in type positions. Casting the dynamic import to this type
// is what makes the live wasm call sites (`session.renderFrame(...)` &c.) checkable against
// the real export signatures — the "single biggest win" of 070 Stage 2.
type WasmModule = typeof import("mclone-web-client-wasm");

// A wasm-return object — camera/frame/report/target/interaction/doorbell. The generated `.d.ts`
// types every `WebSceneHost` method return as `any` (wasm-bindgen cannot describe the
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

interface WebStartupPlan extends WasmReport {
  renderDistance: number;
  remoteWebSocketUrl?: string;
  sectionOcclusionCulling: boolean;
  forceFullbright: boolean;
  renderColorProfile: string;
}

type WebStartupConfig = ReturnType<WasmModule["mclone_web_startup_options_from_query"]>;

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
  blockStateAt?: (x: number, y: number, z: number) => WasmReport | null;
  openNativeTitleUi?: () => WasmReport | null;
  openNativePauseUi?: () => WasmReport | null;
  pauseRendering?: () => void;
  resumeRendering?: () => void;
  renderOverviewFrame?: () => WasmReport | null;
  setDebugOverlay?: (visible: boolean) => WasmReport | null;
  openNativeHelpUi?: () => WasmReport | null;
  closeNativeUi?: () => WasmReport | null;
  handleNativeUiKey?: (key: string) => WasmReport | null;
  handleNativeUiPointerMove?: (clientX: number, clientY: number, pointerType?: string) => WasmReport | null;
  handleNativeUiPointerDown?: (clientX: number, clientY: number, pointerType?: string) => WasmReport | null;
  handleNativeUiPointerUp?: (clientX: number, clientY: number, pointerType?: string) => WasmReport | null;
  setNativeTouchLookSensitivity?: (value: number, available?: boolean, persist?: boolean) => WasmReport | null;
  setNativeTouchControlsMode?: (mode: TouchControlsMode, persist?: boolean) => WasmReport | null;
  touchControlState?: () => any;
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
const AUTHORED_ASSET_PACK_URL = versionedUrl("/first-party-packs/mclone-authored.pbp");
const FALLBACK_ASSET_PACK_URL = versionedUrl("/first-party-packs/mclone-generated-fallback.pbp");

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
    renderPendingWork: false,
    renderDirtyChunkCount: 0,
    renderDirtySectionCount: 0,
    renderInflightSectionCount: 0,
    compileInFlight: false,
    compileInFlightCount: 0,
    compileFinalizingCount: 0,
    streamingSettled: false,
    startupReady: false,
    startupProgressVisible: false,
    startupProgressObserved: false,
    startupProgressReadyChunks: 0,
    startupProgressChunkCount: 0,
    startupProgressPercent: 0,
    startupHoldCameraY: null as number | null,
    minimumPreStartupCameraY: null as number | null,
    startupAdmissionFrame: null as number | null,
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
    flatHudRetainedRebuilds: 0,
    flatHudRetainedCacheHits: 0,
    uiActive: false,
    uiCoversWorld: false,
    nativeUiScreen: "none",
    nativeUiOptionsParent: null,
    lastUiAction: null,
    sessionState: "none",
    sessionKind: "unknown",
    sessionSeed: null,
    sessionSeedText: null,
    sessionRemoteEndpoint: null,
    sessionFailureMessage: null,
    sessionStatusVisible: false,
    sessionStatusOk: true,
    sessionStatusMessage: "",
    statusOverlayVisible: false,
    statusOverlayOk: true,
    statusOverlayMessage: "",
    sectionOcclusionCulling: true,
    forceFullbright: false,
    renderColorProfile: "vanilla",
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
    clientDeferredChunkDropBacklogItems: 0,
    worldgenMailboxKind: "unknown",
    lightStatusMailboxKind: "unknown",
    worldgenMailboxPendingJobs: 0,
    lightStatusMailboxPendingStatuses: 0,
    runnerFrameMetrics: null,
    worldgenJobFrameMetrics: null,
    lightStatusJobFrameMetrics: null,
    debugOverlayVisible: false,
    sessionBusy: false,
    worldCatalogCompletionCount: 0,
    lookSensitivity: DEFAULT_LOOK_SENSITIVITY,
    touchControlsMode: "auto",
    touchLookSensitivityAvailable: false,
    touchControlsVisible: false,
    touchJoystickActive: false,
    touchMovementLeftImpulse: 0,
    touchMovementForwardImpulse: 0,
    touchLookActive: false,
    touchButtonActiveCount: 0,
    clientHost: "worker-integrated",
    remoteWebSocketUrl: null,
    status: "booting",
    bootstrapStatusRetired: false,
  },
};

globalThis.__mcloneWebApp = runtime;
installFirstTouchFullscreen(runtime.state);

async function boot(): Promise<WasmReport> {
  const app = new WebFrameDriver();
  runtime.queueMouseDelta = (dx: number, dy: number) => app.queueMouseDelta(dx, dy);
  runtime.setInputKey = (name: string, down: boolean) => app.setInputKey(name, down);
  runtime.adjustCameraSpeed = (amount: number) => app.adjustCameraSpeed(amount);
  runtime.previewBlockTarget = () => runtime.state.currentTarget;
  runtime.blockStateAt = (x: number, y: number, z: number) => app.blockStateAt(x, y, z);
  runtime.openNativeTitleUi = () => app.openNativeTitleUi();
  runtime.openNativePauseUi = () => app.openNativePauseUi();
  runtime.pauseRendering = () => app.pauseRendering();
  runtime.resumeRendering = () => app.resumeRendering();
  runtime.renderOverviewFrame = () => app.renderOverviewFrame();
  runtime.setDebugOverlay = (visible: boolean) => app.setNativeDebugOverlay(visible);
  runtime.openNativeHelpUi = () => app.openNativeHelpUi();
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
  runtime.setNativeTouchLookSensitivity = (value: number, available?: boolean, persist?: boolean) => (
    app.setNativeTouchLookSensitivity(value, available, persist)
  );
  runtime.setNativeTouchControlsMode = (mode: TouchControlsMode, persist?: boolean) => (
    app.setNativeTouchControlsMode(mode, persist)
  );
  runtime.touchControlState = () => app.touchControls?.snapshot() ?? null;
  try {
    await app.init();
    runtime.ready = true;
    runtime.state.ready = true;
    runtime.state.ok = true;
    runtime.state.failed = false;
    runtime.state.status = "ready";
    app.setNativeStatusOverlay("ready", true, false);
    app.start();
    return snapshotState();
  } catch (error) {
    runtime.ready = false;
    runtime.state.ok = false;
    runtime.state.failed = true;
    runtime.state.status = stringifyError(error);
    app.setNativeStatusOverlay(runtime.state.status, false, true);
    publishRuntimeState(runtime.state);
    return snapshotState();
  }
}

class WebFrameDriver {
  canvas: HTMLCanvasElement;
  module: WasmModule | null;
  session: WebSceneHost | null;
  compiler: RenderCompiler | null;
  assetPack: Uint8Array | null;
  authoredAssetPack: Uint8Array | null;
  fallbackAssetPack: Uint8Array | null;
  keys: InputKeys;
  touchKeys: InputKeys;
  touchMovementImpulse: TouchMovementImpulse;
  touchControls: TouchControls | null;
  pendingTouchOverlay: TouchOverlayState | null;
  lookSensitivity: number;
  touchControlsMode: TouchControlsMode;
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
  pendingAssetCompilerSwap: {
    previous: RenderCompiler;
    candidate: RenderCompiler;
    epoch: number;
  } | null;

  constructor() {
    // Required for the app to run; `init()` re-validates with `instanceof HTMLCanvasElement` and
    // throws if it is missing, so treating it as a non-null canvas here is sound for the lifecycle.
    this.canvas = document.getElementById("mclone-canvas") as HTMLCanvasElement;
    this.module = null;
    this.session = null;
    this.compiler = null;
    this.assetPack = null;
    this.authoredAssetPack = null;
    this.fallbackAssetPack = null;
    this.keys = defaultInputKeys() as InputKeys;
    this.touchKeys = defaultInputKeys() as InputKeys;
    this.touchMovementImpulse = defaultMovementImpulse();
    this.touchControls = null;
    this.pendingTouchOverlay = null;
    const settings = loadStoredSettings();
    this.lookSensitivity = settings.lookSensitivity;
    this.touchControlsMode = settings.touchControlsMode;
    runtime.state.lookSensitivity = this.lookSensitivity;
    runtime.state.touchControlsMode = this.touchControlsMode;
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
    this.pendingAssetCompilerSwap = null;
  }

  async init(): Promise<void> {
    if (!(this.canvas instanceof HTMLCanvasElement)) {
      throw new Error("missing canvas#mclone-canvas");
    }
    runtime.state.pointerLockSupported = typeof this.canvas.requestPointerLock === "function";
    this.canvas.focus();

    runtime.state.status = "loading wasm";
    publishRuntimeState(runtime.state);
    const module = await import(BINDGEN_JS_URL.href) as WasmModule;
    await module.default(BINDGEN_WASM_URL.href);
    setIndexedDbCatalogPolicy(module);
    this.module = module;
    const requiredExports = [
      "mclone_web_startup_options_from_query",
      "mclone_web_create_worker_scene_host_with_startup",
      "mclone_web_create_remote_scene_host_with_startup",
      "mclone_web_catalog_validate_world_id",
      "mclone_web_catalog_prepare_world_list",
      "mclone_web_catalog_prepare_create_world",
      "mclone_web_catalog_prepare_open_world",
      "mclone_web_catalog_prepare_delete_world",
    ];
    for (const name of requiredExports) {
      if (typeof (module as Record<string, any>)[name] !== "function") {
        throw new Error(`missing ${name} export`);
      }
    }
    const { config: startup, plan: startupPlan } = startupOptionsFromLocation(module);
    const remoteWebSocketUrl = startupRemoteWebSocketUrl(startupPlan);
    this.radiusChunks = clampRadiusChunks(startupPlan.renderDistance);
    this.sectionOcclusionCulling = Boolean(startupPlan.sectionOcclusionCulling);
    this.forceFullbright = Boolean(startupPlan.forceFullbright);
    runtime.state.radiusChunks = this.radiusChunks;
    runtime.state.clientHost = remoteWebSocketUrl ? "remote-dedicated" : "worker-integrated";
    runtime.state.remoteWebSocketUrl = remoteWebSocketUrl;
    runtime.state.sectionOcclusionCulling = this.sectionOcclusionCulling;
    runtime.state.forceFullbright = this.forceFullbright;
    runtime.state.renderColorProfile = startupPlan.renderColorProfile;

    runtime.state.status = "loading assets";
    publishRuntimeState(runtime.state);
    const [assetPack, authoredAssetPack, fallbackAssetPack] = await Promise.all([
      fetchAssetPack(ASSET_PACK_URL),
      fetchAssetPack(AUTHORED_ASSET_PACK_URL),
      fetchAssetPack(FALLBACK_ASSET_PACK_URL),
    ]);
    this.assetPack = assetPack;
    this.authoredAssetPack = authoredAssetPack;
    this.fallbackAssetPack = fallbackAssetPack;
    this.compiler = this.createRenderCompiler();
    const compilerWake = (doorbell: WasmReport): void => {
      void this.wakeRenderCompiler(doorbell);
    };
    runtime.state.status = "initializing webgpu";
    publishRuntimeState(runtime.state);
    if (remoteWebSocketUrl) {
      runtime.state.status = "connecting remote websocket";
      publishRuntimeState(runtime.state);
      this.session = await module.mclone_web_create_remote_scene_host_with_startup(
        this.canvas,
        assetPack,
        authoredAssetPack,
        fallbackAssetPack,
        startup,
        compilerWake,
      );
    } else {
      this.session = await module.mclone_web_create_worker_scene_host_with_startup(
        this.canvas,
        assetPack,
        authoredAssetPack,
        fallbackAssetPack,
        startup,
        SERVER_WORKER_URL.href,
        SERVER_JOB_WORKER_URL.href,
        BINDGEN_JS_URL.href,
        BINDGEN_WASM_URL.href,
        compilerWake,
      );
    }
    for (const name of [
      "renderFrame",
      "syncOverviewRenderFrame",
      "renderCompilerSharedSupported",
      "cameraFrameState",
      "resizeCanvas",
      "toggleMovementMode",
      "selectHotbarSlot",
      "previewBlockTarget",
      "blockStateAt",
      "interactBlock",
      "openTitleUi",
      "openPauseUi",
      "openHelpUi",
      "closeUi",
      "uiStatus",
      "handleUiKey",
      "handleUiPointerMove",
      "handleUiPointerDown",
      "handleUiPointerUp",
      "startLocalWorld",
      "startIndexedDbLocalWorld",
      "applyWorldCatalogResponse",
      "applyWorldCatalogError",
      "joinRemoteWebSocket",
      "setDebugOverlayVisible",
      "setStatusOverlay",
      "setTouchLookSensitivity",
      "setTouchControlsMode",
      "setTouchControlsOverlay",
      "setHidden",
    ]) {
      if (typeof (this.session as unknown as Record<string, any>)[name] !== "function") {
        throw new Error(`missing WebSceneHost.${name} export`);
      }
    }
    if (!this.session.renderCompilerSharedSupported()) {
      throw new Error(
        "cross-origin isolation (SharedArrayBuffer/Atomics) is required for the streaming render loop",
      );
    }
    this.setNativeDebugOverlay(defaultDebugOverlayVisible());
    bindInput(this, runtime.state, () => publishRuntimeState(runtime.state));
    document.addEventListener("visibilitychange", () => {
      const hidden = document.visibilityState === "hidden";
      const report = this.session?.setHidden(hidden);
      if (!hidden) {
        this.lastFrameTime = performance.now();
      }
      this.applyNativeUiReport(report);
    });
    this.touchControls = new TouchControls(this, runtime.state);
    this.setNativeTouchControlsMode(this.touchControlsMode, false);
    this.flushNativeTouchControlsOverlay();
    this.setNativeTouchLookSensitivity(
      this.lookSensitivity,
      this.touchControls.snapshot().visible,
      false,
    );
    this.syncCanvasSize();
    this.applyCameraState(this.session.cameraFrameState());
    this.applyTargetState(this.session.previewBlockTarget());

    runtime.state.status = "rendering";
    this.setNativeStatusOverlay(runtime.state.status, true, true);
    publishRuntimeState(runtime.state);
    // 067 Stage 3: warm up the streaming loop to idle so the first presented frame has
    // terrain (the web analog of desktop's pre-render `sync_all_render_sections`).
    await this.warmUpStreamingToIdle();
  }

  start(): void {
    if (this.animationFrame !== 0) {
      return;
    }
    this.lastFrameTime = performance.now();
    const frame = (now: number) => {
      if (!this.tickFrameBusy && !this.sessionBusy) {
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

  pauseRendering(): void {
    if (this.animationFrame !== 0) {
      cancelAnimationFrame(this.animationFrame);
      this.animationFrame = 0;
    }
  }

  resumeRendering(): void {
    this.start();
  }

  renderOverviewFrame(): WasmReport | null {
    if (!this.session || this.sessionBusy || this.tickFrameBusy) {
      return null;
    }
    const report = this.session.syncOverviewRenderFrame(
      Number(runtime.state.centerX) || 0,
      Number(runtime.state.centerZ) || 0,
      this.radiusChunks,
    );
    this.handleSceneFrame(report);
    return report;
  }


  async tickFrame(now: number): Promise<void> {
    if (!this.session) {
      return;
    }
    const frameGapMs = Number.isFinite(now - this.lastFrameTime)
      ? Math.max(0, now - this.lastFrameTime)
      : 0;
    this.recordFrameGap(frameGapMs);
    this.lastFrameTime = now;
    const uiActive = runtime.state.uiActive === true;
    const mouseDeltaX = uiActive ? 0 : this.mouseDeltaX;
    const mouseDeltaY = uiActive ? 0 : this.mouseDeltaY;
    this.mouseDeltaX = 0;
    this.mouseDeltaY = 0;
    this.syncCanvasSize();
    const keys = uiActive ? (defaultInputKeys() as InputKeys) : this.currentInputKeys();
    const movement = uiActive ? defaultMovementImpulse() : this.currentMovementImpulse();
    runtime.state.frameCount += 1;
    runtime.state.tickPhase = "scene-host";
    const frame = await this.renderHostFrame(now, {
      mouseDeltaX,
      mouseDeltaY,
      keyboardTurn: (keys.turnLeft ? 1 : 0) - (keys.turnRight ? 1 : 0),
      forward: keys.forward,
      backward: keys.backward,
      left: keys.left,
      right: keys.right,
      jump: keys.jump,
      descend: keys.descend,
      sneak: keys.shift,
      sprint: keys.sprint,
      analogActive: movement.active,
      analogLeft: movement.left,
      analogForward: movement.forward,
    });
    this.handleSceneFrame(frame);
    this.applyTargetState(this.session.previewBlockTarget());
  }

  async warmUpStreamingToIdle(): Promise<void> {
    // The shared scene host intentionally admits one resident-ring compile at a time.
    // A cold radius-four view can contain more than 200 visible sections, so leave
    // enough room for slower browser/CI worker scheduling while retaining a hard boot
    // failure bound.
    const deadline = performance.now() + 60_000;
    let stableFrames = 0;
    while (performance.now() < deadline) {
      const idle = await this.streamFrameOnce({ awaitWorker: true });
      stableFrames = idle && this.hasRendered && Number(runtime.state.residentSectionCount) > 0
        ? stableFrames + 1
        : 0;
      if (stableFrames >= 6) {
        return;
      }
      await nextAnimationFrame();
    }
    throw new Error("timed out warming up the shared scene host");
  }

  async streamFrameOnce(options: { awaitWorker?: boolean } = {}): Promise<boolean> {
    if (!this.session) {
      return true;
    }
    const frame = await this.renderHostFrame(performance.now(), {
      mouseDeltaX: 0,
      mouseDeltaY: 0,
      keyboardTurn: 0,
      forward: false,
      backward: false,
      left: false,
      right: false,
      jump: false,
      descend: false,
      sneak: false,
      sprint: false,
      analogActive: false,
      analogLeft: 0,
      analogForward: 0,
    });
    this.handleSceneFrame(frame);
    if (options.awaitWorker && this.pendingTimings.size > 0) {
      await Promise.allSettled(
        [...this.pendingTimings.values()]
          .map((pending) => pending.workerPromise)
          .filter((promise): promise is Promise<any> => promise !== null),
      );
    }
    return runtime.state.streamingSettled === true;
  }

  async renderHostFrame(
    now: number,
    input: {
      mouseDeltaX: number;
      mouseDeltaY: number;
      keyboardTurn: number;
      forward: boolean;
      backward: boolean;
      left: boolean;
      right: boolean;
      jump: boolean;
      descend: boolean;
      sneak: boolean;
      sprint: boolean;
      analogActive: boolean;
      analogLeft: number;
      analogForward: number;
    },
  ): Promise<WasmReport> {
    const session = this.session as WebSceneHost;
    return session.renderFrame(
      now,
      input.mouseDeltaX,
      input.mouseDeltaY,
      input.keyboardTurn,
      input.forward,
      input.backward,
      input.left,
      input.right,
      input.jump,
      input.descend,
      input.sneak,
      input.sprint,
      input.analogActive,
      input.analogLeft,
      input.analogForward,
    );
  }

  handleSceneFrame(frame: WasmReport): void {
    if (!frame?.ok) {
      runtime.state.ok = false;
      runtime.state.status = frame?.reason ?? "shared scene frame failed";
      publishRuntimeState(runtime.state);
      return;
    }
    if (frame.state === "restart-required") {
      runtime.state.ok = false;
      runtime.state.status = frame.restartReason ?? "WebGPU restart required";
      this.setNativeStatusOverlay(runtime.state.status, false, true);
      publishRuntimeState(runtime.state);
      return;
    }
    this.hasRendered ||= Boolean(frame.rendered);
    this.applyReport(frame);
    if (frame.rendered) {
      hideBootstrapStatus();
    }
    this.settleAssetCompilerSwap(frame);
    if (
      Number(frame.acceptedCompileSectionCount) > 0
      && runtime.state.lastCompileReport
    ) {
      runtime.state.lastCompileReport = {
        ...runtime.state.lastCompileReport,
        commandCount: Number(frame.commandCount) || 0,
        acceptedCompileSectionCount: Number(frame.acceptedCompileSectionCount) || 0,
        meshBuildCount: Number(frame.meshBuildCount) || 0,
        pendingCompileJobCount: Number(frame.pendingCompileJobCount) || 0,
      };
    }
    const pendingJobs = Number(frame.pendingCompileJobCount) || 0;
    const runnerSettled = Number(frame.runnerCommandQueueDepth) === 0
      && Number(frame.runnerUpdateQueueDepth) === 0
      && Number(frame.runnerPendingJobs) === 0
      && Number(frame.runnerPendingPublications) === 0;
    const streamingSettled = Boolean(frame.streamingIdle)
      && runnerSettled
      && pendingJobs === 0
      && this.pendingTimings.size === 0;
    runtime.state.pendingCompileJobCount = pendingJobs;
    runtime.state.compileInFlight = pendingJobs > 0;
    runtime.state.compileInFlightCount = this.pendingTimings.size;
    runtime.state.compileFinalizingCount = 0;
    runtime.state.streamingSettled = streamingSettled;
    if (streamingSettled) {
      this.loadedCenter = { centerX: Number(frame.centerX), centerZ: Number(frame.centerZ) };
      runtime.state.loadedCenterX = Number(frame.centerX);
      runtime.state.loadedCenterZ = Number(frame.centerZ);
    }
    publishRuntimeState(runtime.state);
    this.dispatchSceneSessionOperation(frame);
  }

  wakeRenderCompiler(doorbell: WasmReport): Promise<any> {
    const compiler = this.compiler as RenderCompiler;
    const module = this.module as WasmModule;
    const requestId = Number(doorbell.requestId);
    const timing = new module.WebCompileTiming(
      ++this.compileSequence,
      "stream",
      this.loadedCenter?.centerX ?? null,
      this.loadedCenter?.centerZ ?? null,
      runtime.state.frameCount,
      runtime.state.renderCount,
      performance.now(),
    );
    timing.updateFromRequest(doorbell);
    timing.setBeginRequestMs(0);
    const pending: PendingCompile = { timing, workerPromise: null, workerError: null };
    const started = performance.now();
    const promise = compiler.compileWithDoorbell(doorbell).then((compiled: any) => {
      timing.setWorkerRoundTripMs(performance.now() - started);
      if (compiled?.report) {
        timing.updateFromWorker(compiled.report);
      }
      timing.setDecodeFinishApplyMs(0);
      timing.finish(
        "accepted",
        null,
        performance.now(),
        runtime.state.frameCount,
        runtime.state.renderCount,
        runtime.state.lastFrameGapMs,
      );
      recordCompileTiming(timing);
      const priorCompileReport = runtime.state.lastCompileReport;
      runtime.state.lastCompileReport = compiled?.report
        ? {
            ...priorCompileReport,
            ...compiled.report,
            workerCompileUsed: true,
            commandCount: Number(priorCompileReport?.commandCount) || 0,
            acceptedCompileSectionCount:
              Number(priorCompileReport?.acceptedCompileSectionCount) || 0,
            meshBuildCount: Number(priorCompileReport?.meshBuildCount) || 0,
            pendingCompileJobCount:
              Number(priorCompileReport?.pendingCompileJobCount) || 0,
          }
        : null;
      this.pendingTimings.delete(requestId);
      runtime.state.compileInFlightCount = this.pendingTimings.size;
      publishRuntimeState(runtime.state);
      return compiled;
    }).catch((error: unknown) => {
      const message = stringifyError(error);
      pending.workerError = message;
      timing.finish(
        "failed",
        message,
        performance.now(),
        runtime.state.frameCount,
        runtime.state.renderCount,
        runtime.state.lastFrameGapMs,
      );
      recordCompileTiming(timing);
      this.pendingTimings.delete(requestId);
      runtime.state.compileInFlightCount = this.pendingTimings.size;
      throw error;
    });
    pending.workerPromise = promise;
    this.pendingTimings.set(requestId, pending);
    runtime.state.compileInFlightCount = this.pendingTimings.size;
    publishActiveCompileTiming(timing);
    return promise;
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
    const wasStartupReady = runtime.state.startupReady === true;
    this.applyCameraState(report);
    runtime.state.startupReady = Boolean(report.startupReady);
    runtime.state.startupProgressVisible = Boolean(report.startupProgressVisible);
    runtime.state.startupProgressObserved ||= runtime.state.startupProgressVisible;
    runtime.state.startupProgressReadyChunks = Number(report.startupProgressReadyChunks) || 0;
    runtime.state.startupProgressChunkCount = Number(report.startupProgressChunkCount) || 0;
    runtime.state.startupProgressPercent = Number(report.startupProgressPercent) || 0;
    if (!runtime.state.startupReady) {
      runtime.state.startupHoldCameraY ??= runtime.state.cameraY;
      runtime.state.minimumPreStartupCameraY = Math.min(
        runtime.state.minimumPreStartupCameraY ?? runtime.state.cameraY,
        runtime.state.cameraY,
      );
    } else if (!wasStartupReady) {
      runtime.state.startupAdmissionFrame = Number(report.frameCount);
    }
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
    runtime.state.renderPendingWork = Boolean(report.renderPendingWork);
    runtime.state.renderDirtyChunkCount = Number(report.renderDirtyChunkCount) || 0;
    runtime.state.renderDirtySectionCount = Number(report.renderDirtySectionCount) || 0;
    runtime.state.renderInflightSectionCount = Number(report.renderInflightSectionCount) || 0;
    runtime.state.runnerKind = report.runnerKind;
    runtime.state.runnerCommandQueueDepth = report.runnerCommandQueueDepth;
    runtime.state.runnerUpdateQueueDepth = report.runnerUpdateQueueDepth;
    runtime.state.runnerPendingJobs = report.runnerPendingJobs;
    runtime.state.runnerPendingPublications = report.runnerPendingPublications;
    runtime.state.runnerPendingPersistenceLoads = report.runnerPendingPersistenceLoads;
    runtime.state.runnerPendingPersistenceSaves = report.runnerPendingPersistenceSaves;
    runtime.state.clientDeferredChunkDropBacklogItems = Number(
      report.clientDeferredChunkDropBacklogItems,
    ) || 0;
    runtime.state.worldgenMailboxKind = report.worldgenMailboxKind;
    runtime.state.lightStatusMailboxKind = report.lightStatusMailboxKind;
    runtime.state.worldgenMailboxPendingJobs = report.worldgenMailboxPendingJobs;
    runtime.state.lightStatusMailboxPendingStatuses = report.lightStatusMailboxPendingStatuses;
    runtime.state.runnerFrameMetrics = report.runnerFrameMetrics ?? null;
    runtime.state.worldgenJobFrameMetrics = report.worldgenJobFrameMetrics ?? null;
    runtime.state.lightStatusJobFrameMetrics = report.lightStatusJobFrameMetrics ?? null;
    runtime.state.renderCount = report.renderCount;
    runtime.state.guiCommandCount = report.guiCommandCount;
    runtime.state.flatHudRetainedRebuilds = Number(report.flatHudRetainedRebuilds) || 0;
    runtime.state.flatHudRetainedCacheHits = Number(report.flatHudRetainedCacheHits) || 0;
    runtime.state.uiActive = Boolean(report.uiActive);
    runtime.state.uiCoversWorld = Boolean(report.uiCoversWorld);
    runtime.state.nativeUiScreen = String(report.uiScreen ?? runtime.state.nativeUiScreen ?? "none");
    runtime.state.nativeUiOptionsParent = report.uiOptionsParent ?? null;
    applySessionReport(report, runtime.state);
    if (typeof report.sectionOcclusionCulling !== "undefined") {
      this.sectionOcclusionCulling = Boolean(report.sectionOcclusionCulling);
    }
    if (typeof report.forceFullbright !== "undefined") {
      this.forceFullbright = Boolean(report.forceFullbright);
    }
    runtime.state.sectionOcclusionCulling = this.sectionOcclusionCulling;
    runtime.state.forceFullbright = this.forceFullbright;
    if (typeof report.renderColorProfile !== "undefined") {
      runtime.state.renderColorProfile = String(report.renderColorProfile);
    }
    if (typeof report.debugOverlayVisible !== "undefined") {
      runtime.state.debugOverlayVisible = Boolean(report.debugOverlayVisible);
    }
    runtime.state.status = "ready";
    this.setNativeStatusOverlay("ready", true, false);
    runtime.state.lastReport = report;
  }

  applyTargetState(target: WasmReport): void {
    if (!target?.ok) {
      return;
    }
    runtime.state.currentTarget = target;
    applyHotbarState(target, runtime.state);
  }

  blockStateAt(x: number, y: number, z: number): WasmReport | null {
    if (!this.session) {
      return null;
    }
    return this.session.blockStateAt(Math.trunc(x), Math.trunc(y), Math.trunc(z));
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
      publishRuntimeState(runtime.state);
      return interaction;
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      this.setNativeStatusOverlay(runtime.state.status, false, true);
      console.error(error);
      publishRuntimeState(runtime.state);
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
    publishRuntimeState(runtime.state);
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
    publishRuntimeState(runtime.state);
    return camera;
  }

  setNativeDebugOverlay(open: boolean): WasmReport | null {
    runtime.state.debugOverlayVisible = Boolean(open);
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.setNativeDebugOverlay(open), 0);
      return null;
    }
    const report = this.session.setDebugOverlayVisible(Boolean(open));
    this.applyNativeUiReport(report);
    return report;
  }

  setNativeStatusOverlay(message: string, ok = true, visible = true): WasmReport | null {
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.setNativeStatusOverlay(message, ok, visible), 0);
      return null;
    }
    const report = this.session.setStatusOverlay(String(message ?? ""), Boolean(ok), Boolean(visible));
    this.applyNativeUiReport(report);
    return report;
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

  openNativeHelpUi(): WasmReport | null {
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.openNativeHelpUi(), 0);
      return null;
    }
    const report = this.session.openHelpUi();
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
    const report = this.session.handleUiPointerDown(point.x, point.y, this.radiusChunks);
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

  setNativeTouchLookSensitivity(
    value: number,
    available = Boolean(this.touchControls?.snapshot().visible),
    persist = true,
  ): WasmReport | null {
    const clamped = clampLookSensitivity(value);
    this.lookSensitivity = clamped;
    runtime.state.lookSensitivity = clamped;
    runtime.state.touchLookSensitivityAvailable = Boolean(available);
    if (persist) {
      storeLookSensitivity(clamped);
    }
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.setNativeTouchLookSensitivity(clamped, available, persist), 0);
      return deferredUiReport();
    }
    const report = this.session.setTouchLookSensitivity(clamped, Boolean(available));
    this.applyNativeUiReport(report);
    return report;
  }

  setNativeTouchControlsMode(mode: TouchControlsMode, persist = true): WasmReport | null {
    this.touchControlsMode = mode;
    runtime.state.touchControlsMode = mode;
    if (persist) {
      storeTouchControlsMode(mode);
    }
    this.touchControls?.setVisible(mode === "on" || hasTouchInput());
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.setNativeTouchControlsMode(mode, persist), 0);
      return deferredUiReport();
    }
    const report = this.session.setTouchControlsMode(mode);
    this.applyNativeUiReport(report);
    return report;
  }

  setNativeTouchControlsOverlay(overlay: TouchOverlayState): WasmReport | null {
    this.pendingTouchOverlay = overlay;
    return this.flushNativeTouchControlsOverlay();
  }

  flushNativeTouchControlsOverlay(): WasmReport | null {
    if (!this.session || !this.pendingTouchOverlay || this.sessionBusy) {
      return null;
    }
    const overlay = this.pendingTouchOverlay;
    const base = this.canvasLocalPointToPixel(overlay.movementBaseX, overlay.movementBaseY);
    const thumb = this.canvasLocalPointToPixel(overlay.movementThumbX, overlay.movementThumbY);
    const report = this.session.setTouchControlsOverlay(
      overlay.visible,
      overlay.movementActive,
      base.x,
      base.y,
      thumb.x,
      thumb.y,
      overlay.jumpPressed,
      overlay.sprintPressed,
      overlay.descendPressed,
      overlay.menuPressed,
    );
    this.applyNativeUiReport(report);
    return report;
  }

  applyNativeUiReport(report: WasmReport | null | undefined, options: { fromPointer?: boolean, pointerType?: string } = {}): void {
    if (!report?.ok) {
      return;
    }
    const wasUiActive = runtime.state.uiActive === true;
    runtime.state.uiActive = Boolean(report.active ?? report.uiActive);
    runtime.state.uiCoversWorld = Boolean(report.coversWorld ?? report.uiCoversWorld);
    runtime.state.nativeUiScreen = String(report.screen ?? report.uiScreen ?? "none");
    runtime.state.nativeUiOptionsParent = report.optionsParent ?? report.uiOptionsParent ?? null;
    applySessionReport(report, runtime.state);
    this.sectionOcclusionCulling = Boolean(report.sectionOcclusionCulling);
    this.forceFullbright = Boolean(report.forceFullbright);
    runtime.state.sectionOcclusionCulling = this.sectionOcclusionCulling;
    runtime.state.forceFullbright = this.forceFullbright;
    if (typeof report.renderColorProfile !== "undefined") {
      runtime.state.renderColorProfile = String(report.renderColorProfile);
    }
    if (typeof report.touchLookSensitivityAvailable !== "undefined") {
      runtime.state.touchLookSensitivityAvailable = Boolean(report.touchLookSensitivityAvailable);
    }
    if (typeof report.debugOverlayVisible !== "undefined") {
      runtime.state.debugOverlayVisible = Boolean(report.debugOverlayVisible);
    }
    if (typeof report.touchLookSensitivity !== "undefined") {
      const sensitivity = clampLookSensitivity(report.touchLookSensitivity);
      this.lookSensitivity = sensitivity;
      runtime.state.lookSensitivity = sensitivity;
      if (report.action === "setTouchLookSensitivity") {
        storeLookSensitivity(sensitivity);
      }
    }
    if (typeof report.touchControlsMode !== "undefined") {
      const mode = report.touchControlsMode === "on" || report.touchControlsMode === "off"
        ? report.touchControlsMode
        : "auto";
      this.touchControlsMode = mode;
      runtime.state.touchControlsMode = mode;
      if (report.action === "setTouchControlsMode") {
        storeTouchControlsMode(mode);
        this.touchControls?.setVisible(mode === "on" || hasTouchInput());
      }
    }
    if (typeof report.renderDistance !== "undefined") {
      this.radiusChunks = clampRadiusChunks(report.renderDistance);
      runtime.state.radiusChunks = this.radiusChunks;
    }
    if (report.action) {
      runtime.state.lastUiAction = report;
    }
    if (runtime.state.uiActive) {
      this.releasePointerLockForUi();
      if (!wasUiActive) {
        this.clearGameplayInput();
      }
    }
    if (
      (report.action === "startWorld" || report.action === "resume")
      && options.fromPointer
      && options.pointerType !== "touch"
    ) {
      this.requestPointerLock();
    }
    publishRuntimeState(runtime.state);
    this.dispatchSceneSessionOperation(report, options);
    this.dispatchWorldCatalogOperation(report, options);
    this.dispatchAssetPackOperation(report);
  }

  dispatchSceneSessionOperation(
    report: WasmReport,
    options: { fromPointer?: boolean, pointerType?: string } = {},
  ): void {
    if (report.sessionStartPending !== true) {
      return;
    }
    const operationKind = String(report.sessionOperationKind ?? "");
    if (operationKind === "remote") {
      const endpoint = String(report.remoteEndpoint ?? report.sessionRemoteEndpoint ?? "").trim();
      if (endpoint.length > 0) {
        void this.completeSceneSessionStart(
          "remote",
          (session) => session.joinRemoteWebSocket(endpoint),
          options,
        );
      }
      return;
    }
    if (operationKind === "localWorld") {
      const seed = parseSeedBigInt(
        report.catalogWorldSeedText
          ?? report.sessionSeedText
          ?? report.catalogWorldSeed
          ?? report.sessionSeed,
      );
      const worldId = String(report.catalogWorldId ?? "").trim();
      const displayName = String(report.catalogWorldDisplayName ?? worldId).trim();
      const requestKind = String(report.catalogSessionRequest ?? "openLocalWorld");
      if (seed !== null && worldId.length > 0) {
        void this.completeSceneSessionStart(
          "localWorld",
          (session) => session.startIndexedDbLocalWorld(
            seed,
            worldId,
            displayName,
            requestKind,
            SERVER_WORKER_URL.href,
            SERVER_JOB_WORKER_URL.href,
            BINDGEN_JS_URL.href,
            BINDGEN_WASM_URL.href,
          ),
          options,
        );
      } else if (seed !== null) {
        void this.completeSceneSessionStart(
          "localWorld",
          (session) => session.startLocalWorld(
            seed,
            SERVER_WORKER_URL.href,
            SERVER_JOB_WORKER_URL.href,
            BINDGEN_JS_URL.href,
            BINDGEN_WASM_URL.href,
          ),
          options,
        );
      }
    }
  }

  dispatchWorldCatalogOperation(
    report: WasmReport,
    options: { fromPointer?: boolean, pointerType?: string } = {},
  ): void {
    if (report.catalogRequest !== true) {
      return;
    }
    void this.completeWorldCatalogRequest(report, options);
  }

  dispatchAssetPackOperation(report: WasmReport): void {
    if (report.assetPackRequest !== true || this.sessionBusy) {
      return;
    }
    void this.completeAssetPackSelection(report);
  }

  async completeAssetPackSelection(report: WasmReport): Promise<void> {
    if (
      !this.session
      || !this.assetPack
      || !this.authoredAssetPack
      || !this.fallbackAssetPack
      || !this.compiler
      || this.sessionBusy
    ) {
      return;
    }
    this.sessionBusy = true;
    runtime.state.sessionBusy = true;
    const priorCompiler = this.compiler;
    let candidateCompiler: RenderCompiler | null = null;
    try {
      await Promise.allSettled(
        [...this.pendingTimings.values()]
          .map((pending) => pending.workerPromise)
          .filter((promise): promise is Promise<any> => promise !== null),
      );
      const selection: RenderCompilerAssetSelection = {
        authoredPack: this.authoredAssetPack,
        referencePack: this.assetPack,
        fallbackPack: this.fallbackAssetPack,
        authoredEnabled: Boolean(report.assetPackAuthoredEnabled),
        referenceEnabled: Boolean(report.assetPackReferenceEnabled),
        epoch: Number(report.assetPackRequestEpoch) || 0,
      };
      candidateCompiler = this.createRenderCompiler(selection);
      await candidateCompiler.ready;
      this.compiler = candidateCompiler;
      const completion = await this.session.completeAssetPackSelection(
        this.authoredAssetPack,
        this.assetPack,
        this.fallbackAssetPack,
      );
      await nextAnimationFrame();
      this.pendingAssetCompilerSwap = {
        previous: priorCompiler,
        candidate: candidateCompiler,
        epoch: selection.epoch,
      };
      candidateCompiler = null;
      this.applyNativeUiReport(completion);
      runtime.state.assetPackCompletionCount =
        (Number(runtime.state.assetPackCompletionCount) || 0) + 1;
    } catch (error) {
      candidateCompiler?.terminate();
      this.compiler = priorCompiler;
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
      publishRuntimeState(runtime.state);
    } finally {
      this.sessionBusy = false;
      runtime.state.sessionBusy = false;
      this.flushNativeTouchControlsOverlay();
    }
  }

  settleAssetCompilerSwap(report: WasmReport): void {
    const swap = this.pendingAssetCompilerSwap;
    if (!swap) {
      return;
    }
    if (
      report.assetReplacementState === "active"
      && Number(report.activeAssetEpoch) === swap.epoch
    ) {
      swap.previous.terminate();
      this.pendingAssetCompilerSwap = null;
      return;
    }
    if (report.assetReplacementState === "failed") {
      swap.candidate.terminate();
      this.compiler = swap.previous;
      this.pendingAssetCompilerSwap = null;
    }
  }

  async completeWorldCatalogRequest(
    report: WasmReport,
    options: { fromPointer?: boolean, pointerType?: string } = {},
  ): Promise<void> {
    if (!this.session) {
      return;
    }
    const session = this.session;
    const requestId = String(report.catalogRequestId ?? "").trim();
    const operation = String(report.catalogOperation ?? "").trim();
    let db: IDBDatabase | null = null;
    try {
      db = await openWorldDb();
      const payload = await this.executeWorldCatalogRequest(db, operation, report);
      const completion = session.applyWorldCatalogResponse(requestId, operation, payload);
      runtime.state.worldCatalogCompletionCount += 1;
      this.applyNativeUiReport(completion, options);
    } catch (error) {
      const message = stringifyError(error);
      console.error(error);
      try {
        const failure = session.applyWorldCatalogError(requestId, message);
        runtime.state.worldCatalogCompletionCount += 1;
        this.applyNativeUiReport(failure, options);
      } catch (completionError) {
        runtime.state.ok = false;
        runtime.state.status = stringifyError(completionError);
        console.error(completionError);
        publishRuntimeState(runtime.state);
      }
    } finally {
      db?.close();
    }
  }

  async executeWorldCatalogRequest(
    db: IDBDatabase,
    operation: string,
    report: WasmReport,
  ): Promise<WebLocalWorldSummary | WebLocalWorldSummary[]> {
    switch (operation) {
      case "listWorlds":
        return listIndexedDbCatalogWorlds(db);
      case "createWorld": {
        const seed = Number(report.catalogWorldSeedText ?? report.catalogWorldSeed);
        const options = {
          displayName: String(report.catalogDisplayName ?? ""),
          seed,
          requestedId: report.catalogRequestedId ?? null,
        };
        return createIndexedDbCatalogWorld(db, options);
      }
      case "openWorld":
        return openIndexedDbCatalogWorld(db, String(report.catalogWorldId ?? ""));
      case "deleteWorld": {
        const activeWorldId = String(report.activeWorldId ?? "").trim();
        return deleteIndexedDbCatalogWorld(
          db,
          String(report.catalogWorldId ?? ""),
          activeWorldId.length > 0 ? activeWorldId : null,
        );
      }
      default:
        throw new Error(`unsupported world catalog operation ${JSON.stringify(operation)}`);
    }
  }

  async completeSceneSessionStart(
    requestedKind: "localWorld" | "remote",
    start: (session: WebSceneHost) => Promise<WasmReport>,
    options: { fromPointer?: boolean, pointerType?: string } = {},
  ): Promise<void> {
    if (!this.session) {
      return;
    }
    // Reserve the wasm host synchronously. A catalog completion can race the
    // rAF render lock; leaving the pending start on the Rust host lets the next
    // frame retry instead of starving behind continuously scheduled frames.
    if (this.sessionBusy) {
      return;
    }
    this.sessionBusy = true;
    runtime.state.sessionBusy = true;
    const session = this.session;
    this.clearGameplayInput();
    this.releasePointerLockForUi();
    let startReport: WasmReport | null = null;
    try {
      startReport = await start(session);
      // wasm-bindgen keeps the exported async `&mut WebSceneHost` borrow until
      // its promise reaction has fully unwound. Resume host calls on the next
      // macrotask so replacement warm-up cannot recursively reborrow it.
      await nextAnimationFrame();
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
    } finally {
      this.sessionBusy = false;
      runtime.state.sessionBusy = false;
      this.flushNativeTouchControlsOverlay();
    }
    if (!startReport?.ok) {
      this.setNativeStatusOverlay(runtime.state.status, false, true);
      publishRuntimeState(runtime.state);
      return;
    }

    this.applyNativeUiReport(startReport);
    if (startReport.sessionState !== "active") {
      publishRuntimeState(runtime.state);
      return;
    }

    this.applySessionHostMode(startReport);
    this.resetStreamingStateForSessionRestart();
    try {
      this.syncCanvasSize();
      this.applyCameraState(this.session.cameraFrameState());
      this.applyTargetState(this.session.previewBlockTarget());
      if (options.fromPointer && options.pointerType !== "touch") {
        this.requestPointerLock();
      }
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      this.setNativeStatusOverlay(runtime.state.status, false, true);
      console.error(error);
      publishRuntimeState(runtime.state);
      return;
    }
    runtime.state.status = requestedKind === "remote" ? "remote session ready" : "world ready";
    this.setNativeStatusOverlay("ready", true, false);
    publishRuntimeState(runtime.state);
  }

  applySessionHostMode(report: WasmReport): void {
    if (report.sessionKind === "remote") {
      runtime.state.clientHost = "remote-dedicated";
      runtime.state.remoteWebSocketUrl = String(report.sessionRemoteEndpoint ?? "");
    } else if (report.sessionKind === "localWorld") {
      runtime.state.clientHost = "worker-integrated";
      runtime.state.remoteWebSocketUrl = null;
    }
  }

  createRenderCompiler(selection?: RenderCompilerAssetSelection): RenderCompiler {
    if (!this.assetPack) {
      throw new Error("asset pack is not loaded");
    }
    return new RenderSectionWorkerCompiler(selection ?? this.assetPack, {
      workerUrl: RENDER_COMPILER_WORKER_URL,
      bindgenJsUrl: BINDGEN_JS_URL,
      bindgenWasmUrl: BINDGEN_WASM_URL,
      workerName: "mclone-render-compiler-app",
    });
  }

  resetStreamingStateForSessionRestart(): void {
    this.hasRendered = false;
    this.loadedCenter = null;
    this.pendingTimings.clear();
    this.finalizingCount = 0;
    runtime.state.loadedCenterX = null;
    runtime.state.loadedCenterZ = null;
    runtime.state.pendingCompileJobCount = 0;
    runtime.state.renderPendingWork = false;
    runtime.state.renderDirtyChunkCount = 0;
    runtime.state.renderDirtySectionCount = 0;
    runtime.state.renderInflightSectionCount = 0;
    runtime.state.compileInFlight = false;
    runtime.state.compileInFlightCount = 0;
    runtime.state.compileFinalizingCount = 0;
    runtime.state.streamingSettled = false;
    runtime.state.startupReady = false;
    runtime.state.startupHoldCameraY = null;
    runtime.state.minimumPreStartupCameraY = null;
    runtime.state.startupAdmissionFrame = null;
    runtime.state.compileQueued = false;
    runtime.state.compileTargetX = null;
    runtime.state.compileTargetZ = null;
    runtime.state.queuedCompileTargetX = null;
    runtime.state.queuedCompileTargetZ = null;
    runtime.state.activeCompileTiming = null;
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

  canvasLocalPointToPixel(localX: number, localY: number): { x: number, y: number } {
    const rect = this.canvas.getBoundingClientRect();
    const scaleX = rect.width > 0 ? this.canvas.width / rect.width : 1;
    const scaleY = rect.height > 0 ? this.canvas.height / rect.height : 1;
    return {
      x: Number(localX) * scaleX,
      y: Number(localY) * scaleY,
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
    runtime.state.sessionBusy = true;
    try {
      return await operation();
    } finally {
      this.sessionBusy = false;
      runtime.state.sessionBusy = false;
      this.flushNativeTouchControlsOverlay();
    }
  }

  async waitForSessionIdle(): Promise<void> {
    for (let attempt = 0; this.sessionBusy && attempt < 10_000; attempt += 1) {
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    }
    if (this.sessionBusy) {
      throw new Error("timed out waiting for WebSceneHost async operation");
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
          publishRuntimeState(runtime.state);
        });
      }
    } else {
      runtime.state.pointerLockFallback = true;
    }
    publishRuntimeState(runtime.state);
  }

  updatePointerLockState(): void {
    runtime.state.pointerLocked = document.pointerLockElement === this.canvas;
    runtime.state.pointerLockFallback =
      runtime.state.pointerLockAttempted && !runtime.state.pointerLocked;
    publishRuntimeState(runtime.state);
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

function defaultDebugOverlayVisible(): boolean {
  return !hasTouchInput() && window.matchMedia("(min-width: 681px)").matches;
}

function parseSeedBigInt(value: unknown): bigint | null {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (/^-?\d+$/.test(trimmed)) {
      return BigInt(trimmed);
    }
    return null;
  }
  const numberValue = Number(value);
  if (Number.isSafeInteger(numberValue)) {
    return BigInt(numberValue);
  }
  return null;
}

function startupOptionsFromLocation(
  module: WasmModule,
): { config: WebStartupConfig; plan: WebStartupPlan } {
  const config = module.mclone_web_startup_options_from_query(
    globalThis.location.search,
  ) as WebStartupConfig;
  const raw = config.browserPlan();
  return {
    config,
    plan: {
      ...raw,
      renderDistance: clampRadiusChunks(raw.renderDistance),
      sectionOcclusionCulling: Boolean(raw.sectionOcclusionCulling),
      forceFullbright: Boolean(raw.forceFullbright),
      renderColorProfile: String(raw.renderColorProfile ?? "vanilla"),
    },
  };
}

function startupRemoteWebSocketUrl(options: WebStartupPlan): string | null {
  if (typeof options.remoteWebSocketUrl !== "string") {
    return null;
  }
  const trimmed = options.remoteWebSocketUrl.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function finiteInteger(value: unknown, fallback: number): number {
  const parsed = Math.trunc(Number(value));
  return Number.isFinite(parsed) ? parsed : fallback;
}

function applySessionReport(report: WasmReport, state: AppRuntimeState): void {
  if (typeof report.sessionState !== "undefined") {
    state.sessionState = String(report.sessionState);
  }
  if (typeof report.sessionKind !== "undefined") {
    const kind = String(report.sessionKind);
    state.sessionKind = kind;
    if (kind !== "localWorld") {
      state.sessionSeed = null;
      state.sessionSeedText = null;
      state.sessionWorldId = null;
    }
    if (kind !== "remote") {
      state.sessionRemoteEndpoint = null;
    }
  }
  if (typeof report.sessionSeed !== "undefined") {
    state.sessionSeed = Number(report.sessionSeed);
  }
  if (typeof report.sessionSeedText !== "undefined") {
    state.sessionSeedText = String(report.sessionSeedText);
  }
  if (typeof report.sessionWorldId !== "undefined") {
    state.sessionWorldId = String(report.sessionWorldId);
  }
  if (typeof report.sessionRemoteEndpoint !== "undefined") {
    state.sessionRemoteEndpoint = String(report.sessionRemoteEndpoint);
  }
  if (typeof report.sessionFailureMessage !== "undefined") {
    state.sessionFailureMessage = String(report.sessionFailureMessage);
  } else if (report.sessionState !== "failed") {
    state.sessionFailureMessage = null;
  }
  if (typeof report.sessionStatusVisible !== "undefined") {
    state.sessionStatusVisible = Boolean(report.sessionStatusVisible);
  }
  if (typeof report.sessionStatusOk !== "undefined") {
    state.sessionStatusOk = Boolean(report.sessionStatusOk);
  }
  if (typeof report.sessionStatusMessage !== "undefined") {
    state.sessionStatusMessage = String(report.sessionStatusMessage);
  } else if (report.sessionStatusVisible === false) {
    state.sessionStatusMessage = "";
  }
  if (typeof report.statusOverlayVisible !== "undefined") {
    state.statusOverlayVisible = Boolean(report.statusOverlayVisible);
  }
  if (typeof report.statusOverlayOk !== "undefined") {
    state.statusOverlayOk = Boolean(report.statusOverlayOk);
  }
  if (typeof report.statusOverlayMessage !== "undefined") {
    state.statusOverlayMessage = String(report.statusOverlayMessage);
  } else if (report.statusOverlayVisible === false) {
    state.statusOverlayMessage = "";
  }
  if (typeof report.worldCatalogPersistent !== "undefined") {
    state.worldCatalogPersistent = Boolean(report.worldCatalogPersistent);
  }
  if (typeof report.worldCatalogLoading !== "undefined") {
    state.worldCatalogLoading = Boolean(report.worldCatalogLoading);
  }
  if (typeof report.worldCatalogEntryCount !== "undefined") {
    state.worldCatalogEntryCount = Number(report.worldCatalogEntryCount) || 0;
  }
  if (typeof report.worldCatalogStatusVisible !== "undefined") {
    state.worldCatalogStatusVisible = Boolean(report.worldCatalogStatusVisible);
  }
  if (typeof report.worldCatalogStatusOk !== "undefined") {
    state.worldCatalogStatusOk = Boolean(report.worldCatalogStatusOk);
  }
  if (typeof report.worldCatalogStatusMessage !== "undefined") {
    state.worldCatalogStatusMessage = String(report.worldCatalogStatusMessage);
  } else if (report.worldCatalogStatusVisible === false) {
    state.worldCatalogStatusMessage = "";
  }
}

function publishRuntimeState(state: AppRuntimeState): void {
  const status = document.getElementById("mclone-bootstrap-status");
  if (!status || status.dataset.retired === "true") {
    return;
  }
  status.textContent = startupStatusLabel(state.status);
}

function hideBootstrapStatus(): void {
  const status = document.getElementById("mclone-bootstrap-status");
  if (!status) {
    return;
  }
  status.dataset.retired = "true";
  status.hidden = true;
  runtime.state.bootstrapStatusRetired = true;
}

function startupStatusLabel(status: unknown): string {
  switch (status) {
    case "loading wasm":
      return "Loading engine…";
    case "loading assets":
      return "Loading assets…";
    case "initializing webgpu":
      return "Preparing renderer…";
    case "connecting remote websocket":
      return "Connecting to server…";
    case "rendering":
      return "Generating world…";
    default:
      return typeof status === "string" && status.length > 0 ? status : "Starting mclone…";
  }
}

function installFirstTouchFullscreen(state: AppRuntimeState): void {
  const root = document.documentElement as HTMLElement & {
    webkitRequestFullscreen?: () => Promise<void> | void;
  };
  const request = root.requestFullscreen?.bind(root) ?? root.webkitRequestFullscreen?.bind(root);
  state.fullscreenSupported = typeof request === "function";
  state.fullscreenAttempted = false;
  state.fullscreenActive = Boolean(document.fullscreenElement);
  state.fullscreenError = null;

  document.addEventListener("fullscreenchange", () => {
    state.fullscreenActive = Boolean(document.fullscreenElement);
  });
  document.addEventListener("fullscreenerror", () => {
    state.fullscreenError = "fullscreen request rejected";
  });

  const onFirstTouch = (event: PointerEvent): void => {
    if (event.pointerType !== "touch" && event.pointerType !== "pen") {
      return;
    }
    window.removeEventListener("pointerup", onFirstTouch, true);
    state.fullscreenAttempted = true;
    if (!request || document.fullscreenElement) {
      return;
    }
    try {
      const result = request();
      if (result && typeof result.then === "function") {
        void result.then(
          () => {
            state.fullscreenActive = Boolean(document.fullscreenElement);
          },
          (error: unknown) => {
            state.fullscreenError = stringifyError(error);
          },
        );
      }
    } catch (error) {
      state.fullscreenError = stringifyError(error);
    }
  };
  // Touch/pen activation is granted on pointerup (mouse activation is on
  // pointerdown), so request fullscreen when the first tap is released.
  window.addEventListener("pointerup", onFirstTouch, {
    capture: true,
    passive: true,
  });
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
