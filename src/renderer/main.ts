import { SectionPos } from "../core/section-pos";
import { Vec3 } from "../world/phys/vec3";
import { type LevelRenderFrame } from "./level-renderer";
import { type CameraState } from "./game-renderer";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { RenderType } from "./render-type";
import {
  createSceneDepthView,
  encodeSceneFrame,
  initializeRendererScene,
  SCENE_DEPTH_FORMAT,
} from "./scene-setup";

const GENERATED_SEED = 12_345n;
const GENERATED_VIEW_DISTANCE = 1;
const GENERATED_CAMERA_PATH = [
  {
    position: new Vec3(8.5, 104.0, 40.5),
    xRot: 60.0,
    yRot: 180.0,
  },
  {
    position: new Vec3(40.5, 104.0, 40.5),
    xRot: 60.0,
    yRot: 180.0,
  },
] as const satisfies readonly CameraState[];

export type BootResult =
  | {
      ok: true;
      format: GPUTextureFormat;
      adapterInfo: string;
      centerPixel: readonly [number, number, number, number];
      terrainPixel: readonly [number, number, number, number];
      clearPixel: readonly [number, number, number, number];
      loadedChunkCount: number;
      solidDrawCount: number;
      cutoutDrawCount: number;
      translucentDrawCount: number;
    }
  | { ok: false; reason: string };

declare global {
  interface Window {
    __mcloneReady: Promise<BootResult>;
  }
}

const READBACK_FORMAT: GPUTextureFormat = "rgba8unorm";
const VALIDATED_RENDER_TYPES = [
  RenderType.solid(),
  RenderType.cutoutMipped(),
  RenderType.cutout(),
  RenderType.translucent(),
  RenderType.translucentMovingBlock(),
  RenderType.translucentNoCrumbling(),
  RenderType.tripwire(),
  RenderType.lines(),
  RenderType.lineStrip(),
] as const;

function rgba8FromColor(color: readonly [number, number, number, number]): readonly [number, number, number, number] {
  return [
    Math.round(color[0] * 255),
    Math.round(color[1] * 255),
    Math.round(color[2] * 255),
    Math.round(color[3] * 255),
  ];
}

function validateShaderPipelines(
  pipelineCache: RenderPipelineCache,
  colorFormat: GPUTextureFormat,
  depthFormat: GPUTextureFormat,
): void {
  for (const renderType of VALIDATED_RENDER_TYPES) {
    pipelineCache.getOrCreate(renderType, colorFormat, depthFormat);
  }
}

async function popValidationError(device: GPUDevice, label: string): Promise<string | undefined> {
  const error = await device.popErrorScope();
  return error ? `${label}: ${error.message}` : undefined;
}

async function readPixel(
  device: GPUDevice,
  texture: GPUTexture,
  x: number,
  y: number,
): Promise<readonly [number, number, number, number]> {
  const readback = device.createBuffer({
    size: 256,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });
  const encoder = device.createCommandEncoder();
  encoder.copyTextureToBuffer(
    {
      texture,
      origin: { x, y },
    },
    {
      buffer: readback,
      bytesPerRow: 256,
      rowsPerImage: 1,
    },
    { width: 1, height: 1 },
  );
  device.queue.submit([encoder.finish()]);
  await device.queue.onSubmittedWorkDone();
  await readback.mapAsync(GPUMapMode.READ);
  const bytes = new Uint8Array(readback.getMappedRange()).slice(0, 4);
  readback.unmap();
  readback.destroy();
  return [bytes[0]!, bytes[1]!, bytes[2]!, bytes[3]!];
}

async function boot(): Promise<BootResult> {
  const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
  if (!canvas) return { ok: false, reason: "canvas #renderer not found" };

  const sceneResult = await initializeRendererScene(canvas, {
    seed: GENERATED_SEED,
    viewDistance: GENERATED_VIEW_DISTANCE,
  });
  if (!sceneResult.ok) {
    return sceneResult;
  }
  const scene = sceneResult.scene;

  let frame: LevelRenderFrame | undefined;
  for (const step of GENERATED_CAMERA_PATH) {
    if (await scene.worldClient.setChunkView({
      type: "set_chunk_view",
      centerChunkX: SectionPos.posToSectionCoord(step.position.x),
      centerChunkZ: SectionPos.posToSectionCoord(step.position.z),
      radius: GENERATED_VIEW_DISTANCE,
    })) {
      scene.levelRenderer.allChanged();
    }

    frame = await scene.gameRenderer.renderLevel(
      0.0,
      Number.MAX_SAFE_INTEGER,
      scene.levelRenderer,
      scene.lightTexture,
      step,
    );
  }

  if (frame === undefined) {
    return { ok: false, reason: "no generated-terrain frame was produced" };
  }

  const solidDraws = frame.layerDraws.get(RenderType.solid()) ?? [];
  const cutoutDraws = frame.layerDraws.get(RenderType.cutout()) ?? [];
  const translucentDraws = frame.layerDraws.get(RenderType.translucent()) ?? [];
  if (solidDraws.length === 0) {
    return { ok: false, reason: "camera-driven level renderer produced no solid drawables for the generated terrain scene" };
  }

  scene.device.pushErrorScope("validation");
  validateShaderPipelines(scene.pipelineCache, scene.format, SCENE_DEPTH_FORMAT);
  const pipelineError = await popValidationError(scene.device, "WebGPU pipeline validation failed");
  if (pipelineError) {
    return { ok: false, reason: pipelineError };
  }

  const canvasDepthView = createSceneDepthView(scene.device, canvas.width, canvas.height);
  const readbackDepthView = createSceneDepthView(scene.device, canvas.width, canvas.height);
  const readbackTexture = scene.device.createTexture({
    size: { width: canvas.width, height: canvas.height },
    format: READBACK_FORMAT,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });

  scene.device.pushErrorScope("validation");
  const encoder = scene.device.createCommandEncoder();
  encodeSceneFrame(
    scene,
    frame,
    {
      view: scene.ctx.getCurrentTexture().createView(),
      depthView: canvasDepthView,
      format: scene.format,
    },
    encoder,
  );
  encodeSceneFrame(
    scene,
    frame,
    {
      view: readbackTexture.createView(),
      depthView: readbackDepthView,
      format: READBACK_FORMAT,
    },
    encoder,
  );
  scene.device.queue.submit([encoder.finish()]);
  await scene.device.queue.onSubmittedWorkDone();
  const submissionError = await popValidationError(scene.device, "WebGPU submission validation failed");
  if (submissionError) {
    return { ok: false, reason: submissionError };
  }

  const centerPixel = await readPixel(scene.device, readbackTexture, Math.floor(canvas.width / 2), Math.floor(canvas.height / 2));
  const terrainPixel = await readPixel(scene.device, readbackTexture, Math.floor(canvas.width / 2), Math.floor((canvas.height * 3) / 4));
  const clearPixel = rgba8FromColor(frame.fogColor);

  const info = scene.adapter.info ?? {};
  const adapterInfo = [info.vendor, info.architecture, info.device, info.description]
    .filter(Boolean)
    .join(" / ") || "unknown";

  return {
    ok: true,
    format: scene.format,
    adapterInfo,
    centerPixel,
    terrainPixel,
    clearPixel,
    loadedChunkCount: scene.level.getLoadedChunkCount(),
    solidDrawCount: solidDraws.length,
    cutoutDrawCount: cutoutDraws.length,
    translucentDrawCount: translucentDraws.length,
  };
}

if (typeof window !== "undefined") {
  window.__mcloneReady = boot();
}
