import type { ClientEntityPresentationState } from "../src/runtime/client/entity-interpolation-service.ts";
import type { RendererScene } from "../src/renderer/scene-setup.ts";
import {
  closeEntityFrameBuffers,
  createSceneDepthTarget,
  encodeSceneFrame,
} from "../src/renderer/scene-setup.ts";
import { collectEntityRenderBatches } from "../src/renderer/entity/entity-batch-renderer.ts";
import {
  SnapshotRenderableCow,
  SnapshotRenderablePig,
  SnapshotRenderablePlayer,
  SnapshotRenderableSheep,
} from "../src/renderer/entity/renderable-entity.ts";
import { GameRenderer } from "../src/renderer/game-renderer.ts";
import { EntityTextureManager } from "../src/renderer/texture/entity-texture-manager.ts";
import { Matrix4f } from "../src/renderer/math/matrix4f.ts";
import { RenderPipelineCache } from "../src/renderer/pipeline/render-pipeline-cache.ts";
import { createOffscreenTextureTarget, readTextureRgba8, requestWebGpuDeviceContext } from "../src/renderer/webgpu-target.ts";
import { Vec3 } from "../src/world/phys/vec3.ts";
import { createDenoExtractedAssetPack } from "./deno-file-asset-source.ts";
import { encodePngRgba } from "./png-rgba.ts";

const WIDTH = 128;
const HEIGHT = 128;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-entity-render-smoke.png";
const CLEAR_PIXEL = [38, 51, 64, 255] as const;

const contextResult = await requestWebGpuDeviceContext();
if (!contextResult.ok) {
  throw new Error(contextResult.reason);
}

const { adapter, device } = contextResult.context;
const entityTextureManager = await EntityTextureManager.create(device, createDenoExtractedAssetPack());
const atlasTexture = createSolidTexture(device, [255, 255, 255, 255], 1, 1);
const lightTexture = createSolidTexture(device, [255, 255, 255, 255], 16, 16);
const target = createOffscreenTextureTarget(device, WIDTH, HEIGHT, FORMAT);
const depthTarget = createSceneDepthTarget(device, WIDTH, HEIGHT);

const player = new SnapshotRenderablePlayer(playerState());
const cow = new SnapshotRenderableCow(cowState());
const pig = new SnapshotRenderablePig(pigState());
const sheep = new SnapshotRenderableSheep(sheepState());
const entityBatches = collectEntityRenderBatches({
  level: {
    getBrightness: () => 15,
  } as never,
  entities: [player, cow, pig, sheep],
  cameraPosition: Vec3.ZERO,
  partialTick: 0,
});
if (entityBatches.length === 0) {
  throw new Error("entity render smoke produced no entity batches");
}

const modelView = new Matrix4f();
modelView.setIdentity();
const projection = Matrix4f.perspective(70, WIDTH / HEIGHT, 0.05, 64);
const frame = {
  frameId: 1,
  modelViewMatrix: modelView.toFloat32Array(),
  projectionMatrix: projection.toFloat32Array(),
  fogStart: 1000,
  fogEnd: 1001,
  fogColor: [CLEAR_PIXEL[0] / 255, CLEAR_PIXEL[1] / 255, CLEAR_PIXEL[2] / 255, 1] as const,
  lightTexture: lightTexture.createView(),
  layerDraws: new Map(),
  entityBatches,
};
const scene = {
  adapter,
  device,
  format: FORMAT,
  atlas: {
    getTextureView: () => atlasTexture.createView(),
  },
  pipelineCache: new RenderPipelineCache(device),
  entityTextureManager,
  textureSamplers: new Map([[
    "0:0",
    device.createSampler({
      magFilter: "nearest",
      minFilter: "nearest",
      mipmapFilter: "nearest",
    }),
  ]]),
  lightSampler: device.createSampler({
    magFilter: "nearest",
    minFilter: "nearest",
    mipmapFilter: "nearest",
  }),
  chunkDrawResources: new WeakMap(),
  entityFrameResources: [],
  entityFrameBufferFrameId: undefined,
} as unknown as RendererScene;

try {
  const encoder = device.createCommandEncoder();
  encodeSceneFrame(scene, frame, {
    view: target.view,
    depthView: depthTarget.view,
    format: target.format,
  }, encoder);
  device.queue.submit([encoder.finish()]);
  const pixels = await readTextureRgba8(device, target.texture, target.width, target.height);
  const nonClearPixels = countPixelsDifferentFrom(pixels, CLEAR_PIXEL);
  if (nonClearPixels < 64) {
    throw new Error(`entity render smoke produced only ${nonClearPixels.toString()} non-clear pixels`);
  }
  await Deno.writeFile(OUTPUT_PATH, encodePngRgba(target.width, target.height, pixels));

  console.log(JSON.stringify({
    ok: true,
    outputPath: OUTPUT_PATH,
    width: target.width,
    height: target.height,
    format: target.format,
    entityBatchCount: entityBatches.length,
    nonClearPixels,
    centerPixel: Array.from(readPixel(pixels, target.width, Math.floor(target.width / 2), Math.floor(target.height / 2))),
    adapter: adapter.info ?? {},
  }));
} finally {
  closeEntityFrameBuffers(scene);
  entityTextureManager.close();
  atlasTexture.destroy();
  lightTexture.destroy();
  depthTarget.texture.destroy();
  target.texture.destroy();
}

function playerState(): ClientEntityPresentationState {
  const authoritative = {
    id: 7,
    uuid: "00000000-0000-0000-0000-000000000007",
    typeId: "minecraft:player",
    category: "misc" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: -1.55, y: -0.9, z: -3.1 },
    rotation: { yaw: 180, pitch: 0 },
    width: 0.6,
    height: 1.8,
    onGround: true,
    age: 0,
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: {},
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
  };
}

function cowState(): ClientEntityPresentationState {
  const authoritative = {
    id: 8,
    uuid: "00000000-0000-0000-0000-000000000008",
    typeId: "minecraft:cow",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 1.45, y: -1.0, z: -3.2 },
    rotation: { yaw: 210, pitch: 0 },
    width: 0.9,
    height: 1.4,
    onGround: true,
    age: 0,
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: {},
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
  };
}

function pigState(): ClientEntityPresentationState {
  const authoritative = {
    id: 9,
    uuid: "00000000-0000-0000-0000-000000000009",
    typeId: "minecraft:pig",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 0.45, y: -1.0, z: -2.9 },
    rotation: { yaw: 190, pitch: 0 },
    width: 0.9,
    height: 0.9,
    onGround: true,
    age: 0,
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: {},
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
  };
}

function sheepState(): ClientEntityPresentationState {
  const authoritative = {
    id: 10,
    uuid: "00000000-0000-0000-0000-000000000010",
    typeId: "minecraft:sheep",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: -0.45, y: -1.0, z: -3.0 },
    rotation: { yaw: 170, pitch: 0 },
    width: 0.9,
    height: 1.3,
    onGround: true,
    age: 0,
    data: { Color: 12 },
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: authoritative.data,
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
  };
}

function createSolidTexture(device: GPUDevice, rgba: readonly [number, number, number, number], width: number, height: number): GPUTexture {
  const texture = device.createTexture({
    size: { width, height },
    format: FORMAT,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
  });
  const unpaddedBytesPerRow = width * 4;
  const bytesPerRow = Math.ceil(unpaddedBytesPerRow / 256) * 256;
  const pixels = new Uint8Array(bytesPerRow * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const index = (y * bytesPerRow) + (x * 4);
      pixels[index + 0] = rgba[0];
      pixels[index + 1] = rgba[1];
      pixels[index + 2] = rgba[2];
      pixels[index + 3] = rgba[3];
    }
  }
  device.queue.writeTexture(
    { texture },
    pixels,
    { bytesPerRow, rowsPerImage: height },
    { width, height },
  );
  return texture;
}

function countPixelsDifferentFrom(pixels: Uint8Array, clearPixel: readonly [number, number, number, number]): number {
  let count = 0;
  for (let offset = 0; offset < pixels.length; offset += 4) {
    if (
      pixels[offset + 0] !== clearPixel[0] ||
      pixels[offset + 1] !== clearPixel[1] ||
      pixels[offset + 2] !== clearPixel[2] ||
      pixels[offset + 3] !== clearPixel[3]
    ) {
      count++;
    }
  }
  return count;
}

function readPixel(pixels: Uint8Array, width: number, x: number, y: number): Uint8Array {
  const offset = ((y * width) + x) * 4;
  return pixels.slice(offset, offset + 4);
}
