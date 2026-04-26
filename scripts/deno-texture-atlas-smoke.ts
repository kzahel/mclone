import { ResourceLocation } from "../src/core/resource-location.ts";
import { RenderPipelineCache } from "../src/renderer/pipeline/render-pipeline-cache.ts";
import { RenderStateShards } from "../src/renderer/render-state-shard.ts";
import { RenderType, RenderTypeCompositeState } from "../src/renderer/render-type.ts";
import { createOffscreenTextureTarget, readTextureRgba8, requestWebGpuDeviceContext } from "../src/renderer/webgpu-target.ts";
import { AnimationMetadataSection } from "../src/renderer/texture/animation-metadata-section.ts";
import { NativeImage } from "../src/renderer/texture/native-image.ts";
import { TextureAtlas, type TextureAtlasSource } from "../src/renderer/texture/texture-atlas.ts";
import { TextureAtlasSprite, TextureAtlasSpriteInfo, type TextureAtlasUploadTarget } from "../src/renderer/texture/texture-atlas-sprite.ts";
import { DefaultVertexFormat } from "../src/renderer/vertex/default-vertex-format.ts";
import { VertexFormat } from "../src/renderer/vertex/vertex-format.ts";
import { encodePngRgba } from "./png-rgba.ts";

const WIDTH = 64;
const HEIGHT = 64;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-texture-atlas-smoke.png";
const MIP_LEVEL = 0;
const ORANGE_PIXEL = new Uint8Array([230, 76, 13, 255]);
const TEAL_PIXEL = new Uint8Array([13, 188, 230, 255]);
const ORANGE_SPRITE = new ResourceLocation("minecraft:block/deno_orange");
const TEAL_SPRITE = new ResourceLocation("minecraft:block/deno_teal");
const SPRITE_LOCATIONS = [ORANGE_SPRITE, TEAL_SPRITE] as const;

class MemoryTextureAtlasSource implements TextureAtlasSource {
  public constructor(private readonly images: ReadonlyMap<string, NativeImage>) {}

  public async getBasicSpriteInfos(spriteNames: readonly ResourceLocation[]): Promise<readonly TextureAtlasSpriteInfo[]> {
    return spriteNames.map((location) => {
      const image = this.images.get(location.toString());
      if (!image) {
        throw new Error(`Missing Deno atlas smoke sprite ${location.toString()}`);
      }

      return new TextureAtlasSpriteInfo(location, image.getWidth(), image.getHeight(), AnimationMetadataSection.EMPTY);
    });
  }

  public async loadSprite(
    atlas: TextureAtlasUploadTarget,
    info: TextureAtlasSpriteInfo,
    atlasWidth: number,
    atlasHeight: number,
    mipLevel: number,
    x: number,
    y: number,
  ): Promise<TextureAtlasSprite | undefined> {
    const source = this.images.get(info.name().toString());
    if (!source) {
      throw new Error(`Missing Deno atlas smoke sprite ${info.name().toString()}`);
    }

    const image = new NativeImage(source.getWidth(), source.getHeight(), false);
    image.copyFrom(source);
    return new TextureAtlasSprite(atlas, info, mipLevel, atlasWidth, atlasHeight, x, y, image);
  }

  public close(): void {
    for (const image of this.images.values()) {
      image.close();
    }
  }
}

const contextResult = await requestWebGpuDeviceContext();
if (!contextResult.ok) {
  throw new Error(contextResult.reason);
}

const { adapter, device } = contextResult.context;
const source = new MemoryTextureAtlasSource(new Map([
  [ORANGE_SPRITE.toString(), solidImage(16, 16, nativeColor(ORANGE_PIXEL))],
  [TEAL_SPRITE.toString(), solidImage(16, 16, nativeColor(TEAL_PIXEL))],
]));
const atlas = new TextureAtlas(TextureAtlas.LOCATION_BLOCKS, device.limits.maxTextureDimension2D);
const preparations = await atlas.prepareToStitch(source, SPRITE_LOCATIONS, MIP_LEVEL);
atlas.reload(device, preparations);

const orangeSprite = atlas.getSprite(ORANGE_SPRITE);
const tealSprite = atlas.getSprite(TEAL_SPRITE);
assertSprite(orangeSprite, ORANGE_SPRITE);
assertSprite(tealSprite, TEAL_SPRITE);

const target = createOffscreenTextureTarget(device, WIDTH, HEIGHT, FORMAT);
const sampler = device.createSampler({
  magFilter: "nearest",
  minFilter: "nearest",
  mipmapFilter: "nearest",
});
const renderType = createDenoAtlasRenderType();
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
  textures: { Sampler0: atlas.getTextureView() },
});
const vertexBuffer = createAtlasQuadVertexBuffer(device, orangeSprite, tealSprite);

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
pass.draw(12);
pass.end();
device.queue.submit([encoder.finish()]);

const pixels = await readTextureRgba8(device, target.texture, target.width, target.height);
const leftPixel = readPixel(pixels, target.width, Math.floor(target.width * 0.25), Math.floor(target.height / 2));
const rightPixel = readPixel(pixels, target.width, Math.floor(target.width * 0.75), Math.floor(target.height / 2));
assertPixel(leftPixel, ORANGE_PIXEL, 1);
assertPixel(rightPixel, TEAL_PIXEL, 1);
await Deno.writeFile(OUTPUT_PATH, encodePngRgba(target.width, target.height, pixels));

vertexBuffer.destroy();
uniformBuffer.destroy();
target.texture.destroy();
atlas.clearTextureData();
source.close();

console.log(JSON.stringify({
  ok: true,
  outputPath: OUTPUT_PATH,
  width: target.width,
  height: target.height,
  format: target.format,
  atlasWidth: preparations.width,
  atlasHeight: preparations.height,
  mipLevel: preparations.mipLevel,
  regions: preparations.regions.map((sprite) => ({
    name: sprite.getName().toString(),
    x: sprite.getX(),
    y: sprite.getY(),
    width: sprite.getWidth(),
    height: sprite.getHeight(),
    u0: sprite.getU0(),
    u1: sprite.getU1(),
    v0: sprite.getV0(),
    v1: sprite.getV1(),
  })),
  renderType: renderType.name,
  shader: pipeline.shaderProgram.name,
  leftPixel: Array.from(leftPixel),
  rightPixel: Array.from(rightPixel),
  byteLength: pixels.byteLength,
  adapter: adapter.info ?? {},
}));

function nativeColor(pixel: Uint8Array): number {
  return NativeImage.combine(pixel[3]!, pixel[2]!, pixel[1]!, pixel[0]!);
}

function solidImage(width: number, height: number, color: number): NativeImage {
  const image = new NativeImage(width, height, false);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      image.setPixelRGBA(x, y, color);
    }
  }

  return image;
}

function assertSprite(sprite: TextureAtlasSprite, location: ResourceLocation): void {
  if (!sprite.getName().equals(location)) {
    throw new Error(`Expected atlas sprite ${location.toString()}, got ${sprite.getName().toString()}`);
  }
}

function createDenoAtlasRenderType(): RenderType {
  return RenderType.create(
    "deno_texture_atlas_quads",
    DefaultVertexFormat.POSITION_TEX,
    VertexFormat.Mode.TRIANGLES,
    256,
    RenderTypeCompositeState.builder()
      .setShaderState(RenderStateShards.POSITION_TEX_SHADER)
      .setTextureState(RenderStateShards.BLOCK_SHEET)
      .setCullState(RenderStateShards.NO_CULL)
      .setWriteMaskState(RenderStateShards.COLOR_WRITE)
      .createCompositeState(false),
  );
}

function createAtlasQuadVertexBuffer(device: GPUDevice, leftSprite: TextureAtlasSprite, rightSprite: TextureAtlasSprite): GPUBuffer {
  const vertexStride = DefaultVertexFormat.POSITION_TEX.getVertexSize();
  const bytes = new Uint8Array(vertexStride * 12);
  const dataView = new DataView(bytes.buffer);
  writeSpriteQuad(dataView, 0, vertexStride, -0.95, -0.9, -0.05, 0.9, leftSprite);
  writeSpriteQuad(dataView, vertexStride * 6, vertexStride, 0.05, -0.9, 0.95, 0.9, rightSprite);
  const buffer = device.createBuffer({
    size: bytes.byteLength,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  device.queue.writeBuffer(buffer, 0, bytes);
  return buffer;
}

function writeSpriteQuad(
  dataView: DataView,
  offset: number,
  vertexStride: number,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  sprite: TextureAtlasSprite,
): void {
  writePositionTexVertex(dataView, offset, x0, y0, 0, sprite.getU0(), sprite.getV1());
  writePositionTexVertex(dataView, offset + vertexStride, x1, y0, 0, sprite.getU1(), sprite.getV1());
  writePositionTexVertex(dataView, offset + (vertexStride * 2), x1, y1, 0, sprite.getU1(), sprite.getV0());
  writePositionTexVertex(dataView, offset + (vertexStride * 3), x0, y0, 0, sprite.getU0(), sprite.getV1());
  writePositionTexVertex(dataView, offset + (vertexStride * 4), x1, y1, 0, sprite.getU1(), sprite.getV0());
  writePositionTexVertex(dataView, offset + (vertexStride * 5), x0, y1, 0, sprite.getU0(), sprite.getV0());
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
        `Deno texture-atlas smoke pixel mismatch at channel ${i.toString()}: expected ${expectedValue.toString()} +/- ${tolerance.toString()}, got ${actualValue?.toString() ?? "undefined"}`,
      );
    }
  }
}
