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
import { Matrix4f } from "./math/matrix4f";
import { BlockModelRepository } from "./model/block-model-repository";
import { BlockModelShaper } from "./model/block-model-shaper";
import { preloadBlockModelSource } from "./model/browser-block-model-source";
import { ModelBakery } from "./model/model-bakery";
import { ModelManager } from "./model/model-manager";
import { LevelRenderer } from "./level-renderer";
import { BrowserTextureAtlasSource, loadNativeImageFromUrl } from "./texture/browser-native-image-loader";
import { NativeImage } from "./texture/native-image";
import { TextureAtlas } from "./texture/texture-atlas";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { RenderType } from "./render-type";
import { VertexBuffer } from "./vertex/vertex-buffer";
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
const SMOKE_CAMERA_POSITION = new Vec3(8.5, 8.5, 0);

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

const CLEAR_COLOR: GPUColor = { r: 0, g: 128 / 255, b: 0, a: 1 };
const READBACK_FORMAT: GPUTextureFormat = "rgba8unorm";
const DEPTH_FORMAT: GPUTextureFormat = "depth24plus";
const WHITE_PIXEL = new Uint8Array([255, 255, 255, 255]);
const IDENTITY_MATRIX = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] as const;
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
  readonly vertexBuffer: VertexBuffer;
  readonly chunkOffset: readonly [number, number, number];
}

interface ChunkPassResources {
  readonly draw: ChunkDrawEntry;
  readonly bindGroup: GPUBindGroup;
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

function createWhiteTexture(device: GPUDevice): GPUTextureView {
  const texture = device.createTexture({
    size: { width: 1, height: 1 },
    format: "rgba8unorm",
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
  });
  device.queue.writeTexture({ texture }, WHITE_PIXEL, { bytesPerRow: 256 }, { width: 1, height: 1 });
  return texture.createView();
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
}

function createSmokeModelViewMatrix(): Float32Array {
  const matrix = new Matrix4f();
  matrix.setIdentity();
  matrix.m00 = 1 / 24;
  matrix.m11 = 1 / 12;
  matrix.m22 = 1 / 32;
  matrix.m03 = -8.5 / 24;
  matrix.m13 = -8.5 / 12;
  return matrix.toFloat32Array();
}

async function compileScene(
  dispatcher: ChunkRenderDispatcher,
  viewArea: ViewArea,
  renderType: RenderType,
): Promise<readonly ChunkDrawEntry[]> {
  dispatcher.setCamera(SMOKE_CAMERA_POSITION);
  viewArea.repositionCamera(SMOKE_CAMERA_POSITION.x, SMOKE_CAMERA_POSITION.z);
  for (const chunk of viewArea.chunks) {
    chunk.rebuildChunkAsync(dispatcher);
  }

  await dispatcher.awaitAllTasks();
  const draws: ChunkDrawEntry[] = [];
  for (const chunk of viewArea.chunks) {
    const compiledChunk = chunk.getCompiledChunk();
    if (compiledChunk.isEmpty(renderType)) {
      continue;
    }

    draws.push({
      vertexBuffer: chunk.getBuffer(renderType),
      chunkOffset: [chunk.getOrigin().getX(), chunk.getOrigin().getY(), chunk.getOrigin().getZ()],
    });
  }

  return draws;
}

function createChunkBindGroup(
  device: GPUDevice,
  pipeline: import("./pipeline/render-pipeline-cache").CachedRenderPipeline,
  sampler: GPUSampler,
  atlasTexture: GPUTextureView,
  whiteTexture: GPUTextureView,
  modelViewMat: Float32Array,
  chunkOffset: readonly [number, number, number],
): GPUBindGroup {
  const uniformBytes = pipeline.shaderProgram.createUniformBufferBytes({
    ModelViewMat: Array.from(modelViewMat),
    ProjMat: Array.from(IDENTITY_MATRIX),
    ChunkOffset: chunkOffset,
    ColorModulator: [1, 1, 1, 1],
    FogStart: [1_000_000],
    FogEnd: [1_000_001],
    FogColor: [0, 0, 0, 0],
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
      Sampler2: whiteTexture,
    },
  });
}

function encodeDrawPass(
  encoder: GPUCommandEncoder,
  view: GPUTextureView,
  depthView: GPUTextureView,
  pipeline: GPURenderPipeline,
  draws: readonly ChunkPassResources[],
): void {
  const pass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view,
        clearValue: CLEAR_COLOR,
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
  pass.setPipeline(pipeline);
  for (const draw of draws) {
    pass.setBindGroup(0, draw.bindGroup);
    draw.draw.vertexBuffer.draw(pass);
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
  const expectedCenterPixel = shadePixel(rgba8FromPixel(centerSpriteImage.getPixelRGBA(8, 8)), SMOKE_CENTER_BRIGHTNESS);
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

  const renderType = RenderType.solid();
  const levelRenderer = new LevelRenderer();
  const blockRenderer = new BlockRenderDispatcher(blockModelShaper, new BlockColors());
  const chunkDispatcher = new ChunkRenderDispatcher(level, levelRenderer, blockRenderer, device, (task) => queueMicrotask(task), false, new ChunkBufferBuilderPack());
  const viewArea = new ViewArea(chunkDispatcher, level, SMOKE_RENDER_DISTANCE, levelRenderer);
  const draws = await compileScene(chunkDispatcher, viewArea, renderType);
  if (draws.length === 0) {
    return { ok: false, reason: "chunk dispatcher compiled no drawables for the smoke scene" };
  }

  for (const draw of draws) {
    if (draw.vertexBuffer.getFormat() !== renderType.format()) {
      return { ok: false, reason: "render type format does not match a compiled chunk vertex format" };
    }
  }

  device.pushErrorScope("validation");
  const pipelineCache = new RenderPipelineCache(device);
  validateShaderPipelines(pipelineCache, format, DEPTH_FORMAT);
  const canvasPipeline = pipelineCache.getOrCreate(renderType, format, DEPTH_FORMAT);
  const readbackPipeline = pipelineCache.getOrCreate(renderType, READBACK_FORMAT, DEPTH_FORMAT);
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
  const whiteTexture = createWhiteTexture(device);
  const modelViewMat = createSmokeModelViewMatrix();
  const canvasDraws = draws.map((draw) => ({
    draw,
    bindGroup: createChunkBindGroup(device, canvasPipeline, sampler, atlasTexture, whiteTexture, modelViewMat, draw.chunkOffset),
  }));
  const readbackDraws = draws.map((draw) => ({
    draw,
    bindGroup: createChunkBindGroup(device, readbackPipeline, sampler, atlasTexture, whiteTexture, modelViewMat, draw.chunkOffset),
  }));
  const canvasDepthView = createDepthView(device, canvas.width, canvas.height);
  const readbackDepthView = createDepthView(device, canvas.width, canvas.height);
  const readbackTexture = device.createTexture({
    size: { width: canvas.width, height: canvas.height },
    format: READBACK_FORMAT,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });

  device.pushErrorScope("validation");
  const encoder = device.createCommandEncoder();
  encodeDrawPass(encoder, ctx.getCurrentTexture().createView(), canvasDepthView, canvasPipeline.pipeline, canvasDraws);
  encodeDrawPass(encoder, readbackTexture.createView(), readbackDepthView, readbackPipeline.pipeline, readbackDraws);
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
