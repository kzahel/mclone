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
import { TextureAtlas } from "./texture/texture-atlas";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { RenderType } from "./render-type";
import { ViewArea } from "./view-area";
import { Vec3 } from "../world/phys/vec3";

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
  readonly sampler: GPUSampler;
  readonly viewDistance: number;
}

export type SceneInitResult =
  | { ok: true; scene: RendererScene }
  | { ok: false; reason: string };

export interface DrawTarget {
  readonly view: GPUTextureView;
  readonly depthView: GPUTextureView;
  readonly format: GPUTextureFormat;
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

export async function initializeRendererScene(
  canvas: HTMLCanvasElement,
  options: SceneInitOptions,
): Promise<SceneInitResult> {
  if (!navigator.gpu) return { ok: false, reason: "navigator.gpu missing (no WebGPU)" };

  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) return { ok: false, reason: "requestAdapter returned null" };

  const device = await adapter.requestDevice();
  const ctx = canvas.getContext("webgpu");
  if (!ctx) return { ok: false, reason: "canvas.getContext('webgpu') returned null" };

  const format = navigator.gpu.getPreferredCanvasFormat();
  ctx.configure({ device, format, alphaMode: "opaque" });

  const generatedBlocks = registerGeneratedRenderBlocks();
  const atlasSource = new BrowserTextureAtlasSource();
  await initializeBiomeColorTables(atlasSource);
  const atlas = new TextureAtlas(SMOKE_ATLAS_LOCATION, device.limits.maxTextureDimension2D);
  const preparations = await atlas.prepareToStitch(atlasSource, generatedBlocks.spriteLocations, 0);
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
  const sampler = device.createSampler({
    magFilter: "nearest",
    minFilter: "nearest",
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
      sampler,
      viewDistance: options.viewDistance,
    },
  };
}

interface ChunkDrawEntry {
  readonly draw: import("./level-renderer").ChunkLayerDraw;
  readonly bindGroup: GPUBindGroup;
}

interface PassLayer {
  readonly pipeline: GPURenderPipeline;
  readonly draws: readonly ChunkDrawEntry[];
}

function uploadBuffer(device: GPUDevice, bytes: Uint8Array, usage: GPUBufferUsageFlags): GPUBuffer {
  const buffer = device.createBuffer({
    size: Math.max(4, Math.ceil(bytes.byteLength / 4) * 4),
    usage,
    mappedAtCreation: true,
  });
  new Uint8Array(buffer.getMappedRange()).set(bytes);
  buffer.unmap();
  return buffer;
}

function createChunkBindGroup(
  device: GPUDevice,
  pipeline: import("./pipeline/render-pipeline-cache").CachedRenderPipeline,
  sampler: GPUSampler,
  atlasTexture: GPUTextureView,
  lightTexture: GPUTextureView,
  modelViewMat: Float32Array,
  projectionMat: Float32Array,
  chunkOffset: readonly [number, number, number],
  fogStart: number,
  fogEnd: number,
  fogColor: readonly [number, number, number, number],
): GPUBindGroup {
  const uniformBytes = pipeline.shaderProgram.createUniformBufferBytes({
    ModelViewMat: Array.from(modelViewMat),
    ProjMat: Array.from(projectionMat),
    ChunkOffset: chunkOffset,
    ColorModulator: [1, 1, 1, 1],
    FogStart: [fogStart],
    FogEnd: [fogEnd],
    FogColor: fogColor,
  });
  const uniformBuffer = uploadBuffer(device, uniformBytes, GPUBufferUsage.UNIFORM);
  return pipeline.shaderProgram.createBindGroup(device, pipeline.bindGroupLayout, {
    uniformBuffer,
    samplers: {
      Sampler0: sampler,
      Sampler2: sampler,
    },
    textures: {
      Sampler0: atlasTexture,
      Sampler2: lightTexture,
    },
  });
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
      draws: drawEntries.map((draw) => ({
        draw,
        bindGroup: createChunkBindGroup(
          scene.device,
          pipeline,
          scene.sampler,
          atlasTexture,
          frame.lightTexture,
          frame.modelViewMatrix,
          frame.projectionMatrix,
          draw.chunkOffset,
          frame.fogStart,
          frame.fogEnd,
          frame.fogColor,
        ),
      })),
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
  return device
    .createTexture({
      size: { width, height },
      format: SCENE_DEPTH_FORMAT,
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    })
    .createView();
}
