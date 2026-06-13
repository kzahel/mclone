import { deleteIndexedDbWorldStorage } from "../runtime/storage/indexeddb-world-storage";
import { Vec3 } from "../world/phys/vec3";
import { ProgressScreen } from "../client/gui/screens/progress-screen";
import { TitleScreen } from "../client/gui/screens/title-screen";
import { OptionsScreen, type GuiOptionsState } from "../client/gui/screens/options-screen";
import { DebugSettingsScreen, type GuiDebugSettingsState } from "../client/gui/screens/debug-settings-screen";
import { ScreenManager } from "../client/gui/screen-manager";
import { type CameraState } from "./game-renderer";
import {
  applyRenderWorldDirtySections,
  closeRendererScene,
  getSceneLoadedChunkCount,
  initializeRendererScene,
  renderSceneUntilSettled,
  resizeCanvasToDisplaySize,
  waitForLoadedChunkRing,
  type RenderSceneQueueStats,
  type RenderWorldPerformanceCounters,
  type RendererScene,
} from "./scene-setup";
import { getExpectedLoadedChunkCount, readBrowserRenderConfig, writeStoredBrowserRenderConfig } from "./browser-render-config";
import {
  readBrowserWorldTransportConfig,
  readBrowserWorldTransportSettings,
  resolveBrowserWorldTransportConfig,
  type BrowserWorldTransportConfig,
  writeStoredBrowserWorldTransportSettings,
} from "./browser-world-transport-config";
import {
  readAutoStartWorldEnabled,
  readDebugLaunchEnabled,
} from "./browser-world-launch-config";
import {
  getGeneratedWorldSmokeScenarioById,
} from "./generated-world-smoke-scenario";
import {
  createGeneratedWorldBrowserBootAdapter,
  resolveGeneratedWorldBrowserCamera,
} from "./generated-world-browser-boot";
import {
  runGeneratedWorldBoot,
  type GeneratedWorldBootResult,
} from "./generated-world-boot";
import type { LevelRenderFrame } from "./level-renderer";
import type { GeneratedWorldSmokeScenarioResult } from "./generated-world-smoke-runner";
import type { OpenWorldPreset, WorldPerformanceSnapshot } from "../runtime/protocol/world-messages";
import { progressFraction, type LoadingProgress, type LoadingProgressSink } from "./loading-progress";
import {
  readDebugSessionConfig,
  writeStartLastWorld,
  writeStoredDebugSessionConfig,
  type DebugSessionConfig,
  type DebugMovementMode,
} from "./debug/debug-session-config";
import { BROWSER_RENDERER_HOST, type BrowserRendererHost } from "./renderer-host";
import { GuiRenderer } from "./gui/gui-renderer";
import { GuiOverlayHost } from "./gui/gui-overlay-host";
import {
  configureBrowserCanvasTarget,
  requestWebGpuDeviceContext,
  type BrowserCanvasTarget,
  type WebGpuDeviceContext,
} from "./webgpu-target";
import { startGpuWorldRuntime, type GpuWorldRuntime } from "./gui/gpu-world-runtime";
import { createChunkViewRequestForCameraState, type DebugInjectedInput } from "./debug/debug-player-controls";
import type { GeneratedChunkLifecycleSnapshot } from "../runtime/protocol/chunk-lifecycle";

export type BootResult =
  | GeneratedWorldSmokeScenarioResult
  | { ok: false; reason: string };

type GpuTitleWorldResult = BootResult | LiveWorldBootResult;
type MainBootResult = BootResult | GpuTitleBootResult;

export interface GpuTitleBootResult {
  readonly ok: true;
  readonly mode: "gpu_title";
  readonly width: number;
  readonly height: number;
  readonly screenTitle: string;
}

interface GpuGuiRuntimeState {
  ready: boolean;
  mode: "title" | "options" | "debug_settings" | "loading" | "world" | "paused" | "error";
  screenTitle: string;
  lastAction?: "start_world" | "options" | "options_done" | "debug_settings" | "debug_settings_done" | "back_to_game" | "disconnect";
  loadingStage?: string;
  loadingDetail?: string;
  loadingProgress?: number;
  worldReady?: boolean;
  worldResult?: GpuTitleWorldResult;
  worldTransport?: "worker" | "remote";
  worldAuthority?: "local" | "dedicated";
  dedicatedSocketUrl?: string;
  seed?: string;
  movementMode?: DebugMovementMode;
  preset?: OpenWorldPreset;
  saveId?: string;
  sessionId?: string;
  playerId?: string;
  playerTick?: number;
  playerPosition?: readonly [number, number, number];
  playerChunkX?: number;
  playerChunkZ?: number;
  chunkViewCenterX?: number;
  chunkViewCenterZ?: number;
  expectedLoadedChunkCount?: number;
  pauseScreenActive?: boolean;
  frameCount: number;
  inputEventCount: number;
  cameraPosition?: readonly [number, number, number];
  cameraYaw?: number;
  cameraPitch?: number;
  loadedChunkCount?: number;
  renderWorldCounters?: RenderWorldPerformanceCounters;
  renderQueueStats?: RenderSceneQueueStats;
  worldPerformance?: WorldPerformanceSnapshot;
  chunkLifecycle?: GeneratedChunkLifecycleSnapshot;
  viewDistance?: number;
  renderDistance?: number;
  fogEnabled?: boolean;
  lightingMode?: string;
  liquidSimulationMode?: string;
  worldStorageMode?: string;
  options?: GuiOptionsState;
  debugSettings?: GuiDebugSettingsState;
  error?: string;
  width: number;
  height: number;
}

interface GpuGuiRuntimeController {
  readonly state: GpuGuiRuntimeState;
  worldReady?: Promise<GpuTitleWorldResult>;
  worldRuntime?: GpuWorldRuntime;
  setInjectedInput(input: DebugInjectedInput | null): void;
}

declare global {
  interface Window {
    __mcloneReady: Promise<MainBootResult>;
    __mcloneGui?: GpuGuiRuntimeController;
    __mcloneDebug?: GpuGuiRuntimeController;
  }
}

const READBACK_FORMAT: GPUTextureFormat = "rgba8unorm";
const DEFAULT_TITLE_WORLD_CAMERA: CameraState = {
  position: new Vec3(8.5, 104.0, 40.5),
  xRot: 30.0,
  yRot: 180.0,
};

interface LiveWorldBootResult {
  readonly ok: true;
  readonly mode: "live_world";
  readonly worldTransport: "worker" | "remote";
  readonly saveId: string;
  readonly viewDistance: number;
  readonly renderDistance: number;
  readonly fogEnabled: boolean;
  readonly lightingMode: string;
  readonly liquidSimulationMode: string;
  readonly loadedChunkCount: number;
  readonly expectedLoadedChunkCount: number;
}

function readWorldTransport(url?: URL): BrowserWorldTransportConfig {
  if (typeof window === "undefined") {
    return { worldTransport: "worker" };
  }

  return readBrowserWorldTransportConfig(url ?? new URL(window.location.href), browserRenderConfigStorage());
}

function parseOptionalFloat(url: URL, key: string): number | undefined {
  const raw = url.searchParams.get(key);
  if (raw === null) {
    return undefined;
  }

  const parsed = Number.parseFloat(raw);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function readSmokeCamera(url: URL): CameraState | undefined {
  const x = parseOptionalFloat(url, "cameraX");
  const y = parseOptionalFloat(url, "cameraY");
  const z = parseOptionalFloat(url, "cameraZ");
  const xRot = parseOptionalFloat(url, "cameraPitch");
  const yRot = parseOptionalFloat(url, "cameraYaw");
  if (x === undefined || y === undefined || z === undefined || xRot === undefined || yRot === undefined) {
    return undefined;
  }

  return {
    position: new Vec3(x, y, z),
    xRot,
    yRot,
  };
}

function readClearWorldStorage(url: URL): boolean {
  const value = url.searchParams.get("clearWorldStorage");
  return value === "1" || value === "true";
}

function readPreserveInitialCamera(url: URL): boolean {
  const value = url.searchParams.get("preserveInitialCamera");
  return value === "1" || value === "true";
}

function readGuiProbeEnabled(url: URL): boolean {
  const value = url.searchParams.get("guiProbe");
  return value === "1" || value === "true";
}

function readGpuTitleEnabled(url: URL): boolean {
  const value = url.searchParams.get("gpuTitle");
  if (value === "0" || value === "false") {
    return false;
  }
  if (value === "1" || value === "true") {
    return true;
  }

  return !url.pathname.endsWith("/smoke.html");
}

function browserRenderConfigStorage(): Storage | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

function readGuiOptionsState(url: URL): GuiOptionsState {
  const config = readBrowserRenderConfig(url, browserRenderConfigStorage());
  return {
    viewDistance: config.viewDistance,
    renderDistance: config.renderDistance,
    fogEnabled: config.fogEnabled,
    lightingMode: config.lightingMode,
    liquidSimulationMode: config.liquidSimulationMode,
    autoJump: config.autoJump,
  };
}

interface GuiDebugSettingsBundle {
  debugSession: DebugSessionConfig;
  debugSettings: GuiDebugSettingsState;
}

function readGuiDebugSettingsState(url: URL): GuiDebugSettingsBundle {
  const storage = browserRenderConfigStorage();
  const renderConfig = readBrowserRenderConfig(url, storage);
  const debugSession = readDebugSessionConfig(url, storage);
  const worldTransportSettings = readBrowserWorldTransportSettings(url, storage);
  return {
    debugSession,
    debugSettings: {
      movementMode: debugSession.movementMode,
      preset: debugSession.preset,
      worldStorageMode: renderConfig.worldStorageMode,
      showDebugInfo: debugSession.showDebugInfo,
      showChunkBorders: debugSession.showChunkBorders,
      worldAuthority: worldTransportSettings.worldAuthority,
      dedicatedSocketUrl: worldTransportSettings.dedicatedSocketUrl,
    },
  };
}

function copyGuiOptionsState(options: GuiOptionsState): GuiOptionsState {
  return {
    viewDistance: options.viewDistance,
    renderDistance: options.renderDistance,
    fogEnabled: options.fogEnabled,
    lightingMode: options.lightingMode,
    liquidSimulationMode: options.liquidSimulationMode,
    autoJump: options.autoJump,
  };
}

function copyGuiDebugSettingsState(debugSettings: GuiDebugSettingsState): GuiDebugSettingsState {
  return {
    movementMode: debugSettings.movementMode,
    preset: debugSettings.preset,
    worldStorageMode: debugSettings.worldStorageMode,
    showDebugInfo: debugSettings.showDebugInfo,
    showChunkBorders: debugSettings.showChunkBorders,
    worldAuthority: debugSettings.worldAuthority,
    dedicatedSocketUrl: debugSettings.dedicatedSocketUrl,
  };
}

function resolveGuiWorldTransportConfig(debugSettings: GuiDebugSettingsState): BrowserWorldTransportConfig {
  return resolveBrowserWorldTransportConfig({
    worldAuthority: debugSettings.worldAuthority,
    dedicatedSocketUrl: debugSettings.dedicatedSocketUrl,
  });
}

function persistGuiSettingsState(
  options: GuiOptionsState,
  debugSession: DebugSessionConfig,
  debugSettings: GuiDebugSettingsState,
): void {
  const storage = browserRenderConfigStorage();
  if (storage === undefined) {
    return;
  }
  try {
    writeStoredBrowserRenderConfig(storage, {
      ...options,
      worldStorageMode: debugSettings.worldStorageMode,
    });
    writeStoredDebugSessionConfig(storage, debugSession);
    writeStartLastWorld(storage, debugSession);
    writeStoredBrowserWorldTransportSettings(storage, {
      worldAuthority: debugSettings.worldAuthority,
      dedicatedSocketUrl: debugSettings.dedicatedSocketUrl,
    });
  } catch {
    // WebGPU: localStorage may be unavailable in strict browser contexts; keep the in-memory GUI state usable.
  }
}

interface GeneratedWorldSmokeBootOptions {
  readonly rendererHost?: BrowserRendererHost;
  readonly onProgress?: LoadingProgressSink;
}

type GeneratedWorldSmokeBootResult =
  GeneratedWorldBootResult;

async function bootGpuTitle(canvas: HTMLCanvasElement, url: URL): Promise<MainBootResult> {
  const initialSize = GuiRenderer.calculateGuiSize(canvas.width, canvas.height);
  const screenManager = new ScreenManager(initialSize.guiWidth, initialSize.guiHeight);
  const optionsState = readGuiOptionsState(url);
  const debugSettingsState = readGuiDebugSettingsState(url);
  const runtimeConfig = resolveGuiWorldTransportConfig(debugSettingsState.debugSettings);
  const debugLaunch = readDebugLaunchEnabled(url);
  const autoStartWorld = readAutoStartWorldEnabled(url);
  const state: GpuGuiRuntimeState = {
    ready: false,
    mode: autoStartWorld ? "loading" : "title",
    screenTitle: autoStartWorld ? "Progress Screen" : "Title Screen",
    frameCount: 0,
    inputEventCount: 0,
    worldTransport: runtimeConfig.worldTransport,
    worldAuthority: debugSettingsState.debugSettings.worldAuthority,
    dedicatedSocketUrl: debugSettingsState.debugSettings.dedicatedSocketUrl,
    seed: debugSettingsState.debugSession.seed.toString(),
    movementMode: debugSettingsState.debugSession.movementMode,
    preset: debugSettingsState.debugSession.preset,
    viewDistance: optionsState.viewDistance,
    renderDistance: optionsState.renderDistance,
    fogEnabled: optionsState.fogEnabled,
    lightingMode: optionsState.lightingMode,
    liquidSimulationMode: optionsState.liquidSimulationMode,
    worldStorageMode: debugSettingsState.debugSettings.worldStorageMode,
    options: copyGuiOptionsState(optionsState),
    debugSettings: copyGuiDebugSettingsState(debugSettingsState.debugSettings),
    width: initialSize.guiWidth,
    height: initialSize.guiHeight,
  };
  let pendingInjectedInput: DebugInjectedInput | null = null;
  const controller: GpuGuiRuntimeController = {
    state,
    setInjectedInput(input: DebugInjectedInput | null): void {
      pendingInjectedInput = input;
      this.worldRuntime?.setInjectedInput(input);
    },
  };
  if (typeof window !== "undefined") {
    window.__mcloneGui = controller;
    if (debugLaunch) {
      window.__mcloneDebug = controller;
    }
  }

  const deviceContextResult = await requestWebGpuDeviceContext();
  if (!deviceContextResult.ok) {
    state.mode = "error";
    state.error = deviceContextResult.reason;
    return deviceContextResult;
  }

  const { device, format } = deviceContextResult.context;
  const resized = resizeCanvasToDisplaySize(canvas, device.limits.maxTextureDimension2D);
  const size = GuiRenderer.calculateGuiSize(resized.width, resized.height);
  screenManager.resize(size.guiWidth, size.guiHeight);
  state.width = size.guiWidth;
  state.height = size.guiHeight;
  const target = configureBrowserCanvasTarget(canvas, device, format);
  if (target === undefined) {
    state.mode = "error";
    state.error = "canvas.getContext('webgpu') returned null";
    return { ok: false, reason: "canvas.getContext('webgpu') returned null" };
  }

  let renderQueued = false;
  let host: GuiOverlayHost | undefined;
  const renderGuiFrameNow = (): void => {
    const activeHost = host;
    if (
      activeHost === undefined
      || screenManager.currentScreen === null
      || state.mode === "world"
      || state.mode === "paused"
    ) {
      return;
    }

    const resized = resizeCanvasToDisplaySize(canvas, device.limits.maxTextureDimension2D);
    const guiSize = GuiRenderer.calculateGuiSize(resized.width, resized.height);
    state.width = guiSize.guiWidth;
    state.height = guiSize.guiHeight;
    const view = target.ctx.getCurrentTexture().createView();
    const encoder = device.createCommandEncoder();
    const clearPass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view,
          clearValue: { r: 0.06, g: 0.07, b: 0.08, a: 1 },
          loadOp: "clear",
          storeOp: "store",
        },
      ],
    });
    clearPass.end();
    activeHost.encode(encoder, view, target.format, resized.width, resized.height);
    device.queue.submit([encoder.finish()]);
  };
  const scheduleGuiFrame = (): void => {
    if (host === undefined || renderQueued) {
      return;
    }

    renderQueued = true;
    requestAnimationFrame(() => {
      renderQueued = false;
      renderGuiFrameNow();
    });
  };

  let startWorldPromise: Promise<GpuTitleWorldResult> | undefined;
  const onSettingsChanged = (): void => {
    debugSettingsState.debugSession = {
      seed: debugSettingsState.debugSession.seed,
      movementMode: debugSettingsState.debugSettings.movementMode,
      preset: debugSettingsState.debugSettings.preset,
      showDebugInfo: debugSettingsState.debugSettings.showDebugInfo,
      showChunkBorders: debugSettingsState.debugSettings.showChunkBorders,
    };
    persistGuiSettingsState(optionsState, debugSettingsState.debugSession, debugSettingsState.debugSettings);
    state.options = copyGuiOptionsState(optionsState);
    state.debugSettings = copyGuiDebugSettingsState(debugSettingsState.debugSettings);
    state.seed = debugSettingsState.debugSession.seed.toString();
    state.movementMode = debugSettingsState.debugSession.movementMode;
    state.preset = debugSettingsState.debugSession.preset;
    state.viewDistance = optionsState.viewDistance;
    state.renderDistance = optionsState.renderDistance;
    state.fogEnabled = optionsState.fogEnabled;
    state.lightingMode = optionsState.lightingMode;
    state.liquidSimulationMode = optionsState.liquidSimulationMode;
    state.worldStorageMode = debugSettingsState.debugSettings.worldStorageMode;
    state.worldAuthority = debugSettingsState.debugSettings.worldAuthority;
    state.dedicatedSocketUrl = debugSettingsState.debugSettings.dedicatedSocketUrl;
    state.worldTransport = resolveGuiWorldTransportConfig(debugSettingsState.debugSettings).worldTransport;
  };
  const startWorld = (): void => {
    state.lastAction = "start_world";
    state.error = undefined;
    state.worldReady = undefined;
    state.worldResult = undefined;
    const guiOverlayHost = host;
    if (guiOverlayHost === undefined) {
      state.mode = "error";
      state.error = "GUI overlay host is not initialized";
      return;
    }
    if (startWorldPromise !== undefined) {
      return;
    }

    const worldPromise = startGpuTitleWorld({
      canvas,
      url,
      deviceContext: deviceContextResult.context,
      target,
      guiOverlayHost,
      controller,
      screenManager,
      state,
      optionsState,
      onOptionsChanged: onSettingsChanged,
      debugSession: debugSettingsState.debugSession,
      debugSettingsState: debugSettingsState.debugSettings,
      onDebugSettingsChanged: onSettingsChanged,
      onDisconnectFromWorld: disconnectToTitle,
      renderGuiFrameNow,
      debugLaunch,
      getPendingInjectedInput: () => pendingInjectedInput,
    });
    startWorldPromise = worldPromise;
    controller.worldReady = worldPromise;
    void worldPromise.finally(() => {
      if (startWorldPromise === worldPromise && controller.worldRuntime === undefined) {
        startWorldPromise = undefined;
        if (controller.worldReady === worldPromise) {
          controller.worldReady = undefined;
        }
      }
    });
  };

  let titleScreen: TitleScreen;
  const openOptions = (): void => {
    state.lastAction = "options";
    state.mode = "options";
    state.screenTitle = "options.title";
    screenManager.setScreen(new OptionsScreen(titleScreen, optionsState, {
      onChanged: onSettingsChanged,
      onDone: () => {
        state.lastAction = "options_done";
        state.mode = "title";
        state.screenTitle = "Title Screen";
      },
    }));
    scheduleGuiFrame();
  };
  const openDebugSettings = (): void => {
    state.lastAction = "debug_settings";
    state.mode = "debug_settings";
    state.screenTitle = "debug.settings.title";
    screenManager.setScreen(new DebugSettingsScreen(titleScreen, debugSettingsState.debugSettings, {
      onChanged: onSettingsChanged,
      onDone: () => {
        state.lastAction = "debug_settings_done";
        state.mode = "title";
        state.screenTitle = "Title Screen";
      },
    }));
    scheduleGuiFrame();
  };
  const resetTitleState = (): void => {
    state.ready = true;
    state.mode = "title";
    state.screenTitle = "Title Screen";
    state.worldReady = undefined;
    state.worldResult = undefined;
    state.pauseScreenActive = false;
    state.loadingStage = undefined;
    state.loadingDetail = undefined;
    state.loadingProgress = undefined;
    state.saveId = undefined;
    state.sessionId = undefined;
    state.playerId = undefined;
    state.playerTick = undefined;
    state.playerPosition = undefined;
    state.playerChunkX = undefined;
    state.playerChunkZ = undefined;
    state.chunkViewCenterX = undefined;
    state.chunkViewCenterZ = undefined;
    state.expectedLoadedChunkCount = undefined;
    state.frameCount = 0;
    state.inputEventCount = 0;
    state.cameraPosition = undefined;
    state.cameraYaw = undefined;
    state.cameraPitch = undefined;
    state.loadedChunkCount = undefined;
    state.renderWorldCounters = undefined;
    state.renderQueueStats = undefined;
    state.worldPerformance = undefined;
    state.error = undefined;
  };
  const disconnectToTitle = async (closeWorld: () => Promise<void> | void): Promise<void> => {
    startWorldPromise = undefined;
    controller.worldReady = undefined;
    controller.worldRuntime = undefined;
    const progressScreen = new ProgressScreen(true);
    progressScreen.progressStart("Saving world");
    progressScreen.updateProgress({ stage: "Closing world", fraction: 0 });
    state.mode = "loading";
    state.screenTitle = "Progress Screen";
    state.pauseScreenActive = false;
    state.loadingStage = "Closing world";
    state.loadingDetail = undefined;
    state.loadingProgress = 0;
    screenManager.setScreen(progressScreen);
    renderGuiFrameNow();

    try {
      await closeWorld();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      state.mode = "error";
      state.error = reason;
      progressScreen.updateProgress({ stage: "Error", detail: reason, fraction: 1 });
      renderGuiFrameNow();
      return;
    }

    resetTitleState();
    screenManager.setScreen(titleScreen);
    scheduleGuiFrame();
  };
  titleScreen = new TitleScreen({
    onStartWorld: startWorld,
    onOptions: openOptions,
    onDebugSettings: openDebugSettings,
  });
  if (!autoStartWorld) {
    screenManager.setScreen(titleScreen);
  }
  host = await GuiOverlayHost.create(device, canvas, screenManager);
  host.attachInput(scheduleGuiFrame);
  window.addEventListener("resize", scheduleGuiFrame);
  if (autoStartWorld) {
    startWorld();
  } else {
    state.ready = true;
    scheduleGuiFrame();
  }

  return {
    ok: true,
    mode: "gpu_title",
    width: size.guiWidth,
    height: size.guiHeight,
    screenTitle: "Title Screen",
  };
}

interface StartGpuTitleWorldOptions {
  readonly canvas: HTMLCanvasElement;
  readonly url: URL;
  readonly deviceContext: WebGpuDeviceContext;
  readonly target: BrowserCanvasTarget;
  readonly guiOverlayHost: GuiOverlayHost;
  readonly controller: GpuGuiRuntimeController;
  readonly screenManager: ScreenManager;
  readonly state: GpuGuiRuntimeState;
  readonly optionsState: GuiOptionsState;
  readonly onOptionsChanged: () => void;
  readonly debugSession: DebugSessionConfig;
  readonly debugSettingsState: GuiDebugSettingsState;
  readonly onDebugSettingsChanged: () => void;
  readonly onDisconnectFromWorld: (closeWorld: () => Promise<void> | void) => Promise<void>;
  readonly renderGuiFrameNow: () => void;
  readonly debugLaunch: boolean;
  readonly getPendingInjectedInput: () => DebugInjectedInput | null;
}

async function startGpuTitleWorld(options: StartGpuTitleWorldOptions): Promise<GpuTitleWorldResult> {
  const progressScreen = new ProgressScreen(true);
  progressScreen.setChunkLifecycleDiagnosticsVisible(true);
  options.state.mode = "loading";
  options.state.screenTitle = "Progress Screen";
  options.screenManager.setScreen(progressScreen);

  const reportChunkLifecycle = (snapshot: GeneratedChunkLifecycleSnapshot | undefined): void => {
    progressScreen.updateChunkLifecycle(snapshot);
    options.state.chunkLifecycle = snapshot;
  };

  const reportProgress = (progress: LoadingProgress): void => {
    progressScreen.updateProgress(progress);
    options.state.loadingStage = progress.stage;
    options.state.loadingDetail = progress.detail;
    options.state.loadingProgress = progressFraction(progress);
    options.renderGuiFrameNow();
  };

  reportProgress({ stage: "Opening world", fraction: 0 });
  let closeWorld: (() => Promise<void> | void) | undefined;
  try {
    const bootResult = shouldUseSmokeTitleWorldStart(options.url)
      ? await runGpuTitleSmokeWorldBoot(options, reportProgress)
      : await runGpuTitleLiveWorldBoot(options, reportProgress, reportChunkLifecycle);
    if (!bootResult.ok) {
      options.state.mode = "error";
      options.state.error = bootResult.reason;
      reportProgress({ stage: "Error", detail: bootResult.reason, fraction: 1 });
      return bootResult;
    }

    const scene = bootResult.scene;
    closeWorld = bootResult.close;
    options.state.mode = "world";
    options.state.screenTitle = "";
    options.state.pauseScreenActive = false;
    options.state.worldResult = bootResult.result;
    options.screenManager.setScreen(null);
    options.state.frameCount = 0;
    options.state.inputEventCount = 0;
    options.controller.worldRuntime = await startGpuWorldRuntime({
      scene,
      canvas: options.canvas,
      screenManager: options.screenManager,
      guiOverlayHost: options.guiOverlayHost,
      initialCamera: bootResult.camera,
      initialFrame: bootResult.frame,
      state: options.state,
      optionsState: options.optionsState,
      onOptionsChanged: options.onOptionsChanged,
      debugSettingsState: options.debugSettingsState,
      onDebugSettingsChanged: options.onDebugSettingsChanged,
      movementMode: options.debugSettingsState.movementMode,
      preserveInitialCamera: readPreserveInitialCamera(options.url),
      requirePointerLock: true,
      onDisconnect: async () => {
        await options.onDisconnectFromWorld(closeWorld!);
      },
      onError: (message) => {
        options.state.mode = "error";
        options.state.error = message;
      },
    });
    options.controller.worldRuntime.setInjectedInput(options.getPendingInjectedInput());
    options.state.worldReady = true;
    return bootResult.result;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    if (closeWorld !== undefined) {
      try {
        await closeWorld();
      } catch {}
    }
    options.state.mode = "error";
    options.state.error = reason;
    reportProgress({ stage: "Error", detail: reason, fraction: 1 });
    return { ok: false, reason };
  }
}

function shouldUseSmokeTitleWorldStart(url: URL): boolean {
  return url.pathname.endsWith("/smoke.html");
}

type GpuTitleWorldBootRun =
  | {
    readonly ok: true;
    readonly scene: RendererScene;
    readonly frame: LevelRenderFrame;
    readonly camera: CameraState;
    readonly result: GeneratedWorldSmokeScenarioResult | LiveWorldBootResult;
    close(): Promise<void> | void;
  }
  | { readonly ok: false; readonly reason: string };

async function runGpuTitleSmokeWorldBoot(
  options: StartGpuTitleWorldOptions,
  onProgress: LoadingProgressSink,
): Promise<GpuTitleWorldBootRun> {
  const bootResult = await runGeneratedWorldSmokeBoot(options.canvas, options.url, {
    rendererHost: createPinnedBrowserRendererHost(options.deviceContext, options.target),
    onProgress,
  });
  if (!bootResult.ok) {
    return bootResult;
  }

  return {
    ok: true,
    scene: bootResult.scene,
    frame: bootResult.run.frame,
    camera: bootResult.camera,
    result: bootResult.run.result,
    close: bootResult.close,
  };
}

async function runGpuTitleLiveWorldBoot(
  options: StartGpuTitleWorldOptions,
  onProgress: LoadingProgressSink,
  onChunkLifecycle?: (snapshot: GeneratedChunkLifecycleSnapshot | undefined) => void,
): Promise<GpuTitleWorldBootRun> {
  const runtimeConfig = resolveGuiWorldTransportConfig(options.debugSettingsState);
  const renderConfig = readBrowserRenderConfig(options.url, browserRenderConfigStorage());
  if (readClearWorldStorage(options.url)) {
    onProgress({ stage: "Clearing stored world", fraction: 0.01 });
    if (typeof indexedDB === "undefined") {
      return { ok: false, reason: "clearWorldStorage requested but IndexedDB is unavailable" };
    }
    await deleteIndexedDbWorldStorage(indexedDB);
  }

  const sceneResult = await initializeRendererScene(options.canvas, {
    seed: options.debugSession.seed,
    preset: options.debugSettingsState.preset,
    viewDistance: options.optionsState.viewDistance,
    renderDistance: options.optionsState.renderDistance,
    fogEnabled: options.optionsState.fogEnabled,
    engineConfig: {
      lightingMode: options.optionsState.lightingMode,
      liquidSimulationMode: options.optionsState.liquidSimulationMode,
    },
    worldTransport: runtimeConfig.worldTransport,
    remoteWorldHostUrl: runtimeConfig.remoteWorldHostUrl,
    skyColor: renderConfig.skyColor,
    clearColorScale: renderConfig.clearColorScale,
    worldStorageMode: options.debugSettingsState.worldStorageMode,
    rendererHost: createPinnedBrowserRendererHost(options.deviceContext, options.target),
    pollDebugOptions: () => options.state.mode === "loading" || options.debugSettingsState.showDebugInfo || options.debugSettingsState.showChunkBorders
      ? { chunkLifecycle: true }
      : undefined,
    onProgress,
  });
  if (!sceneResult.ok) {
    return sceneResult;
  }

  const scene = sceneResult.scene;
  const publishLoadingChunkLifecycle = (): void => {
    const snapshot = scene.clientRuntime.publishPresentationState().chunkLifecycle;
    options.state.chunkLifecycle = snapshot;
    onChunkLifecycle?.(snapshot);
  };
  try {
    const camera = readSmokeCamera(options.url) ?? DEFAULT_TITLE_WORLD_CAMERA;
    const chunkViewRequest = createChunkViewRequestForCameraState(camera, scene.viewDistance);
    if (await scene.clientRuntime.setChunkInterest(chunkViewRequest)) {
      applyRenderWorldDirtySections(scene);
      scene.levelRenderer.allChanged();
    }
    publishLoadingChunkLifecycle();

    const expectedLoadedChunkCount = getExpectedLoadedChunkCount(scene.viewDistance);
    options.state.expectedLoadedChunkCount = expectedLoadedChunkCount;
    options.state.saveId = scene.saveMetadata.saveId;
    options.state.viewDistance = scene.viewDistance;
    options.state.renderDistance = scene.gameRenderer.getRenderDistance();
    options.state.fogEnabled = options.optionsState.fogEnabled;
    options.state.lightingMode = options.optionsState.lightingMode;
    options.state.liquidSimulationMode = options.optionsState.liquidSimulationMode;
    options.state.worldStorageMode = options.debugSettingsState.worldStorageMode;
    if (!await waitForLoadedChunkRing(scene, 1, {
      maxAttempts: Math.max(240, expectedLoadedChunkCount * 4),
      onProgress: (progress) => {
        publishLoadingChunkLifecycle();
        const loadedChunkCount = getSceneLoadedChunkCount(scene);
        onProgress({
          ...progress,
          stage: "Loading initial terrain",
          fraction: 0.92 + (Math.min(loadedChunkCount, expectedLoadedChunkCount) / expectedLoadedChunkCount * 0.06),
        });
      },
    })) {
      closeRendererScene(scene);
      return {
        ok: false,
        reason: `expected at least one loaded chunk for viewDistance=${scene.viewDistance.toString()}, got ${getSceneLoadedChunkCount(scene).toString()}`,
      };
    }

    publishLoadingChunkLifecycle();
    onProgress({ stage: "Building first terrain frame", fraction: 0.98 });
    scene.levelRenderer.allChanged();
    const frame = await renderSceneUntilSettled(scene, camera, {
      maxAttempts: 20,
      stablePasses: 1,
      pollIntervalMs: 25,
    });
    const loadedChunkCount = getSceneLoadedChunkCount(scene);
    options.state.loadedChunkCount = loadedChunkCount;
    return {
      ok: true,
      scene,
      frame,
      camera,
      close: () => {
        closeRendererScene(scene);
      },
      result: {
        ok: true,
        mode: "live_world",
        worldTransport: runtimeConfig.worldTransport,
        saveId: scene.saveMetadata.saveId,
        viewDistance: scene.viewDistance,
        renderDistance: scene.gameRenderer.getRenderDistance(),
        fogEnabled: options.optionsState.fogEnabled,
        lightingMode: options.optionsState.lightingMode,
        liquidSimulationMode: options.optionsState.liquidSimulationMode,
        loadedChunkCount,
        expectedLoadedChunkCount,
      },
    };
  } catch (error) {
    closeRendererScene(scene);
    throw error;
  }
}

function createPinnedBrowserRendererHost(
  deviceContext: WebGpuDeviceContext,
  target: BrowserCanvasTarget,
): BrowserRendererHost {
  return {
    requestWebGpuDeviceContext: async (progress) => {
      progress?.onRequestAdapter?.();
      progress?.onRequestDevice?.();
      return { ok: true, context: deviceContext };
    },
    createCanvasTarget: () => target,
    createRenderWorldWorkerEndpoint: () => BROWSER_RENDERER_HOST.createRenderWorldWorkerEndpoint(),
  };
}

async function boot(): Promise<MainBootResult> {
  const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
  if (!canvas) return { ok: false, reason: "canvas #renderer not found" };

  const url = typeof window === "undefined" ? new URL("http://127.0.0.1/") : new URL(window.location.href);
  if (readGpuTitleEnabled(url)) {
    return bootGpuTitle(canvas, url);
  }

  return bootGeneratedWorldSmoke(canvas, url);
}

async function bootGeneratedWorldSmoke(
  canvas: HTMLCanvasElement,
  url: URL,
  options: GeneratedWorldSmokeBootOptions = {},
): Promise<BootResult> {
  const bootResult = await runGeneratedWorldSmokeBoot(canvas, url, options);
  return bootResult.ok ? bootResult.run.result : bootResult;
}

async function runGeneratedWorldSmokeBoot(
  canvas: HTMLCanvasElement,
  url: URL,
  options: GeneratedWorldSmokeBootOptions = {},
): Promise<GeneratedWorldSmokeBootResult> {
  const runtimeConfig = readWorldTransport(url);
  const scenario = getGeneratedWorldSmokeScenarioById(url.searchParams.get("generatedWorldScenario"));
  const renderConfig = readBrowserRenderConfig(url, browserRenderConfigStorage());
  const requestedCamera = readSmokeCamera(url);
  if (readClearWorldStorage(url)) {
    if (typeof indexedDB === "undefined") {
      return { ok: false, reason: "clearWorldStorage requested but IndexedDB is unavailable" };
    }
    try {
      await deleteIndexedDbWorldStorage(indexedDB);
    } catch (error) {
      return {
        ok: false,
        reason: error instanceof Error ? error.message : String(error),
      };
    }
  }

  return runGeneratedWorldBoot({
    scenario,
    adapter: createGeneratedWorldBrowserBootAdapter({
      canvas,
      renderConfig,
      runtimeConfig,
      readbackFormat: READBACK_FORMAT,
      rendererHost: options.rendererHost,
      guiProbe: readGuiProbeEnabled(url),
    }),
    camera: resolveGeneratedWorldBrowserCamera(scenario.steps.length, scenario.camera, requestedCamera),
    worldTransport: runtimeConfig.worldTransport,
    lightingMode: renderConfig.lightingMode,
    liquidSimulationMode: renderConfig.liquidSimulationMode,
    onProgress: options.onProgress,
  });
}

if (typeof window !== "undefined") {
  window.__mcloneReady = boot();
}
