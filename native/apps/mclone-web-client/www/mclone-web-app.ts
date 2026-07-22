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
  WebSceneOperation,
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

interface WebBootstrapResourceRequest {
  requestId: number;
  url: string;
}

interface WebBootstrapPlan extends WasmReport {
  resources: WebBootstrapResourceRequest[];
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
const MAX_FRAME_DT_SECONDS = 0.05;

const runtime: AppRuntime = {
  ready: false,
  state: {
    ok: false,
    ready: false,
    failed: false,
    width: 0,
    height: 0,
    pointerCaptureBlocked: false,
    frameCount: 0,
    lastFrameGapMs: 0,
    maxFrameGapMs: 0,
    pointerLockSupported: false,
    pointerLockAttempted: false,
    pointerLocked: false,
    pointerLockFallback: false,
    tickFrameBusy: false,
    tickPhase: "idle",
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
    app.start();
    return;
  } catch (error) {
    runtime.ready = false;
    runtime.state.ok = false;
    runtime.state.failed = true;
    runtime.state.status = stringifyError(error);
    app.reportHostFailure(runtime.state.status);
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
  touchControls: TouchControls | null;
  pointerDragging: boolean;
  pointerDown: { button: number, enabled: boolean, movement: number } | null;
  animationFrame: number;
  lastFrameTime: number;
  tickFrameBusy: boolean;
  pendingSceneOperations: Set<Promise<void>>;

  constructor() {
    // Required for the app to run; `init()` re-validates with `instanceof HTMLCanvasElement` and
    // throws if it is missing, so treating it as a non-null canvas here is sound for the lifecycle.
    this.canvas = document.getElementById("mclone-canvas") as HTMLCanvasElement;
    this.module = null;
    this.session = null;
    this.touchControls = null;
    this.pointerDragging = false;
    this.pointerDown = null;
    this.animationFrame = 0;
    this.lastFrameTime = 0;
    this.tickFrameBusy = false;
    this.pendingSceneOperations = new Set();
  }

  async init(): Promise<void> {
    if (!(this.canvas instanceof HTMLCanvasElement)) {
      throw new Error("missing canvas#mclone-canvas");
    }
    runtime.state.pointerLockSupported = typeof this.canvas.requestPointerLock === "function";
    this.canvas.focus();

    publishRuntimeState(runtime.state);
    const module = await import(BINDGEN_JS_URL.href) as WasmModule;
    await module.default(BINDGEN_WASM_URL.href);
    this.module = module;
    const requiredExports = [
      "mclone_web_startup_options_from_query",
      "mclone_web_create_scene_host_with_startup",
      "WebBootstrapResources",
      "WebHostCapabilities",
    ];
    for (const name of requiredExports) {
      if (typeof (module as Record<string, any>)[name] !== "function") {
        throw new Error(`missing ${name} export`);
      }
    }
    const startup = startupOptionsFromLocation(module);
    runtime.state.generationProfile = String(startup.generationProfile);
    runtime.state.worldTopology = String(startup.worldTopology);

    publishRuntimeState(runtime.state);
    const resources = await fetchBootstrapResources(module, startup.browserPlan());
    const renderWorkerTransportFactory = () => new PolledWorkerTransport(
      RENDER_COMPILER_WORKER_URL,
      "mclone-render-compiler-app",
    );
    publishRuntimeState(runtime.state);
    const capabilities = new module.WebHostCapabilities(
      hasTouchInput(),
      this.canvas.getBoundingClientRect().width,
    );
    this.session = await module.mclone_web_create_scene_host_with_startup(
      this.canvas,
      resources,
      capabilities,
      startup,
      SERVER_WORKER_URL.href,
      SERVER_JOB_WORKER_URL.href,
      BINDGEN_JS_URL.href,
      BINDGEN_WASM_URL.href,
      renderWorkerTransportFactory,
    );
    if (!this.session.renderCompilerSharedSupported()) {
      throw new Error(
        "cross-origin isolation (SharedArrayBuffer/Atomics) is required for the streaming render loop",
      );
    }
    bindInput(this, runtime.state, () => publishRuntimeState(runtime.state));
    document.addEventListener("visibilitychange", () => {
      const hidden = document.visibilityState === "hidden";
      try {
        const report = this.session?.setHidden(hidden) ?? null;
        if (!hidden) {
          this.lastFrameTime = performance.now();
        }
        this.applyNativeUiReport(report);
      } catch (error) {
        runtime.state.ok = false;
        runtime.state.status = stringifyError(error);
        console.error(error);
        publishRuntimeState(runtime.state);
      }
    });
    this.touchControls = new TouchControls(this);
    this.syncCanvasSize();
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
      if (!this.tickFrameBusy) {
        this.tickFrameBusy = true;
        runtime.state.tickFrameBusy = true;
        void this.tickFrame(now).finally(() => {
          this.tickFrameBusy = false;
          runtime.state.tickFrameBusy = false;
          runtime.state.tickPhase = "idle";
          publishRuntimeState(runtime.state);
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
    return this.session;
  }

  wasmModuleForObserver(): WasmModule | null {
    return this.module;
  }

  observerSnapshot(
    operation: WasmReport | null | undefined,
  ): WasmReport | null {
    if (!this.session) {
      return operation ?? null;
    }
    const snapshot = this.session.diagnosticSnapshot() as WasmReport;
    return snapshot?.ok ? { ...snapshot, ...(operation ?? {}) } : operation ?? null;
  }

  async waitForObserverIdle(pauseFrames = false): Promise<void> {
    if (pauseFrames) {
      this.pauseRendering();
    }
    while (this.tickFrameBusy) {
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
      return this.observerSnapshot(report);
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
    return await operation(this.session);
  }

  observerCanvasPoint(clientX: number, clientY: number): { x: number; y: number } {
    return this.canvasPixelPoint(clientX, clientY);
  }

  async shutdownForObserver(): Promise<WasmReport | null> {
    await this.waitForObserverIdle(true);
    while (this.pendingSceneOperations.size > 0) {
      await Promise.allSettled([...this.pendingSceneOperations]);
    }
    const report = this.session?.shutdown() ?? null;
    smokeObserver?.observeReport(report);
    publishRuntimeState(runtime.state);
    return report;
  }

  backgroundCycleForObserver(): WasmReport | null {
    if (!this.session) {
      return null;
    }
    const report = this.session.setHidden(true);
    this.applyNativeUiReport(report);
    this.applyNativeUiReport(this.session.setHidden(false));
    this.lastFrameTime = performance.now();
    return report;
  }

  drainSceneOperations(
    options: { fromPointer?: boolean; pointerType?: string } = {},
  ): void {
    if (!this.session) return;
    try {
      for (;;) {
        const operation = this.session.takeSceneOperation(
          SERVER_WORKER_URL.href,
          SERVER_JOB_WORKER_URL.href,
          BINDGEN_JS_URL.href,
          BINDGEN_WASM_URL.href,
        );
        if (!operation) break;
        this.trackSceneOperation(this.executeSceneOperation(operation, options));
      }
    } catch (error) {
      runtime.state.ok = false;
      runtime.state.status = stringifyError(error);
      console.error(error);
      publishRuntimeState(runtime.state);
    }
  }

  async executeSceneOperation(
    operation: WebSceneOperation,
    options: { fromPointer?: boolean; pointerType?: string } = {},
  ): Promise<void> {
    const session = this.session;
    if (!session) {
      operation.free();
      return;
    }
    let db: IDBDatabase | null = null;
    let execution: WebCatalogExecution | null = null;
    let effectReport: WasmReport | null = null;
    let consumed = false;
    try {
      execution = operation.takeIndexedDbExecution() ?? null;
      if (execution) {
        db = await openWorldDb();
        await execution.awaitWriterRetirements();
        await executeIndexedDbCatalogExecution(db, execution);
        operation.completeIndexedDbExecution(execution);
      } else {
        effectReport = await operation.start();
      }
      if (this.session !== session) return;
      const completion = session.completeSceneOperation(operation);
      consumed = true;
      smokeObserver?.observeSceneOperationCompletion(effectReport, completion);
      this.applyNativeUiReport(completion, options);
      this.syncCanvasSize();
      if (
        options.fromPointer
        && options.pointerType !== "touch"
        && completion.sessionActive === true
      ) {
        this.requestPointerLock();
      }
      runtime.state.status = "ready";
      publishRuntimeState(runtime.state);
    } catch (error) {
      const message = stringifyError(error);
      console.error(error);
      if (!consumed && this.session === session) {
        try {
          if (execution) {
            operation.completeIndexedDbExecution(execution, message);
          }
          const completion = session.completeSceneOperation(operation);
          consumed = true;
          smokeObserver?.observeSceneOperationCompletion(effectReport, completion);
          this.applyNativeUiReport(completion, options);
        } catch (completionError) {
          runtime.state.ok = false;
          runtime.state.status = stringifyError(completionError);
          console.error(completionError);
          publishRuntimeState(runtime.state);
        }
      }
    } finally {
      execution?.free();
      db?.close();
      operation.free();
      this.drainSceneOperations();
    }
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
      if (await this.streamFrameOnce()) {
        return;
      }
      await nextAnimationFrame();
    }
    throw new Error("timed out warming up the shared scene host");
  }

  async streamFrameOnce(): Promise<boolean> {
    if (!this.session) {
      return true;
    }
    const frame = await this.renderHostFrame(performance.now());
    this.handleSceneFrame(frame);
    await smokeObserver?.waitForStartupProgressCapture(frame);
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
      this.reportHostFailure(runtime.state.status);
      publishRuntimeState(runtime.state);
      return;
    }
    smokeObserver?.observeReport(frame);
    const wasPointerCaptureBlocked = runtime.state.pointerCaptureBlocked === true;
    runtime.state.pointerCaptureBlocked = Boolean(frame.active ?? frame.uiActive);
    if (frame.clearTransientInput === true) {
      this.pointerDragging = false;
      this.pointerDown = null;
      this.touchControls?.clearAll();
    }
    if (frame.releasePointerCapture === true) {
      this.releasePointerLockForUi();
    } else if (frame.pointerCaptureDesired === true
      || (wasPointerCaptureBlocked && !runtime.state.pointerCaptureBlocked)) {
      this.requestPointerLock();
    }
    if (frame.rendered) {
      hideBootstrapStatus();
    }
    publishRuntimeState(runtime.state);
    this.drainSceneOperations();
  }

  reportHostFailure(message: string): WasmReport | null {
    if (!this.session) {
      return null;
    }
    const report = this.session.reportHostFailure(String(message ?? ""));
    this.applyNativeUiReport(report);
    return report;
  }

  setTouchInputAvailable(available: boolean): WasmReport | null {
    if (!this.session) {
      return null;
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
    this.drainSceneOperations(options);
  }

  trackSceneOperation(task: Promise<void>): void {
    this.pendingSceneOperations.add(task);
    void task.finally(() => this.pendingSceneOperations.delete(task));
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
    if (this.session) {
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
    if (!this.session) {
      return;
    }
    this.applyRawInputReport(this.session.clearRawInput());
  }

  withRawInput(operation: (session: WebSceneHost) => WasmReport): boolean {
    if (!this.session) {
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

function startupOptionsFromLocation(
  module: WasmModule,
): WebStartupConfig {
  return module.mclone_web_startup_options_from_query(
    globalThis.location.search,
  ) as WebStartupConfig;
}

async function fetchBootstrapResources(
  module: WasmModule,
  rawPlan: unknown,
): Promise<InstanceType<WasmModule["WebBootstrapResources"]>> {
  const plan = rawPlan as WebBootstrapPlan;
  if (!Array.isArray(plan?.resources)) {
    throw new Error("browser bootstrap plan has no resource requests");
  }
  const responses = await Promise.all(plan.resources.map(async (request) => {
    const requestId = Number(request?.requestId);
    const url = typeof request?.url === "string" ? request.url : "";
    if (!Number.isInteger(requestId) || requestId < 0 || url.length === 0) {
      throw new Error("invalid browser bootstrap resource request");
    }
    return {
      requestId,
      bytes: await fetchAssetPack(versionedUrl(url)),
    };
  }));
  const resources = new module.WebBootstrapResources();
  for (const response of responses) {
    resources.add(response.requestId, response.bytes);
  }
  return resources;
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
  status.textContent = state.failed
    ? String(state.status || "mclone failed to start")
    : "Starting mclone…";
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

function stringifyError(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}

void boot();
