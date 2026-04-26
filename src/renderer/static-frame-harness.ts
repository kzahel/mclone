import { BlockPos } from "../core/block-pos";
import type { ChunkSnapshotMessage } from "../runtime/protocol/world-messages";
import { buildChunkSnapshot, createBlockStateResolver } from "../world/level/chunk-snapshot";
import { ClientChunkCache } from "../world/level/client-chunk-cache";
import type { BlockState } from "../world/level/block/state/block-state";
import type { GeneratedRenderBlockPalette } from "../world/level/generated-render-blocks";
import { packChunkSnapshot } from "../world/level/packed-chunk-snapshot";
import { StaticRenderLevel } from "../world/level/static-render-level";
import { Vec3 } from "../world/phys/vec3";
import { ChunkBiomeContainer } from "../worldgen/biome/chunk-biome-container";
import { OverworldBiomeSource } from "../worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../worldgen/chunk/chunk-block-buffer";
import type { BlockRenderDispatcher } from "./block/block-render-dispatcher";
import { ChunkRenderDispatcher } from "./chunk/chunk-render-dispatcher";
import {
  RenderWorldWorkerClient,
  RenderWorldWorkerUpdateSink,
  type RenderWorldWorkerPerformanceCounters,
} from "./chunk/render-world-worker-client";
import { ChunkBufferBuilderPack } from "./chunk-buffer-builder-pack";
import { GameRenderer, type CameraState } from "./game-renderer";
import { LevelRenderer, type LevelRenderFrame } from "./level-renderer";
import { LightTexture } from "./light-texture";
import { RenderPipelineCache } from "./pipeline/render-pipeline-cache";
import type { HeadlessRendererHost, RenderWorldWorkerEndpointFactory } from "./renderer-host";
import {
  createSceneDepthTarget,
  encodeSceneFrame,
  type DrawTarget,
  type RendererScene,
} from "./scene-setup";
import { TextureAtlas } from "./texture/texture-atlas";
import { ViewArea } from "./view-area";
import { readTextureRgba8 } from "./webgpu-target";

export interface StoneWallSnapshotOptions {
  readonly blocks: GeneratedRenderBlockPalette;
  readonly seed: bigint;
  readonly minBuildHeight: number;
  readonly worldHeight: number;
  readonly skyColor: Vec3;
  readonly chunkRadius?: number;
}

export interface StaticRendererFrameHarnessOptions {
  readonly device: GPUDevice;
  readonly rendererHost: RenderWorldWorkerEndpointFactory;
  readonly atlas: TextureAtlas;
  readonly blocks: GeneratedRenderBlockPalette;
  readonly blockRenderer: BlockRenderDispatcher;
  readonly snapshots: readonly ChunkSnapshotMessage[];
  readonly seed: bigint;
  readonly minBuildHeight: number;
  readonly worldHeight: number;
  readonly width: number;
  readonly height: number;
  readonly viewDistance: number;
  readonly renderDistance: number;
  readonly skyColor: Vec3;
}

export interface StaticRendererFrameHarness {
  readonly renderWorldWorker: RenderWorldWorkerClient;
  readonly renderWorldUpdateSink: RenderWorldWorkerUpdateSink;
  readonly levelRenderer: LevelRenderer;
  readonly gameRenderer: GameRenderer;
  readonly lightTexture: LightTexture;
  readonly chunkDispatcher: ChunkRenderDispatcher;
  readonly viewArea: ViewArea;
  readonly scene: RendererScene;
  readonly dirtySectionCount: number;
  close(): void;
}

export interface StaticRendererFrameResult {
  readonly frame: LevelRenderFrame;
  readonly drawCount: number;
  readonly renderedChunkCount: number;
  readonly workerCounters: RenderWorldWorkerPerformanceCounters;
}

export interface HeadlessFrameReadbackOptions {
  readonly rendererHost: Pick<HeadlessRendererHost, "createOffscreenTarget">;
  readonly scene: RendererScene;
  readonly frame: LevelRenderFrame;
  readonly width: number;
  readonly height: number;
  readonly format: GPUTextureFormat;
  readonly centerX?: number;
  readonly centerY?: number;
  readonly clearTolerance?: number;
}

export interface HeadlessFrameReadbackResult {
  readonly width: number;
  readonly height: number;
  readonly format: GPUTextureFormat;
  readonly pixels: Uint8Array;
  readonly centerPixel: Uint8Array;
  readonly clearPixel: Uint8Array;
  readonly nonClearPixels: number;
  readonly byteLength: number;
}

export function createStoneWallChunkSnapshotMessages(options: StoneWallSnapshotOptions): ChunkSnapshotMessage[] {
  const chunkRadius = options.chunkRadius ?? 1;
  const biomeSource = new OverworldBiomeSource(options.seed);
  const resolver = createBlockStateResolver(options.blocks.airState);
  const sourceLevel = new StaticRenderLevel(
    options.blocks.airState,
    15,
    15,
    options.minBuildHeight,
    options.worldHeight,
    options.skyColor,
  );
  for (let chunkX = -chunkRadius; chunkX <= chunkRadius; chunkX++) {
    for (let chunkZ = -chunkRadius; chunkZ <= chunkRadius; chunkZ++) {
      sourceLevel.getChunk(chunkX, chunkZ);
    }
  }

  const stone = options.blocks.blockStateById[ChunkBlockId.STONE]!;
  fillWall(sourceLevel, stone);

  const messages: ChunkSnapshotMessage[] = [];
  for (let chunkX = -chunkRadius; chunkX <= chunkRadius; chunkX++) {
    for (let chunkZ = -chunkRadius; chunkZ <= chunkRadius; chunkZ++) {
      const chunk = sourceLevel.getChunk(chunkX, chunkZ, false)!;
      const snapshot = buildChunkSnapshot(
        chunk,
        new ChunkBiomeContainer(
          options.minBuildHeight,
          options.worldHeight,
          chunkX,
          chunkZ,
          biomeSource,
        ).writeBiomes(),
        options.minBuildHeight,
        options.worldHeight,
      );
      messages.push({
        type: "chunk_snapshot",
        snapshot: packChunkSnapshot(snapshot, options.blocks.blockStateIds, resolver),
      });
    }
  }

  return messages;
}

export async function createStaticRendererFrameHarness(
  options: StaticRendererFrameHarnessOptions,
): Promise<StaticRendererFrameHarness> {
  let renderWorldWorker: RenderWorldWorkerClient | undefined;
  let lightTexture: LightTexture | undefined;
  let chunkDispatcher: ChunkRenderDispatcher | undefined;
  let viewArea: ViewArea | undefined;

  try {
    renderWorldWorker = new RenderWorldWorkerClient(
      options.rendererHost.createRenderWorldWorkerEndpoint(),
    );
    await renderWorldWorker.initialize({
      type: "initialize_render_world",
      seed: options.seed,
      minBuildHeight: options.minBuildHeight,
      height: options.worldHeight,
    });

    const renderWorldUpdateSink = new RenderWorldWorkerUpdateSink(renderWorldWorker);
    await renderWorldUpdateSink.ingestUpdates(options.snapshots);

    const biomeSource = new OverworldBiomeSource(options.seed);
    const mainThreadLevel = new ClientChunkCache({
      airState: options.blocks.airState,
      minBuildHeight: options.minBuildHeight,
      height: options.worldHeight,
      biomeSource,
      biomeZoomSeed: options.seed,
      blockStateResolver: createBlockStateResolver(options.blocks.airState),
      blockStateIds: options.blocks.blockStateIds,
      skyLight: 15,
      blockLight: 15,
      skyColor: options.skyColor,
    });
    const levelRenderer = new LevelRenderer();
    const gameRenderer = new GameRenderer(options.width, options.height, options.renderDistance);
    lightTexture = new LightTexture(gameRenderer, mainThreadLevel, options.device);
    lightTexture.tick();
    chunkDispatcher = new ChunkRenderDispatcher(
      mainThreadLevel,
      levelRenderer,
      options.blockRenderer,
      options.device,
      (task) => queueMicrotask(task),
      false,
      new ChunkBufferBuilderPack(),
      undefined,
      renderWorldWorker,
    );
    viewArea = new ViewArea(chunkDispatcher, mainThreadLevel, options.viewDistance, levelRenderer);
    levelRenderer.setLevel(mainThreadLevel, chunkDispatcher, viewArea, options.viewDistance);
    const dirtySectionCount = applyDirtySections(
      renderWorldUpdateSink,
      viewArea,
      levelRenderer,
      options.minBuildHeight,
      options.worldHeight,
    );
    const scene = createStaticRendererScene(
      options.device,
      options.atlas,
      renderWorldUpdateSink,
      viewArea,
      chunkDispatcher,
      levelRenderer,
      gameRenderer,
      lightTexture,
      options.minBuildHeight,
      options.worldHeight,
      options.viewDistance,
    );

    if (lightTexture === undefined || chunkDispatcher === undefined || viewArea === undefined) {
      throw new Error("static renderer frame harness was not fully initialized");
    }

    const openLightTexture = lightTexture;
    const openChunkDispatcher = chunkDispatcher;
    const openViewArea = viewArea;
    let closed = false;
    return {
      renderWorldWorker,
      renderWorldUpdateSink,
      levelRenderer,
      gameRenderer,
      lightTexture: openLightTexture,
      chunkDispatcher: openChunkDispatcher,
      viewArea: openViewArea,
      scene,
      dirtySectionCount,
      close: () => {
        if (closed) {
          return;
        }

        closed = true;
        openViewArea.releaseAllBuffers();
        openChunkDispatcher.dispose();
        openLightTexture.close();
      },
    };
  } catch (error) {
    viewArea?.releaseAllBuffers();
    chunkDispatcher?.dispose();
    lightTexture?.close();
    if (chunkDispatcher === undefined) {
      renderWorldWorker?.close();
    }

    throw error;
  }
}

export async function renderStaticRendererFrame(
  harness: StaticRendererFrameHarness,
  camera: CameraState,
): Promise<StaticRendererFrameResult> {
  const frame = await harness.gameRenderer.renderLevel(0, Number.MAX_SAFE_INTEGER, harness.levelRenderer, harness.lightTexture, camera, {
    waitForChunkTasks: true,
  });
  return {
    frame,
    drawCount: countFrameDraws(frame),
    renderedChunkCount: harness.levelRenderer.countRenderedChunks(),
    workerCounters: harness.renderWorldUpdateSink.getPerformanceCounters(),
  };
}

export function encodeRendererHarnessFrame(
  scene: RendererScene,
  frame: LevelRenderFrame,
  target: DrawTarget,
  encoder: GPUCommandEncoder,
): void {
  encodeSceneFrame(scene, frame, target, encoder);
}

export async function renderFrameToHeadlessTarget(
  options: HeadlessFrameReadbackOptions,
): Promise<HeadlessFrameReadbackResult> {
  const target = options.rendererHost.createOffscreenTarget(options.scene.device, options.width, options.height, options.format);
  const depthTarget = createSceneDepthTarget(options.scene.device, options.width, options.height);
  try {
    const encoder = options.scene.device.createCommandEncoder();
    encodeRendererHarnessFrame(options.scene, options.frame, { view: target.view, depthView: depthTarget.view, format: target.format }, encoder);
    options.scene.device.queue.submit([encoder.finish()]);

    const pixels = await readTextureRgba8(options.scene.device, target.texture, target.width, target.height);
    const centerPixel = readPixel(
      pixels,
      target.width,
      options.centerX ?? Math.floor(target.width / 2),
      options.centerY ?? Math.floor(target.height / 2),
    );
    const clearPixel = rgbaColorToPixel(options.frame.fogColor);
    const nonClearPixels = countPixelsDifferentFrom(pixels, clearPixel, options.clearTolerance ?? 6);
    return {
      width: target.width,
      height: target.height,
      format: target.format,
      pixels,
      centerPixel,
      clearPixel,
      nonClearPixels,
      byteLength: pixels.byteLength,
    };
  } finally {
    depthTarget.texture.destroy();
    target.texture.destroy();
  }
}

export function countFrameDraws(frame: LevelRenderFrame): number {
  let count = 0;
  for (const draws of frame.layerDraws.values()) {
    count += draws.length;
  }

  return count;
}

export function readPixel(pixels: Uint8Array, width: number, x: number, y: number): Uint8Array {
  const offset = ((y * width) + x) * 4;
  return pixels.slice(offset, offset + 4);
}

export function rgbaColorToPixel(color: readonly [number, number, number, number]): Uint8Array {
  return new Uint8Array(color.map((component) => Math.max(0, Math.min(255, Math.round(component * 255)))));
}

export function countPixelsDifferentFrom(pixels: Uint8Array, expected: Uint8Array, tolerance: number): number {
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
  minBuildHeight: number,
  worldHeight: number,
): number {
  let dirtyCount = 0;
  for (const dirtySection of sink.drainDirtySections()) {
    if (dirtySection.y < minBuildHeight || dirtySection.y >= minBuildHeight + worldHeight) {
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

function createStaticRendererScene(
  device: GPUDevice,
  atlas: TextureAtlas,
  renderWorldUpdateSink: RenderWorldWorkerUpdateSink,
  viewArea: ViewArea,
  chunkDispatcher: ChunkRenderDispatcher,
  levelRenderer: LevelRenderer,
  gameRenderer: GameRenderer,
  lightTexture: LightTexture,
  minBuildHeight: number,
  worldHeight: number,
  viewDistance: number,
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
      minBuildHeight,
      maxBuildHeight: minBuildHeight + worldHeight,
    },
    viewDistance,
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
