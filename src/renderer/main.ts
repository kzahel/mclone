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
  getSceneLoadedChunkCount,
  initializeRendererScene,
  renderSceneUntilSettled,
  resizeCanvasToDisplaySize,
  waitForLoadedChunkRing,
  type RendererScene,
} from "./scene-setup";
import { getExpectedLoadedChunkCount, readBrowserRenderConfig, writeStoredBrowserRenderConfig } from "./browser-render-config";
import {
  getGeneratedWorldSmokeScenarioById,
} from "./generated-world-smoke-scenario";
import {
  createGeneratedWorldBrowserBootAdapter,
  resolveGeneratedWorldBrowserCamera,
  type GeneratedWorldBrowserWorldTransportConfig,
} from "./generated-world-browser-boot";
import {
  runGeneratedWorldBoot,
  type GeneratedWorldBootResult,
} from "./generated-world-boot";
import type { LevelRenderFrame } from "./level-renderer";
import type { GeneratedWorldSmokeScenarioResult } from "./generated-world-smoke-runner";
import { progressFraction, type LoadingProgress, type LoadingProgressSink } from "./loading-progress";
import {
  readDebugSessionConfig,
  writeStartLastWorld,
  writeStoredDebugSessionConfig,
  type DebugSessionConfig,
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
import { createChunkViewRequestForCameraState } from "./debug/debug-player-controls";

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
  lastAction?: "continue" | "start_world" | "options" | "options_done" | "debug_settings" | "debug_settings_done" | "back_to_game";
  loadingStage?: string;
  loadingDetail?: string;
  loadingProgress?: number;
  worldReady?: boolean;
  worldResult?: GpuTitleWorldResult;
  pauseScreenActive?: boolean;
  frameCount: number;
  inputEventCount: number;
  cameraPosition?: readonly [number, number, number];
  cameraYaw?: number;
  cameraPitch?: number;
  loadedChunkCount?: number;
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
}

declare global {
  interface Window {
    __mcloneReady: Promise<MainBootResult>;
    __mcloneGui?: GpuGuiRuntimeController;
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
  readonly lightingMode: string;
  readonly liquidSimulationMode: string;
  readonly loadedChunkCount: number;
  readonly expectedLoadedChunkCount: number;
}

function readWorldTransport(): GeneratedWorldBrowserWorldTransportConfig {
  if (typeof window === "undefined") {
    return { worldTransport: "worker" };
  }

  const url = new URL(window.location.href);
  const worldTransport = url.searchParams.get("worldTransport");
  if (worldTransport === "remote") {
    return {
      worldTransport: "remote",
      remoteWorldHostUrl: url.searchParams.get("worldHostUrl") ?? undefined,
    };
  }

  return { worldTransport: "worker" };
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
    lightingMode: config.lightingMode,
    liquidSimulationMode: config.liquidSimulationMode,
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
  return {
    debugSession,
    debugSettings: {
      movementMode: debugSession.movementMode,
      preset: debugSession.preset,
      worldStorageMode: renderConfig.worldStorageMode,
    },
  };
}

function copyGuiOptionsState(options: GuiOptionsState): GuiOptionsState {
  return {
    viewDistance: options.viewDistance,
    renderDistance: options.renderDistance,
    lightingMode: options.lightingMode,
    liquidSimulationMode: options.liquidSimulationMode,
  };
}

function copyGuiDebugSettingsState(debugSettings: GuiDebugSettingsState): GuiDebugSettingsState {
  return {
    movementMode: debugSettings.movementMode,
    preset: debugSettings.preset,
    worldStorageMode: debugSettings.worldStorageMode,
  };
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
  const deviceContextResult = await requestWebGpuDeviceContext();
  if (!deviceContextResult.ok) {
    return deviceContextResult;
  }

  const { device, format } = deviceContextResult.context;
  resizeCanvasToDisplaySize(canvas, device.limits.maxTextureDimension2D);
  const target = configureBrowserCanvasTarget(canvas, device, format);
  if (target === undefined) {
    return { ok: false, reason: "canvas.getContext('webgpu') returned null" };
  }

  const size = GuiRenderer.calculateGuiSize(canvas.width, canvas.height);
  const screenManager = new ScreenManager(size.guiWidth, size.guiHeight);
  const optionsState = readGuiOptionsState(url);
  const debugSettingsState = readGuiDebugSettingsState(url);
  const state: GpuGuiRuntimeState = {
    ready: false,
    mode: "title",
    screenTitle: "Title Screen",
    frameCount: 0,
    inputEventCount: 0,
    options: copyGuiOptionsState(optionsState),
    debugSettings: copyGuiDebugSettingsState(debugSettingsState.debugSettings),
    width: size.guiWidth,
    height: size.guiHeight,
  };
  const controller: GpuGuiRuntimeController = { state };
  if (typeof window !== "undefined") {
    window.__mcloneGui = controller;
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
    };
    persistGuiSettingsState(optionsState, debugSettingsState.debugSession, debugSettingsState.debugSettings);
    state.options = copyGuiOptionsState(optionsState);
    state.debugSettings = copyGuiDebugSettingsState(debugSettingsState.debugSettings);
  };
  const startWorld = (): void => {
    state.lastAction = "start_world";
    const guiOverlayHost = host;
    if (guiOverlayHost === undefined) {
      state.mode = "error";
      state.error = "GUI overlay host is not initialized";
      return;
    }
    if (startWorldPromise !== undefined) {
      return;
    }

    startWorldPromise = startGpuTitleWorld({
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
      renderGuiFrameNow,
    });
    controller.worldReady = startWorldPromise;
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
  titleScreen = new TitleScreen({
    onContinue: () => {
      state.lastAction = "continue";
    },
    onStartWorld: startWorld,
    onOptions: openOptions,
    onDebugSettings: openDebugSettings,
  });
  screenManager.setScreen(titleScreen);
  host = await GuiOverlayHost.create(device, canvas, screenManager);
  host.attachInput(scheduleGuiFrame);
  window.addEventListener("resize", scheduleGuiFrame);
  state.ready = true;
  scheduleGuiFrame();

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
  readonly renderGuiFrameNow: () => void;
}

async function startGpuTitleWorld(options: StartGpuTitleWorldOptions): Promise<GpuTitleWorldResult> {
  const progressScreen = new ProgressScreen(true);
  options.state.mode = "loading";
  options.state.screenTitle = "Progress Screen";
  options.screenManager.setScreen(progressScreen);

  const reportProgress = (progress: LoadingProgress): void => {
    progressScreen.updateProgress(progress);
    options.state.loadingStage = progress.stage;
    options.state.loadingDetail = progress.detail;
    options.state.loadingProgress = progressFraction(progress);
    options.renderGuiFrameNow();
  };

  reportProgress({ stage: "Opening world", fraction: 0 });
  try {
    const bootResult = shouldUseSmokeTitleWorldStart(options.url)
      ? await runGpuTitleSmokeWorldBoot(options, reportProgress)
      : await runGpuTitleLiveWorldBoot(options, reportProgress);
    if (!bootResult.ok) {
      options.state.mode = "error";
      options.state.error = bootResult.reason;
      reportProgress({ stage: "Error", detail: bootResult.reason, fraction: 1 });
      return bootResult;
    }

    const scene = bootResult.scene;
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
      onError: (message) => {
        options.state.mode = "error";
        options.state.error = message;
      },
    });
    options.state.worldReady = true;
    return bootResult.result;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
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
  };
}

async function runGpuTitleLiveWorldBoot(
  options: StartGpuTitleWorldOptions,
  onProgress: LoadingProgressSink,
): Promise<GpuTitleWorldBootRun> {
  const runtimeConfig = readWorldTransport();
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
    onProgress,
  });
  if (!sceneResult.ok) {
    return sceneResult;
  }

  const scene = sceneResult.scene;
  const camera = readSmokeCamera(options.url) ?? DEFAULT_TITLE_WORLD_CAMERA;
  const chunkViewRequest = createChunkViewRequestForCameraState(camera, scene.viewDistance);
  if (await scene.clientRuntime.setChunkInterest(chunkViewRequest)) {
    applyRenderWorldDirtySections(scene);
    scene.levelRenderer.allChanged();
  }

  const expectedLoadedChunkCount = getExpectedLoadedChunkCount(scene.viewDistance);
  if (!await waitForLoadedChunkRing(scene, expectedLoadedChunkCount, {
    maxAttempts: Math.max(2400, expectedLoadedChunkCount * 32),
    onProgress: (progress) => onProgress({
      ...progress,
      fraction: 0.92 + ((progressFraction(progress) ?? 0) * 0.06),
    }),
  })) {
    return {
      ok: false,
      reason: `expected ${expectedLoadedChunkCount.toString()} loaded chunks for viewDistance=${scene.viewDistance.toString()}, got ${getSceneLoadedChunkCount(scene).toString()}`,
    };
  }

  onProgress({ stage: "Building first frame", fraction: 0.98 });
  scene.levelRenderer.allChanged();
  const frame = await renderSceneUntilSettled(scene, camera);
  const loadedChunkCount = getSceneLoadedChunkCount(scene);
  return {
    ok: true,
    scene,
    frame,
    camera,
    result: {
      ok: true,
      mode: "live_world",
      worldTransport: runtimeConfig.worldTransport,
      saveId: scene.saveMetadata.saveId,
      viewDistance: scene.viewDistance,
      renderDistance: scene.gameRenderer.getRenderDistance(),
      lightingMode: options.optionsState.lightingMode,
      liquidSimulationMode: options.optionsState.liquidSimulationMode,
      loadedChunkCount,
      expectedLoadedChunkCount,
    },
  };
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
  const runtimeConfig = readWorldTransport();
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
