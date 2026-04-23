import { BlockPos } from "../core/block-pos";
import { Registry } from "../core/registry";
import { ResourceLocation } from "../core/resource-location";
import { AirBlock } from "../world/level/block/air-block";
import { Block } from "../world/level/block/block";
import { BlockBehaviour } from "../world/level/block/state/block-behaviour";
import type { BlockState } from "../world/level/block/state/block-state";
import { Material as BlockMaterial } from "../world/level/material/material";
import { StaticRenderLevel } from "../world/level/static-render-level";
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
import { GameRenderer } from "./game-renderer";
import { LevelRenderer } from "./level-renderer";
import { LightTexture } from "./light-texture";
import { BrowserTextureAtlasSource, loadNativeImageFromUrl } from "./texture/browser-native-image-loader";
import { NativeImage } from "./texture/native-image";
import { TextureAtlas } from "./texture/texture-atlas";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { RenderType } from "./render-type";
import { ViewArea } from "./view-area";

const SMOKE_ATLAS_LOCATION = new ResourceLocation("minecraft:textures/atlas/blocks.png");
const SMOKE_CENTER_BLOCK = new ResourceLocation("minecraft:orange_wool");
const SMOKE_VISUAL_BLOCK = new ResourceLocation("minecraft:stone");
const SMOKE_CENTER_SPRITE = new ResourceLocation("minecraft:block/orange_wool");
const SMOKE_VISUAL_SPRITE = new ResourceLocation("minecraft:block/stone");
const SMOKE_BLOCKS = [SMOKE_CENTER_BLOCK, SMOKE_VISUAL_BLOCK] as const;
const SMOKE_SPRITES = [SMOKE_CENTER_SPRITE, SMOKE_VISUAL_SPRITE] as const;
const SMOKE_CENTER_BRIGHTNESS = 0.8;
const SMOKE_RENDER_DISTANCE = 1;
const SMOKE_CAMERA_POSITION = new Vec3(8.5, 8.5, 32);
const SMOKE_CAMERA_X_ROT = 0.0;
const SMOKE_CAMERA_Y_ROT = 180.0;

export type BootResult =
  | {
      ok: true;
      format: GPUTextureFormat;
      adapterInfo: string;
      centerPixel: readonly [number, number, number, number];
      expectedCenterPixel: readonly [number, number, number, number];
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

function rgba8FromPixel(pixel: number): readonly [number, number, number, number] {
  return [NativeImage.getR(pixel), NativeImage.getG(pixel), NativeImage.getB(pixel), NativeImage.getA(pixel)];
}

function shadePixel(
  pixel: readonly [number, number, number, number],
  brightness: number,
): readonly [number, number, number, number] {
  return [
    Math.round(pixel[0] * brightness),
    Math.round(pixel[1] * brightness),
    Math.round(pixel[2] * brightness),
    pixel[3],
  ];
}

function modulatePixel(
  pixel: readonly [number, number, number, number],
  multiplier: readonly [number, number, number, number],
): readonly [number, number, number, number] {
  return [
    Math.round((pixel[0] * multiplier[0]) / 255),
    Math.round((pixel[1] * multiplier[1]) / 255),
    Math.round((pixel[2] * multiplier[2]) / 255),
    Math.round((pixel[3] * multiplier[3]) / 255),
  ];
}

function createAirState(): BlockState {
  const properties = BlockBehaviour.Properties.of(BlockMaterial.AIR).noCollission().noOcclusion();
  properties.isAir = true;
  return new AirBlock(properties).defaultBlockState();
}

function fillBox(level: StaticRenderLevel, from: BlockPos, to: BlockPos, state: BlockState): void {
  for (let z = from.getZ(); z <= to.getZ(); z++) {
    for (let y = from.getY(); y <= to.getY(); y++) {
      for (let x = from.getX(); x <= to.getX(); x++) {
        level.setBlock(new BlockPos(x, y, z), state);
      }
    }
  }
}

function populateSmokeLevel(
  level: StaticRenderLevel,
  centerState: BlockState,
  visualState: BlockState,
): void {
  fillBox(level, new BlockPos(0, 0, 0), new BlockPos(15, 15, 0), centerState);
  fillBox(level, new BlockPos(-12, 0, 0), new BlockPos(-5, 7, 7), visualState);
  fillBox(level, new BlockPos(20, 2, 0), new BlockPos(27, 9, 5), visualState);
  for (let chunkX = -2; chunkX <= 2; chunkX++) {
    for (let chunkZ = -1; chunkZ <= 2; chunkZ++) {
      level.getChunk(chunkX, chunkZ);
    }
  }
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

async function readCenterPixel(
  device: GPUDevice,
  texture: GPUTexture,
  width: number,
  height: number,
): Promise<readonly [number, number, number, number]> {
  const readback = device.createBuffer({
    size: 256,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });
  const encoder = device.createCommandEncoder();
  encoder.copyTextureToBuffer(
    {
      texture,
      origin: { x: Math.floor(width / 2), y: Math.floor(height / 2) },
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

  const atlasSource = new BrowserTextureAtlasSource();
  const centerSpriteImage = await loadNativeImageFromUrl(atlasSource.resolveTextureUrl(SMOKE_CENTER_SPRITE));
  const centerSpritePixel = rgba8FromPixel(centerSpriteImage.getPixelRGBA(8, 8));
  centerSpriteImage.close();

  const atlas = new TextureAtlas(SMOKE_ATLAS_LOCATION, device.limits.maxTextureDimension2D);
  const preparations = await atlas.prepareToStitch(atlasSource, SMOKE_SPRITES, 0);
  atlas.reload(device, preparations);

  const centerSprite = atlas.getSprite(SMOKE_CENTER_SPRITE);
  const visualSprite = atlas.getSprite(SMOKE_VISUAL_SPRITE);
  if (!centerSprite.getName().equals(SMOKE_CENTER_SPRITE)) {
    return { ok: false, reason: `center smoke sprite ${SMOKE_CENTER_SPRITE} was missing from the stitched atlas` };
  }

  if (!visualSprite.getName().equals(SMOKE_VISUAL_SPRITE)) {
    return { ok: false, reason: `visual smoke sprite ${SMOKE_VISUAL_SPRITE} was missing from the stitched atlas` };
  }

  Registry.BLOCK.clear();
  const centerBlock = new Block(BlockBehaviour.Properties.of(BlockMaterial.WOOL)).setLocation(SMOKE_CENTER_BLOCK);
  const visualBlock = new Block(BlockBehaviour.Properties.of(BlockMaterial.STONE)).setLocation(SMOKE_VISUAL_BLOCK);
  Registry.register(Registry.BLOCK, centerBlock.getLocation()!, centerBlock);
  Registry.register(Registry.BLOCK, visualBlock.getLocation()!, visualBlock);

  const modelSource = await preloadBlockModelSource(SMOKE_BLOCKS);
  const repository = new BlockModelRepository(modelSource);
  const bakery = new ModelBakery(repository, (material) => atlas.getSprite(material.texture()));
  const modelManager = new ModelManager(bakery.getMissingBakedModel());
  bakery.bakeTopLevelBlockModels(modelManager);
  const blockModelShaper = new BlockModelShaper(modelManager);
  blockModelShaper.rebuildCache();

  const centerState = centerBlock.defaultBlockState();
  const visualState = visualBlock.defaultBlockState();
  const level = new StaticRenderLevel(createAirState());
  populateSmokeLevel(level, centerState, visualState);

  const levelRenderer = new LevelRenderer();
  const blockRenderer = new BlockRenderDispatcher(blockModelShaper, new BlockColors());
  const chunkDispatcher = new ChunkRenderDispatcher(level, levelRenderer, blockRenderer, device, (task) => queueMicrotask(task), false, new ChunkBufferBuilderPack());
  const viewArea = new ViewArea(chunkDispatcher, level, SMOKE_RENDER_DISTANCE, levelRenderer);
  levelRenderer.setLevel(level, chunkDispatcher, viewArea, SMOKE_RENDER_DISTANCE);
  const gameRenderer = new GameRenderer(canvas.width, canvas.height, 64);
  const lightTexture = new LightTexture(gameRenderer, level, device);
  lightTexture.tick();
  const frame = await gameRenderer.renderLevel(0.0, Number.MAX_SAFE_INTEGER, levelRenderer, lightTexture, {
    position: SMOKE_CAMERA_POSITION,
    xRot: SMOKE_CAMERA_X_ROT,
    yRot: SMOKE_CAMERA_Y_ROT,
  });
  const lightmapPixel = rgba8FromPixel(lightTexture.samplePacked(LightTexture.FULL_BRIGHT));
  const expectedCenterPixel = modulatePixel(shadePixel(centerSpritePixel, SMOKE_CENTER_BRIGHTNESS), lightmapPixel);
  const solidDraws = frame.layerDraws.get(RenderType.solid()) ?? [];
  if (solidDraws.length === 0) {
    return { ok: false, reason: "camera-driven level renderer produced no solid drawables for the smoke scene" };
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

  const centerPixel = await readCenterPixel(device, readbackTexture, canvas.width, canvas.height);

  const info = adapter.info ?? {};
  const adapterInfo = [info.vendor, info.architecture, info.device, info.description]
    .filter(Boolean)
    .join(" / ") || "unknown";

  return { ok: true, format, adapterInfo, centerPixel, expectedCenterPixel };
}

if (typeof window !== "undefined") {
  window.__mcloneReady = boot();
}
