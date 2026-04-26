import { RenderStateShards } from "../src/renderer/render-state-shard.ts";
import { RenderType, RenderTypeCompositeState } from "../src/renderer/render-type.ts";
import { createOffscreenTextureTarget, readTextureRgba8, requestWebGpuDeviceContext } from "../src/renderer/webgpu-target.ts";
import { RenderPipelineCache } from "../src/renderer/pipeline/render-pipeline-cache.ts";
import { DefaultVertexFormat } from "../src/renderer/vertex/default-vertex-format.ts";
import { VertexFormat } from "../src/renderer/vertex/vertex-format.ts";
import { encodePngRgba } from "./png-rgba.ts";

const WIDTH = 64;
const HEIGHT = 64;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-render-pipeline-smoke.png";
const EXPECTED_CENTER_PIXEL = new Uint8Array([51, 204, 77, 255]);

const contextResult = await requestWebGpuDeviceContext();
if (!contextResult.ok) {
  throw new Error(contextResult.reason);
}

const { adapter, device } = contextResult.context;
const target = createOffscreenTextureTarget(device, WIDTH, HEIGHT, FORMAT);
const renderType = createDenoPositionColorTriangleRenderType();
const pipeline = new RenderPipelineCache(device).getOrCreate(renderType, target.format);
const uniformBytes = pipeline.shaderProgram.createUniformBufferBytes();
const uniformBuffer = device.createBuffer({
  size: uniformBytes.byteLength,
  usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
});
device.queue.writeBuffer(uniformBuffer, 0, uniformBytes);
const bindGroup = pipeline.shaderProgram.createBindGroup(device, pipeline.bindGroupLayout, { uniformBuffer });
const vertexBuffer = createPositionColorTriangleVertexBuffer(device);

const encoder = device.createCommandEncoder();
const pass = encoder.beginRenderPass({
  colorAttachments: [{
    view: target.view,
    clearValue: { r: 0, g: 0, b: 0, a: 1 },
    loadOp: "clear",
    storeOp: "store",
  }],
});
pass.setPipeline(pipeline.pipeline);
pass.setBindGroup(0, bindGroup);
pass.setVertexBuffer(0, vertexBuffer);
pass.draw(3);
pass.end();
device.queue.submit([encoder.finish()]);

const pixels = await readTextureRgba8(device, target.texture, target.width, target.height);
const centerPixel = readPixel(pixels, target.width, Math.floor(target.width / 2), Math.floor(target.height / 2));
assertPixel(centerPixel, EXPECTED_CENTER_PIXEL, 1);
await Deno.writeFile(OUTPUT_PATH, encodePngRgba(target.width, target.height, pixels));

vertexBuffer.destroy();
uniformBuffer.destroy();
target.texture.destroy();

console.log(JSON.stringify({
  ok: true,
  outputPath: OUTPUT_PATH,
  width: target.width,
  height: target.height,
  format: target.format,
  renderType: renderType.name,
  shader: pipeline.shaderProgram.name,
  centerPixel: Array.from(centerPixel),
  byteLength: pixels.byteLength,
  adapter: adapter.info ?? {},
}));

function createDenoPositionColorTriangleRenderType(): RenderType {
  return RenderType.create(
    "deno_position_color_triangle",
    DefaultVertexFormat.POSITION_COLOR,
    VertexFormat.Mode.TRIANGLES,
    256,
    RenderTypeCompositeState.builder()
      .setShaderState(RenderStateShards.POSITION_COLOR_SHADER)
      .setCullState(RenderStateShards.NO_CULL)
      .setWriteMaskState(RenderStateShards.COLOR_WRITE)
      .createCompositeState(false),
  );
}

function createPositionColorTriangleVertexBuffer(device: GPUDevice): GPUBuffer {
  const vertexStride = DefaultVertexFormat.POSITION_COLOR.getVertexSize();
  const bytes = new Uint8Array(vertexStride * 3);
  const dataView = new DataView(bytes.buffer);
  writePositionColorVertex(dataView, 0, -0.8, -0.8, 0, 51, 204, 77, 255);
  writePositionColorVertex(dataView, vertexStride, 0.8, -0.8, 0, 51, 204, 77, 255);
  writePositionColorVertex(dataView, vertexStride * 2, 0, 0.8, 0, 51, 204, 77, 255);
  const buffer = device.createBuffer({
    size: bytes.byteLength,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  device.queue.writeBuffer(buffer, 0, bytes);
  return buffer;
}

function writePositionColorVertex(
  dataView: DataView,
  offset: number,
  x: number,
  y: number,
  z: number,
  r: number,
  g: number,
  b: number,
  a: number,
): void {
  dataView.setFloat32(offset + 0, x, true);
  dataView.setFloat32(offset + 4, y, true);
  dataView.setFloat32(offset + 8, z, true);
  dataView.setUint8(offset + 12, r);
  dataView.setUint8(offset + 13, g);
  dataView.setUint8(offset + 14, b);
  dataView.setUint8(offset + 15, a);
}

function readPixel(pixels: Uint8Array, width: number, x: number, y: number): Uint8Array {
  const offset = ((y * width) + x) * 4;
  return pixels.slice(offset, offset + 4);
}

function assertPixel(actual: Uint8Array, expected: Uint8Array, tolerance: number): void {
  for (let i = 0; i < expected.length; i++) {
    const actualValue = actual[i];
    const expectedValue = expected[i]!;
    if (actualValue === undefined || Math.abs(actualValue - expectedValue) > tolerance) {
      throw new Error(
        `Deno render-pipeline smoke pixel mismatch at channel ${i.toString()}: expected ${expectedValue.toString()} +/- ${tolerance.toString()}, got ${actualValue?.toString() ?? "undefined"}`,
      );
    }
  }
}
