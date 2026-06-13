import { Gui0ProbeScreen } from "../client/gui/screens/gui0-probe-screen";
import { ScreenManager } from "../client/gui/screen-manager";
import type { CameraState } from "./game-renderer";
import {
  type BrowserRenderConfig,
} from "./browser-render-config";
import {
  type GeneratedWorldBootAdapter,
  type GeneratedWorldBootSceneContext,
  type GeneratedWorldBootSceneResult,
} from "./generated-world-boot";
import { createGeneratedWorldBrowserPresentationHost } from "./generated-world-browser-presentation-host";
import { GuiOverlayHost } from "./gui/gui-overlay-host";
import { GuiRenderer } from "./gui/gui-renderer";
import type { BrowserRendererHost } from "./renderer-host";
import type { BrowserWorldTransportConfig } from "./browser-world-transport-config";
import {
  closeRendererScene,
  initializeRendererScene,
  type RendererScene,
} from "./scene-setup";
import {
  createGeneratedWorldRuntimeTopology,
  resolveBrowserGeneratedWorldStorageTopology,
  resolveGeneratedWorldLightingTopology,
  worldTransportToRuntimeWorldHost,
} from "./generated-world-runtime-topology";

export type GeneratedWorldBrowserWorldTransportConfig = BrowserWorldTransportConfig;

export interface GeneratedWorldBrowserBootAdapterOptions {
  readonly canvas: HTMLCanvasElement;
  readonly renderConfig: BrowserRenderConfig;
  readonly runtimeConfig: GeneratedWorldBrowserWorldTransportConfig;
  readonly readbackFormat: GPUTextureFormat;
  readonly rendererHost?: BrowserRendererHost;
  readonly guiProbe?: boolean;
}

export function createGeneratedWorldBrowserBootAdapter(
  options: GeneratedWorldBrowserBootAdapterOptions,
): GeneratedWorldBootAdapter {
  return {
    describeTopology: () => {
      const worldHost = worldTransportToRuntimeWorldHost(options.runtimeConfig.worldTransport);
      return createGeneratedWorldRuntimeTopology({
        host: "browser",
        worldHost,
        lighting: resolveGeneratedWorldLightingTopology(options.renderConfig.lightingMode, worldHost),
        liquidSimulation: options.renderConfig.liquidSimulationMode,
        storage: resolveBrowserGeneratedWorldStorageTopology(worldHost, options.renderConfig.worldStorageMode),
        assetSource: "browser-asset-pack",
        renderTarget: "canvas",
      });
    },
    createScene: (context) => createGeneratedWorldBrowserScene(options, context),
    createPresentationHost: async ({ scene }) => createGeneratedWorldBrowserPresentationHost({
      canvas: options.canvas,
      readbackFormat: options.readbackFormat,
      guiProbeOverlay: options.guiProbe === true ? await createGuiProbeOverlay(scene) : undefined,
    }),
  };
}

export function resolveGeneratedWorldBrowserCamera(
  scenarioStepCount: number,
  scenarioCamera: CameraState,
  requestedCamera?: CameraState,
): CameraState | undefined {
  return scenarioStepCount === 1 ? requestedCamera ?? scenarioCamera : undefined;
}

async function createGeneratedWorldBrowserScene(
  options: GeneratedWorldBrowserBootAdapterOptions,
  context: GeneratedWorldBootSceneContext,
): Promise<GeneratedWorldBootSceneResult> {
  const sceneResult = await initializeRendererScene(options.canvas, {
    seed: context.scenario.seed,
    viewDistance: options.renderConfig.viewDistance,
    renderDistance: options.renderConfig.renderDistance,
    fogEnabled: options.renderConfig.fogEnabled,
    worldTransport: options.runtimeConfig.worldTransport,
    remoteWorldHostUrl: options.runtimeConfig.remoteWorldHostUrl,
    preset: context.scenario.preset,
    engineConfig: {
      lightingMode: options.renderConfig.lightingMode,
      liquidSimulationMode: options.renderConfig.liquidSimulationMode,
    },
    worldStorageMode: options.renderConfig.worldStorageMode,
    skyColor: options.renderConfig.skyColor,
    clearColorScale: options.renderConfig.clearColorScale,
    onProgress: context.onProgress,
    rendererHost: options.rendererHost,
  });
  if (!sceneResult.ok) {
    return sceneResult;
  }

  return {
    ok: true,
    scene: sceneResult.scene,
    close: () => {
      closeRendererScene(sceneResult.scene);
    },
  };
}

async function createGuiProbeOverlay(scene: RendererScene): Promise<GuiOverlayHost> {
  const size = GuiRenderer.calculateGuiSize(scene.canvas.width, scene.canvas.height);
  const screenManager = new ScreenManager(size.guiWidth, size.guiHeight);
  screenManager.setScreen(new Gui0ProbeScreen());
  return GuiOverlayHost.create(scene.device, scene.canvas, screenManager);
}
