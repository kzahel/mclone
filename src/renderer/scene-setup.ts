import { ResourceLocation } from "../core/resource-location";
import type { OpenWorldPreset } from "../runtime/protocol/world-messages";
import type { WorldClient } from "../runtime/protocol/world-client";
import type { WorldSaveMetadata } from "../runtime/storage/world-storage";
import { RemoteWorldClient, RemoteWorldTransport } from "../runtime/transport/remote-world-transport";
import { TransportWorldClient } from "../runtime/transport/local-world-transport";
import { WorkerWorldTransport, createGeneratedWorldWorker } from "../runtime/transport/worker-world-transport";
import { OverworldBiomeSource } from "../worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../worldgen/chunk/chunk-block-buffer";
import { ClientChunkCache } from "../world/level/client-chunk-cache";
import { createBlockStateResolver } from "../world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../world/level/generated-render-blocks";
import { FoliageColor } from "../world/level/foliage-color";
import { GrassColor } from "../world/level/grass-color";
import { BlockColors } from "./block/block-colors";
import { BlockRenderDispatcher } from "./block/block-render-dispatcher";
import { ChunkRenderDispatcher } from "./chunk/chunk-render-dispatcher";
import { ChunkMeshWorkerClient, createChunkMeshWorker } from "./chunk/mesh-worker-client";
import { ChunkBufferBuilderPack } from "./chunk-buffer-builder-pack";
import { BlockModelRepository } from "./model/block-model-repository";
import { BlockModelShaper } from "./model/block-model-shaper";
import { preloadBlockModelSource } from "./model/browser-block-model-source";
import { ModelBakery } from "./model/model-bakery";
import { ModelManager } from "./model/model-manager";
import { LevelRenderer, type LevelRenderFrame } from "./level-renderer";
import { LightTexture } from "./light-texture";
import { GameRenderer } from "./game-renderer";
import { BrowserTextureAtlasSource } from "./texture/browser-native-image-loader";
import { DEFAULT_BLOCK_ATLAS_MIP_LEVEL, TextureAtlas } from "./texture/texture-atlas";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { type CompositeRenderType, RenderType } from "./render-type";
import { ViewArea } from "./view-area";
import { Vec3 } from "../world/phys/vec3";
import type { VertexBuffer } from "./vertex/vertex-buffer";

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
  readonly fov?: number;
  readonly worldTransport?: "worker" | "remote";
  readonly remoteWorldHostUrl?: string;
  readonly preset?: OpenWorldPreset;
  readonly skyColor?: Vec3;
  readonly clearColorScale?: number;
}

export interface RendererScene {
  readonly adapter: GPUAdapter;
  readonly device: GPUDevice;
  readonly format: GPUTextureFormat;
  readonly canvas: HTMLCanvasElement;
  readonly ctx: GPUCanvasContext;
  readonly saveMetadata: WorldSaveMetadata;
  readonly atlas: TextureAtlas;
  readonly worldClient: WorldClient;
  readonly level: ClientChunkCache;
  readonly levelRenderer: LevelRenderer;
  readonly gameRenderer: GameRenderer;
  readonly lightTexture: LightTexture;
  readonly pipelineCache: RenderPipelineCache;
  readonly textureSamplers: ReadonlyMap<string, GPUSampler>;
  readonly lightSampler: GPUSampler;
  readonly viewDistance: number;
  readonly chunkDrawResources: WeakMap<VertexBuffer, Map<GPUBindGroupLayout, CachedChunkDrawResources>>;
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
): WorldClient {
  const levelFactory = (worldOpened: import("../runtime/protocol/world-messages").WorldOpenedMessage) => new ClientChunkCache({
    airState,
    minBuildHeight: worldOpened.minBuildHeight,
    height: worldOpened.height,
    biomeSource,
    biomeZoomSeed: options.seed,
    blockStateResolver,
    skyColor: options.skyColor,
    clearColorScale: options.clearColorScale,
  });

  if (options.worldTransport === "remote") {
    return new RemoteWorldClient(
      new RemoteWorldTransport(options.remoteWorldHostUrl ?? "http://127.0.0.1:4173"),
      levelFactory,
    );
  }

  return new TransportWorldClient(
    new WorkerWorldTransport(createGeneratedWorldWorker()),
    levelFactory,
  );
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

export async function initializeRendererScene(
  canvas: HTMLCanvasElement,
  options: SceneInitOptions,
): Promise<SceneInitResult> {
  if (!navigator.gpu) return { ok: false, reason: "navigator.gpu missing (no WebGPU)" };

  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) return { ok: false, reason: "requestAdapter returned null" };

  const device = await adapter.requestDevice();
  resizeCanvasToDisplaySize(canvas, device.limits.maxTextureDimension2D);
  const ctx = canvas.getContext("webgpu");
  if (!ctx) return { ok: false, reason: "canvas.getContext('webgpu') returned null" };

  const format = navigator.gpu.getPreferredCanvasFormat();
  ctx.configure({ device, format, alphaMode: "opaque" });

  const generatedBlocks = registerGeneratedRenderBlocks();
  const atlasSource = new BrowserTextureAtlasSource();
  await initializeBiomeColorTables(atlasSource);
  const atlas = new TextureAtlas(SMOKE_ATLAS_LOCATION, device.limits.maxTextureDimension2D);
  const preparations = await atlas.prepareToStitch(atlasSource, generatedBlocks.spriteLocations, DEFAULT_BLOCK_ATLAS_MIP_LEVEL);
  atlas.reload(device, preparations);

  const modelSource = await preloadBlockModelSource(generatedBlocks.blockLocations);
  const repository = new BlockModelRepository(modelSource);
  const bakery = new ModelBakery(repository, (material) => atlas.getSprite(material.texture()));
  const modelManager = new ModelManager(bakery.getMissingBakedModel());
  bakery.bakeTopLevelBlockModels(modelManager);
  const blockModelShaper = new BlockModelShaper(modelManager);
  blockModelShaper.rebuildCache();

  const biomeSource = new OverworldBiomeSource(options.seed);
  const blockStateResolver = createBlockStateResolver(generatedBlocks.airState);
  const worldClient = createWorldClient(options, generatedBlocks.airState, biomeSource, blockStateResolver);
  const worldOpened = await worldClient.openWorld({
    type: "open_world",
    seed: options.seed,
    preset: options.preset ?? "browser_smoke",
  });
  const level = worldClient.getLevel();
  const meshWorker = new ChunkMeshWorkerClient(createChunkMeshWorker());
  await meshWorker.initialize({
    type: "initialize_mesh_worker",
    seed: options.seed,
    minBuildHeight: worldOpened.minBuildHeight,
    height: worldOpened.height,
  });

  const levelRenderer = new LevelRenderer();
  const blockRenderer = new BlockRenderDispatcher(
    blockModelShaper,
    BlockColors.createDefault(),
    (location) => atlas.getSprite(location),
    generatedBlocks.blockStateById[ChunkBlockId.WATER]!,
    generatedBlocks.blockStateById[ChunkBlockId.LAVA]!,
  );
  const chunkDispatcher = new ChunkRenderDispatcher(
    level,
    levelRenderer,
    blockRenderer,
    device,
    (task) => queueMicrotask(task),
    false,
    new ChunkBufferBuilderPack(),
    meshWorker,
  );
  const viewArea = new ViewArea(chunkDispatcher, level, options.viewDistance, levelRenderer);
  levelRenderer.setLevel(level, chunkDispatcher, viewArea, options.viewDistance);

  const gameRenderer = new GameRenderer(
    canvas.width,
    canvas.height,
    options.renderDistance ?? Math.max(32, (options.viewDistance + 2) * 16),
    options.fov,
  );
  const lightTexture = new LightTexture(gameRenderer, level, device);
  lightTexture.tick();

  const pipelineCache = new RenderPipelineCache(device);
  const textureSamplers = createTextureSamplers(device);
  const lightSampler = device.createSampler({
    magFilter: "linear",
    minFilter: "linear",
    mipmapFilter: "nearest",
  });

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
      worldClient,
      level,
      levelRenderer,
      gameRenderer,
      lightTexture,
      pipelineCache,
      textureSamplers,
      lightSampler,
      viewDistance: options.viewDistance,
      chunkDrawResources: new WeakMap<VertexBuffer, Map<GPUBindGroupLayout, CachedChunkDrawResources>>(),
    },
  };
}

interface ChunkDrawEntry {
  readonly draw: import("./level-renderer").ChunkLayerDraw;
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
  readonly draws: readonly ChunkDrawEntry[];
}

const COLOR_MODULATOR = [1, 1, 1, 1] as const;

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
): ChunkDrawEntry {
  return {
    draw,
    bindGroup: createChunkBindGroup(scene, pipeline, frame, draw, renderType, atlasTexture),
  };
}

export function encodeSceneFrame(
  scene: RendererScene,
  frame: LevelRenderFrame,
  target: DrawTarget,
  encoder: GPUCommandEncoder,
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
      entry.draw.vertexBuffer.draw(pass);
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
