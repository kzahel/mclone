import { deleteIndexedDbWorldStorage } from "../runtime/storage/indexeddb-world-storage";
import { Vec3 } from "../world/phys/vec3";
import { Gui0ProbeScreen } from "../client/gui/screens/gui0-probe-screen";
import { TitleScreen } from "../client/gui/screens/title-screen";
import { ScreenManager } from "../client/gui/screen-manager";
import { type CameraState } from "./game-renderer";
import {
  createSceneDepthTarget,
  encodeSceneFrame,
  initializeRendererScene,
  resizeCanvasToDisplaySize,
} from "./scene-setup";
import { readBrowserRenderConfig } from "./browser-render-config";
import {
  getGeneratedWorldSmokeScenarioById,
} from "./generated-world-smoke-scenario";
import {
  runGeneratedWorldSmokeScenario,
  type GeneratedWorldSmokeRenderTarget,
  type GeneratedWorldSmokeScenarioResult,
} from "./generated-world-smoke-runner";
import { GuiRenderer } from "./gui/gui-renderer";
import { GuiOverlayHost } from "./gui/gui-overlay-host";
import { configureBrowserCanvasTarget, readTextureRgba8, requestWebGpuDeviceContext } from "./webgpu-target";

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
  mode: "title";
  screenTitle: string;
  lastAction?: "continue" | "start_world" | "options";
  width: number;
  height: number;
}

interface GpuGuiRuntimeController {
  readonly state: GpuGuiRuntimeState;
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

async function createGuiProbeOverlay(scene: import("./scene-setup").RendererScene): Promise<GuiOverlayHost> {
  const size = GuiRenderer.calculateGuiSize(scene.canvas.width, scene.canvas.height);
  const screenManager = new ScreenManager(size.guiWidth, size.guiHeight);
  screenManager.setScreen(new Gui0ProbeScreen());
  return GuiOverlayHost.create(scene.device, scene.canvas, screenManager);
}

function encodeGuiProbeOverlay(
  overlay: GuiOverlayHost | undefined,
  encoder: GPUCommandEncoder,
  view: GPUTextureView,
  format: GPUTextureFormat,
  pixelWidth: number,
  pixelHeight: number,
): void {
  if (overlay === undefined) {
    return;
  }

  overlay.encode(encoder, view, format, pixelWidth, pixelHeight);
}

async function bootGpuTitle(canvas: HTMLCanvasElement): Promise<MainBootResult> {
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
    width: size.guiWidth,
    height: size.guiHeight,
  };
  if (typeof window !== "undefined") {
    window.__mcloneGui = { state };
  }

  let renderQueued = false;
  let host: GuiOverlayHost | undefined;
  const render = (): void => {
    if (host === undefined || renderQueued) {
      return;
    }

    renderQueued = true;
    requestAnimationFrame(() => {
      const activeHost = host;
      if (activeHost === undefined) {
        renderQueued = false;
        return;
      }

      renderQueued = false;
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
    });
  };

  const titleScreen = new TitleScreen({
    onContinue: () => {
      state.lastAction = "continue";
    },
    onStartWorld: () => {
      state.lastAction = "start_world";
    },
    onOptions: () => {
      state.lastAction = "options";
    },
  });
  screenManager.setScreen(titleScreen);
  host = await GuiOverlayHost.create(device, canvas, screenManager);
  host.attachInput(render);
  window.addEventListener("resize", render);
  state.ready = true;
  render();

  return {
    ok: true,
    mode: "gpu_title",
    width: size.guiWidth,
    height: size.guiHeight,
    screenTitle: "Title Screen",
  };
}

async function boot(): Promise<MainBootResult> {
  const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
  if (!canvas) return { ok: false, reason: "canvas #renderer not found" };

  const runtimeConfig = readWorldTransport();
  const url = typeof window === "undefined" ? new URL("http://127.0.0.1/") : new URL(window.location.href);
  if (readGpuTitleEnabled(url)) {
    return bootGpuTitle(canvas);
  }

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
  });
  if (!sceneResult.ok) {
    return sceneResult;
  }
  const scene = sceneResult.scene;
  const guiProbeOverlay = readGuiProbeEnabled(url) ? await createGuiProbeOverlay(scene) : undefined;
  const camera = scenario.steps.length === 1 ? requestedCamera ?? scenario.camera : undefined;
  try {
    const run = await runGeneratedWorldSmokeScenario({
      scene,
      scenario,
      camera,
      target: createBrowserSmokeRenderTarget(canvas, guiProbeOverlay),
      worldTransport: runtimeConfig.worldTransport,
      format: scene.format,
      lightingMode: renderConfig.lightingMode,
      liquidSimulationMode: renderConfig.liquidSimulationMode,
    });
    return run.result;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return { ok: false, reason };
  }
}

function createBrowserSmokeRenderTarget(canvas: HTMLCanvasElement, guiProbeOverlay?: GuiOverlayHost): GeneratedWorldSmokeRenderTarget {
  return {
    get width() {
      return canvas.width;
    },
    get height() {
      return canvas.height;
    },
    format: READBACK_FORMAT,
    renderFrame: async ({ scene, frame }) => {
      const canvasDepthTarget = createSceneDepthTarget(scene.device, canvas.width, canvas.height);
      const readbackDepthTarget = createSceneDepthTarget(scene.device, canvas.width, canvas.height);
      const canvasTexture = scene.ctx.getCurrentTexture();
      const canvasWorldView = canvasTexture.createView();
      const canvasGuiView = canvasTexture.createView();
      const readbackTexture = scene.device.createTexture({
        size: { width: canvas.width, height: canvas.height },
        format: READBACK_FORMAT,
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      });
      try {
        const encoder = scene.device.createCommandEncoder();
        encodeSceneFrame(
          scene,
          frame,
          {
            view: canvasWorldView,
            depthView: canvasDepthTarget.view,
            format: scene.format,
          },
          encoder,
        );
        encodeGuiProbeOverlay(guiProbeOverlay, encoder, canvasGuiView, scene.format, canvas.width, canvas.height);
        const readbackWorldView = readbackTexture.createView();
        const readbackGuiView = readbackTexture.createView();
        encodeSceneFrame(
          scene,
          frame,
          {
            view: readbackWorldView,
            depthView: readbackDepthTarget.view,
            format: READBACK_FORMAT,
          },
          encoder,
        );
        encodeGuiProbeOverlay(guiProbeOverlay, encoder, readbackGuiView, READBACK_FORMAT, canvas.width, canvas.height);
        scene.device.queue.submit([encoder.finish()]);
        await scene.device.queue.onSubmittedWorkDone();
        return {
          width: canvas.width,
          height: canvas.height,
          format: READBACK_FORMAT,
          pixels: await readTextureRgba8(scene.device, readbackTexture, canvas.width, canvas.height),
        };
      } finally {
        canvasDepthTarget.texture.destroy();
        readbackDepthTarget.texture.destroy();
        readbackTexture.destroy();
      }
    },
  };
}

if (typeof window !== "undefined") {
  window.__mcloneReady = boot();
}
