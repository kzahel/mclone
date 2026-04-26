import { deleteIndexedDbWorldStorage } from "../runtime/storage/indexeddb-world-storage";
import { Vec3 } from "../world/phys/vec3";
import { type CameraState } from "./game-renderer";
import {
  createSceneDepthTarget,
  encodeSceneFrame,
  initializeRendererScene,
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
import { readTextureRgba8 } from "./webgpu-target";

type BootWorldTransport = "worker" | "remote";

export type BootResult =
  | GeneratedWorldSmokeScenarioResult
  | { ok: false; reason: string };

declare global {
  interface Window {
    __mcloneReady: Promise<BootResult>;
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

async function boot(): Promise<BootResult> {
  const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
  if (!canvas) return { ok: false, reason: "canvas #renderer not found" };

  const runtimeConfig = readWorldTransport();
  const url = typeof window === "undefined" ? new URL("http://127.0.0.1/") : new URL(window.location.href);
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
  const camera = scenario.steps.length === 1 ? requestedCamera ?? scenario.camera : undefined;
  try {
    const run = await runGeneratedWorldSmokeScenario({
      scene,
      scenario,
      camera,
      target: createBrowserSmokeRenderTarget(canvas),
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

function createBrowserSmokeRenderTarget(canvas: HTMLCanvasElement): GeneratedWorldSmokeRenderTarget {
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
            view: scene.ctx.getCurrentTexture().createView(),
            depthView: canvasDepthTarget.view,
            format: scene.format,
          },
          encoder,
        );
        encodeSceneFrame(
          scene,
          frame,
          {
            view: readbackTexture.createView(),
            depthView: readbackDepthTarget.view,
            format: READBACK_FORMAT,
          },
          encoder,
        );
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
