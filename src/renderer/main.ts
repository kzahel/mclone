import { ResourceLocation } from "../core/resource-location";
import { OverworldBiomeSource } from "../worldgen/biome/overworld-biome-source";
import { NoiseBasedChunkGenerator } from "../worldgen/levelgen/noise-based-chunk-generator";
import { ChunkBlockId } from "../worldgen/chunk/chunk-block-buffer";
import { GeneratedRenderLevel } from "../world/level/generated-render-level";
import { registerGeneratedRenderBlocks } from "../world/level/generated-render-blocks";
import { FoliageColor } from "../world/level/foliage-color";
import { GrassColor } from "../world/level/grass-color";
import { Vec3 } from "../world/phys/vec3";
import { BlockColors } from "./block/block-colors";
import { BlockRenderDispatcher } from "./block/block-render-dispatcher";
import { ChunkRenderDispatcher } from "./chunk/chunk-render-dispatcher";
import { ChunkBufferBuilderPack } from "./chunk-buffer-builder-pack";
import { BlockModelRepository } from "./model/block-model-repository";
import { BlockModelShaper } from "./model/block-model-shaper";
import { preloadBlockModelSource } from "./model/browser-block-model-source";
import { ModelBakery } from "./model/model-bakery";
import { ModelManager } from "./model/model-manager";
import { LevelRenderer, type LevelRenderFrame } from "./level-renderer";
import { LightTexture } from "./light-texture";
import { type CameraState, GameRenderer } from "./game-renderer";
import { BrowserTextureAtlasSource } from "./texture/browser-native-image-loader";
import { TextureAtlas } from "./texture/texture-atlas";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { RenderType } from "./render-type";
import { ViewArea } from "./view-area";

const SMOKE_ATLAS_LOCATION = new ResourceLocation("minecraft:textures/atlas/blocks.png");
const GRASS_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/grass");
const FOLIAGE_COLORMAP_LOCATION = new ResourceLocation("minecraft:colormap/foliage");
const GENERATED_SEED = 12_345n;
const GENERATED_VIEW_DISTANCE = 1;
const GENERATED_CAMERA_PATH = [
  {
    position: new Vec3(-258.5, 70.0, -1004.5),
    xRot: 50.0,
    yRot: 0.0,
  },
  {
    position: new Vec3(-242.5, 70.0, -1004.5),
    xRot: 50.0,
    yRot: 0.0,
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
const DEPTH_FORMAT: GPUTextureFormat = "depth24plus";
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

interface ChunkDrawEntry {
  readonly chunkOffset: readonly [number, number, number];
  readonly vertexBuffer: import("./vertex/vertex-buffer").VertexBuffer;
}

interface ChunkPassResources {
  readonly draw: ChunkDrawEntry;
  readonly bindGroup: GPUBindGroup;
}

interface PassLayer {
  readonly pipeline: GPURenderPipeline;
  readonly draws: readonly ChunkPassResources[];
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

function createDepthView(device: GPUDevice, width: number, height: number): GPUTextureView {
  return device
    .createTexture({
      size: { width, height },
      format: DEPTH_FORMAT,
      usage: GPUTextureUsage.RENDER_ATTACHMENT,
    })
    .createView();
}

function rgba8FromColor(color: readonly [number, number, number, number]): readonly [number, number, number, number] {
  return [
    Math.round(color[0] * 255),
    Math.round(color[1] * 255),
    Math.round(color[2] * 255),
    Math.round(color[3] * 255),
  ];
}

async function initializeBiomeColorTables(atlasSource: BrowserTextureAtlasSource): Promise<void> {
  const [grassPixels, foliagePixels] = await Promise.all([
    atlasSource.loadColorMap(GRASS_COLORMAP_LOCATION),
    atlasSource.loadColorMap(FOLIAGE_COLORMAP_LOCATION),
  ]);
  GrassColor.init(grassPixels);
  FoliageColor.init(foliagePixels);
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

function encodeDrawPass(
  encoder: GPUCommandEncoder,
  view: GPUTextureView,
  depthView: GPUTextureView,
  clearColor: GPUColor,
  layers: readonly PassLayer[],
): void {
  const pass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view,
        clearValue: clearColor,
        loadOp: "clear",
        storeOp: "store",
      },
    ],
    depthStencilAttachment: {
      view: depthView,
      depthClearValue: 1,
      depthLoadOp: "clear",
      depthStoreOp: "store",
    },
  });
  for (const layer of layers) {
    pass.setPipeline(layer.pipeline);
    for (const draw of layer.draws) {
      pass.setBindGroup(0, draw.bindGroup);
      draw.draw.vertexBuffer.draw(pass);
    }
  }
  pass.end();
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

  const biomeSource = new OverworldBiomeSource(GENERATED_SEED);
  const generator = new NoiseBasedChunkGenerator(biomeSource, GENERATED_SEED);
  const level = new GeneratedRenderLevel(generatedBlocks.airState, generator, biomeSource, GENERATED_SEED, generatedBlocks.blockStateById);

  const levelRenderer = new LevelRenderer();
  const blockRenderer = new BlockRenderDispatcher(
    blockModelShaper,
    BlockColors.createDefault(),
    (location) => atlas.getSprite(location),
    generatedBlocks.blockStateById[ChunkBlockId.WATER]!,
    generatedBlocks.blockStateById[ChunkBlockId.LAVA]!,
  );
  const chunkDispatcher = new ChunkRenderDispatcher(level, levelRenderer, blockRenderer, device, (task) => queueMicrotask(task), false, new ChunkBufferBuilderPack());
  const viewArea = new ViewArea(chunkDispatcher, level, GENERATED_VIEW_DISTANCE, levelRenderer);
  levelRenderer.setLevel(level, chunkDispatcher, viewArea, GENERATED_VIEW_DISTANCE);
  const gameRenderer = new GameRenderer(canvas.width, canvas.height, 64);
  const lightTexture = new LightTexture(gameRenderer, level, device);
  lightTexture.tick();

  let frame: LevelRenderFrame | undefined;
  for (const step of GENERATED_CAMERA_PATH) {
    if (level.ensureChunksForCamera(step.position.x, step.position.z, GENERATED_VIEW_DISTANCE)) {
      levelRenderer.allChanged();
    }

    frame = await gameRenderer.renderLevel(
      0.0,
      Number.MAX_SAFE_INTEGER,
      levelRenderer,
      lightTexture,
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

  device.pushErrorScope("validation");
  const pipelineCache = new RenderPipelineCache(device);
  validateShaderPipelines(pipelineCache, format, DEPTH_FORMAT);
  const pipelineError = await popValidationError(device, "WebGPU pipeline validation failed");
  if (pipelineError) {
    return { ok: false, reason: pipelineError };
  }

  const sampler = device.createSampler({
    magFilter: "nearest",
    minFilter: "nearest",
    mipmapFilter: "nearest",
  });
  const atlasTexture = atlas.getTextureView();
  const clearColor: GPUColor = {
    r: frame.fogColor[0],
    g: frame.fogColor[1],
    b: frame.fogColor[2],
    a: frame.fogColor[3],
  };
  const renderOrder = [
    RenderType.solid(),
    RenderType.cutoutMipped(),
    RenderType.cutout(),
    RenderType.translucent(),
    RenderType.tripwire(),
  ] as const;
  const canvasLayers: PassLayer[] = [];
  const readbackLayers: PassLayer[] = [];
  for (const renderType of renderOrder) {
    const drawEntries = frame.layerDraws.get(renderType);
    if (drawEntries === undefined || drawEntries.length === 0) {
      continue;
    }

    const canvasPipeline = pipelineCache.getOrCreate(renderType, format, DEPTH_FORMAT);
    const readbackPipeline = pipelineCache.getOrCreate(renderType, READBACK_FORMAT, DEPTH_FORMAT);
    for (const draw of drawEntries) {
      if (draw.vertexBuffer.getFormat() !== renderType.format()) {
        return { ok: false, reason: "render type format does not match a compiled chunk vertex format" };
      }
    }

    canvasLayers.push({
      pipeline: canvasPipeline.pipeline,
      draws: drawEntries.map((draw) => ({
        draw,
        bindGroup: createChunkBindGroup(
          device,
          canvasPipeline,
          sampler,
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
    readbackLayers.push({
      pipeline: readbackPipeline.pipeline,
      draws: drawEntries.map((draw) => ({
        draw,
        bindGroup: createChunkBindGroup(
          device,
          readbackPipeline,
          sampler,
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
  const canvasDepthView = createDepthView(device, canvas.width, canvas.height);
  const readbackDepthView = createDepthView(device, canvas.width, canvas.height);
  const readbackTexture = device.createTexture({
    size: { width: canvas.width, height: canvas.height },
    format: READBACK_FORMAT,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });

  device.pushErrorScope("validation");
  const encoder = device.createCommandEncoder();
  encodeDrawPass(encoder, ctx.getCurrentTexture().createView(), canvasDepthView, clearColor, canvasLayers);
  encodeDrawPass(encoder, readbackTexture.createView(), readbackDepthView, clearColor, readbackLayers);
  device.queue.submit([encoder.finish()]);
  await device.queue.onSubmittedWorkDone();
  const submissionError = await popValidationError(device, "WebGPU submission validation failed");
  if (submissionError) {
    return { ok: false, reason: submissionError };
  }

  const centerPixel = await readPixel(device, readbackTexture, Math.floor(canvas.width / 2), Math.floor(canvas.height / 2));
  const terrainPixel = await readPixel(device, readbackTexture, Math.floor(canvas.width / 2), Math.floor((canvas.height * 3) / 4));
  const clearPixel = rgba8FromColor(frame.fogColor);

  const info = adapter.info ?? {};
  const adapterInfo = [info.vendor, info.architecture, info.device, info.description]
    .filter(Boolean)
    .join(" / ") || "unknown";

  return {
    ok: true,
    format,
    adapterInfo,
    centerPixel,
    terrainPixel,
    clearPixel,
    loadedChunkCount: level.getLoadedChunkCount(),
    solidDrawCount: solidDraws.length,
    cutoutDrawCount: cutoutDraws.length,
    translucentDrawCount: translucentDraws.length,
  };
}

if (typeof window !== "undefined") {
  window.__mcloneReady = boot();
}
