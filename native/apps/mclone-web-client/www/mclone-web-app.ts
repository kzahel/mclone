import { fetchAssetPack } from "./mclone-render-compiler-shared.js";
import { PolledWorkerTransport } from "./mclone-worker-transport.js";
import { bindInput } from "./mclone-web-input.js";
import type { WebSmokeObserver } from "./mclone-web-smoke-observer.js";
import { TouchControls, hasTouchInput } from "./mclone-web-touch.js";
import {
  executeIndexedDbCatalogExecution,
  openWorldDb,
} from "./mclone-web-world-catalog.js";
import type {
  WebCatalogExecution,
  WebLobbyRuntimeStart,
  WebSceneHost,
} from "mclone-web-client-wasm";

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

interface WebStartupPlan extends WasmReport {
  renderDistance: number;
  remoteWebSocketUrl?: string;
  sectionOcclusionCulling: boolean;
  forceFullbright: boolean;
  renderColorProfile: string;
  generationProfile: string;
}

type WebStartupConfig = ReturnType<WasmModule["mclone_web_startup_options_from_query"]>;

interface AppRuntimeState extends Record<string, any> {
  ok: boolean;
  ready: boolean;
  failed: boolean;
  status: string;
  frameCount: number;
  lastFrameGapMs: number;
  maxFrameGapMs: number;
}

interface AppRuntime {
  ready: boolean;
  state: AppRuntimeState;
}

declare global {
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
    radiusChunks: DEFAULT_RADIUS_CHUNKS,
    width: 0,
    height: 0,
    pointerCaptureBlocked: false,
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
    sessionBusy: false,
    status: "booting",
    bootstrapStatusRetired: false,
  },
};

let smokeObserver: WebSmokeObserver | null = null;

installFirstTouchFullscreen(runtime.state);

async function boot(): Promise<void> {
  const app = new WebFrameDriver();
  await installSmokeObserverIfRequested(app);
  try {
    await app.init();
    runtime.ready = true;
    runtime.state.ready = true;
    runtime.state.ok = true;
    runtime.state.failed = false;
    runtime.state.status = "ready";
    publishRuntimeState(runtime.state);
    app.setNativeStatusOverlay("ready", true, false);
    app.start();
    return;
  } catch (error) {
    runtime.ready = false;
    runtime.state.ok = false;
    runtime.state.failed = true;
    runtime.state.status = stringifyError(error);
    app.setNativeStatusOverlay(runtime.state.status, false, true);
    publishRuntimeState(runtime.state);
  }
}

async function installSmokeObserverIfRequested(app: WebFrameDriver): Promise<void> {
  const parameters = new URLSearchParams(globalThis.location.search);
  if (parameters.get("smokeObserver") !== "1") {
    return;
  }
  const observer = await import("./mclone-web-smoke-observer.js");
  smokeObserver = observer.installWebSmokeObserver(app);
  smokeObserver.observePlatformState(runtime);
}

class WebFrameDriver {
  canvas: HTMLCanvasElement;
  module: WasmModule | null;
  session: WebSceneHost | null;
  assetPack: Uint8Array | null;
  authoredAssetPack: Uint8Array | null;
  fallbackAssetPack: Uint8Array | null;
  touchControls: TouchControls | null;
  pointerDragging: boolean;
  pointerDown: { button: number, enabled: boolean, movement: number } | null;
  radiusChunks: number;
  sectionOcclusionCulling: boolean;
  forceFullbright: boolean;
  animationFrame: number;
  lastFrameTime: number;
  tickFrameBusy: boolean;
  sessionBusy: boolean;
  pendingLobbyRuntimeStarts: Set<Promise<void>>;
  lobbyOperationDrainActive: boolean;
  worldCatalogOperationTail: Promise<void>;

  constructor() {
    // Required for the app to run; `init()` re-validates with `instanceof HTMLCanvasElement` and
    // throws if it is missing, so treating it as a non-null canvas here is sound for the lifecycle.
    this.canvas = document.getElementById("mclone-canvas") as HTMLCanvasElement;
    this.module = null;
    this.session = null;
    this.assetPack = null;
    this.authoredAssetPack = null;
    this.fallbackAssetPack = null;
    this.touchControls = null;
    this.pointerDragging = false;
    this.pointerDown = null;
    this.radiusChunks = DEFAULT_RADIUS_CHUNKS;
    this.sectionOcclusionCulling = true;
    this.forceFullbright = false;
    this.animationFrame = 0;
    this.lastFrameTime = 0;
    this.tickFrameBusy = false;
    this.sessionBusy = false;
    this.pendingLobbyRuntimeStarts = new Set();
    this.lobbyOperationDrainActive = false;
    this.worldCatalogOperationTail = Promise.resolve();
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
    this.module = module;
    const requiredExports = [
      "mclone_web_startup_options_from_query",
      "mclone_web_create_worker_scene_host_with_startup",
      "mclone_web_create_remote_scene_host_with_startup",
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
    const renderWorkerTransportFactory = () => new PolledWorkerTransport(
      RENDER_COMPILER_WORKER_URL,
      "mclone-render-compiler-app",
    );
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
        BINDGEN_JS_URL.href,
        BINDGEN_WASM_URL.href,
        renderWorkerTransportFactory,
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
        renderWorkerTransportFactory,
      );
    }
    for (const name of [
      "renderFrame",
      "handleRawKey",
      "handleRawPointerButton",
      "handleRawPointerMove",
      "handleRawMouseMotion",
      "handleRawWheel",
      "handleRawTouch",
      "clearRawInput",
      "renderCompilerSharedSupported",
      "resizeCanvas",
      "startPendingSession",
      "takeWorldCatalogExecution",
      "applyWorldCatalogExecution",
      "applyWorldCatalogError",
      "setDebugOverlayVisible",
      "setStatusOverlay",
      "setTouchInputAvailable",
      "setHidden",
      "takeLobbyRuntimeStart",
      "completeLobbyWorldStart",
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
      void this.withSessionAsync(() => this.session?.setHidden(hidden) ?? null)
        .then((report) => {
          if (!hidden) {
            this.lastFrameTime = performance.now();
          }
          this.applyNativeUiReport(report);
        })
        .catch((error) => {
          runtime.state.ok = false;
          runtime.state.status = stringifyError(error);
          console.error(error);
          publishRuntimeState(runtime.state);
        });
    });
    this.touchControls = new TouchControls(this);
    this.syncCanvasSize();
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

  sceneHostForObserver(): WebSceneHost | null {
    return this.sessionBusy ? null : this.session;
  }

  async waitForObserverIdle(pauseFrames = false): Promise<void> {
    if (pauseFrames) {
      this.pauseRendering();
    }
    while (this.tickFrameBusy || this.sessionBusy) {
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  }

  async renderSingleObserverFrame(): Promise<WasmReport | null> {
    await this.waitForObserverIdle(true);
    this.tickFrameBusy = true;
    runtime.state.tickFrameBusy = true;
    try {
      const report = await this.renderHostFrame(performance.now());
      this.handleSceneFrame(report);
      return report;
    } finally {
      this.tickFrameBusy = false;
      runtime.state.tickFrameBusy = false;
      runtime.state.tickPhase = "idle";
      publishRuntimeState(runtime.state);
    }
  }

  applyObserverReport(
    report: WasmReport | null | undefined,
    options: { fromPointer?: boolean; pointerType?: string } = {},
  ): void {
    this.applyNativeUiReport(report, options);
  }

  async withObserverSceneHost<T>(
    operation: (session: WebSceneHost) => T | Promise<T>,
  ): Promise<Awaited<T> | null> {
    if (!this.session) {
      return null;
    }
    const session = this.session;
    return this.withSessionAsync(() => operation(session));
  }

  observerCanvasPoint(clientX: number, clientY: number): { x: number; y: number } {
    return this.canvasPixelPoint(clientX, clientY);
  }

  observerRenderRadius(): number {
    return this.radiusChunks;
  }

  async shutdownForObserver(): Promise<WasmReport | null> {
    await this.waitForObserverIdle(true);
    await this.worldCatalogOperationTail;
    await Promise.allSettled([...this.pendingLobbyRuntimeStarts]);
    const report = this.session?.shutdown() ?? null;
    smokeObserver?.observeReport(report);
    publishRuntimeState(runtime.state);
    return report;
  }

  backgroundCycleForObserver(): WasmReport | null {
    if (!this.session || this.sessionBusy) {
      return null;
    }
    const report = this.session.setHidden(true);
    this.applyNativeUiReport(report);
    this.applyNativeUiReport(this.session.setHidden(false));
    this.lastFrameTime = performance.now();
    return report;
  }

  drainLobbyOperations(): void {
    if (!this.session || this.sessionBusy || this.lobbyOperationDrainActive) {
      return;
    }
    this.lobbyOperationDrainActive = true;
    try {
      for (;;) {
        const start = this.session.takeLobbyRuntimeStart(
          SERVER_WORKER_URL.href,
          SERVER_JOB_WORKER_URL.href,
          BINDGEN_JS_URL.href,
          BINDGEN_WASM_URL.href,
        );
        if (!start) {
          break;
        }
        this.launchLobbyRuntime(start);
      }
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
      publishRuntimeState(runtime.state);
    } finally {
      this.lobbyOperationDrainActive = false;
    }
  }

  launchLobbyRuntime(start: WebLobbyRuntimeStart): void {
    if (!this.session) return;
    const task = (async () => {
      try {
        const result = await start.start();
        smokeObserver?.observeLobbyRuntimeStart(result);
        await this.waitForSessionIdle();
        if (!this.session) return;
        const report = this.session.completeLobbyWorldStart(start);
        this.applyNativeUiReport(report);
        this.drainLobbyOperations();
      } catch (error) {
        runtime.state.ok = false;
        runtime.state.status = stringifyError(error);
        console.error(error);
        publishRuntimeState(runtime.state);
      } finally {
        start.free();
      }
    })();
    this.pendingLobbyRuntimeStarts.add(task);
    void task.finally(() => this.pendingLobbyRuntimeStarts.delete(task));
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
    this.syncCanvasSize();
    runtime.state.frameCount += 1;
    runtime.state.tickPhase = "scene-host";
    const frame = await this.renderHostFrame(now);
    this.handleSceneFrame(frame);
  }

  async warmUpStreamingToIdle(): Promise<void> {
    // The shared scene host intentionally admits one resident-ring compile at a time.
    // A cold radius-four view can contain more than 200 visible sections, so leave
    // enough room for slower browser/CI worker scheduling while retaining a hard boot
    // failure bound.
    const deadline = performance.now() + 60_000;
    while (performance.now() < deadline) {
      if (this.sessionBusy) {
        await nextAnimationFrame();
        continue;
      }
      if (await this.streamFrameOnce({ awaitWorker: true })) {
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
    const frame = await this.renderHostFrame(performance.now());
    this.handleSceneFrame(frame);
    if (options.awaitWorker && Number(frame.renderWorkerPendingRequestCount) > 0) {
      await nextAnimationFrame();
    }
    return Boolean(frame.initialPresentationReady);
  }

  async renderHostFrame(now: number): Promise<WasmReport> {
    const session = this.session as WebSceneHost;
    return session.renderFrame(now);
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
    smokeObserver?.observeReport(frame);
    if (frame.rendered) {
      hideBootstrapStatus();
    }
    publishRuntimeState(runtime.state);
    this.dispatchSceneSessionOperation(frame);
    if (!this.sessionBusy) {
      this.drainLobbyOperations();
    }
  }

  setNativeDebugOverlay(open: boolean): WasmReport | null {
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

  setTouchInputAvailable(available: boolean): WasmReport | null {
    if (!this.session) {
      return null;
    }
    if (this.sessionBusy) {
      setTimeout(() => this.setTouchInputAvailable(available), 0);
      return deferredUiReport();
    }
    const report = this.session.setTouchInputAvailable(Boolean(available));
    this.applyNativeUiReport(report);
    return report;
  }

  applyNativeUiReport(
    report: WasmReport | null | undefined,
    options: { fromPointer?: boolean; pointerType?: string } = {},
  ): void {
    if (!report?.ok) {
      return;
    }
    smokeObserver?.observeReport(report);
    const wasPointerCaptureBlocked = runtime.state.pointerCaptureBlocked === true;
    runtime.state.pointerCaptureBlocked = Boolean(report.active ?? report.uiActive);
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
    if (typeof report.renderDistance !== "undefined") {
      this.radiusChunks = clampRadiusChunks(report.renderDistance);
      runtime.state.radiusChunks = this.radiusChunks;
    }
    if (runtime.state.pointerCaptureBlocked) {
      this.releasePointerLockForUi();
      if (!wasPointerCaptureBlocked) {
        this.clearGameplayInput();
      }
    }
    if (report.releasePointerCapture === true) {
      this.releasePointerLockForUi();
    } else if (report.pointerCaptureDesired === true) {
      this.requestPointerLock();
    }
    publishRuntimeState(runtime.state);
    this.dispatchSceneSessionOperation(report, options);
    this.dispatchWorldCatalogOperation(report, options);
    this.dispatchAssetPackOperation(report);
    if (!this.sessionBusy) {
      this.drainLobbyOperations();
    }
  }

  dispatchSceneSessionOperation(
    report: WasmReport,
    options: { fromPointer?: boolean, pointerType?: string } = {},
  ): void {
    if (report.sessionStartPending !== true) {
      return;
    }
    void this.completeSceneSessionStart(
      (session) => session.startPendingSession(
        SERVER_WORKER_URL.href,
        SERVER_JOB_WORKER_URL.href,
        BINDGEN_JS_URL.href,
        BINDGEN_WASM_URL.href,
      ),
      options,
    );
  }

  dispatchWorldCatalogOperation(
    report: WasmReport,
    options: { fromPointer?: boolean, pointerType?: string } = {},
  ): void {
    if (report.catalogRequest !== true) {
      return;
    }
    this.worldCatalogOperationTail = this.worldCatalogOperationTail
      .then(() => this.completeWorldCatalogRequest(report, options))
      .catch((error: unknown) => {
        runtime.state.ok = false;
        runtime.state.status = stringifyError(error);
        console.error(error);
        publishRuntimeState(runtime.state);
      });
  }

  dispatchAssetPackOperation(report: WasmReport): void {
    if (report.assetPackRequest !== true || this.sessionBusy) {
      return;
    }
    void this.completeAssetPackSelection(report);
  }

  async completeAssetPackSelection(_report: WasmReport): Promise<void> {
    if (
      !this.session
      || !this.assetPack
      || !this.authoredAssetPack
      || !this.fallbackAssetPack
      || this.sessionBusy
    ) {
      return;
    }
    this.sessionBusy = true;
    runtime.state.sessionBusy = true;
    try {
      const completion = await this.session.completeAssetPackSelection(
        this.authoredAssetPack,
        this.assetPack,
        this.fallbackAssetPack,
      );
      await nextAnimationFrame();
      this.applyNativeUiReport(completion);
      smokeObserver?.observeAssetPackCompletion();
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
      publishRuntimeState(runtime.state);
    } finally {
      this.sessionBusy = false;
      runtime.state.sessionBusy = false;
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
    let db: IDBDatabase | null = null;
    let execution: WebCatalogExecution | null = null;
    try {
      db = await openWorldDb();
      // Asset replacement and session startup keep wasm-bindgen's mutable host
      // borrow alive across their promises. A catalog request can be queued by
      // the same UI report, so wait until those operations release the host
      // before taking its synchronous catalog execution ticket.
      await this.waitForSessionIdle();
      execution = session.takeWorldCatalogExecution(requestId);
      await execution.awaitWriterRetirements();
      await executeIndexedDbCatalogExecution(db, execution);
      await this.waitForSessionIdle();
      const completion = session.applyWorldCatalogExecution(requestId, execution);
      smokeObserver?.observeWorldCatalogCompletion();
      this.applyNativeUiReport(completion, options);
    } catch (error) {
      const message = stringifyError(error);
      console.error(error);
      try {
        await this.waitForSessionIdle();
        const failure = session.applyWorldCatalogError(requestId, message);
        smokeObserver?.observeWorldCatalogCompletion();
        this.applyNativeUiReport(failure, options);
      } catch (completionError) {
        runtime.state.ok = false;
        runtime.state.status = stringifyError(completionError);
        console.error(completionError);
        publishRuntimeState(runtime.state);
      }
    } finally {
      execution?.free();
      db?.close();
    }
  }

  async completeSceneSessionStart(
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

    this.resetStreamingStateForSessionRestart();
    try {
      this.syncCanvasSize();
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
    runtime.state.status = "session ready";
    this.setNativeStatusOverlay("ready", true, false);
    publishRuntimeState(runtime.state);
  }

  resetStreamingStateForSessionRestart(): void {
    smokeObserver?.resetForSessionRestart();
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
    this.pointerDragging = false;
    this.pointerDown = null;
    this.touchControls?.clearAll();
    if (this.session && !this.sessionBusy) {
      this.session.clearRawInput();
    }
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

  handleRawKey(code: string, key: string, pressed: boolean, repeat: boolean): boolean {
    return this.withRawInput((session) => session.handleRawKey(code, key, pressed, repeat));
  }

  handleRawPointerButton(
    button: number,
    pressed: boolean,
    click: boolean,
    clientX: number,
    clientY: number,
  ): boolean {
    const point = this.canvasPixelPoint(clientX, clientY);
    return this.withRawInput((session) => session.handleRawPointerButton(
      button,
      pressed,
      click,
      point.x,
      point.y,
    ));
  }

  handleRawPointerMove(clientX: number, clientY: number): boolean {
    const point = this.canvasPixelPoint(clientX, clientY);
    return this.withRawInput((session) => session.handleRawPointerMove(point.x, point.y));
  }

  handleRawMouseMotion(dx: number, dy: number): boolean {
    return this.withRawInput((session) => session.handleRawMouseMotion(dx, dy));
  }

  handleRawWheel(deltaY: number, deltaMode: number): boolean {
    return this.withRawInput((session) => session.handleRawWheel(deltaY, deltaMode));
  }

  handleRawTouch(
    phase: "start" | "move" | "end" | "cancel",
    pointerId: number,
    clientX: number,
    clientY: number,
  ): boolean {
    const point = this.canvasPixelPoint(clientX, clientY);
    return this.withRawInput((session) => session.handleRawTouch(
      phase,
      pointerId,
      point.x,
      point.y,
    ));
  }

  clearRawInput(): void {
    this.pointerDragging = false;
    this.pointerDown = null;
    if (!this.session || this.sessionBusy) {
      if (this.sessionBusy) {
        setTimeout(() => this.clearRawInput(), 0);
      }
      return;
    }
    this.applyRawInputReport(this.session.clearRawInput());
  }

  withRawInput(operation: (session: WebSceneHost) => WasmReport): boolean {
    if (!this.session || this.sessionBusy) {
      if (this.sessionBusy) {
        setTimeout(() => this.withRawInput(operation), 0);
      }
      return true;
    }
    const report = operation(this.session);
    this.applyRawInputReport(report);
    return Boolean(report?.handled);
  }

  applyRawInputReport(report: WasmReport | null | undefined): void {
    if (!report?.ok) {
      return;
    }
    if (report.clearTransientInput) {
      this.pointerDragging = false;
      this.pointerDown = null;
      this.touchControls?.clearAll();
    }
    if (report.releasePointerCapture) {
      this.releasePointerLockForUi();
    } else if (report.pointerCaptureDesired === true) {
      this.requestPointerLock();
    }
    this.applyNativeUiReport(report);
  }

  recordFrameGap(frameGapMs: number): void {
    if (!Number.isFinite(frameGapMs)) {
      return;
    }
    runtime.state.lastFrameGapMs = frameGapMs;
    runtime.state.maxFrameGapMs = Math.max(runtime.state.maxFrameGapMs, frameGapMs);
  }

  requestPointerLock(): void {
    if (runtime.state.pointerCaptureBlocked === true) {
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

function defaultDebugOverlayVisible(): boolean {
  return !hasTouchInput() && window.matchMedia("(min-width: 681px)").matches;
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
      generationProfile: String(raw.generationProfile ?? "overworld"),
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

function publishRuntimeState(state: AppRuntimeState): void {
  smokeObserver?.observePlatformState(runtime);
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
    handled: runtime.state.pointerCaptureBlocked === true,
    deferred: true,
  };
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}

void boot();
