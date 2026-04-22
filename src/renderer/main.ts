import { BlockPos } from "../core/block-pos";
import { Registry } from "../core/registry";
import { ResourceLocation } from "../core/resource-location";
import { Block } from "../world/level/block/block";
import { StaticBlockAndTintGetter } from "../world/level/static-block-and-tint-getter";
import { BlockBehaviour } from "../world/level/block/state/block-behaviour";
import type { BlockState } from "../world/level/block/state/block-state";
import { Material as BlockMaterial } from "../world/level/material/material";
import { Vector3f } from "./math/vector3f";
import { BlockColors } from "./block/block-colors";
import { BlockRenderDispatcher } from "./block/block-render-dispatcher";
import { ModelBlockRenderer } from "./block/model-block-renderer";
import { BlockModelRepository } from "./model/block-model-repository";
import { BlockModelShaper } from "./model/block-model-shaper";
import { preloadBlockModelSource } from "./model/browser-block-model-source";
import { ModelBakery } from "./model/model-bakery";
import { ModelManager } from "./model/model-manager";
import { BrowserTextureAtlasSource, loadNativeImageFromUrl } from "./texture/browser-native-image-loader";
import { NativeImage } from "./texture/native-image";
import { TextureAtlas } from "./texture/texture-atlas";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { RenderType } from "./render-type";
import { PoseStack } from "./vertex/pose-stack";
import { BufferBuilder } from "./vertex/buffer-builder";
import { DefaultVertexFormat } from "./vertex/default-vertex-format";
import { VertexBuffer } from "./vertex/vertex-buffer";
import { VertexFormat } from "./vertex/vertex-format";

const SMOKE_ATLAS_LOCATION = new ResourceLocation("minecraft:textures/atlas/blocks.png");
const SMOKE_CENTER_BLOCK = new ResourceLocation("minecraft:orange_wool");
const SMOKE_VISUAL_BLOCK = new ResourceLocation("minecraft:stone");
const SMOKE_CENTER_SPRITE = new ResourceLocation("minecraft:block/orange_wool");
const SMOKE_VISUAL_SPRITE = new ResourceLocation("minecraft:block/stone");
const SMOKE_BLOCKS = [SMOKE_CENTER_BLOCK, SMOKE_VISUAL_BLOCK] as const;
const SMOKE_SPRITES = [SMOKE_CENTER_SPRITE, SMOKE_VISUAL_SPRITE] as const;
const SMOKE_CENTER_POS = new BlockPos(0, 0, 0);
const SMOKE_VISUAL_POS = new BlockPos(2, 0, 0);
const SMOKE_CENTER_BRIGHTNESS = 0.8;

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
  return new Block(properties).defaultBlockState();
}

function renderSmokeBlock(
  dispatcher: BlockRenderDispatcher,
  level: StaticBlockAndTintGetter,
  state: BlockState,
  pos: BlockPos,
  builder: BufferBuilder,
  transform: (poseStack: PoseStack) => void,
): void {
  const poseStack = new PoseStack();
  poseStack.pushPose();
  transform(poseStack);
  if (!dispatcher.renderBatched(state, pos, level, poseStack, builder, true)) {
    throw new Error(`Smoke block ${state.getBlock()} at ${pos} did not render`);
  }

  poseStack.popPose();
}

function buildScene(
  device: GPUDevice,
  dispatcher: BlockRenderDispatcher,
  level: StaticBlockAndTintGetter,
  centerState: BlockState,
  visualState: BlockState,
): VertexBuffer {
  const builder = new BufferBuilder(256);
  builder.begin(VertexFormat.Mode.QUADS, DefaultVertexFormat.BLOCK);
  ModelBlockRenderer.enableCaching();
  try {
    renderSmokeBlock(dispatcher, level, centerState, SMOKE_CENTER_POS, builder, (poseStack) => {
      poseStack.translate(-0.4, -0.4, 0.1);
      poseStack.scale(0.8, 0.8, 0.8);
    });
    renderSmokeBlock(dispatcher, level, visualState, SMOKE_VISUAL_POS, builder, (poseStack) => {
      poseStack.translate(-0.9, -0.8, 0.2);
      poseStack.scale(0.35, 0.35, 0.35);
      poseStack.translate(0.5, 0.5, 0.5);
      poseStack.mulPose(Vector3f.YP.rotationDegrees(-35));
      poseStack.mulPose(Vector3f.XP.rotationDegrees(25));
      poseStack.translate(-0.5, -0.5, -0.5);
    });
  } finally {
    ModelBlockRenderer.clearCache();
  }

  builder.end();

  const vertexBuffer = new VertexBuffer(device);
  vertexBuffer.upload(builder);
  return vertexBuffer;
}

function encodeDrawPass(
  encoder: GPUCommandEncoder,
  view: GPUTextureView,
  depthView: GPUTextureView,
  pipeline: GPURenderPipeline,
  bindGroup: GPUBindGroup,
  vertexBuffer: VertexBuffer,
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
  pass.setBindGroup(0, bindGroup);
  vertexBuffer.draw(pass);
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
  const level = new StaticBlockAndTintGetter(createAirState());
  level.setBlock(SMOKE_CENTER_POS, centerState);
  level.setBlock(SMOKE_VISUAL_POS, visualState);

  const vertexBuffer = buildScene(device, new BlockRenderDispatcher(blockModelShaper, new BlockColors()), level, centerState, visualState);
  const vertexFormat = vertexBuffer.getFormat();
  if (!vertexFormat) return { ok: false, reason: "vertex buffer format missing after upload" };

  const renderType = RenderType.solid();
  if (renderType.format() !== vertexFormat) {
    return { ok: false, reason: "render type format does not match the uploaded vertex format" };
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
  const uniformBytes = canvasPipeline.shaderProgram.createUniformBufferBytes({
    ModelViewMat: IDENTITY_MATRIX,
    ProjMat: IDENTITY_MATRIX,
    ChunkOffset: [0, 0, 0],
    ColorModulator: [1, 1, 1, 1],
    FogStart: [1_000_000],
    FogEnd: [1_000_001],
    FogColor: [0, 0, 0, 0],
  });
  const uniformBuffer = uploadBuffer(device, uniformBytes, GPUBufferUsage.UNIFORM);
  const bindGroup = canvasPipeline.shaderProgram.createBindGroup(device, canvasPipeline.bindGroupLayout, {
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
  const canvasDepthView = createDepthView(device, canvas.width, canvas.height);
  const readbackDepthView = createDepthView(device, canvas.width, canvas.height);
  const readbackTexture = device.createTexture({
    size: { width: canvas.width, height: canvas.height },
    format: READBACK_FORMAT,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });

  device.pushErrorScope("validation");
  const encoder = device.createCommandEncoder();
  encodeDrawPass(encoder, ctx.getCurrentTexture().createView(), canvasDepthView, canvasPipeline.pipeline, bindGroup, vertexBuffer);
  encodeDrawPass(encoder, readbackTexture.createView(), readbackDepthView, readbackPipeline.pipeline, bindGroup, vertexBuffer);
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
