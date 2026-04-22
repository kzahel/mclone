import { BufferBuilder } from "./vertex/buffer-builder";
import { DefaultVertexFormat } from "./vertex/default-vertex-format";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { RenderType } from "./render-type";
import { ResourceLocation } from "../core/resource-location";
import { BrowserTextureAtlasSource, loadNativeImageFromUrl } from "./texture/browser-native-image-loader";
import { NativeImage } from "./texture/native-image";
import { TextureAtlas } from "./texture/texture-atlas";
import type { TextureAtlasSprite } from "./texture/texture-atlas-sprite";
import { VertexBuffer } from "./vertex/vertex-buffer";
import { VertexFormat } from "./vertex/vertex-format";

const SMOKE_ATLAS_LOCATION = new ResourceLocation("minecraft:textures/atlas/blocks.png");
const SMOKE_CENTER_SPRITE = new ResourceLocation("minecraft:block/orange_wool");
const SMOKE_VISUAL_SPRITE = new ResourceLocation("minecraft:block/grass_block_top");
const SMOKE_SPRITES = [SMOKE_CENTER_SPRITE, SMOKE_VISUAL_SPRITE] as const;

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

function pushSpriteQuad(
  builder: BufferBuilder,
  sprite: TextureAtlasSprite,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
): void {
  builder.vertex(x0, y0, 0, 1, 1, 1, 1, sprite.getU0(), sprite.getV1(), 0, 0, 0, 0, 1);
  builder.vertex(x1, y0, 0, 1, 1, 1, 1, sprite.getU1(), sprite.getV1(), 0, 0, 0, 0, 1);
  builder.vertex(x1, y1, 0, 1, 1, 1, 1, sprite.getU1(), sprite.getV0(), 0, 0, 0, 0, 1);
  builder.vertex(x0, y1, 0, 1, 1, 1, 1, sprite.getU0(), sprite.getV0(), 0, 0, 0, 0, 1);
}

function buildScene(device: GPUDevice, centerSprite: TextureAtlasSprite, visualSprite: TextureAtlasSprite): VertexBuffer {
  const builder = new BufferBuilder(256);
  builder.begin(VertexFormat.Mode.QUADS, DefaultVertexFormat.BLOCK);
  pushSpriteQuad(builder, centerSprite, -0.5, -0.5, 0.5, 0.5);
  pushSpriteQuad(builder, visualSprite, -0.95, -0.85, -0.55, -0.45);
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
  const expectedCenterPixel = rgba8FromPixel(centerSpriteImage.getPixelRGBA(8, 8));
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

  const vertexBuffer = buildScene(device, centerSprite, visualSprite);
  const vertexFormat = vertexBuffer.getFormat();
  if (!vertexFormat) return { ok: false, reason: "vertex buffer format missing after upload" };

  const renderType = RenderType.solid();
  if (renderType.format() !== vertexFormat) {
    return { ok: false, reason: "render type format does not match the uploaded vertex format" };
  }

  const pipelineCache = new RenderPipelineCache(device);
  const canvasPipeline = pipelineCache.getOrCreate(renderType, format, DEPTH_FORMAT);
  const readbackPipeline = pipelineCache.getOrCreate(renderType, READBACK_FORMAT, DEPTH_FORMAT);
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

  const encoder = device.createCommandEncoder();
  encodeDrawPass(encoder, ctx.getCurrentTexture().createView(), canvasDepthView, canvasPipeline.pipeline, bindGroup, vertexBuffer);
  encodeDrawPass(encoder, readbackTexture.createView(), readbackDepthView, readbackPipeline.pipeline, bindGroup, vertexBuffer);
  device.queue.submit([encoder.finish()]);
  await device.queue.onSubmittedWorkDone();
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
