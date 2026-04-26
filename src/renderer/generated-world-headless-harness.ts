import type { AssetPack } from "./assets/asset-pack";
import { prepareAssetPackRendererResources } from "./chunk/asset-pack-mesh-context";
import { ChunkRenderDispatcher } from "./chunk/chunk-render-dispatcher";
import { RenderWorldWorkerClient, RenderWorldWorkerUpdateSink } from "./chunk/render-world-worker-client";
import { ChunkBufferBuilderPack } from "./chunk-buffer-builder-pack";
import { GameRenderer, type CameraState } from "./game-renderer";
import { LevelRenderer } from "./level-renderer";
import { LightTexture } from "./light-texture";
import { type LoadingProgressSink } from "./loading-progress";
import type { RenderWorldWorkerEndpointFactory } from "./renderer-host";
import { type RendererScene } from "./scene-setup";
import { createRendererHarnessScene } from "./static-frame-harness";
import { TextureAtlas } from "./texture/texture-atlas";
import { ViewArea } from "./view-area";
import { DEFAULT_PLAYER_PROFILE, type OpenWorldPreset, type PlayerProfile, type WorldEngineConfig } from "../runtime/protocol/world-messages";
import type { WorldSaveMetadata } from "../runtime/storage/world-storage";
import type { WorldTransport } from "../runtime/transport/local-world-transport";
import { TransportWorldClient } from "../runtime/transport/local-world-transport";
import { type ClientRuntime, WorldClientRuntimeFacade } from "../runtime/client/client-runtime";
import { createBlockStateResolver } from "../world/level/chunk-snapshot";
import { ClientChunkCache } from "../world/level/client-chunk-cache";
import { registerGeneratedRenderBlocks } from "../world/level/generated-render-blocks";
import { Vec3 } from "../world/phys/vec3";
import { OverworldBiomeSource } from "../worldgen/biome/overworld-biome-source";
import { renderGeneratedWorldSmokeFrame, type GeneratedWorldSmokeFrameResult } from "./generated-world-smoke-runner";

export interface GeneratedWorldHeadlessHarnessOptions {
  readonly adapter: GPUAdapter;
  readonly device: GPUDevice;
  readonly format: GPUTextureFormat;
  readonly rendererHost: RenderWorldWorkerEndpointFactory;
  readonly worldTransport: WorldTransport;
  readonly assetPack: AssetPack;
  readonly seed: bigint;
  readonly width: number;
  readonly height: number;
  readonly viewDistance: number;
  readonly renderDistance: number;
  readonly preset?: OpenWorldPreset;
  readonly engineConfig?: WorldEngineConfig;
  readonly playerProfile?: PlayerProfile;
  readonly skyColor?: Vec3;
  readonly clearColorScale?: number;
  readonly onProgress?: LoadingProgressSink;
}

export interface GeneratedWorldHeadlessHarness {
  readonly scene: RendererScene;
  readonly clientRuntime: ClientRuntime;
  readonly saveMetadata: WorldSaveMetadata;
  readonly atlas: TextureAtlas;
  readonly renderWorldWorker: RenderWorldWorkerClient;
  readonly renderWorldUpdateSink: RenderWorldWorkerUpdateSink;
  close(): void;
}

export type GeneratedWorldHeadlessFrameResult = GeneratedWorldSmokeFrameResult;

export async function createGeneratedWorldHeadlessHarness(
  options: GeneratedWorldHeadlessHarnessOptions,
): Promise<GeneratedWorldHeadlessHarness> {
  const generatedBlocks = registerGeneratedRenderBlocks();
  const atlas = new TextureAtlas(TextureAtlas.LOCATION_BLOCKS, options.device.limits.maxTextureDimension2D);
  const assetResources = await prepareAssetPackRendererResources(
    options.assetPack,
    generatedBlocks,
    atlas,
    options.onProgress,
  );
  atlas.reload(options.device, assetResources.preparations);

  let clientRuntime: ClientRuntime | undefined;
  let renderWorldWorker: RenderWorldWorkerClient | undefined;
  let lightTexture: LightTexture | undefined;
  let chunkDispatcher: ChunkRenderDispatcher | undefined;
  let viewArea: ViewArea | undefined;
  try {
    const biomeSource = new OverworldBiomeSource(options.seed);
    const blockStateResolver = createBlockStateResolver(generatedBlocks.airState);
    const worldClient = new TransportWorldClient(
      options.worldTransport,
      (worldOpened) => new ClientChunkCache({
        airState: generatedBlocks.airState,
        minBuildHeight: worldOpened.minBuildHeight,
        height: worldOpened.height,
        biomeSource,
        biomeZoomSeed: options.seed,
        blockStateResolver,
        blockStateIds: generatedBlocks.blockStateIds,
        skyColor: options.skyColor,
        clearColorScale: options.clearColorScale,
      }),
      {
        pollUpdateMaxMessages: 4,
        worldProgressSink: options.onProgress,
      },
    );
    clientRuntime = new WorldClientRuntimeFacade(worldClient);

    const worldOpened = await clientRuntime.openWorld({
      type: "open_world",
      seed: options.seed,
      preset: options.preset ?? "browser_smoke",
      config: options.engineConfig,
      playerProfile: options.playerProfile ?? DEFAULT_PLAYER_PROFILE,
      storageMode: "none",
    });
    const compatibilityLevel = clientRuntime.getClientWorld().getRenderView().getRenderLevel();

    renderWorldWorker = new RenderWorldWorkerClient(
      options.rendererHost.createRenderWorldWorkerEndpoint(),
      options.onProgress,
    );
    await renderWorldWorker.initialize({
      type: "initialize_render_world",
      seed: options.seed,
      minBuildHeight: worldOpened.minBuildHeight,
      height: worldOpened.height,
    });
    const renderWorldUpdateSink = new RenderWorldWorkerUpdateSink(renderWorldWorker);
    clientRuntime.setRenderWorldUpdateSink(renderWorldUpdateSink);

    const levelRenderer = new LevelRenderer();
    chunkDispatcher = new ChunkRenderDispatcher(
      compatibilityLevel,
      levelRenderer,
      assetResources.blockRenderer,
      options.device,
      (task) => queueMicrotask(task),
      false,
      new ChunkBufferBuilderPack(),
      undefined,
      renderWorldWorker,
    );
    viewArea = new ViewArea(chunkDispatcher, compatibilityLevel, options.viewDistance, levelRenderer);
    levelRenderer.setLevel(compatibilityLevel, chunkDispatcher, viewArea, options.viewDistance);
    const gameRenderer = new GameRenderer(options.width, options.height, options.renderDistance);
    lightTexture = new LightTexture(gameRenderer, compatibilityLevel, options.device);
    lightTexture.tick();

    const scene = createRendererHarnessScene({
      adapter: options.adapter,
      device: options.device,
      format: options.format,
      saveMetadata: worldOpened.saveMetadata,
      clientRuntime,
      atlas,
      renderWorldUpdateSink,
      viewArea,
      chunkDispatcher,
      levelRenderer,
      gameRenderer,
      lightTexture,
      minBuildHeight: compatibilityLevel.getMinBuildHeight(),
      worldHeight: compatibilityLevel.getHeight(),
      viewDistance: options.viewDistance,
    });

    let closed = false;
    return {
      scene,
      clientRuntime,
      saveMetadata: worldOpened.saveMetadata,
      atlas,
      renderWorldWorker,
      renderWorldUpdateSink,
      close: () => {
        if (closed) {
          return;
        }

        closed = true;
        viewArea!.releaseAllBuffers();
        chunkDispatcher!.dispose();
        lightTexture!.close();
        clientRuntime!.close();
        atlas.clearTextureData();
        assetResources.atlasSource.close();
      },
    };
  } catch (error) {
    viewArea?.releaseAllBuffers();
    chunkDispatcher?.dispose();
    lightTexture?.close();
    if (chunkDispatcher === undefined) {
      renderWorldWorker?.close();
    }
    clientRuntime?.close();
    atlas.clearTextureData();
    assetResources.atlasSource.close();
    throw error;
  }
}

export async function renderGeneratedWorldHeadlessFrame(
  harness: GeneratedWorldHeadlessHarness,
  camera: CameraState,
  expectedLoadedChunkCount: number,
): Promise<GeneratedWorldHeadlessFrameResult> {
  return renderGeneratedWorldSmokeFrame(harness.scene, camera, expectedLoadedChunkCount);
}
