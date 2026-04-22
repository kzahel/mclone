import { BufferBuilder } from "./vertex/buffer-builder";
import { DefaultVertexFormat } from "./vertex/default-vertex-format";
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

const SHADER_SOURCE = `
struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
  @location(0) position: vec3<f32>,
  @location(1) color: vec4<f32>,
) -> VertexOutput {
  var output: VertexOutput;
  output.position = vec4<f32>(position, 1.0);
  output.color = color;
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return input.color;
}
`;

function createVertexLayout(format: VertexFormat): GPUVertexBufferLayout {
  const names = format.getElementAttributeNames();
  const offsets = format.getOffsets();
  const attributes: GPUVertexAttribute[] = [];

  for (let index = 0; index < names.length; index++) {
    const name = names[index]!;
    const offset = offsets[index]!;
    if (name === "Position") {
      attributes.push({ shaderLocation: 0, offset, format: "float32x3" });
    } else if (name === "Color") {
      attributes.push({ shaderLocation: 1, offset, format: "unorm8x4" });
    }
  }

  return {
    arrayStride: format.getVertexSize(),
    attributes,
  };
}

function createPipeline(
  device: GPUDevice,
  format: GPUTextureFormat,
  vertexFormat: VertexFormat,
  topology: GPUPrimitiveTopology,
): GPURenderPipeline {
  const shaderModule = device.createShaderModule({ code: SHADER_SOURCE });
  return device.createRenderPipeline({
    layout: "auto",
    vertex: {
      module: shaderModule,
      entryPoint: "vs_main",
      buffers: [createVertexLayout(vertexFormat)],
    },
    fragment: {
      module: shaderModule,
      entryPoint: "fs_main",
      targets: [{ format }],
    },
    primitive: {
      topology,
    },
  });
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
  pipeline: GPURenderPipeline,
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
  });
  pass.setPipeline(pipeline);
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

  const topology = vertexBuffer.getPrimitiveTopology();
  const canvasPipeline = createPipeline(device, format, vertexFormat, topology);
  const readbackPipeline = createPipeline(device, READBACK_FORMAT, vertexFormat, topology);
  const readbackTexture = device.createTexture({
    size: { width: canvas.width, height: canvas.height },
    format: READBACK_FORMAT,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });

  const encoder = device.createCommandEncoder();
  encodeDrawPass(encoder, ctx.getCurrentTexture().createView(), canvasPipeline, vertexBuffer);
  encodeDrawPass(encoder, readbackTexture.createView(), readbackPipeline, vertexBuffer);
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
