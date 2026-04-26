import { deleteIndexedDbWorldStorage } from "../runtime/storage/indexeddb-world-storage";
import { Vec3 } from "../world/phys/vec3";
import { Gui0ProbeScreen } from "../client/gui/screens/gui0-probe-screen";
import { ProgressScreen } from "../client/gui/screens/progress-screen";
import { TitleScreen } from "../client/gui/screens/title-screen";
import { ScreenManager } from "../client/gui/screen-manager";
import { type CameraState } from "./game-renderer";
import {
  initializeRendererScene,
  resizeCanvasToDisplaySize,
  type RendererScene,
} from "./scene-setup";
import { readBrowserRenderConfig } from "./browser-render-config";
import {
  getGeneratedWorldSmokeScenarioById,
} from "./generated-world-smoke-scenario";
import { createGeneratedWorldBrowserPresentationHost } from "./generated-world-browser-presentation-host";
import {
  runGeneratedWorldSmokeScenario,
  type GeneratedWorldSmokeRun,
  type GeneratedWorldSmokeScenarioResult,
} from "./generated-world-smoke-runner";
import { progressFraction, type LoadingProgress, type LoadingProgressSink } from "./loading-progress";
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

type BootWorldTransport = "worker" | "remote";

export type BootResult =
  | GeneratedWorldSmokeScenarioResult
  | { ok: false; reason: string };

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
  mode: "title" | "loading" | "world" | "error";
  screenTitle: string;
  lastAction?: "continue" | "start_world" | "options";
  loadingStage?: string;
  loadingDetail?: string;
  loadingProgress?: number;
  worldReady?: boolean;
  worldResult?: BootResult;
  frameCount: number;
  inputEventCount: number;
  cameraPosition?: readonly [number, number, number];
  cameraYaw?: number;
  cameraPitch?: number;
  loadedChunkCount?: number;
  error?: string;
  width: number;
  height: number;
}

interface GpuGuiRuntimeController {
  readonly state: GpuGuiRuntimeState;
  worldReady?: Promise<BootResult>;
  worldRuntime?: GpuWorldRuntime;
}

declare global {
  interface Window {
    __mcloneReady: Promise<MainBootResult>;
    __mcloneGui?: GpuGuiRuntimeController;
  }
}

const READBACK_FORMAT: GPUTextureFormat = "rgba8unorm";

function readWorldTransport(): {
  readonly worldTransport: BootWorldTransport;
  readonly remoteWorldHostUrl?: string;
} {
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
  return value === "1" || value === "true";
}

interface GeneratedWorldSmokeBootOptions {
  readonly rendererHost?: BrowserRendererHost;
  readonly onProgress?: LoadingProgressSink;
}

type GeneratedWorldSmokeBootResult =
  | {
    readonly ok: true;
    readonly scene: RendererScene;
    readonly run: GeneratedWorldSmokeRun;
    readonly camera: CameraState;
  }
  | { readonly ok: false; readonly reason: string };

async function createGuiProbeOverlay(scene: RendererScene): Promise<GuiOverlayHost> {
  const size = GuiRenderer.calculateGuiSize(scene.canvas.width, scene.canvas.height);
  const screenManager = new ScreenManager(size.guiWidth, size.guiHeight);
  screenManager.setScreen(new Gui0ProbeScreen());
  return GuiOverlayHost.create(scene.device, scene.canvas, screenManager);
}

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
  const state: GpuGuiRuntimeState = {
    ready: false,
    mode: "title",
    screenTitle: "Title Screen",
    frameCount: 0,
    inputEventCount: 0,
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
    if (activeHost === undefined || screenManager.currentScreen === null || state.mode === "world") {
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

  let startWorldPromise: Promise<BootResult> | undefined;
  const startWorld = (): void => {
    state.lastAction = "start_world";
    if (startWorldPromise !== undefined) {
      return;
    }

    startWorldPromise = startGpuTitleWorld({
      canvas,
      url,
      deviceContext: deviceContextResult.context,
      target,
      controller,
      screenManager,
      state,
      renderGuiFrameNow,
    });
    controller.worldReady = startWorldPromise;
  };

  const titleScreen = new TitleScreen({
    onContinue: () => {
      state.lastAction = "continue";
    },
    onStartWorld: startWorld,
    onOptions: () => {
      state.lastAction = "options";
    },
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
  readonly controller: GpuGuiRuntimeController;
  readonly screenManager: ScreenManager;
  readonly state: GpuGuiRuntimeState;
  readonly renderGuiFrameNow: () => void;
}

async function startGpuTitleWorld(options: StartGpuTitleWorldOptions): Promise<BootResult> {
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
    const bootResult = await runGeneratedWorldSmokeBoot(options.canvas, options.url, {
      rendererHost: createPinnedBrowserRendererHost(options.deviceContext, options.target),
      onProgress: reportProgress,
    });
    if (!bootResult.ok) {
      options.state.mode = "error";
      options.state.error = bootResult.reason;
      reportProgress({ stage: "Error", detail: bootResult.reason, fraction: 1 });
      return bootResult;
    }

    options.state.mode = "world";
    options.state.screenTitle = "";
    options.state.worldResult = bootResult.run.result;
    options.screenManager.setScreen(null);
    options.state.frameCount = 0;
    options.state.inputEventCount = 0;
    options.controller.worldRuntime = await startGpuWorldRuntime({
      scene: bootResult.scene,
      canvas: options.canvas,
      initialCamera: bootResult.camera,
      initialFrame: bootResult.run.frame,
      state: options.state,
      onError: (message) => {
        options.state.mode = "error";
        options.state.error = message;
      },
    });
    options.state.worldReady = true;
    return bootResult.run.result;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    options.state.mode = "error";
    options.state.error = reason;
    reportProgress({ stage: "Error", detail: reason, fraction: 1 });
    return { ok: false, reason };
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
  const runtimeConfig = readWorldTransport();
  const scenario = getGeneratedWorldSmokeScenarioById(url.searchParams.get("generatedWorldScenario"));
  const renderConfig = readBrowserRenderConfig(url);
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

  const sceneResult = await initializeRendererScene(canvas, {
    seed: scenario.seed,
    viewDistance: renderConfig.viewDistance,
    renderDistance: renderConfig.renderDistance,
    worldTransport: runtimeConfig.worldTransport,
    remoteWorldHostUrl: runtimeConfig.remoteWorldHostUrl,
    preset: scenario.preset,
    engineConfig: {
      lightingMode: renderConfig.lightingMode,
      liquidSimulationMode: renderConfig.liquidSimulationMode,
    },
    worldStorageMode: renderConfig.worldStorageMode,
    skyColor: renderConfig.skyColor,
    clearColorScale: renderConfig.clearColorScale,
    onProgress: options.onProgress,
    rendererHost: options.rendererHost,
  });
  if (!sceneResult.ok) {
    return sceneResult;
  }
  const scene = sceneResult.scene;
  const guiProbeOverlay = readGuiProbeEnabled(url) ? await createGuiProbeOverlay(scene) : undefined;
  const camera = scenario.steps.length === 1 ? requestedCamera ?? scenario.camera : undefined;
  const runtimeCamera = camera ?? scenario.steps[scenario.steps.length - 1]?.camera ?? scenario.camera;
  try {
    const run = await runGeneratedWorldSmokeScenario({
      scene,
      scenario,
      camera,
      presentationHost: createGeneratedWorldBrowserPresentationHost({
        canvas,
        guiProbeOverlay,
        readbackFormat: READBACK_FORMAT,
      }),
      worldTransport: runtimeConfig.worldTransport,
      format: scene.format,
      lightingMode: renderConfig.lightingMode,
      liquidSimulationMode: renderConfig.liquidSimulationMode,
      onProgress: options.onProgress,
    });
    return { ok: true, scene, run, camera: runtimeCamera };
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return { ok: false, reason };
  }
}

if (typeof window !== "undefined") {
  window.__mcloneReady = boot();
}
