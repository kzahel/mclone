import { BlockPos } from "../src/core/block-pos.ts";
import type { ChunkSnapshotMessage } from "../src/runtime/protocol/world-messages.ts";
import { RenderWorldWorkerClient, RenderWorldWorkerUpdateSink, type RenderWorldWorkerClientEndpoint } from "../src/renderer/chunk/render-world-worker-client.ts";
import { ChunkRenderDispatcher } from "../src/renderer/chunk/chunk-render-dispatcher.ts";
import { ChunkBufferBuilderPack } from "../src/renderer/chunk-buffer-builder-pack.ts";
import type { BlockRenderDispatcher } from "../src/renderer/block/block-render-dispatcher.ts";
import { createOffscreenTextureTarget, readTextureRgba8, requestWebGpuDeviceContext } from "../src/renderer/webgpu-target.ts";
import { GameRenderer } from "../src/renderer/game-renderer.ts";
import { LevelRenderer, type LevelRenderFrame } from "../src/renderer/level-renderer.ts";
import { LightTexture } from "../src/renderer/light-texture.ts";
import { RenderPipelineCache } from "../src/renderer/pipeline/render-pipeline-cache.ts";
import { createSceneDepthTarget, encodeSceneFrame, type RendererScene } from "../src/renderer/scene-setup.ts";
import { TextureAtlas } from "../src/renderer/texture/texture-atlas.ts";
import type { TextureAtlasSprite } from "../src/renderer/texture/texture-atlas-sprite.ts";
import { ViewArea } from "../src/renderer/view-area.ts";
import { ClientChunkCache } from "../src/world/level/client-chunk-cache.ts";
import { createBlockStateResolver, buildChunkSnapshot } from "../src/world/level/chunk-snapshot.ts";
import { packChunkSnapshot } from "../src/world/level/packed-chunk-snapshot.ts";
import { registerGeneratedRenderBlocks } from "../src/world/level/generated-render-blocks.ts";
import { StaticRenderLevel } from "../src/world/level/static-render-level.ts";
import type { BlockState } from "../src/world/level/block/state/block-state.ts";
import { Vec3 } from "../src/world/phys/vec3.ts";
import { ChunkBiomeContainer } from "../src/worldgen/biome/chunk-biome-container.ts";
import { OverworldBiomeSource } from "../src/worldgen/biome/overworld-biome-source.ts";
import { ChunkBlockId } from "../src/worldgen/chunk/chunk-block-buffer.ts";
import { encodePngRgba } from "./png-rgba.ts";
import {
  DENO_STATIC_WORLD_HEIGHT,
  DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
  DENO_STATIC_WORLD_SEED,
  DENO_STATIC_WORLD_STONE_TEXTURE,
  DENO_STATIC_WORLD_TEXTURE_MIP_LEVEL,
  DENO_STATIC_WORLD_VIEW_DISTANCE,
  DenoStaticWorldTextureAtlasSource,
  createDenoStaticWorldBlockRenderer,
} from "./deno-static-world-smoke-shared.ts";

const WIDTH = 128;
const HEIGHT = 128;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-static-world-smoke.png";
const CAMERA_POSITION = new Vec3(8.5, 8.5, 28.0);
const CAMERA_STATE = {
  position: CAMERA_POSITION,
  xRot: 0,
  yRot: 180,
} as const;
const SKY_COLOR = new Vec3(0.55, 0.7, 1.0);
const RENDER_DISTANCE = 128;
const MIN_NON_CLEAR_PIXELS = 1_000;

const contextResult = await requestWebGpuDeviceContext();
if (!contextResult.ok) {
  throw new Error(contextResult.reason);
}

const { adapter, device } = contextResult.context;
const blocks = registerGeneratedRenderBlocks();
const source = new DenoStaticWorldTextureAtlasSource();
const atlas = new TextureAtlas(TextureAtlas.LOCATION_BLOCKS, device.limits.maxTextureDimension2D);
const preparations = await atlas.prepareToStitch(source, [DENO_STATIC_WORLD_STONE_TEXTURE], DENO_STATIC_WORLD_TEXTURE_MIP_LEVEL);
atlas.reload(device, preparations);
const stoneSprite = atlas.getSprite(DENO_STATIC_WORLD_STONE_TEXTURE);

const renderWorldWorker = new RenderWorldWorkerClient(
  new Worker(new URL("./deno-static-world-worker.ts", import.meta.url), {
    type: "module",
    name: "mclone-deno-static-world-worker",
  }) as unknown as RenderWorldWorkerClientEndpoint,
);
await renderWorldWorker.initialize({
  type: "initialize_render_world",
  seed: DENO_STATIC_WORLD_SEED,
  minBuildHeight: DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
  height: DENO_STATIC_WORLD_HEIGHT,
});
const renderWorldUpdateSink = new RenderWorldWorkerUpdateSink(renderWorldWorker);
await renderWorldUpdateSink.ingestUpdates(createStaticChunkSnapshotMessages(blocks));

const biomeSource = new OverworldBiomeSource(DENO_STATIC_WORLD_SEED);
const mainThreadLevel = new ClientChunkCache({
  airState: blocks.airState,
  minBuildHeight: DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
  height: DENO_STATIC_WORLD_HEIGHT,
  biomeSource,
  biomeZoomSeed: DENO_STATIC_WORLD_SEED,
  blockStateResolver: createBlockStateResolver(blocks.airState),
  blockStateIds: blocks.blockStateIds,
  skyLight: 15,
  blockLight: 15,
  skyColor: SKY_COLOR,
});
const levelRenderer = new LevelRenderer();
const gameRenderer = new GameRenderer(WIDTH, HEIGHT, RENDER_DISTANCE);
const lightTexture = new LightTexture(gameRenderer, mainThreadLevel, device);
lightTexture.tick();
const chunkDispatcher = new ChunkRenderDispatcher(
  mainThreadLevel,
  levelRenderer,
  createDenoStaticWorldBlockRendererForMainThread(stoneSprite, blocks),
  device,
  (task) => queueMicrotask(task),
  false,
  new ChunkBufferBuilderPack(),
  undefined,
  renderWorldWorker,
);
const viewArea = new ViewArea(chunkDispatcher, mainThreadLevel, DENO_STATIC_WORLD_VIEW_DISTANCE, levelRenderer);
levelRenderer.setLevel(mainThreadLevel, chunkDispatcher, viewArea, DENO_STATIC_WORLD_VIEW_DISTANCE);
const dirtySectionCount = applyDirtySections(renderWorldUpdateSink, viewArea, levelRenderer);

const frame = await gameRenderer.renderLevel(0, Number.MAX_SAFE_INTEGER, levelRenderer, lightTexture, CAMERA_STATE, {
  waitForChunkTasks: true,
});
const solidDrawCount = countFrameDraws(frame);
if (solidDrawCount <= 0) {
  throw new Error("Deno static-world smoke produced no chunk draws");
}

const target = createOffscreenTextureTarget(device, WIDTH, HEIGHT, FORMAT);
const depthTarget = createSceneDepthTarget(device, WIDTH, HEIGHT);
const scene = createHeadlessScene(device, atlas, renderWorldUpdateSink, viewArea, chunkDispatcher, levelRenderer, gameRenderer, lightTexture);
const encoder = device.createCommandEncoder();
encodeSceneFrame(scene, frame, { view: target.view, depthView: depthTarget.view, format: target.format }, encoder);
device.queue.submit([encoder.finish()]);

const pixels = await readTextureRgba8(device, target.texture, target.width, target.height);
const centerPixel = readPixel(pixels, target.width, Math.floor(target.width / 2), Math.floor(target.height / 2));
const clearPixel = rgbaColorToPixel(frame.fogColor);
const nonClearPixels = countPixelsDifferentFrom(pixels, clearPixel, 6);
if (nonClearPixels < MIN_NON_CLEAR_PIXELS) {
  throw new Error(`Deno static-world smoke rendered too few non-clear pixels: ${nonClearPixels.toString()}`);
}
assertStoneLike(centerPixel);
await Deno.writeFile(OUTPUT_PATH, encodePngRgba(target.width, target.height, pixels));

const workerCounters = renderWorldUpdateSink.getPerformanceCounters();
const renderedChunkCount = levelRenderer.countRenderedChunks();

viewArea.releaseAllBuffers();
chunkDispatcher.dispose();
lightTexture.close();
depthTarget.texture.destroy();
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
  stoneSprite: {
    u0: stoneSprite.getU0(),
    u1: stoneSprite.getU1(),
    v0: stoneSprite.getV0(),
    v1: stoneSprite.getV1(),
  },
  dirtySectionCount,
  renderedChunkCount,
  solidDrawCount,
  nonClearPixels,
  centerPixel: Array.from(centerPixel),
  byteLength: pixels.byteLength,
  workerCounters,
  adapter: adapter.info ?? {},
}));

function createDenoStaticWorldBlockRendererForMainThread(
  stoneSprite: TextureAtlasSprite,
  blocks: ReturnType<typeof registerGeneratedRenderBlocks>,
): BlockRenderDispatcher {
  return createDenoStaticWorldBlockRenderer(
    stoneSprite,
    blocks.blockStateById[ChunkBlockId.WATER]!,
    blocks.blockStateById[ChunkBlockId.LAVA]!,
  );
}

function createStaticChunkSnapshotMessages(blocks: ReturnType<typeof registerGeneratedRenderBlocks>): ChunkSnapshotMessage[] {
  const biomeSource = new OverworldBiomeSource(DENO_STATIC_WORLD_SEED);
  const resolver = createBlockStateResolver(blocks.airState);
  const sourceLevel = new StaticRenderLevel(
    blocks.airState,
    15,
    15,
    DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
    DENO_STATIC_WORLD_HEIGHT,
    SKY_COLOR,
  );
  for (let chunkX = -1; chunkX <= 1; chunkX++) {
    for (let chunkZ = -1; chunkZ <= 1; chunkZ++) {
      sourceLevel.getChunk(chunkX, chunkZ);
    }
  }

  const stone = blocks.blockStateById[ChunkBlockId.STONE]!;
  fillWall(sourceLevel, stone);

  const messages: ChunkSnapshotMessage[] = [];
  for (let chunkX = -1; chunkX <= 1; chunkX++) {
    for (let chunkZ = -1; chunkZ <= 1; chunkZ++) {
      const chunk = sourceLevel.getChunk(chunkX, chunkZ, false)!;
      const snapshot = buildChunkSnapshot(
        chunk,
        new ChunkBiomeContainer(
          DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
          DENO_STATIC_WORLD_HEIGHT,
          chunkX,
          chunkZ,
          biomeSource,
        ).writeBiomes(),
        DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
        DENO_STATIC_WORLD_HEIGHT,
      );
      messages.push({
        type: "chunk_snapshot",
        snapshot: packChunkSnapshot(snapshot, blocks.blockStateIds, resolver),
      });
    }
  }

  return messages;
}

function fillWall(level: StaticRenderLevel, state: BlockState): void {
  for (let y = 0; y < 16; y++) {
    for (let x = 0; x < 16; x++) {
      level.setBlock(new BlockPos(x, y, 0), state);
    }
  }
}

function applyDirtySections(
  sink: RenderWorldWorkerUpdateSink,
  viewArea: ViewArea,
  levelRenderer: LevelRenderer,
): number {
  let dirtyCount = 0;
  for (const dirtySection of sink.drainDirtySections()) {
    if (dirtySection.y < DENO_STATIC_WORLD_MIN_BUILD_HEIGHT || dirtySection.y >= DENO_STATIC_WORLD_MIN_BUILD_HEIGHT + DENO_STATIC_WORLD_HEIGHT) {
      continue;
    }

    viewArea.setDirty(
      Math.floor(dirtySection.x / 16),
      Math.floor(dirtySection.y / 16),
      Math.floor(dirtySection.z / 16),
      false,
    );
    dirtyCount++;
  }

  if (dirtyCount > 0) {
    levelRenderer.requestUpdate();
  }

  return dirtyCount;
}

function createHeadlessScene(
  device: GPUDevice,
  atlas: TextureAtlas,
  renderWorldUpdateSink: RenderWorldWorkerUpdateSink,
  viewArea: ViewArea,
  chunkDispatcher: ChunkRenderDispatcher,
  levelRenderer: LevelRenderer,
  gameRenderer: GameRenderer,
  lightTexture: LightTexture,
): RendererScene {
  return {
    device,
    atlas,
    renderWorldUpdateSink,
    viewArea,
    chunkDispatcher,
    levelRenderer,
    gameRenderer,
    lightTexture,
    pipelineCache: new RenderPipelineCache(device),
    textureSamplers: createTextureSamplers(device),
    lightSampler: device.createSampler({
      magFilter: "linear",
      minFilter: "linear",
      mipmapFilter: "nearest",
    }),
    worldBounds: {
      minBuildHeight: DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
      maxBuildHeight: DENO_STATIC_WORLD_MIN_BUILD_HEIGHT + DENO_STATIC_WORLD_HEIGHT,
    },
    viewDistance: DENO_STATIC_WORLD_VIEW_DISTANCE,
    chunkDrawResources: new WeakMap(),
  } as unknown as RendererScene;
}

function createTextureSamplers(device: GPUDevice): ReadonlyMap<string, GPUSampler> {
  const samplers = new Map<string, GPUSampler>();
  for (const state of [
    { blur: false, mipmap: false },
    { blur: false, mipmap: true },
    { blur: true, mipmap: false },
    { blur: true, mipmap: true },
  ] as const) {
    samplers.set(`${state.blur ? 1 : 0}:${state.mipmap ? 1 : 0}`, device.createSampler({
      magFilter: state.blur ? "linear" : "nearest",
      minFilter: state.blur ? "linear" : "nearest",
      mipmapFilter: state.mipmap ? "linear" : "nearest",
    }));
  }

  return samplers;
}

function countFrameDraws(frame: LevelRenderFrame): number {
  let count = 0;
  for (const draws of frame.layerDraws.values()) {
    count += draws.length;
  }

  return count;
}

function readPixel(pixels: Uint8Array, width: number, x: number, y: number): Uint8Array {
  const offset = ((y * width) + x) * 4;
  return pixels.slice(offset, offset + 4);
}

function rgbaColorToPixel(color: readonly [number, number, number, number]): Uint8Array {
  return new Uint8Array(color.map((component) => Math.max(0, Math.min(255, Math.round(component * 255)))));
}

function countPixelsDifferentFrom(pixels: Uint8Array, expected: Uint8Array, tolerance: number): number {
  let count = 0;
  for (let offset = 0; offset < pixels.byteLength; offset += 4) {
    for (let channel = 0; channel < 4; channel++) {
      if (Math.abs(pixels[offset + channel]! - expected[channel]!) > tolerance) {
        count++;
        break;
      }
    }
  }

  return count;
}

function assertStoneLike(pixel: Uint8Array): void {
  const [red, green, blue, alpha] = pixel;
  if (alpha !== 255 || red === undefined || green === undefined || blue === undefined) {
    throw new Error(`Deno static-world center pixel is not opaque RGBA: ${Array.from(pixel).join(",")}`);
  }

  const maxChannel = Math.max(red, green, blue);
  const minChannel = Math.min(red, green, blue);
  if (maxChannel - minChannel > 12 || minChannel < 64 || maxChannel > 220) {
    throw new Error(`Deno static-world center pixel does not look like the stone wall: ${Array.from(pixel).join(",")}`);
  }
}
