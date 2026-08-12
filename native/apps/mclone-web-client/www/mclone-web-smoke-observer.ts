// Explicit test-client exposure for browser smoke probes. The ordinary app
// loads this module only when `smokeObserver=1` is present in the page query.
// Game/engine vocabulary is permitted here because this is not a production
// platform adapter.

import type {
  WebSceneHost,
  WebSceneSmokeHarness,
} from "mclone-web-client-wasm";

type WasmReport = Record<string, any>;
type TouchControlsMode = "auto" | "on" | "off";

interface SmokeRuntimeState extends Record<string, any> {
  ok: boolean;
  ready: boolean;
  failed: boolean;
  status: string;
}

interface SmokeRuntime {
  ready: boolean;
  state: SmokeRuntimeState;
  adjustCameraSpeed?: (amount: number) => WasmReport | null;
  previewBlockTarget?: () => WasmReport | null;
  blockStateAt?: (x: number, y: number, z: number) => WasmReport | null;
  interactBlock?: (action: string) => Promise<WasmReport | null>;
  frameEmbeddedPreview?: () => WasmReport | null;
  frameTerrainComposition?: (
    eye: [number, number, number],
    target: [number, number, number],
  ) => WasmReport | null;
  frameInteractionSurface?: () => WasmReport | null;
  renderOneFrameForSmoke?: () => Promise<WasmReport | null>;
  renderHalfSpaceTerrainProof?: () => Promise<WasmReport | null>;
  renderPreparedFigureProof?: () => Promise<WasmReport | null>;
  renderActorCompositionProof?: () => Promise<WasmReport | null>;
  rebuildRenderResourcesForSmoke?: () => WasmReport | null;
  openNativeTitleUi?: () => WasmReport | null;
  openNativePauseUi?: () => WasmReport | null;
  pauseRendering?: () => void;
  resumeRendering?: () => void;
  renderOverviewFrame?: () => WasmReport | null;
  setDebugOverlay?: (visible: boolean) => WasmReport | null;
  openNativeHelpUi?: () => WasmReport | null;
  closeNativeUi?: () => WasmReport | null;
  handleNativeUiKey?: (key: string) => WasmReport | null;
  handleNativeUiPointerMove?: (
    clientX: number,
    clientY: number,
    pointerType?: string,
  ) => WasmReport | null;
  handleNativeUiPointerDown?: (
    clientX: number,
    clientY: number,
    pointerType?: string,
  ) => WasmReport | null;
  handleNativeUiPointerUp?: (
    clientX: number,
    clientY: number,
    pointerType?: string,
  ) => WasmReport | null;
  setNativeTouchControlsMode?: (
    mode: TouchControlsMode,
    persist?: boolean,
  ) => WasmReport | null;
  touchControlState?: () => WasmReport | null;
  beginLobbySmoke?: (chunkSpan?: number) => WasmReport | null;
  sceneBorrowExcluded?: () => boolean;
  backgroundSaveForSmoke?: () => WasmReport | null;
  shutdownForSmoke?: () => Promise<WasmReport | null>;
  releaseStartupProgressCapture?: () => void;
}

interface SmokeBridge {
  sceneHostForObserver(): WebSceneHost | null;
  wasmModuleForObserver(): Pick<
    typeof import("mclone-web-client-wasm"),
    "WebSceneSmokeHarness"
  > | null;
  observerSnapshot(report: WasmReport | null | undefined): WasmReport | null;
  waitForObserverIdle(pauseFrames?: boolean): Promise<void>;
  renderSingleObserverFrame(): Promise<WasmReport | null>;
  applyObserverReport(
    report: WasmReport | null | undefined,
    options?: { fromPointer?: boolean; pointerType?: string },
  ): void;
  withObserverSceneHost<T>(
    operation: (session: WebSceneHost) => T | Promise<T>,
  ): Promise<Awaited<T> | null>;
  observerCanvasPoint(clientX: number, clientY: number): { x: number; y: number };
  pauseRendering(): void;
  resumeRendering(): void;
  touchControls: { snapshot(): WasmReport } | null;
  drainSceneOperations(): void;
  backgroundCycleForObserver(): WasmReport | null;
  shutdownForObserver(): Promise<WasmReport | null>;
}

export interface WebSmokeObserver {
  observePlatformState(runtime: { ready: boolean; state: Record<string, any> }): void;
  observeStartup(startup: Record<string, any>): void;
  observeReport(report: WasmReport | null | undefined): void;
  latestReport(): WasmReport | null;
  observeTarget(target: WasmReport | null | undefined): void;
  resetForSessionRestart(): void;
  observeLobbyRuntimeStart(result: WasmReport): void;
  observeAssetPackCompletion(): void;
  observeWorldCatalogCompletion(): void;
  observeSceneOperationCompletion(
    effect: WasmReport | null,
    completion: WasmReport,
  ): void;
  waitForStartupProgressCapture(report: WasmReport): Promise<void>;
}

declare global {
  var __mcloneWebApp: SmokeRuntime;
}

export function installWebSmokeObserver(
  app: SmokeBridge,
): WebSmokeObserver {
  let latestObservedReport: WasmReport | null = null;
  const holdStartupProgress = new URLSearchParams(globalThis.location.search)
    .get("holdStartupProgress") === "1";
  let startupProgressCaptureConsumed = false;
  let releaseStartupProgressCapture: (() => void) | null = null;
  const runtime: SmokeRuntime = {
    ready: false,
    state: {
      ok: false,
      ready: false,
      failed: false,
      status: "booting",
      startupProgressObserved: false,
      startupHoldCameraY: null,
      minimumPreStartupCameraY: null,
      startupAdmissionFrame: null,
      startupAdmissionCameraY: null,
      loadedCenterX: null,
      loadedCenterZ: null,
      interactionCount: 0,
      interactionStatus: "idle",
      lastInteraction: null,
      currentTarget: null,
    },
  };
  runtime.releaseStartupProgressCapture = () => {
    const release = releaseStartupProgressCapture;
    releaseStartupProgressCapture = null;
    release?.();
  };
  const observerRenderRadius = (): number => {
    const radius = Math.round(Number(runtime.state.radiusChunks));
    return Number.isFinite(radius) && radius >= 1 ? radius : 1;
  };
  const observer: WebSmokeObserver = {
    observePlatformState(platformRuntime): void {
      runtime.ready = platformRuntime.ready;
      Object.assign(runtime.state, platformRuntime.state);
      const touch = app.touchControls?.snapshot();
      if (touch) {
        runtime.state.touchPointerActiveCount = touch.activePointerCount;
      }
    },
    observeStartup(startup): void {
      runtime.state.generationProfile = String(startup.generationProfile);
      runtime.state.worldTopology = String(startup.worldTopology);
      runtime.state.showcaseId = startup.showcaseId == null
        ? null
        : String(startup.showcaseId);
      runtime.state.showcaseRevision = startup.showcaseRevision == null
        ? null
        : Number(startup.showcaseRevision);
      runtime.state.showcaseEntryEye = startup.showcaseEntryEye == null
        ? null
        : String(startup.showcaseEntryEye);
      runtime.state.showcaseEntryTarget = startup.showcaseEntryTarget == null
        ? null
        : String(startup.showcaseEntryTarget);
      runtime.state.showcaseEntityCount = startup.showcaseEntityCount == null
        ? null
        : Number(startup.showcaseEntityCount);
      runtime.state.showcaseMallardCount = startup.showcaseMallardCount == null
        ? null
        : Number(startup.showcaseMallardCount);
      runtime.state.showcaseMallardNestCount = startup.showcaseMallardNestCount == null
        ? null
        : Number(startup.showcaseMallardNestCount);
      runtime.state.showcaseFieldGuideBits = startup.showcaseFieldGuideBits == null
        ? null
        : Number(startup.showcaseFieldGuideBits);
    },
    observeReport(report): void {
      if (!report?.ok) {
        return;
      }
      report = app.observerSnapshot(report);
      if (!report?.ok) {
        return;
      }
      latestObservedReport = report;
      const wasStartupReady = runtime.state.startupReady === true;
      const previouslyRenderedSky = runtime.state.skyRendered === true;
      Object.assign(runtime.state, report);
      if (report.rendered === false && previouslyRenderedSky) {
        runtime.state.skyRendered = true;
      }
      runtime.state.uiActive = Boolean(report.active ?? report.uiActive);
      runtime.state.uiCoversWorld = Boolean(report.coversWorld ?? report.uiCoversWorld);
      runtime.state.nativeUiScreen = String(
        report.screen ?? report.uiScreen ?? runtime.state.nativeUiScreen ?? "none",
      );
      runtime.state.nativeUiOptionsParent =
        report.optionsParent ?? report.uiOptionsParent ?? null;
      if (typeof report.touchLookSensitivity !== "undefined") {
        runtime.state.lookSensitivity = Number(report.touchLookSensitivity);
      }
      if (report.sessionKind === "remote") {
        runtime.state.clientHost = "remote-dedicated";
        runtime.state.remoteWebSocketUrl = String(report.sessionRemoteEndpoint ?? "");
      } else if (report.sessionKind === "localWorld") {
        runtime.state.clientHost = "worker-integrated";
        runtime.state.remoteWebSocketUrl = null;
      }
      if (report.startupProgressVisible === true) {
        runtime.state.startupProgressObserved = true;
      }
      if (report.startupReady === false) {
        const cameraY = Number(report.cameraY);
        if (Number.isFinite(cameraY)) {
          runtime.state.startupHoldCameraY ??= cameraY;
          runtime.state.minimumPreStartupCameraY = Math.min(
            runtime.state.minimumPreStartupCameraY ?? cameraY,
            cameraY,
          );
        }
      } else if (report.startupReady === true && !wasStartupReady) {
        runtime.state.startupAdmissionFrame = Number(report.frameCount);
        runtime.state.startupAdmissionCameraY = Number(report.cameraY);
      }
      if (report.streamingIdle === true) {
        runtime.state.loadedCenterX = Number(report.centerX);
        runtime.state.loadedCenterZ = Number(report.centerZ);
      }
      if (typeof report.streamingIdle !== "undefined") {
        runtime.state.streamingSettled = Boolean(report.streamingIdle);
      }
      if (typeof report.pendingCompileJobCount !== "undefined") {
        const pendingJobs = Number(report.pendingCompileJobCount) || 0;
        runtime.state.compileInFlight = pendingJobs > 0;
      }
      if (typeof report.renderWorkerPendingRequestCount !== "undefined") {
        runtime.state.compileInFlightCount =
          Number(report.renderWorkerPendingRequestCount) || 0;
      }
      if (report.rendered === true || runtime.state.lastReport == null) {
        runtime.state.lastReport = report;
      }
      const target = app.sceneHostForObserver()?.previewBlockTarget();
      if (target?.ok) {
        runtime.state.currentTarget = target;
        if (typeof target.selectedHotbarSlot !== "undefined") {
          runtime.state.selectedHotbarSlot = Number(target.selectedHotbarSlot);
        }
      }
    },
    latestReport(): WasmReport | null {
      return latestObservedReport;
    },
    observeTarget(target): void {
      if (target?.ok) {
        runtime.state.currentTarget = target;
        if (typeof target.selectedHotbarSlot !== "undefined") {
          runtime.state.selectedHotbarSlot = Number(target.selectedHotbarSlot);
        }
      }
    },
    resetForSessionRestart(): void {
      Object.assign(runtime.state, {
        loadedCenterX: null,
        loadedCenterZ: null,
        compileInFlight: false,
        compileInFlightCount: 0,
        streamingSettled: false,
        startupReady: false,
        startupHoldCameraY: null,
        minimumPreStartupCameraY: null,
        startupAdmissionFrame: null,
        startupAdmissionCameraY: null,
      });
      const session = app.sceneHostForObserver();
      if (session) {
        observer.observeReport(session.cameraFrameState());
        observer.observeTarget(session.previewBlockTarget());
      }
    },
    observeLobbyRuntimeStart(result): void {
      runtime.state.lobbyRuntimeStartCount =
        (Number(runtime.state.lobbyRuntimeStartCount) || 0) + 1;
      runtime.state.lastLobbyRuntimeStart = result;
    },
    observeAssetPackCompletion(): void {
      runtime.state.assetPackCompletionCount =
        (Number(runtime.state.assetPackCompletionCount) || 0) + 1;
    },
    observeWorldCatalogCompletion(): void {
      runtime.state.worldCatalogCompletionCount =
        (Number(runtime.state.worldCatalogCompletionCount) || 0) + 1;
    },
    observeSceneOperationCompletion(_effect, completion): void {
      switch (completion.sceneOperationReceipt) {
        case "active-runtime":
          observer.resetForSessionRestart();
          break;
        case "lobby-runtime":
          observer.observeLobbyRuntimeStart(completion);
          break;
        case "assets":
          observer.observeAssetPackCompletion();
          break;
        case "catalog":
          observer.observeWorldCatalogCompletion();
          break;
      }
    },
    waitForStartupProgressCapture(report): Promise<void> {
      if (
        !holdStartupProgress
        || startupProgressCaptureConsumed
        || report.startupProgressVisible !== true
      ) {
        return Promise.resolve();
      }
      startupProgressCaptureConsumed = true;
      return new Promise((resolve) => {
        releaseStartupProgressCapture = resolve;
      });
    },
  };
  let smokeHarness: WebSceneSmokeHarness | null = null;
  const applySmoke = (
    operation: (harness: WebSceneSmokeHarness, session: WebSceneHost) => WasmReport,
  ): WasmReport | null => {
    const session = app.sceneHostForObserver();
    const module = app.wasmModuleForObserver();
    if (!session || !module) {
      return null;
    }
    smokeHarness ??= new module.WebSceneSmokeHarness();
    const report = operation(smokeHarness, session);
    app.applyObserverReport(report);
    return observer.latestReport() ?? report;
  };
  const apply = (
    operation: (session: WebSceneHost) => WasmReport,
    options: { fromPointer?: boolean; pointerType?: string } = {},
  ): WasmReport | null => {
    const session = app.sceneHostForObserver();
    if (!session) {
      return null;
    }
    const report = operation(session);
    app.applyObserverReport(report, options);
    return observer.latestReport() ?? report;
  };
  runtime.adjustCameraSpeed = (amount) => apply((session) => session.adjustCameraSpeed(amount));
  runtime.previewBlockTarget = () => runtime.state.currentTarget ?? null;
  runtime.blockStateAt = (x, y, z) => {
    const session = app.sceneHostForObserver();
    return session?.blockStateAt(Math.trunc(x), Math.trunc(y), Math.trunc(z)) ?? null;
  };
  runtime.interactBlock = async (action) => {
    const report = await app.withObserverSceneHost((session) => session.interactBlock(action));
    if (report?.ok) {
      runtime.state.interactionCount =
        (Number(runtime.state.interactionCount) || 0) + 1;
      runtime.state.lastInteraction = report;
      runtime.state.interactionStatus = interactionStatus(report);
      observer.observeReport(report);
    }
    return report;
  };
  runtime.frameEmbeddedPreview = () => apply((session) => session.frameEmbeddedPreview());
  runtime.frameTerrainComposition = (eye, target) => apply(
    (session) => session.frameTerrainComposition(...eye, ...target),
  );
  runtime.frameInteractionSurface = () => {
    const report = apply((session) => session.frameInteractionSurface());
    observer.observeTarget(app.sceneHostForObserver()?.previewBlockTarget());
    return report;
  };
  runtime.renderOneFrameForSmoke = () => app.renderSingleObserverFrame();
  runtime.renderHalfSpaceTerrainProof = async () => {
    await app.waitForObserverIdle(true);
    return applySmoke((harness, session) => harness.renderHalfSpaceTerrainProof(session));
  };
  runtime.renderPreparedFigureProof = async () => {
    await app.waitForObserverIdle(true);
    return applySmoke((harness, session) => harness.renderPreparedFigureProof(session));
  };
  runtime.renderActorCompositionProof = async () => {
    await app.waitForObserverIdle(true);
    return applySmoke((harness, session) => harness.renderActorCompositionProof(session));
  };
  runtime.rebuildRenderResourcesForSmoke = () => applySmoke(
    (harness, session) => harness.rebuildRenderResourcesForSmoke(session),
  );
  runtime.openNativeTitleUi = () => apply((session) => session.openTitleUi());
  runtime.openNativePauseUi = () => apply((session) => session.openPauseUi());
  runtime.pauseRendering = () => app.pauseRendering();
  runtime.resumeRendering = () => app.resumeRendering();
  runtime.renderOverviewFrame = () => {
    const session = app.sceneHostForObserver();
    if (!session) {
      return null;
    }
    const camera = session.cameraFrameState();
    return apply((host) => host.syncOverviewRenderFrame(
      Number(camera.centerX) || 0,
      Number(camera.centerZ) || 0,
      observerRenderRadius(),
    ));
  };
  runtime.setDebugOverlay = (visible) => apply(
    (session) => session.setDebugOverlayVisible(visible),
  );
  runtime.openNativeHelpUi = () => apply((session) => session.openHelpUi());
  runtime.closeNativeUi = () => apply((session) => session.closeUi());
  runtime.handleNativeUiKey = (key) => apply((session) => session.handleUiKey(key));
  runtime.handleNativeUiPointerMove = (clientX, clientY, pointerType) => {
    const point = app.observerCanvasPoint(clientX, clientY);
    return apply(
      (session) => session.handleUiPointerMove(
        point.x,
        point.y,
        observerRenderRadius(),
      ),
      { pointerType },
    );
  };
  runtime.handleNativeUiPointerDown = (clientX, clientY, pointerType) => {
    const point = app.observerCanvasPoint(clientX, clientY);
    return apply(
      (session) => session.handleUiPointerDown(
        point.x,
        point.y,
        observerRenderRadius(),
      ),
      { pointerType },
    );
  };
  runtime.handleNativeUiPointerUp = (clientX, clientY, pointerType) => {
    const point = app.observerCanvasPoint(clientX, clientY);
    return apply(
      (session) => session.handleUiPointerUp(
        point.x,
        point.y,
        observerRenderRadius(),
      ),
      { fromPointer: true, pointerType },
    );
  };
  runtime.setNativeTouchControlsMode = (mode, _persist) => apply(
    (session) => session.setTouchControlsMode(mode),
  );
  runtime.touchControlState = () => ({
    visible: runtime.state.touchControlsVisible === true,
    activePointerCount: app.touchControls?.snapshot().activePointerCount ?? 0,
  });
  runtime.beginLobbySmoke = (chunkSpan = 2) => {
    const report = applySmoke(
      (harness, session) => harness.beginLobbySmokeWithChunkSpan(session, chunkSpan),
    );
    app.drainSceneOperations();
    return report;
  };
  runtime.sceneBorrowExcluded = () => app.sceneHostForObserver() === null;
  runtime.backgroundSaveForSmoke = () => app.backgroundCycleForObserver();
  runtime.shutdownForSmoke = () => app.shutdownForObserver();
  globalThis.__mcloneWebApp = runtime;
  return observer;
}

function interactionStatus(report: WasmReport): string {
  if (typeof report.reason === "string" && report.reason.length > 0) {
    return report.reason;
  }
  if (typeof report.action === "string" && report.action.length > 0) {
    return report.action;
  }
  return report.ok ? "ok" : "failed";
}
