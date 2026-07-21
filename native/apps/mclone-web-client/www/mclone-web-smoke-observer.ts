// Explicit test-client exposure for browser smoke probes. The ordinary app
// loads this module only when `smokeObserver=1` is present in the page query.
// Game/engine vocabulary is permitted here because this is not a production
// platform adapter.

import type { TouchControlsMode } from "./mclone-web-settings.js";
import type { WebSceneHost } from "mclone-web-client-wasm";

type WasmReport = Record<string, any>;

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
  setNativeTouchLookSensitivity?: (
    value: number,
    available?: boolean,
    persist?: boolean,
  ) => WasmReport | null;
  setNativeTouchControlsMode?: (
    mode: TouchControlsMode,
    persist?: boolean,
  ) => WasmReport | null;
  touchControlState?: () => WasmReport | null;
  beginLobbySmoke?: (chunkSpan?: number) => WasmReport | null;
  backgroundSaveForSmoke?: () => WasmReport | null;
  shutdownForSmoke?: () => Promise<WasmReport | null>;
}

interface SmokeBridge {
  sceneHostForObserver(): WebSceneHost | null;
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
  observerRenderRadius(): number;
  pauseRendering(): void;
  resumeRendering(): void;
  setNativeDebugOverlay(visible: boolean): WasmReport | null;
  setNativeTouchLookSensitivity(
    value: number,
    available?: boolean,
    persist?: boolean,
  ): WasmReport | null;
  setNativeTouchControlsMode(
    mode: TouchControlsMode,
    persist?: boolean,
  ): WasmReport | null;
  touchControls: { snapshot(): WasmReport } | null;
  drainLobbyOperations(): void;
  backgroundCycleForObserver(): WasmReport | null;
  shutdownForObserver(): Promise<WasmReport | null>;
}

export interface WebSmokeObserver {
  observePlatformState(runtime: { ready: boolean; state: Record<string, any> }): void;
  observeReport(report: WasmReport | null | undefined): void;
  observeTarget(target: WasmReport | null | undefined): void;
  resetForSessionRestart(): void;
  observeLobbyRuntimeStart(result: WasmReport): void;
  observeAssetPackCompletion(): void;
  observeWorldCatalogCompletion(): void;
}

declare global {
  var __mcloneWebApp: SmokeRuntime;
}

export function installWebSmokeObserver(
  app: SmokeBridge,
): WebSmokeObserver {
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
  const observer: WebSmokeObserver = {
    observePlatformState(platformRuntime): void {
      runtime.ready = platformRuntime.ready;
      Object.assign(runtime.state, platformRuntime.state);
      const touch = app.touchControls?.snapshot();
      if (touch) {
        runtime.state.touchControlsVisible = touch.visible;
        runtime.state.touchPointerActiveCount = touch.activePointerCount;
      }
    },
    observeReport(report): void {
      if (!report?.ok) {
        return;
      }
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
    return report;
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
  runtime.frameInteractionSurface = () => {
    const report = apply((session) => session.frameInteractionSurface());
    observer.observeTarget(app.sceneHostForObserver()?.previewBlockTarget());
    return report;
  };
  runtime.renderOneFrameForSmoke = () => app.renderSingleObserverFrame();
  runtime.renderHalfSpaceTerrainProof = async () => {
    await app.waitForObserverIdle(true);
    return app.sceneHostForObserver()?.renderHalfSpaceTerrainProof() ?? null;
  };
  runtime.renderPreparedFigureProof = async () => {
    await app.waitForObserverIdle(true);
    return app.sceneHostForObserver()?.renderPreparedFigureProof() ?? null;
  };
  runtime.renderActorCompositionProof = async () => {
    await app.waitForObserverIdle(true);
    return app.sceneHostForObserver()?.renderActorCompositionProof() ?? null;
  };
  runtime.rebuildRenderResourcesForSmoke = () => (
    apply((session) => session.rebuildRenderResourcesForSmoke())
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
      app.observerRenderRadius(),
    ));
  };
  runtime.setDebugOverlay = (visible) => app.setNativeDebugOverlay(visible);
  runtime.openNativeHelpUi = () => apply((session) => session.openHelpUi());
  runtime.closeNativeUi = () => apply((session) => session.closeUi());
  runtime.handleNativeUiKey = (key) => apply((session) => session.handleUiKey(key));
  runtime.handleNativeUiPointerMove = (clientX, clientY, pointerType) => {
    const point = app.observerCanvasPoint(clientX, clientY);
    return apply(
      (session) => session.handleUiPointerMove(
        point.x,
        point.y,
        app.observerRenderRadius(),
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
        app.observerRenderRadius(),
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
        app.observerRenderRadius(),
      ),
      { fromPointer: true, pointerType },
    );
  };
  runtime.setNativeTouchLookSensitivity = (value, available, persist) => (
    app.setNativeTouchLookSensitivity(value, available, persist)
  );
  runtime.setNativeTouchControlsMode = (mode, persist) => (
    app.setNativeTouchControlsMode(mode, persist)
  );
  runtime.touchControlState = () => app.touchControls?.snapshot() ?? null;
  runtime.beginLobbySmoke = (chunkSpan = 2) => {
    const report = apply(
      (session) => session.beginLobbySmokeWithChunkSpan(chunkSpan),
    );
    app.drainLobbyOperations();
    return report;
  };
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
