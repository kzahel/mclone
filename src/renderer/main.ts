import { SectionPos } from "../core/section-pos";
import { Vec3 } from "../world/phys/vec3";
import { type LevelRenderFrame } from "./level-renderer";
import { type CameraState } from "./game-renderer";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { RenderType } from "./render-type";
import {
  applyRenderWorldDirtySections,
  createSceneDepthView,
  encodeSceneFrame,
  getSceneLoadedChunkCount,
  getSceneRenderQueueStats,
  getSceneRenderWorldPerformanceCounters,
  initializeRendererScene,
  renderSceneUntilSettled,
  SCENE_DEPTH_FORMAT,
  waitForLoadedChunkRing,
  type RenderSceneQueueStats,
  type RenderWorldPerformanceCounters,
  type RendererScene,
} from "./scene-setup";
import { getExpectedLoadedChunkCount, readBrowserRenderConfig } from "./browser-render-config";

const GENERATED_SEED = 12_345n;
const DEFAULT_SMOKE_CAMERA = {
  position: new Vec3(960.5, 132.0, -8127.5),
  xRot: 60.0,
  yRot: 225.0,
} as const satisfies CameraState;

type BootWorldTransport = "worker" | "remote";

export type BootResult =
  | {
      ok: true;
      worldTransport: BootWorldTransport;
      meshTransport: "worker";
      saveId: string;
      sessionId?: string;
      playerId?: string;
      sessionRevision?: number;
      playerInputSequence?: number;
      playerStateRevision?: number;
      playerTick?: number;
      playerPosition?: readonly [number, number, number];
      format: GPUTextureFormat;
      adapterInfo: string;
      centerPixel: readonly [number, number, number, number];
      terrainPixel: readonly [number, number, number, number];
      clearPixel: readonly [number, number, number, number];
      loadedChunkCount: number;
      expectedLoadedChunkCount: number;
      viewDistance: number;
      renderDistance: number;
      solidDrawCount: number;
      cutoutDrawCount: number;
      translucentDrawCount: number;
      renderWorldCounters: RenderWorldPerformanceCounters;
      renderQueueStats: RenderSceneQueueStats;
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

function rgba8FromColor(color: readonly [number, number, number, number]): readonly [number, number, number, number] {
  return [
    Math.round(color[0] * 255),
    Math.round(color[1] * 255),
    Math.round(color[2] * 255),
    Math.round(color[3] * 255),
  ];
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
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

async function renderSmokeCamera(scene: RendererScene, camera: CameraState, expectedLoadedChunkCount: number): Promise<LevelRenderFrame> {
  if (await scene.worldClient.setChunkView({
    type: "set_chunk_view",
    centerChunkX: SectionPos.posToSectionCoord(camera.position.x),
    centerChunkZ: SectionPos.posToSectionCoord(camera.position.z),
    radius: scene.viewDistance,
  })) {
    applyRenderWorldDirtySections(scene);
    scene.levelRenderer.allChanged();
  }

  if (!await waitForLoadedChunkRing(scene, expectedLoadedChunkCount)) {
    throw new Error(
      `expected ${expectedLoadedChunkCount.toString()} loaded chunks for viewDistance=${scene.viewDistance.toString()}, got ${getSceneLoadedChunkCount(scene).toString()}`,
    );
  }

  // Mesh requests issued before the full chunk ring arrives can legitimately
  // return "not ready"; force one visibility pass after the ring is present.
  scene.levelRenderer.allChanged();
  return renderSceneUntilSettled(scene, camera);
}

async function boot(): Promise<BootResult> {
  const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
  if (!canvas) return { ok: false, reason: "canvas #renderer not found" };

  const runtimeConfig = readWorldTransport();
  const url = typeof window === "undefined" ? new URL("http://127.0.0.1/") : new URL(window.location.href);
  const renderConfig = readBrowserRenderConfig(url);
  const requestedCamera = readSmokeCamera(url);
  const sceneResult = await initializeRendererScene(canvas, {
    seed: GENERATED_SEED,
    viewDistance: renderConfig.viewDistance,
    renderDistance: renderConfig.renderDistance,
    worldTransport: runtimeConfig.worldTransport,
    remoteWorldHostUrl: runtimeConfig.remoteWorldHostUrl,
    skyColor: renderConfig.skyColor,
    clearColorScale: renderConfig.clearColorScale,
  });
  if (!sceneResult.ok) {
    return sceneResult;
  }
  const scene = sceneResult.scene;
  const expectedLoadedChunkCount = getExpectedLoadedChunkCount(scene.viewDistance);
  const camera = requestedCamera ?? DEFAULT_SMOKE_CAMERA;
  let frame: LevelRenderFrame;
  try {
    frame = await renderSmokeCamera(scene, camera, expectedLoadedChunkCount);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return { ok: false, reason };
  }

  const solidDraws = frame.layerDraws.get(RenderType.solid()) ?? [];
  const cutoutDraws = frame.layerDraws.get(RenderType.cutout()) ?? [];
  const translucentDraws = frame.layerDraws.get(RenderType.translucent()) ?? [];
  if (solidDraws.length === 0) {
    return { ok: false, reason: "camera-driven level renderer produced no solid drawables for the generated terrain scene" };
  }

  const initialPlayerState = scene.worldClient.getPlayerState();
  if (initialPlayerState !== undefined) {
    await scene.worldClient.setPlayerInput({
      type: "set_player_input",
      input: {
        sequence: 1,
        moveX: 1,
        moveY: 0,
        moveZ: 0,
        yaw: 180,
        pitch: 60,
      },
    });
    for (let attempt = 0; attempt < 5; attempt++) {
      await sleep(60);
      if (await scene.worldClient.pollUpdates()) {
        applyRenderWorldDirtySections(scene);
      }
      const updatedPlayerState = scene.worldClient.getPlayerState();
      if (updatedPlayerState !== undefined && updatedPlayerState.revision > initialPlayerState.revision) {
        break;
      }
    }
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
  const sessionState = scene.worldClient.getSessionState();
  const playerState = scene.worldClient.getPlayerState();

  return {
    ok: true,
    worldTransport: runtimeConfig.worldTransport,
    meshTransport: "worker",
    saveId: scene.saveMetadata.saveId,
    sessionId: sessionState?.sessionId,
    playerId: sessionState?.playerId,
    sessionRevision: sessionState?.revision,
    playerInputSequence: playerState?.acknowledgedInputSequence,
    playerStateRevision: playerState?.revision,
    playerTick: playerState?.tick,
    playerPosition: playerState ? [playerState.position.x, playerState.position.y, playerState.position.z] : undefined,
    format: scene.format,
    adapterInfo,
    centerPixel,
    terrainPixel,
    clearPixel,
    loadedChunkCount: getSceneLoadedChunkCount(scene),
    expectedLoadedChunkCount,
    viewDistance: scene.viewDistance,
    renderDistance: scene.gameRenderer.getRenderDistance(),
    solidDrawCount: solidDraws.length,
    cutoutDrawCount: cutoutDraws.length,
    translucentDrawCount: translucentDraws.length,
    renderWorldCounters: getSceneRenderWorldPerformanceCounters(scene),
    renderQueueStats: getSceneRenderQueueStats(scene),
  };
}

if (typeof window !== "undefined") {
  window.__mcloneReady = boot();
}
