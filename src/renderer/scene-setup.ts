import { ResourceLocation } from "../core/resource-location";
import { SectionPos } from "../core/section-pos";
import { type ClientRuntime, WorldClientRuntimeFacade } from "../runtime/client/client-runtime";
import { createBrowserIntegratedServer } from "../runtime/host/browser-integrated-server";
import { DEFAULT_PLAYER_PROFILE, type OpenWorldPreset, type PlayerProfile, type WorldEngineConfig, type WorldProgressMessage, type WorldStorageMode } from "../runtime/protocol/world-messages";
import type { WorldSaveMetadata } from "../runtime/storage/world-storage";
import { RemoteWorldClient, RemoteWorldWebSocketTransport } from "../runtime/transport/remote-world-transport";
import { TransportWorldClient } from "../runtime/transport/local-world-transport";
import { OverworldBiomeSource } from "../worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../worldgen/chunk/chunk-block-buffer";
import { ClientChunkCache } from "../world/level/client-chunk-cache";
import { createBlockStateResolver } from "../world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../world/level/generated-render-blocks";
import { FoliageColor } from "../world/level/foliage-color";
import { GrassColor } from "../world/level/grass-color";
import { getBrowserAssetPack } from "./assets/asset-pack";
import type { LoadingProgressSink } from "./loading-progress";
import { scaleProgress } from "./loading-progress";
import { BlockColors } from "./block/block-colors";
import { BlockRenderDispatcher } from "./block/block-render-dispatcher";
import { ChunkRenderDispatcher } from "./chunk/chunk-render-dispatcher";
import { RenderWorldWorkerClient, RenderWorldWorkerUpdateSink } from "./chunk/render-world-worker-client";
import { ChunkBufferBuilderPack } from "./chunk-buffer-builder-pack";
import { BlockModelShaper } from "./model/block-model-shaper";
import { BuiltInModel } from "./model/built-in-model";
import { ItemOverrides } from "./model/item-overrides";
import { ItemTransforms } from "./model/item-transforms";
import { ModelManager } from "./model/model-manager";
import { LevelRenderer, type LevelRenderFrame } from "./level-renderer";
import { LightTexture } from "./light-texture";
import { GameRenderer, type CameraState } from "./game-renderer";
import { EntityTextureManager } from "./texture/entity-texture-manager";
import { BrowserTextureAtlasSource } from "./texture/browser-native-image-loader";
import { DEFAULT_BLOCK_ATLAS_MIP_LEVEL, TextureAtlas } from "./texture/texture-atlas";
import { MissingTextureAtlasSprite } from "./texture/missing-texture-atlas-sprite";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { CompositeRenderType, RenderType } from "./render-type";
import { ViewArea } from "./view-area";
import { Vec3 } from "../world/phys/vec3";
import { VertexBuffer } from "./vertex/vertex-buffer";
import { BROWSER_RENDERER_HOST, type BrowserRendererHost } from "./renderer-host";
import type { GeneratedChunkLifecycleSnapshot, WorldDebugRequestOptions } from "../runtime/protocol/chunk-lifecycle";
import { createChunkBorderDebugVertexBuffer } from "./debug/chunk-border-debug-renderer";

const SMOKE_ATLAS_LOCATION = new ResourceLocation("minecraft:textures/atlas/blocks.png");
const GRASS_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/grass");
const FOLIAGE_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/foliage");

export const SCENE_DEPTH_FORMAT: GPUTextureFormat = "depth24plus";

const RENDER_ORDER = [
  RenderType.solid(),
  RenderType.cutoutMipped(),
  RenderType.cutout(),
  RenderType.translucent(),
  RenderType.tripwire(),
] as const;

export interface SceneInitOptions {
  readonly seed: bigint;
  readonly viewDistance: number;
  readonly renderDistance?: number;
  readonly fogEnabled?: boolean;
  readonly fov?: number;
  readonly worldTransport?: "worker" | "remote";
  readonly remoteWorldHostUrl?: string;
  readonly preset?: OpenWorldPreset;
  readonly engineConfig?: WorldEngineConfig;
  readonly worldStorageMode?: WorldStorageMode;
  readonly playerProfile?: PlayerProfile;
  readonly skyColor?: Vec3;
  readonly clearColorScale?: number;
  readonly onProgress?: LoadingProgressSink;
  readonly rendererHost?: BrowserRendererHost;
  readonly pollDebugOptions?: () => WorldDebugRequestOptions | undefined;
}

export interface RendererScene {
  readonly adapter: GPUAdapter;
  readonly device: GPUDevice;
  readonly format: GPUTextureFormat;
  readonly canvas: HTMLCanvasElement;
  readonly ctx: GPUCanvasContext;
  readonly saveMetadata: WorldSaveMetadata;
  readonly atlas: TextureAtlas;
  readonly clientRuntime: ClientRuntime;
  readonly renderWorldUpdateSink: RenderWorldWorkerUpdateSink;
  readonly worldBounds: RenderWorldBounds;
  readonly levelRenderer: LevelRenderer;
  readonly gameRenderer: GameRenderer;
  readonly lightTexture: LightTexture;
  readonly pipelineCache: RenderPipelineCache;
  readonly entityTextureManager: EntityTextureManager;
  readonly textureSamplers: ReadonlyMap<string, GPUSampler>;
  readonly lightSampler: GPUSampler;
  readonly viewDistance: number;
  readonly viewArea: ViewArea;
  readonly chunkDispatcher: ChunkRenderDispatcher;
  readonly chunkDrawResources: WeakMap<VertexBuffer, Map<GPUBindGroupLayout, CachedChunkDrawResources>>;
  readonly entityFrameResources: EntityFrameGpuResources[];
  entityFrameBufferFrameId: number | undefined;
}

export interface EntityFrameGpuResources {
  readonly vertexBuffer: VertexBuffer;
  readonly uniformBuffer: GPUBuffer;
}

export interface RenderWorldBounds {
  readonly minBuildHeight: number;
  readonly maxBuildHeight: number;
}

export interface RenderWorldPerformanceCounters {
  readonly ingestBatchCount: number;
  readonly meshBuildRequestCount: number;
  readonly meshNotReadyResponseCount: number;
  readonly meshCompletionCount: number;
  readonly mainThreadGpuUploadCount: number;
}

export interface RenderSceneQueueStats {
  readonly renderedChunkCount: number;
  readonly pendingVisibleChunkCompileCount: number;
  readonly queuedChunkBuildCount: number;
  readonly activeChunkBuildCount: number;
}

export interface StableSceneFrameOptions {
  readonly maxAttempts?: number;
  readonly stablePasses?: number;
  readonly pollIntervalMs?: number;
  readonly onProgress?: LoadingProgressSink;
}

export type SceneInitResult =
  | { ok: true; scene: RendererScene }
  | { ok: false; reason: string };

export interface DrawTarget {
  readonly view: GPUTextureView;
  readonly depthView: GPUTextureView;
  readonly format: GPUTextureFormat;
}

export interface SceneDepthTarget {
  readonly texture: GPUTexture;
  readonly view: GPUTextureView;
}

export interface CanvasResizeResult {
  readonly changed: boolean;
  readonly width: number;
  readonly height: number;
}

interface TextureSamplerState {
  readonly blur: boolean;
  readonly mipmap: boolean;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function worldProgressFraction(progress: WorldProgressMessage): number {
  const phaseFraction = progress.total > 0 ? progress.current / progress.total : 1;
  const clampedPhaseFraction = Math.max(0, Math.min(1, phaseFraction));
  const loadingPhases = [
    "Checking saved chunks",
    "Generating status chunks",
    "Advancing FEATURES",
    "Computing light",
    "Publishing chunks",
  ];
  const phaseIndex = loadingPhases.indexOf(progress.stage);
  if (phaseIndex >= 0) {
    return (phaseIndex + clampedPhaseFraction) / loadingPhases.length;
  }

  switch (progress.stage) {
    case "Generating terrain chunks":
      return clampedPhaseFraction / 3;
    case "Decorating chunks":
      return (1 + clampedPhaseFraction) / 3;
    case "Publishing chunks":
      return (2 + clampedPhaseFraction) / 3;
    default:
      return clampedPhaseFraction;
  }
}

function reportWorldProgress(sink: LoadingProgressSink | undefined, progress: WorldProgressMessage): void {
  sink?.({
    stage: progress.stage,
    detail: progress.detail,
    current: progress.current,
    total: progress.total,
    fraction: 0.92 + (worldProgressFraction(progress) * 0.06),
  });
}

function clampCanvasSize(width: number, height: number, maxTextureDimension: number): {
  readonly width: number;
  readonly height: number;
} {
  if (width <= maxTextureDimension && height <= maxTextureDimension) {
    return { width, height };
  }

  const scale = Math.min(maxTextureDimension / width, maxTextureDimension / height);
  return {
    width: Math.max(1, Math.floor(width * scale)),
    height: Math.max(1, Math.floor(height * scale)),
  };
}

export function resizeCanvasToDisplaySize(
  canvas: HTMLCanvasElement,
  maxTextureDimension: number = Number.MAX_SAFE_INTEGER,
): CanvasResizeResult {
  const devicePixelRatio = typeof window === "undefined" ? 1 : Math.max(1, window.devicePixelRatio || 1);
  const clientWidth = Math.max(1, Math.floor(canvas.clientWidth || canvas.width));
  const clientHeight = Math.max(1, Math.floor(canvas.clientHeight || canvas.height));
  const clamped = clampCanvasSize(
    Math.max(1, Math.round(clientWidth * devicePixelRatio)),
    Math.max(1, Math.round(clientHeight * devicePixelRatio)),
    maxTextureDimension,
  );
  const changed = canvas.width !== clamped.width || canvas.height !== clamped.height;
  if (changed) {
    canvas.width = clamped.width;
    canvas.height = clamped.height;
  }

  return {
    changed,
    width: clamped.width,
    height: clamped.height,
  };
}

function createWorldClient(
  options: SceneInitOptions,
  airState: import("../world/level/block/state/block-state").BlockState,
  biomeSource: OverworldBiomeSource,
  blockStateResolver: ReturnType<typeof createBlockStateResolver>,
  blockStateIds: import("../world/level/block/state/block-state-id").BlockStateIdMap,
): TransportWorldClient {
  const levelFactory = (worldOpened: import("../runtime/protocol/world-messages").WorldOpenedMessage) => new ClientChunkCache({
    airState,
    minBuildHeight: worldOpened.minBuildHeight,
    height: worldOpened.height,
    biomeSource,
    biomeZoomSeed: options.seed,
    blockStateResolver,
    blockStateIds,
    skyColor: options.skyColor,
    clearColorScale: options.clearColorScale,
  });

  if (options.worldTransport === "remote") {
    return new RemoteWorldClient(
      new RemoteWorldWebSocketTransport(options.remoteWorldHostUrl ?? "http://127.0.0.1:4173"),
      levelFactory,
      {
        pollDebugOptions: options.pollDebugOptions,
        worldProgressSink: (progress) => reportWorldProgress(options.onProgress, progress),
      },
    );
  }

  return new TransportWorldClient(
    createBrowserIntegratedServer(),
    levelFactory,
    {
      pollUpdateMaxMessages: 4,
      pollDebugOptions: options.pollDebugOptions,
      worldProgressSink: (progress) => reportWorldProgress(options.onProgress, progress),
    },
  );
}

export function getSceneLoadedChunkCount(scene: RendererScene): number {
  return scene.renderWorldUpdateSink.getStats().loadedChunkCount;
}

export function getSceneRenderWorldPerformanceCounters(scene: RendererScene): RenderWorldPerformanceCounters {
  return {
    ...scene.renderWorldUpdateSink.getPerformanceCounters(),
    mainThreadGpuUploadCount: scene.chunkDispatcher.getMainThreadGpuUploadCount(),
  };
}

export function getSceneRenderQueueStats(scene: RendererScene): RenderSceneQueueStats {
  return {
    renderedChunkCount: scene.levelRenderer.countRenderedChunks(),
    pendingVisibleChunkCompileCount: scene.levelRenderer.getPendingVisibleChunkCompileCount(),
    queuedChunkBuildCount: scene.chunkDispatcher.getToBatchCount(),
    activeChunkBuildCount: scene.chunkDispatcher.getActiveTaskCount(),
  };
}

export function closeEntityFrameBuffers(scene: RendererScene): void {
  for (const resource of scene.entityFrameResources) {
    resource.vertexBuffer.close();
    resource.uniformBuffer.destroy();
  }
  scene.entityFrameResources.length = 0;
}

export function closeRendererScene(scene: RendererScene): void {
  scene.clientRuntime.close();
  scene.viewArea.releaseAllBuffers();
  scene.chunkDispatcher.dispose();
  scene.lightTexture.close();
  closeEntityFrameBuffers(scene);
  scene.entityTextureManager.close();
  scene.atlas.clearTextureData();
}

export function applyRenderWorldDirtySections(scene: RendererScene): number {
  let dirtyCount = 0;
  for (const dirtySection of scene.renderWorldUpdateSink.drainDirtySections()) {
    if (dirtySection.y < scene.worldBounds.minBuildHeight || dirtySection.y >= scene.worldBounds.maxBuildHeight) {
      continue;
    }

    scene.viewArea.setDirty(
      SectionPos.blockToSectionCoord(dirtySection.x),
      SectionPos.blockToSectionCoord(dirtySection.y),
      SectionPos.blockToSectionCoord(dirtySection.z),
      false,
    );
    dirtyCount++;
  }

  if (dirtyCount > 0) {
    scene.levelRenderer.requestUpdate();
  }

  return dirtyCount;
}

export async function waitForLoadedChunkRing(
  scene: RendererScene,
  expectedLoadedChunkCount: number,
  options: Pick<StableSceneFrameOptions, "maxAttempts" | "pollIntervalMs" | "onProgress"> = {},
): Promise<boolean> {
  options.onProgress?.({
    stage: "Loading terrain chunks",
    current: getSceneLoadedChunkCount(scene),
    total: expectedLoadedChunkCount,
  });
  if (getSceneLoadedChunkCount(scene) >= expectedLoadedChunkCount) {
    return true;
  }

  const maxAttempts = options.maxAttempts ?? 320;
  const pollIntervalMs = options.pollIntervalMs ?? 50;
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    await sleep(pollIntervalMs);
    if (await scene.clientRuntime.drainTransportUpdates()) {
      applyRenderWorldDirtySections(scene);
    }
    const loadedChunkCount = getSceneLoadedChunkCount(scene);
    if (loadedChunkCount > 0) {
      options.onProgress?.({
        stage: "Loading terrain chunks",
        current: loadedChunkCount,
        total: expectedLoadedChunkCount,
      });
    }
    if (loadedChunkCount >= expectedLoadedChunkCount) {
      return true;
    }
  }

  return getSceneLoadedChunkCount(scene) >= expectedLoadedChunkCount;
}

export async function renderSceneUntilSettled(
  scene: RendererScene,
  camera: CameraState,
  options: StableSceneFrameOptions = {},
): Promise<LevelRenderFrame> {
  const maxAttempts = options.maxAttempts ?? 30;
  const stablePassTarget = options.stablePasses ?? 3;
  const pollIntervalMs = options.pollIntervalMs ?? 50;
  let frame: LevelRenderFrame | undefined;
  let bestFrame: LevelRenderFrame | undefined;
  let bestRenderedChunkCount = -1;
  let previousRenderedChunkCount = -1;
  let stablePasses = 0;

  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    if (await scene.clientRuntime.drainTransportUpdates()) {
      applyRenderWorldDirtySections(scene);
    }

    frame = await scene.gameRenderer.renderLevel(
      0.0,
      Number.MAX_SAFE_INTEGER,
      scene.levelRenderer,
      scene.lightTexture,
      camera,
      {
        waitForChunkTasks: true,
        entityPresentation: scene.clientRuntime.publishPresentationState().entityPresentation,
      },
    );

    const queueStats = getSceneRenderQueueStats(scene);
    const hasSolidDraws = (frame.layerDraws.get(RenderType.solid())?.length ?? 0) > 0;
    const renderQueueIdle = queueStats.pendingVisibleChunkCompileCount === 0
      && queueStats.queuedChunkBuildCount === 0
      && queueStats.activeChunkBuildCount === 0;

    if (queueStats.renderedChunkCount > bestRenderedChunkCount) {
      bestFrame = frame;
      bestRenderedChunkCount = queueStats.renderedChunkCount;
    }

    if (hasSolidDraws && renderQueueIdle && queueStats.renderedChunkCount === previousRenderedChunkCount) {
      stablePasses++;
      if (stablePasses >= stablePassTarget) {
        return frame;
      }
    } else {
      stablePasses = 0;
    }
    previousRenderedChunkCount = queueStats.renderedChunkCount;

    await sleep(pollIntervalMs);
    scene.levelRenderer.requestUpdate();
  }

  return bestFrame ?? frame!;
}

async function initializeBiomeColorTables(atlasSource: BrowserTextureAtlasSource): Promise<void> {
  const [grassPixels, foliagePixels] = await Promise.all([
    atlasSource.loadColorMap(GRASS_COLORMAP_LOCATION),
    atlasSource.loadColorMap(FOLIAGE_COLORMAP_LOCATION),
  ]);
  GrassColor.init(grassPixels);
  FoliageColor.init(foliagePixels);
}

function textureSamplerKey(state: TextureSamplerState): string {
  return `${state.blur ? 1 : 0}:${state.mipmap ? 1 : 0}`;
}

// WebGPU: map Minecraft's GL min/mag/mipmap filter tuple onto an explicit sampler descriptor.
function createTextureSampler(device: GPUDevice, state: TextureSamplerState): GPUSampler {
  return device.createSampler({
    magFilter: state.blur ? "linear" : "nearest",
    minFilter: state.blur ? "linear" : "nearest",
    mipmapFilter: state.mipmap ? "linear" : "nearest",
  });
}

function createTextureSamplers(device: GPUDevice): ReadonlyMap<string, GPUSampler> {
  const samplers = new Map<string, GPUSampler>();
  for (const state of [
    { blur: false, mipmap: false },
    { blur: false, mipmap: true },
    { blur: true, mipmap: false },
    { blur: true, mipmap: true },
  ] satisfies readonly TextureSamplerState[]) {
    samplers.set(textureSamplerKey(state), createTextureSampler(device, state));
  }

  return samplers;
}

function getTextureSampler(scene: RendererScene, state: TextureSamplerState): GPUSampler {
  const sampler = scene.textureSamplers.get(textureSamplerKey(state));
  if (sampler === undefined) {
    throw new Error(`Missing texture sampler for blur=${state.blur} mipmap=${state.mipmap}`);
  }

  return sampler;
}

function getAtlasSampler(scene: RendererScene, renderType: CompositeRenderType): GPUSampler {
  const textureState = renderType.state().textureState.getTextures()[0];
  if (textureState === undefined) {
    return getTextureSampler(scene, { blur: false, mipmap: false });
  }

  return getTextureSampler(scene, textureState);
}

function createFallbackBlockModelShaper(atlas: TextureAtlas): BlockModelShaper {
  const missingModel = new BuiltInModel(
    ItemTransforms.NO_TRANSFORMS,
    ItemOverrides.EMPTY,
    atlas.getSprite(MissingTextureAtlasSprite.getLocation()),
    false,
  );
  const modelManager = new ModelManager(missingModel);
  const blockModelShaper = new BlockModelShaper(modelManager);
  blockModelShaper.rebuildCache();
  return blockModelShaper;
}

export async function initializeRendererScene(
  canvas: HTMLCanvasElement,
  options: SceneInitOptions,
): Promise<SceneInitResult> {
  const rendererHost = options.rendererHost ?? BROWSER_RENDERER_HOST;
  const deviceContextResult = await rendererHost.requestWebGpuDeviceContext({
    onRequestAdapter: () => options.onProgress?.({ stage: "Requesting WebGPU adapter", fraction: 0.02 }),
    onRequestDevice: () => options.onProgress?.({ stage: "Requesting WebGPU device", fraction: 0.04 }),
  });
  if (!deviceContextResult.ok) {
    return deviceContextResult;
  }
  const { adapter, device, format } = deviceContextResult.context;
  resizeCanvasToDisplaySize(canvas, device.limits.maxTextureDimension2D);
  const target = rendererHost.createCanvasTarget(canvas, device, format);
  if (!target) return { ok: false, reason: "canvas.getContext('webgpu') returned null" };
  const ctx = target.ctx;

  const generatedBlocks = registerGeneratedRenderBlocks();
  options.onProgress?.({ stage: "Loading asset pack", fraction: 0.05 });
  const assetPack = await getBrowserAssetPack(scaleProgress(options.onProgress, 0.05, 0.22, "Loading asset pack"));
  const atlasSource = new BrowserTextureAtlasSource(assetPack);
  options.onProgress?.({ stage: "Loading biome color tables", fraction: 0.24 });
  await initializeBiomeColorTables(atlasSource);
  options.onProgress?.({ stage: "Preparing texture atlas", fraction: 0.28 });
  const atlas = new TextureAtlas(SMOKE_ATLAS_LOCATION, device.limits.maxTextureDimension2D);
  const preparations = await atlas.prepareToStitch(atlasSource, generatedBlocks.spriteLocations, DEFAULT_BLOCK_ATLAS_MIP_LEVEL);
  options.onProgress?.({ stage: "Uploading texture atlas", fraction: 0.4 });
  atlas.reload(device, preparations);
  const blockModelShaper = createFallbackBlockModelShaper(atlas);

  options.onProgress?.({ stage: "Opening world", fraction: 0.44 });
  const biomeSource = new OverworldBiomeSource(options.seed);
  const blockStateResolver = createBlockStateResolver(generatedBlocks.airState);
  const worldClient = createWorldClient(options, generatedBlocks.airState, biomeSource, blockStateResolver, generatedBlocks.blockStateIds);
  const clientRuntime = new WorldClientRuntimeFacade(worldClient);
  const storageMode = options.worldTransport === "remote" ? undefined : options.worldStorageMode;
  const worldOpened = await clientRuntime.openWorld({
    type: "open_world",
    seed: options.seed,
    preset: options.preset ?? "browser_smoke",
    config: options.engineConfig,
    playerProfile: options.playerProfile ?? DEFAULT_PLAYER_PROFILE,
    ...(storageMode === undefined || storageMode === "default" ? {} : { storageMode }),
  });
  const compatibilityLevel = clientRuntime.getClientWorld().getRenderView().getRenderLevel();

  options.onProgress?.({ stage: "Initializing render worker", fraction: 0.5 });
  const renderWorldWorker = new RenderWorldWorkerClient(
    rendererHost.createRenderWorldWorkerEndpoint(),
    scaleProgress(options.onProgress, 0.5, 0.9, "Initializing render worker"),
  );
  await renderWorldWorker.initialize({
    type: "initialize_render_world",
    seed: options.seed,
    minBuildHeight: worldOpened.minBuildHeight,
    height: worldOpened.height,
  });
  options.onProgress?.({ stage: "Preparing renderer", fraction: 0.92 });
  const renderWorldUpdateSink = new RenderWorldWorkerUpdateSink(renderWorldWorker);
  clientRuntime.setRenderWorldUpdateSink(renderWorldUpdateSink);

  const levelRenderer = new LevelRenderer();
  const blockRenderer = new BlockRenderDispatcher(
    blockModelShaper,
    BlockColors.createDefault(),
    (location) => atlas.getSprite(location),
    generatedBlocks.blockStateById[ChunkBlockId.WATER]!,
    generatedBlocks.blockStateById[ChunkBlockId.LAVA]!,
  );
  const chunkDispatcher = new ChunkRenderDispatcher(
    compatibilityLevel,
    levelRenderer,
    blockRenderer,
    device,
    (task) => queueMicrotask(task),
    false,
    new ChunkBufferBuilderPack(),
    undefined,
    renderWorldWorker,
  );
  const viewArea = new ViewArea(chunkDispatcher, compatibilityLevel, options.viewDistance, levelRenderer);
  levelRenderer.setLevel(compatibilityLevel, chunkDispatcher, viewArea, options.viewDistance);

  const gameRenderer = new GameRenderer(
    canvas.width,
    canvas.height,
    options.renderDistance ?? Math.max(32, (options.viewDistance + 2) * 16),
    options.fov,
    options.fogEnabled ?? true,
  );
  const lightTexture = new LightTexture(gameRenderer, compatibilityLevel, device);
  lightTexture.tick();

  const pipelineCache = new RenderPipelineCache(device);
  const entityTextureManager = await EntityTextureManager.create(device, assetPack);
  const textureSamplers = createTextureSamplers(device);
  const lightSampler = device.createSampler({
    magFilter: "linear",
    minFilter: "linear",
    mipmapFilter: "nearest",
  });

  options.onProgress?.({ stage: "Renderer ready", fraction: 1 });

  return {
    ok: true,
    scene: {
      adapter,
      device,
      format,
      canvas,
      ctx,
      saveMetadata: worldOpened.saveMetadata,
      atlas,
      clientRuntime,
      renderWorldUpdateSink,
      worldBounds: {
        minBuildHeight: compatibilityLevel.getMinBuildHeight(),
        maxBuildHeight: compatibilityLevel.getMaxBuildHeight(),
      },
      levelRenderer,
      gameRenderer,
      lightTexture,
      pipelineCache,
      entityTextureManager,
      textureSamplers,
      lightSampler,
      viewDistance: options.viewDistance,
      viewArea,
      chunkDispatcher,
      chunkDrawResources: new WeakMap<VertexBuffer, Map<GPUBindGroupLayout, CachedChunkDrawResources>>(),
      entityFrameResources: [],
      entityFrameBufferFrameId: undefined,
    },
  };
}

interface DrawEntry {
  readonly vertexBuffer: VertexBuffer;
  readonly bindGroup: GPUBindGroup;
}

interface CachedChunkDrawResources {
  readonly uniformBuffer: GPUBuffer;
  readonly uniformBytes: Uint8Array;
  readonly bindGroup: GPUBindGroup;
  readonly atlasTexture: GPUTextureView;
  readonly lightTexture: GPUTextureView;
  readonly atlasSampler: GPUSampler;
  readonly lightSampler: GPUSampler;
}

interface PassLayer {
  readonly pipeline: GPURenderPipeline;
  readonly draws: readonly DrawEntry[];
}

export interface EncodeSceneFrameOptions {
  readonly chunkBorderDebugSnapshot?: GeneratedChunkLifecycleSnapshot;
}

const COLOR_MODULATOR = [1, 1, 1, 1] as const;
const LEVEL_DIFFUSE_LIGHT_0 = normalizeVector3([0.2, 1.0, -0.7]);
const LEVEL_DIFFUSE_LIGHT_1 = normalizeVector3([-0.2, 1.0, 0.7]);

function normalizeVector3(value: readonly [number, number, number]): readonly [number, number, number] {
  const length = Math.hypot(value[0], value[1], value[2]);
  return [value[0] / length, value[1] / length, value[2] / length];
}

function transformDirection(matrix: Float32Array, value: readonly [number, number, number]): readonly [number, number, number] {
  return [
    (matrix[0]! * value[0]) + (matrix[4]! * value[1]) + (matrix[8]! * value[2]),
    (matrix[1]! * value[0]) + (matrix[5]! * value[1]) + (matrix[9]! * value[2]),
    (matrix[2]! * value[0]) + (matrix[6]! * value[1]) + (matrix[10]! * value[2]),
  ];
}

function createChunkDrawResources(
  scene: RendererScene,
  pipeline: import("./pipeline/render-pipeline-cache").CachedRenderPipeline,
  atlasTexture: GPUTextureView,
  lightTexture: GPUTextureView,
  atlasSampler: GPUSampler,
  lightSampler: GPUSampler,
): CachedChunkDrawResources {
  const uniformBuffer = scene.device.createBuffer({
    size: Math.max(4, pipeline.shaderProgram.uniformBufferSize),
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });
  return {
    uniformBuffer,
    uniformBytes: new Uint8Array(pipeline.shaderProgram.uniformBufferSize),
    bindGroup: pipeline.shaderProgram.createBindGroup(scene.device, pipeline.bindGroupLayout, {
      uniformBuffer,
      samplers: {
        Sampler0: atlasSampler,
        Sampler2: lightSampler,
      },
      textures: {
        Sampler0: atlasTexture,
        Sampler2: lightTexture,
      },
    }),
    atlasTexture,
    lightTexture,
    atlasSampler,
    lightSampler,
  };
}

function getChunkDrawResources(
  scene: RendererScene,
  pipeline: import("./pipeline/render-pipeline-cache").CachedRenderPipeline,
  draw: import("./level-renderer").ChunkLayerDraw,
  atlasTexture: GPUTextureView,
  lightTexture: GPUTextureView,
  atlasSampler: GPUSampler,
  lightSampler: GPUSampler,
): CachedChunkDrawResources {
  let resourcesByLayout = scene.chunkDrawResources.get(draw.vertexBuffer);
  if (resourcesByLayout === undefined) {
    resourcesByLayout = new Map<GPUBindGroupLayout, CachedChunkDrawResources>();
    scene.chunkDrawResources.set(draw.vertexBuffer, resourcesByLayout);
  }

  const cached = resourcesByLayout.get(pipeline.bindGroupLayout);
  if (
    cached !== undefined &&
    cached.atlasTexture === atlasTexture &&
    cached.lightTexture === lightTexture &&
    cached.atlasSampler === atlasSampler &&
    cached.lightSampler === lightSampler
  ) {
    return cached;
  }

  const created = createChunkDrawResources(scene, pipeline, atlasTexture, lightTexture, atlasSampler, lightSampler);
  resourcesByLayout.set(pipeline.bindGroupLayout, created);
  return created;
}

function updateChunkUniforms(
  scene: RendererScene,
  pipeline: import("./pipeline/render-pipeline-cache").CachedRenderPipeline,
  resources: CachedChunkDrawResources,
  frame: LevelRenderFrame,
  chunkOffset: readonly [number, number, number],
): void {
  pipeline.shaderProgram.writeUniformBufferBytes(resources.uniformBytes, {
    ModelViewMat: frame.modelViewMatrix,
    ProjMat: frame.projectionMatrix,
    ChunkOffset: chunkOffset,
    ColorModulator: COLOR_MODULATOR,
    FogStart: [frame.fogStart],
    FogEnd: [frame.fogEnd],
    FogColor: frame.fogColor,
  });
  scene.device.queue.writeBuffer(resources.uniformBuffer, 0, resources.uniformBytes as Uint8Array<ArrayBuffer>);
}

function createChunkBindGroup(
  scene: RendererScene,
  pipeline: import("./pipeline/render-pipeline-cache").CachedRenderPipeline,
  frame: LevelRenderFrame,
  draw: import("./level-renderer").ChunkLayerDraw,
  renderType: CompositeRenderType,
  atlasTexture: GPUTextureView,
): GPUBindGroup {
  const resources = getChunkDrawResources(
    scene,
    pipeline,
    draw,
    atlasTexture,
    frame.lightTexture,
    getAtlasSampler(scene, renderType),
    scene.lightSampler,
  );
  updateChunkUniforms(scene, pipeline, resources, frame, draw.chunkOffset);
  return resources.bindGroup;
}

function createChunkBindGroupEntry(
  scene: RendererScene,
  pipeline: import("./pipeline/render-pipeline-cache").CachedRenderPipeline,
  frame: LevelRenderFrame,
  draw: import("./level-renderer").ChunkLayerDraw,
  renderType: CompositeRenderType,
  atlasTexture: GPUTextureView,
): DrawEntry {
  return {
    vertexBuffer: draw.vertexBuffer,
    bindGroup: createChunkBindGroup(scene, pipeline, frame, draw, renderType, atlasTexture),
  };
}

function prepareEntityFrameBufferEpoch(scene: RendererScene, frame: LevelRenderFrame): void {
  if (scene.entityFrameBufferFrameId === frame.frameId) {
    return;
  }

  closeEntityFrameBuffers(scene);
  scene.entityFrameBufferFrameId = frame.frameId;
}

function createEntityBindGroupEntry(
  scene: RendererScene,
  pipeline: import("./pipeline/render-pipeline-cache").CachedRenderPipeline,
  frame: LevelRenderFrame,
  renderType: CompositeRenderType,
  batch: import("./entity/entity-batch-renderer").EntityRenderBatch,
): DrawEntry {
  const vertexBuffer = new VertexBuffer(scene.device);
  vertexBuffer.uploadRaw(batch.drawState, batch.buffer);
  const uniformBuffer = scene.device.createBuffer({
    size: Math.max(4, pipeline.shaderProgram.uniformBufferSize),
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });
  const uniformBytes = new Uint8Array(pipeline.shaderProgram.uniformBufferSize);
  const light0 = transformDirection(frame.modelViewMatrix, LEVEL_DIFFUSE_LIGHT_0);
  const light1 = transformDirection(frame.modelViewMatrix, LEVEL_DIFFUSE_LIGHT_1);
  pipeline.shaderProgram.writeUniformBufferBytes(uniformBytes, {
    ModelViewMat: frame.modelViewMatrix,
    ProjMat: frame.projectionMatrix,
    ColorModulator: COLOR_MODULATOR,
    Light0_Direction: light0,
    Light1_Direction: light1,
    FogStart: [frame.fogStart],
    FogEnd: [frame.fogEnd],
    FogColor: frame.fogColor,
  });
  scene.device.queue.writeBuffer(uniformBuffer, 0, uniformBytes as Uint8Array<ArrayBuffer>);

  scene.entityFrameResources.push({
    vertexBuffer,
    uniformBuffer,
  });
  return {
    vertexBuffer,
    bindGroup: pipeline.shaderProgram.createBindGroup(scene.device, pipeline.bindGroupLayout, {
      uniformBuffer,
      samplers: {
        Sampler0: getAtlasSampler(scene, renderType),
        Sampler1: getTextureSampler(scene, { blur: false, mipmap: false }),
        Sampler2: scene.lightSampler,
      },
      textures: {
        Sampler0: scene.entityTextureManager.getTextureView(renderType.state().textureState.cutoutTexture()),
        Sampler1: scene.entityTextureManager.getOverlayTextureView(),
        Sampler2: frame.lightTexture,
      },
    }),
  };
}

function createChunkBorderDebugBindGroupEntry(
  scene: RendererScene,
  pipeline: import("./pipeline/render-pipeline-cache").CachedRenderPipeline,
  frame: LevelRenderFrame,
  vertexBuffer: VertexBuffer,
): DrawEntry {
  const uniformBuffer = scene.device.createBuffer({
    size: Math.max(4, pipeline.shaderProgram.uniformBufferSize),
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });
  const uniformBytes = new Uint8Array(pipeline.shaderProgram.uniformBufferSize);
  pipeline.shaderProgram.writeUniformBufferBytes(uniformBytes, {
    ModelViewMat: frame.modelViewMatrix,
    ProjMat: frame.projectionMatrix,
    ColorModulator: COLOR_MODULATOR,
    LineWidth: [0],
    ScreenSize: [scene.canvas.width, scene.canvas.height],
    FogStart: [frame.fogStart],
    FogEnd: [frame.fogEnd],
    FogColor: [frame.fogColor[0], frame.fogColor[1], frame.fogColor[2], 0],
  });
  scene.device.queue.writeBuffer(uniformBuffer, 0, uniformBytes as Uint8Array<ArrayBuffer>);

  scene.entityFrameResources.push({
    vertexBuffer,
    uniformBuffer,
  });
  return {
    vertexBuffer,
    bindGroup: pipeline.shaderProgram.createBindGroup(scene.device, pipeline.bindGroupLayout, {
      uniformBuffer,
    }),
  };
}

function appendChunkBorderDebugPassLayer(
  scene: RendererScene,
  frame: LevelRenderFrame,
  target: DrawTarget,
  snapshot: GeneratedChunkLifecycleSnapshot | undefined,
  layers: PassLayer[],
): void {
  const debugBuffer = createChunkBorderDebugVertexBuffer(scene.device, frame, snapshot, scene.worldBounds);
  if (debugBuffer === undefined) {
    return;
  }

  const renderType = RenderType.lines();
  if (debugBuffer.vertexBuffer.getFormat() !== renderType.format()) {
    throw new Error("render type format does not match chunk border debug vertex format");
  }

  const pipeline = scene.pipelineCache.getOrCreate(renderType, target.format, SCENE_DEPTH_FORMAT);
  layers.push({
    pipeline: pipeline.pipeline,
    draws: [createChunkBorderDebugBindGroupEntry(scene, pipeline, frame, debugBuffer.vertexBuffer)],
  });
}

function appendEntityPassLayers(
  scene: RendererScene,
  frame: LevelRenderFrame,
  target: DrawTarget,
  layers: PassLayer[],
): void {
  prepareEntityFrameBufferEpoch(scene, frame);
  for (const batch of frame.entityBatches) {
    const renderType = batch.renderType;
    if (!(renderType instanceof CompositeRenderType)) {
      throw new Error("entity batch render type must be a composite render type");
    }

    if (batch.drawState.format() !== renderType.format()) {
      throw new Error("render type format does not match an entity batch vertex format");
    }

    if (batch.drawState.vertexCount() === 0) {
      continue;
    }

    const pipeline = scene.pipelineCache.getOrCreate(renderType, target.format, SCENE_DEPTH_FORMAT);
    layers.push({
      pipeline: pipeline.pipeline,
      draws: [createEntityBindGroupEntry(scene, pipeline, frame, renderType, batch)],
    });
  }
}

export function encodeSceneFrame(
  scene: RendererScene,
  frame: LevelRenderFrame,
  target: DrawTarget,
  encoder: GPUCommandEncoder,
  options: EncodeSceneFrameOptions = {},
): void {
  const layers: PassLayer[] = [];
  const atlasTexture = scene.atlas.getTextureView();
  for (const renderType of RENDER_ORDER) {
    const drawEntries = frame.layerDraws.get(renderType);
    if (drawEntries === undefined || drawEntries.length === 0) {
      continue;
    }

    for (const draw of drawEntries) {
      if (draw.vertexBuffer.getFormat() !== renderType.format()) {
        throw new Error("render type format does not match a compiled chunk vertex format");
      }
    }

    const pipeline = scene.pipelineCache.getOrCreate(renderType, target.format, SCENE_DEPTH_FORMAT);
    layers.push({
      pipeline: pipeline.pipeline,
      draws: drawEntries.map((draw) => createChunkBindGroupEntry(scene, pipeline, frame, draw, renderType, atlasTexture)),
    });
  }
  appendEntityPassLayers(scene, frame, target, layers);
  appendChunkBorderDebugPassLayer(scene, frame, target, options.chunkBorderDebugSnapshot, layers);

  const clearColor: GPUColor = {
    r: frame.fogColor[0],
    g: frame.fogColor[1],
    b: frame.fogColor[2],
    a: frame.fogColor[3],
  };
  const pass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view: target.view,
        clearValue: clearColor,
        loadOp: "clear",
        storeOp: "store",
      },
    ],
    depthStencilAttachment: {
      view: target.depthView,
      depthClearValue: 1,
      depthLoadOp: "clear",
      depthStoreOp: "store",
    },
  });
  for (const layer of layers) {
    pass.setPipeline(layer.pipeline);
    for (const entry of layer.draws) {
      pass.setBindGroup(0, entry.bindGroup);
      entry.vertexBuffer.draw(pass);
    }
  }
  pass.end();
}

export function createSceneDepthView(device: GPUDevice, width: number, height: number): GPUTextureView {
  return createSceneDepthTarget(device, width, height).view;
}

export function createSceneDepthTarget(device: GPUDevice, width: number, height: number): SceneDepthTarget {
  const texture = device.createTexture({
    size: { width, height },
    format: SCENE_DEPTH_FORMAT,
    usage: GPUTextureUsage.RENDER_ATTACHMENT,
  });
  return {
    texture,
    view: texture.createView(),
  };
}
