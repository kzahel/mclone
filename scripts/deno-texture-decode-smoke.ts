import { RenderStateShards } from "../src/renderer/render-state-shard.ts";
import { RenderType, RenderTypeCompositeState } from "../src/renderer/render-type.ts";
import { createOffscreenTextureTarget, readTextureRgba8, requestWebGpuDeviceContext } from "../src/renderer/webgpu-target.ts";
import { RenderPipelineCache } from "../src/renderer/pipeline/render-pipeline-cache.ts";
import { NativeImage } from "../src/renderer/texture/native-image.ts";
import { PngNativeImageDecoder } from "../src/renderer/texture/png-native-image-decoder.ts";
import { DefaultVertexFormat } from "../src/renderer/vertex/default-vertex-format.ts";
import { VertexFormat } from "../src/renderer/vertex/vertex-format.ts";
import { encodePngRgba } from "./png-rgba.ts";

const WIDTH = 64;
const HEIGHT = 64;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-texture-decode-smoke.png";
const TEXTURE_PIXEL = new Uint8Array([230, 76, 13, 255]);
const SOURCE_WIDTH = 64;
const SOURCE_HEIGHT = 64;

const contextResult = await requestWebGpuDeviceContext();
if (!contextResult.ok) {
  throw new Error(contextResult.reason);
}

const sourcePngBytes = encodePngRgba(SOURCE_WIDTH, SOURCE_HEIGHT, repeatedTexturePixels(SOURCE_WIDTH * SOURCE_HEIGHT));
const decodedImage = await new PngNativeImageDecoder().decode(new Blob([sourcePngBytes], { type: "image/png" }));
assertDecodedImage(decodedImage);

const { adapter, device } = contextResult.context;
const target = createOffscreenTextureTarget(device, WIDTH, HEIGHT, FORMAT);
const sampledTexture = createSampledTexture(device, decodedImage);
const uploadedSourcePixels = await readTextureRgba8(device, sampledTexture, decodedImage.getWidth(), decodedImage.getHeight());
assertPixel(readPixel(uploadedSourcePixels, decodedImage.getWidth(), 0, 0), TEXTURE_PIXEL, 0);
const sampler = device.createSampler({
  magFilter: "nearest",
  minFilter: "nearest",
  mipmapFilter: "nearest",
});
const renderType = createDenoPositionTexRenderType();
const pipeline = new RenderPipelineCache(device).getOrCreate(renderType, target.format);
const uniformBytes = pipeline.shaderProgram.createUniformBufferBytes();
const uniformBuffer = device.createBuffer({
  size: uniformBytes.byteLength,
  usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
});
device.queue.writeBuffer(uniformBuffer, 0, uniformBytes);
const bindGroup = pipeline.shaderProgram.createBindGroup(device, pipeline.bindGroupLayout, {
  uniformBuffer,
  samplers: { Sampler0: sampler },
  textures: { Sampler0: sampledTexture.createView() },
});
const vertexBuffer = createPositionTexQuadVertexBuffer(device);

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
pass.draw(6);
pass.end();
device.queue.submit([encoder.finish()]);

const pixels = await readTextureRgba8(device, target.texture, target.width, target.height);
const centerPixel = readPixel(pixels, target.width, Math.floor(target.width / 2), Math.floor(target.height / 2));
assertPixel(centerPixel, TEXTURE_PIXEL, 1);
await Deno.writeFile(OUTPUT_PATH, encodePngRgba(target.width, target.height, pixels));

decodedImage.close();
vertexBuffer.destroy();
uniformBuffer.destroy();
sampledTexture.destroy();
target.texture.destroy();

console.log(JSON.stringify({
  ok: true,
  outputPath: OUTPUT_PATH,
  width: target.width,
  height: target.height,
  format: target.format,
  sourceWidth: SOURCE_WIDTH,
  sourceHeight: SOURCE_HEIGHT,
  renderType: renderType.name,
  shader: pipeline.shaderProgram.name,
  centerPixel: Array.from(centerPixel),
  byteLength: pixels.byteLength,
  adapter: adapter.info ?? {},
}));

function repeatedTexturePixels(pixelCount: number): Uint8Array {
  const pixels = new Uint8Array(pixelCount * 4);
  for (let pixel = 0; pixel < pixelCount; pixel++) {
    pixels.set(TEXTURE_PIXEL, pixel * 4);
  }
  return pixels;
}

function assertDecodedImage(image: NativeImage): void {
  if (image.getWidth() !== SOURCE_WIDTH || image.getHeight() !== SOURCE_HEIGHT) {
    throw new Error(`Decoded image size mismatch: ${image.getWidth().toString()}x${image.getHeight().toString()}`);
  }

  const pixel = image.getPixelRGBA(0, 0);
  const actual = new Uint8Array([
    NativeImage.getR(pixel),
    NativeImage.getG(pixel),
    NativeImage.getB(pixel),
    NativeImage.getA(pixel),
  ]);
  assertPixel(actual, TEXTURE_PIXEL, 0);
}

function createSampledTexture(device: GPUDevice, image: NativeImage): GPUTexture {
  const texture = device.createTexture({
    size: { width: image.getWidth(), height: image.getHeight(), depthOrArrayLayers: 1 },
    format: "rgba8unorm",
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.COPY_SRC,
  });
  image.upload(device.queue, texture, 0, 0, 0, 0, 0, image.getWidth(), image.getHeight());
  return texture;
}

function createDenoPositionTexRenderType(): RenderType {
  return RenderType.create(
    "deno_position_tex_quad",
    DefaultVertexFormat.POSITION_TEX,
    VertexFormat.Mode.TRIANGLES,
    256,
    RenderTypeCompositeState.builder()
      .setShaderState(RenderStateShards.POSITION_TEX_SHADER)
      .setTextureState(RenderStateShards.NO_TEXTURE)
      .setCullState(RenderStateShards.NO_CULL)
      .setWriteMaskState(RenderStateShards.COLOR_WRITE)
      .createCompositeState(false),
  );
}

function createPositionTexQuadVertexBuffer(device: GPUDevice): GPUBuffer {
  const vertexStride = DefaultVertexFormat.POSITION_TEX.getVertexSize();
  const bytes = new Uint8Array(vertexStride * 6);
  const dataView = new DataView(bytes.buffer);
  writePositionTexVertex(dataView, 0, -0.9, -0.9, 0, 0, 1);
  writePositionTexVertex(dataView, vertexStride, 0.9, -0.9, 0, 1, 1);
  writePositionTexVertex(dataView, vertexStride * 2, 0.9, 0.9, 0, 1, 0);
  writePositionTexVertex(dataView, vertexStride * 3, -0.9, -0.9, 0, 0, 1);
  writePositionTexVertex(dataView, vertexStride * 4, 0.9, 0.9, 0, 1, 0);
  writePositionTexVertex(dataView, vertexStride * 5, -0.9, 0.9, 0, 0, 0);
  const buffer = device.createBuffer({
    size: bytes.byteLength,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  device.queue.writeBuffer(buffer, 0, bytes);
  return buffer;
}

function writePositionTexVertex(
  dataView: DataView,
  offset: number,
  x: number,
  y: number,
  z: number,
  u: number,
  v: number,
): void {
  dataView.setFloat32(offset + 0, x, true);
  dataView.setFloat32(offset + 4, y, true);
  dataView.setFloat32(offset + 8, z, true);
  dataView.setFloat32(offset + 12, u, true);
  dataView.setFloat32(offset + 16, v, true);
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
        `Deno texture smoke pixel mismatch at channel ${i.toString()}: expected ${expectedValue.toString()} +/- ${tolerance.toString()}, got ${actualValue?.toString() ?? "undefined"}`,
      );
    }
  }
}
