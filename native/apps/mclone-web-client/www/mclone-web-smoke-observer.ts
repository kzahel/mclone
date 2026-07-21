// Explicit test-client exposure for browser smoke probes. The ordinary app
// loads this module only when `smokeObserver=1` is present in the page query.
// Game/engine vocabulary is permitted here because this is not a production
// platform adapter.

import type { TouchControlsMode } from "./mclone-web-settings.js";

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
  adjustCameraSpeed(amount: number): WasmReport | null;
  blockStateAt(x: number, y: number, z: number): WasmReport | null;
  interactBlock(action: string): Promise<WasmReport | null>;
  frameEmbeddedPreview(): WasmReport | null;
  frameInteractionSurface(): WasmReport | null;
  renderOneFrameForSmoke(): Promise<WasmReport | null>;
  renderHalfSpaceTerrainProof(): Promise<WasmReport | null>;
  renderPreparedFigureProof(): Promise<WasmReport | null>;
  renderActorCompositionProof(): Promise<WasmReport | null>;
  rebuildRenderResourcesForSmoke(): WasmReport | null;
  openNativeTitleUi(): WasmReport | null;
  openNativePauseUi(): WasmReport | null;
  pauseRendering(): void;
  resumeRendering(): void;
  renderOverviewFrame(): WasmReport | null;
  setNativeDebugOverlay(visible: boolean): WasmReport | null;
  openNativeHelpUi(): WasmReport | null;
  closeNativeUi(): WasmReport | null;
  handleNativeUiKey(key: string): WasmReport | null;
  handleNativeUiPointerMove(
    clientX: number,
    clientY: number,
    pointerType?: string,
  ): WasmReport | null;
  handleNativeUiPointerDown(
    clientX: number,
    clientY: number,
    pointerType?: string,
  ): WasmReport | null;
  handleNativeUiPointerUp(
    clientX: number,
    clientY: number,
    pointerType?: string,
  ): WasmReport | null;
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
  beginLobbySmoke(chunkSpan?: number): WasmReport | null;
  backgroundSaveForSmoke(): WasmReport | null;
  shutdownForSmoke(): Promise<WasmReport | null>;
}

declare global {
  var __mcloneWebApp: SmokeRuntime;
}

export function installWebSmokeObserver(
  runtime: SmokeRuntime,
  app: SmokeBridge,
): void {
  runtime.adjustCameraSpeed = (amount) => app.adjustCameraSpeed(amount);
  runtime.previewBlockTarget = () => runtime.state.currentTarget ?? null;
  runtime.blockStateAt = (x, y, z) => app.blockStateAt(x, y, z);
  runtime.interactBlock = (action) => app.interactBlock(action);
  runtime.frameEmbeddedPreview = () => app.frameEmbeddedPreview();
  runtime.frameInteractionSurface = () => app.frameInteractionSurface();
  runtime.renderOneFrameForSmoke = () => app.renderOneFrameForSmoke();
  runtime.renderHalfSpaceTerrainProof = () => app.renderHalfSpaceTerrainProof();
  runtime.renderPreparedFigureProof = () => app.renderPreparedFigureProof();
  runtime.renderActorCompositionProof = () => app.renderActorCompositionProof();
  runtime.rebuildRenderResourcesForSmoke = () => app.rebuildRenderResourcesForSmoke();
  runtime.openNativeTitleUi = () => app.openNativeTitleUi();
  runtime.openNativePauseUi = () => app.openNativePauseUi();
  runtime.pauseRendering = () => app.pauseRendering();
  runtime.resumeRendering = () => app.resumeRendering();
  runtime.renderOverviewFrame = () => app.renderOverviewFrame();
  runtime.setDebugOverlay = (visible) => app.setNativeDebugOverlay(visible);
  runtime.openNativeHelpUi = () => app.openNativeHelpUi();
  runtime.closeNativeUi = () => app.closeNativeUi();
  runtime.handleNativeUiKey = (key) => app.handleNativeUiKey(key);
  runtime.handleNativeUiPointerMove = (clientX, clientY, pointerType) => (
    app.handleNativeUiPointerMove(clientX, clientY, pointerType)
  );
  runtime.handleNativeUiPointerDown = (clientX, clientY, pointerType) => (
    app.handleNativeUiPointerDown(clientX, clientY, pointerType)
  );
  runtime.handleNativeUiPointerUp = (clientX, clientY, pointerType) => (
    app.handleNativeUiPointerUp(clientX, clientY, pointerType)
  );
  runtime.setNativeTouchLookSensitivity = (value, available, persist) => (
    app.setNativeTouchLookSensitivity(value, available, persist)
  );
  runtime.setNativeTouchControlsMode = (mode, persist) => (
    app.setNativeTouchControlsMode(mode, persist)
  );
  runtime.touchControlState = () => app.touchControls?.snapshot() ?? null;
  runtime.beginLobbySmoke = (chunkSpan = 2) => app.beginLobbySmoke(chunkSpan);
  runtime.backgroundSaveForSmoke = () => app.backgroundSaveForSmoke();
  runtime.shutdownForSmoke = () => app.shutdownForSmoke();
  globalThis.__mcloneWebApp = runtime;
}
