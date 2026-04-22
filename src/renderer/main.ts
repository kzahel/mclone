import { BufferBuilder } from "./vertex/buffer-builder";
import { DefaultVertexFormat } from "./vertex/default-vertex-format";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import { RenderType } from "./render-type";
import { VertexBuffer } from "./vertex/vertex-buffer";
import { VertexFormat } from "./vertex/vertex-format";

export const QUAD_COLOR_RGBA8 = [224, 96, 48, 255] as const;

export type BootResult =
  | { ok: true; format: GPUTextureFormat; adapterInfo: string; centerPixel: readonly [number, number, number, number] }
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

function buildQuad(device: GPUDevice): VertexBuffer {
  const builder = new BufferBuilder(256);
  const color = QUAD_COLOR_RGBA8.map((channel) => channel / 255);
  builder.begin(VertexFormat.Mode.QUADS, DefaultVertexFormat.BLOCK);
  builder.vertex(-0.5, -0.5, 0, color[0]!, color[1]!, color[2]!, color[3]!, 0, 0, 0, 0, 0, 0, 1);
  builder.vertex(0.5, -0.5, 0, color[0]!, color[1]!, color[2]!, color[3]!, 1, 0, 0, 0, 0, 0, 1);
  builder.vertex(0.5, 0.5, 0, color[0]!, color[1]!, color[2]!, color[3]!, 1, 1, 0, 0, 0, 0, 1);
  builder.vertex(-0.5, 0.5, 0, color[0]!, color[1]!, color[2]!, color[3]!, 0, 1, 0, 0, 0, 0, 1);
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

  const vertexBuffer = buildQuad(device);
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
  });
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
      Sampler0: whiteTexture,
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

  return { ok: true, format, adapterInfo, centerPixel };
}

if (typeof window !== "undefined") {
  window.__mcloneReady = boot();
}
